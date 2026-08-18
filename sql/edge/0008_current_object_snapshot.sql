-- Edge 当前 RustFS 对象快照收口：只保留最近一次成功扫描的对象状态，扫描过程使用暂存表隔离失败扫描。

CREATE TABLE IF NOT EXISTS local_object_scan_stage (
  scan_run_id UUID NOT NULL,
  bucket VARCHAR(255) NOT NULL,
  object_key TEXT NOT NULL,
  etag VARCHAR(255) NOT NULL,
  size_bytes BIGINT NOT NULL,
  last_modified TIMESTAMP NOT NULL,
  metadata_json JSONB,
  scanned_at TIMESTAMP NOT NULL,
  stable_status VARCHAR(32) NOT NULL,
  CONSTRAINT pk_local_object_scan_stage PRIMARY KEY (scan_run_id, bucket, object_key),
  CONSTRAINT ck_local_object_scan_stage_stable_status
    CHECK (stable_status IN ('UNKNOWN', 'STABLE', 'UNSTABLE', 'SOURCE_CHANGED'))
);

COMMENT ON TABLE local_object_scan_stage IS 'Edge 单次 RustFS 扫描暂存表；仅在扫描成功后原子替换当前快照，失败扫描必须清理。';
COMMENT ON COLUMN local_object_scan_stage.scan_run_id IS '所属 edge_scan_run 的扫描运行 ID。';
COMMENT ON COLUMN local_object_scan_stage.scanned_at IS '本轮扫描确认该对象状态的 UTC 时间。';

DELETE FROM local_object_snapshot AS duplicate
USING local_object_snapshot AS retained
WHERE duplicate.bucket = retained.bucket
  AND duplicate.object_key = retained.object_key
  AND (duplicate.scanned_at, duplicate.id) < (retained.scanned_at, retained.id);

DROP INDEX IF EXISTS idx_local_object_snapshot_source;

CREATE UNIQUE INDEX IF NOT EXISTS uq_local_object_snapshot_bucket_key
  ON local_object_snapshot(bucket, object_key);

COMMENT ON TABLE local_object_snapshot IS 'Edge 最近一次成功 RustFS 扫描的当前对象快照；每个 bucket + object_key 仅保留一行。';
COMMENT ON INDEX uq_local_object_snapshot_bucket_key IS '保证当前对象快照按 bucket + object_key 唯一，防止重复扫描数据累积。';
