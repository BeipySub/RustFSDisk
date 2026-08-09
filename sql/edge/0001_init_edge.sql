-- 作用：边缘端基础表、唯一约束和必要索引。
-- 适用端：边缘端。
-- 约束来源：docs/v1.0冻结/数据库设计.md。

CREATE TABLE IF NOT EXISTS local_object_snapshot (
  id BIGSERIAL PRIMARY KEY,
  bucket VARCHAR(255) NOT NULL,
  object_key TEXT NOT NULL,
  etag VARCHAR(255) NOT NULL,
  size_bytes BIGINT NOT NULL,
  last_modified TIMESTAMP NOT NULL,
  metadata_json JSONB,
  scan_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

COMMENT ON TABLE local_object_snapshot IS '边缘端 RustFS 对象扫描快照，用于统计、稳定性判断和导出编排。';
COMMENT ON COLUMN local_object_snapshot.last_modified IS '从 RustFS/S3 获取后归一化为 UTC。';

CREATE INDEX IF NOT EXISTS idx_local_object_snapshot_scan_time ON local_object_snapshot(scan_time);
CREATE INDEX IF NOT EXISTS idx_local_object_snapshot_source ON local_object_snapshot(bucket, object_key);

CREATE TABLE IF NOT EXISTS export_job (
  id BIGSERIAL PRIMARY KEY,
  export_job_id UUID NOT NULL,
  edge_code VARCHAR(255) NOT NULL,
  status VARCHAR(32) NOT NULL,
  object_count BIGINT NOT NULL DEFAULT 0,
  copied_count BIGINT NOT NULL DEFAULT 0,
  total_bytes BIGINT NOT NULL DEFAULT 0,
  copied_bytes BIGINT NOT NULL DEFAULT 0,
  start_time TIMESTAMP,
  finish_time TIMESTAMP,
  error_message TEXT,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  CONSTRAINT ck_export_job_status CHECK (status IN ('PENDING', 'SCANNING', 'PLANNING', 'COPYING', 'SEALED', 'FAILED', 'CANCELLED'))
);

COMMENT ON TABLE export_job IS '边缘端一次导出任务的阶段状态、低频 checkpoint 和结果。';
COMMENT ON COLUMN export_job.status IS '导出任务状态；API/WS 序列化为 export_job_status。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_job_business_id ON export_job(export_job_id);

CREATE TABLE IF NOT EXISTS export_object (
  id BIGSERIAL PRIMARY KEY,
  export_job_id UUID NOT NULL,
  disk_id UUID,
  bucket VARCHAR(255) NOT NULL,
  object_key TEXT NOT NULL,
  etag VARCHAR(255) NOT NULL,
  size_bytes BIGINT NOT NULL,
  last_modified TIMESTAMP NOT NULL,
  plaintext_sha256 VARCHAR(64),
  ciphertext_sha256 VARCHAR(64),
  ciphertext_size_bytes BIGINT,
  encrypted BOOLEAN NOT NULL DEFAULT TRUE,
  encryption_alg VARCHAR(64),
  data_key_id UUID,
  nonce VARCHAR(255),
  tag VARCHAR(255),
  aad TEXT,
  chunked BOOLEAN NOT NULL DEFAULT FALSE,
  chunk_group_id UUID,
  chunk_index INTEGER NOT NULL DEFAULT 0,
  chunk_total INTEGER NOT NULL DEFAULT 1,
  chunk_offset_bytes BIGINT NOT NULL DEFAULT 0,
  chunk_size_bytes BIGINT NOT NULL,
  chunk_sha256 VARCHAR(64),
  partial_path TEXT,
  relative_data_path TEXT,
  relative_meta_path TEXT,
  status VARCHAR(32) NOT NULL,
  error_code VARCHAR(64),
  error_message TEXT,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  update_time TIMESTAMP,
  CONSTRAINT ck_export_object_status CHECK (status IN ('PENDING', 'ASSIGNED', 'COPYING', 'EXPORTED', 'FAILED', 'SOURCE_CHANGED', 'SKIPPED')),
  CONSTRAINT ck_export_object_chunk_total CHECK (chunk_total >= 1 AND chunk_total <= 1000000),
  CONSTRAINT ck_export_object_chunk_index CHECK (chunk_index >= 0 AND chunk_index < chunk_total)
);

COMMENT ON TABLE export_object IS '边缘端导出对象任务表；多盘并行对象分配锁和断点恢复核心表。';
COMMENT ON COLUMN export_object.status IS '对象任务状态；不包含 IMPORTED，序列化为 object_status。';
COMMENT ON COLUMN export_object.partial_path IS '.partial 临时密文路径；不得写入 manifest。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_plain_active_source
  ON export_object(export_job_id, bucket, object_key, etag, size_bytes, last_modified)
  WHERE chunked = FALSE
    AND status IN ('PENDING', 'ASSIGNED', 'COPYING', 'EXPORTED');

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_chunk_active_source
  ON export_object(export_job_id, bucket, object_key, etag, size_bytes, last_modified, chunk_index)
  WHERE chunked = TRUE
    AND status IN ('PENDING', 'ASSIGNED', 'COPYING', 'EXPORTED');

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_data_key_nonce
  ON export_object(data_key_id, nonce)
  WHERE data_key_id IS NOT NULL AND nonce IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_chunk_group_index
  ON export_object(chunk_group_id, chunk_index)
  WHERE chunked = TRUE AND chunk_group_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_export_object_assignment
  ON export_object(export_job_id, status, disk_id, id);

CREATE TABLE IF NOT EXISTS disk_runtime (
  id BIGSERIAL PRIMARY KEY,
  sn VARCHAR(255) NOT NULL,
  disk_id UUID,
  device_path VARCHAR(512) NOT NULL,
  mount_path TEXT,
  capacity_bytes BIGINT NOT NULL,
  free_bytes BIGINT NOT NULL,
  reserve_bytes BIGINT NOT NULL,
  object_budget_bytes BIGINT NOT NULL,
  status VARCHAR(32) NOT NULL,
  last_error_code VARCHAR(64),
  error_message TEXT,
  partial_residue_count INTEGER NOT NULL DEFAULT 0,
  partial_residue_bytes BIGINT NOT NULL DEFAULT 0,
  last_seen_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  CONSTRAINT ck_disk_runtime_status CHECK (status IN ('DETECTED', 'CHECKING', 'READY', 'COPYING', 'CLEANING', 'REINITIALIZING', 'DONE', 'REJECTED', 'REMOVED', 'ERROR'))
);

COMMENT ON TABLE disk_runtime IS '边缘端运输盘运行状态、容量检测结果和低频 checkpoint。';
COMMENT ON COLUMN disk_runtime.status IS '运行态；API/WS 序列化为 runtime_status，不表示盘内生命周期。';

CREATE INDEX IF NOT EXISTS idx_disk_runtime_disk_id ON disk_runtime(disk_id);
CREATE INDEX IF NOT EXISTS idx_disk_runtime_status ON disk_runtime(status);
CREATE INDEX IF NOT EXISTS idx_disk_runtime_last_seen ON disk_runtime(last_seen_at);
