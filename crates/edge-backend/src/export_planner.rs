use chrono::NaiveDateTime;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub const GIB: u64 = 1_073_741_824;
pub const MIN_RESERVE_BYTES: u64 = GIB;
pub const MAX_RESERVE_BYTES: u64 = 8 * GIB;
pub const PACK_OBJECT_OVERHEAD_BYTES: u64 = 4096;
pub const OBJECT_EXCEEDS_DISK_BUDGET: &str = "OBJECT_EXCEEDS_DISK_BUDGET";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapacityOverhead {
    pub estimated_manifest_bytes: u64,
    pub estimated_metadata_bytes: u64,
    pub estimated_log_bytes: u64,
    pub encryption_overhead_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityBudget {
    pub free_bytes: u64,
    pub reserve_bytes: u64,
    pub object_budget_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedObject {
    Pack {
        size_bytes: i64,
        estimated_landing_bytes: i64,
    },
    Failed {
        error_code: &'static str,
        error_message: String,
    },
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq, Eq)]
pub struct AssignedExportObject {
    pub id: i64,
    pub bucket: String,
    pub object_key: String,
    pub etag: String,
    pub size_bytes: i64,
    pub last_modified: NaiveDateTime,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct StableSnapshot {
    bucket: String,
    object_key: String,
    etag: String,
    size_bytes: i64,
    last_modified: NaiveDateTime,
}

pub fn calculate_capacity_budget(free_bytes: u64, overhead: CapacityOverhead) -> CapacityBudget {
    let reserve_bytes = (free_bytes / 50).clamp(MIN_RESERVE_BYTES, MAX_RESERVE_BYTES);
    let overhead_bytes = overhead
        .estimated_manifest_bytes
        .saturating_add(overhead.estimated_metadata_bytes)
        .saturating_add(overhead.estimated_log_bytes)
        .saturating_add(overhead.encryption_overhead_bytes);
    let object_budget_bytes = free_bytes
        .saturating_sub(reserve_bytes)
        .saturating_sub(overhead_bytes);

    CapacityBudget {
        free_bytes,
        reserve_bytes,
        object_budget_bytes,
    }
}

pub fn plan_object(size_bytes: u64, max_single_disk_object_budget_bytes: u64) -> PlannedObject {
    let estimated_landing_bytes = size_bytes.saturating_add(PACK_OBJECT_OVERHEAD_BYTES);
    if max_single_disk_object_budget_bytes == 0
        || estimated_landing_bytes > max_single_disk_object_budget_bytes
    {
        return PlannedObject::Failed {
            error_code: OBJECT_EXCEEDS_DISK_BUDGET,
            error_message: format!(
                "object estimated landing bytes {estimated_landing_bytes} exceed max disk object budget {max_single_disk_object_budget_bytes}"
            ),
        };
    }

    PlannedObject::Pack {
        size_bytes: saturating_i64(size_bytes),
        estimated_landing_bytes: saturating_i64(estimated_landing_bytes),
    }
}

pub async fn update_disk_capacity_budget(
    pool: &PgPool,
    disk_id: Uuid,
    free_bytes: u64,
    overhead: CapacityOverhead,
) -> Result<CapacityBudget, sqlx::Error> {
    let budget = calculate_capacity_budget(free_bytes, overhead);
    sqlx::query(
        r#"
        UPDATE disk_runtime
        SET free_bytes = $2,
            reserve_bytes = $3,
            object_budget_bytes = $4,
            last_seen_at = NOW() AT TIME ZONE 'UTC'
        WHERE disk_id = $1
        "#,
    )
    .bind(disk_id)
    .bind(saturating_i64(budget.free_bytes))
    .bind(saturating_i64(budget.reserve_bytes))
    .bind(saturating_i64(budget.object_budget_bytes))
    .execute(pool)
    .await?;

    Ok(budget)
}

pub async fn create_export_plan_from_stable_snapshots(
    pool: &PgPool,
    export_job_id: Uuid,
    edge_code: &str,
    max_single_disk_object_budget_bytes: u64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO export_job(export_job_id, edge_code, status, start_time)
        VALUES ($1, $2, 'PENDING', NOW() AT TIME ZONE 'UTC')
        ON CONFLICT (export_job_id) DO NOTHING
        "#,
    )
    .bind(export_job_id)
    .bind(edge_code)
    .execute(&mut *tx)
    .await?;

    let snapshots = sqlx::query_as::<_, StableSnapshot>(
        r#"
        WITH latest_scan AS (
          SELECT scan_started_at, scan_finished_at
          FROM edge_scan_run
          WHERE scan_status = 'DONE'
            AND scan_finished_at IS NOT NULL
          ORDER BY scan_finished_at DESC
          LIMIT 1
        )
        SELECT bucket, object_key, etag, size_bytes, last_modified
        FROM local_object_snapshot
        CROSS JOIN latest_scan
        WHERE stable_status = 'STABLE'
          AND scanned_at >= latest_scan.scan_started_at
          AND scanned_at <= latest_scan.scan_finished_at
        ORDER BY id ASC
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;

    let mut planned_count = 0_i64;
    let mut planned_bytes = 0_i64;

    for snapshot in snapshots {
        let planned = plan_object(
            snapshot.size_bytes.max(0) as u64,
            max_single_disk_object_budget_bytes,
        );
        match planned {
            PlannedObject::Pack {
                size_bytes,
                estimated_landing_bytes,
            } => {
                insert_pack_object(&mut tx, export_job_id, &snapshot, estimated_landing_bytes)
                    .await?;
                planned_count += 1;
                planned_bytes = planned_bytes.saturating_add(size_bytes);
            }
            PlannedObject::Failed {
                error_code,
                error_message,
            } => {
                insert_failed_object(
                    &mut tx,
                    export_job_id,
                    &snapshot,
                    error_code,
                    &error_message,
                )
                .await?;
            }
        }
    }

    sqlx::query(
        r#"
        UPDATE export_job
        SET object_count = $2,
            total_bytes = $3
        WHERE export_job_id = $1
        "#,
    )
    .bind(export_job_id)
    .bind(planned_count)
    .bind(planned_bytes)
    .execute(&mut *tx)
    .await?;

    tx.commit().await
}

pub const ASSIGNMENT_SQL: &str = r#"
WITH candidate AS (
  SELECT id, estimated_landing_bytes
  FROM export_object
  WHERE export_job_id = $1
    AND status = 'PENDING'
    AND disk_id IS NULL
    AND estimated_landing_bytes <= $3
  ORDER BY id ASC
  FOR UPDATE SKIP LOCKED
  LIMIT $4
),
picked AS (
  SELECT id
  FROM (
    SELECT id,
           SUM(estimated_landing_bytes) OVER (ORDER BY id ASC) AS running_bytes
    FROM candidate
  ) AS ranked
  WHERE running_bytes <= $3
  LIMIT $5
)
UPDATE export_object AS eo
SET status = 'ASSIGNED',
    disk_id = $2
FROM picked
WHERE eo.id = picked.id
RETURNING eo.id,
          eo.bucket,
          eo.object_key,
          eo.etag,
          eo.size_bytes,
          eo.last_modified
"#;

pub async fn assign_export_objects(
    pool: &PgPool,
    export_job_id: Uuid,
    disk_id: Uuid,
    object_budget_bytes: u64,
    candidate_limit: i64,
    batch_size: i64,
) -> Result<Vec<AssignedExportObject>, sqlx::Error> {
    if object_budget_bytes == 0 || candidate_limit <= 0 || batch_size <= 0 {
        return Ok(Vec::new());
    }

    let mut tx = pool.begin().await?;
    let assigned = sqlx::query_as::<_, AssignedExportObject>(ASSIGNMENT_SQL)
        .bind(export_job_id)
        .bind(disk_id)
        .bind(saturating_i64(object_budget_bytes))
        .bind(candidate_limit)
        .bind(batch_size)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(assigned)
}

async fn insert_pack_object(
    tx: &mut Transaction<'_, Postgres>,
    export_job_id: Uuid,
    snapshot: &StableSnapshot,
    estimated_landing_bytes: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO export_object(
            object_id, export_job_id, bucket, object_key, storage_mode, etag, size_bytes,
            estimated_landing_bytes, last_modified, frame_total, status
        )
        VALUES ($1, $2, $3, $4, 'PACK', $5, $6, $7, $8, 0, 'PENDING')
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(export_job_id)
    .bind(&snapshot.bucket)
    .bind(&snapshot.object_key)
    .bind(&snapshot.etag)
    .bind(snapshot.size_bytes)
    .bind(estimated_landing_bytes)
    .bind(snapshot.last_modified)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_failed_object(
    tx: &mut Transaction<'_, Postgres>,
    export_job_id: Uuid,
    snapshot: &StableSnapshot,
    error_code: &str,
    error_message: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO export_object(
            object_id, export_job_id, bucket, object_key, storage_mode, etag, size_bytes,
            estimated_landing_bytes, last_modified, frame_total, status, error_code, error_message
        )
        VALUES ($1, $2, $3, $4, 'PACK', $5, $6, 0, $7, 0, 'FAILED', $8, $9)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(export_job_id)
    .bind(&snapshot.bucket)
    .bind(&snapshot.object_key)
    .bind(&snapshot.etag)
    .bind(snapshot.size_bytes)
    .bind(snapshot.last_modified)
    .bind(error_code)
    .bind(error_message)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn saturating_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_budget_uses_protocol_reserve_formula() {
        let tiny = calculate_capacity_budget(20 * GIB, CapacityOverhead::default());
        assert_eq!(tiny.reserve_bytes, MIN_RESERVE_BYTES);

        let large = calculate_capacity_budget(1_000 * GIB, CapacityOverhead::default());
        assert_eq!(large.reserve_bytes, MAX_RESERVE_BYTES);

        let mid_free = 300 * GIB;
        let mid = calculate_capacity_budget(
            mid_free,
            CapacityOverhead {
                estimated_manifest_bytes: 10,
                estimated_metadata_bytes: 20,
                estimated_log_bytes: 30,
                encryption_overhead_bytes: 40,
            },
        );
        assert_eq!(mid.reserve_bytes, 6 * GIB);
        assert_eq!(mid.object_budget_bytes, mid_free - 6 * GIB - 100);
    }

    #[test]
    fn object_within_disk_budget_is_planned_as_pack() {
        let planned = plan_object(128 * 1024 * 1024, GIB);
        let PlannedObject::Pack {
            size_bytes,
            estimated_landing_bytes,
        } = planned
        else {
            panic!("expected pack object");
        };

        assert_eq!(size_bytes, 128 * 1024 * 1024);
        assert_eq!(
            estimated_landing_bytes,
            (128 * 1024 * 1024 + PACK_OBJECT_OVERHEAD_BYTES) as i64
        );
    }

    #[test]
    fn object_exceeding_disk_budget_is_marked_failed() {
        let planned = plan_object(GIB, GIB);
        let PlannedObject::Failed {
            error_code,
            error_message,
        } = planned
        else {
            panic!("expected failed object");
        };

        assert_eq!(error_code, OBJECT_EXCEEDS_DISK_BUDGET);
        assert!(error_message.contains("exceed max disk object budget"));
    }

    #[test]
    fn assignment_sql_keeps_locking_and_returning_in_one_statement() {
        assert!(ASSIGNMENT_SQL.contains("FOR UPDATE SKIP LOCKED"));
        assert!(ASSIGNMENT_SQL.contains("UPDATE export_object AS eo"));
        assert!(ASSIGNMENT_SQL.contains("RETURNING eo.id"));
        assert!(ASSIGNMENT_SQL.contains("running_bytes <= $3"));
    }
}
