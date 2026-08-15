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
  scanned_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  stable_status VARCHAR(32) NOT NULL DEFAULT 'UNKNOWN',
  CONSTRAINT ck_local_object_snapshot_stable_status CHECK (stable_status IN ('UNKNOWN', 'STABLE', 'UNSTABLE', 'SOURCE_CHANGED'))
);

COMMENT ON TABLE local_object_snapshot IS '边缘端 RustFS 对象扫描快照表；用于统计、稳定性判断和导出编排。';
COMMENT ON COLUMN local_object_snapshot.id IS '数据库自增主键。';
COMMENT ON COLUMN local_object_snapshot.bucket IS '源 RustFS bucket 名称。';
COMMENT ON COLUMN local_object_snapshot.object_key IS '源 RustFS object key；数据库用 object_key 避免和通用 key 概念混淆。';
COMMENT ON COLUMN local_object_snapshot.etag IS '源对象 ETag，用于对象身份和稳定性判断。';
COMMENT ON COLUMN local_object_snapshot.size_bytes IS '源对象字节数。';
COMMENT ON COLUMN local_object_snapshot.last_modified IS '源对象 last_modified，从 RustFS/S3 获取后归一化为 UTC。';
COMMENT ON COLUMN local_object_snapshot.metadata_json IS '扫描到的源对象 metadata JSON；写 manifest 时输出为 metadata。';
COMMENT ON COLUMN local_object_snapshot.scanned_at IS '本次快照扫描入库的 UTC 时间。';
COMMENT ON COLUMN local_object_snapshot.stable_status IS '稳定性判断状态；UNKNOWN 表示未知，STABLE 表示稳定可进入导出队列，UNSTABLE 表示不稳定暂不导出，SOURCE_CHANGED 表示源对象发生变化且本轮不导出。';

CREATE INDEX IF NOT EXISTS idx_local_object_snapshot_scanned_at ON local_object_snapshot(scanned_at);
CREATE INDEX IF NOT EXISTS idx_local_object_snapshot_source ON local_object_snapshot(bucket, object_key);

CREATE TABLE IF NOT EXISTS export_job (
  id BIGSERIAL PRIMARY KEY,
  export_job_id UUID NOT NULL,
  disk_id UUID,
  edge_code VARCHAR(255) NOT NULL,
  status VARCHAR(32) NOT NULL,
  object_count BIGINT NOT NULL DEFAULT 0,
  copied_count BIGINT NOT NULL DEFAULT 0,
  total_bytes BIGINT NOT NULL DEFAULT 0,
  copied_bytes BIGINT NOT NULL DEFAULT 0,
  start_time TIMESTAMP,
  finish_time TIMESTAMP,
  error_message TEXT,
  CONSTRAINT ck_export_job_status CHECK (status IN ('PENDING', 'SCANNING', 'COPYING', 'SEALED', 'FAILED', 'CANCELLED'))
);

COMMENT ON TABLE export_job IS '边缘端导出任务表；记录导出阶段状态、低频 checkpoint、统计进度和结果。';
COMMENT ON COLUMN export_job.id IS '数据库自增主键。';
COMMENT ON COLUMN export_job.export_job_id IS '导出任务业务 ID；用于 API、日志、manifest 和审计追踪。';
COMMENT ON COLUMN export_job.disk_id IS '运输盘逻辑 ID；单盘任务可填，多盘任务通过 export_object.disk_id 关联。';
COMMENT ON COLUMN export_job.edge_code IS '执行该导出任务的边缘站点编码。';
COMMENT ON COLUMN export_job.status IS '导出任务状态；PENDING 表示待处理，SCANNING 表示扫描中，COPYING 表示拷贝中，SEALED 表示已封盘，FAILED 表示导出失败，CANCELLED 表示已取消；API/WS 序列化为 export_job_status。';
COMMENT ON COLUMN export_job.object_count IS '本次导出计划对象或分块总数。';
COMMENT ON COLUMN export_job.copied_count IS '已完成写盘并校验的对象或分块数量。';
COMMENT ON COLUMN export_job.total_bytes IS '本次导出计划写入的明文总字节数。';
COMMENT ON COLUMN export_job.copied_bytes IS '已完成写盘并校验的明文字节数。';
COMMENT ON COLUMN export_job.start_time IS '导出任务开始的 UTC 时间。';
COMMENT ON COLUMN export_job.finish_time IS '导出任务结束或封盘完成的 UTC 时间。';
COMMENT ON COLUMN export_job.error_message IS '导出任务失败、取消或异常时的错误说明。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_job_business_id ON export_job(export_job_id);

CREATE TABLE IF NOT EXISTS export_object (
  id BIGSERIAL PRIMARY KEY,
  object_id UUID NOT NULL,
  export_job_id UUID NOT NULL,
  disk_id UUID,
  bucket VARCHAR(255) NOT NULL,
  object_key TEXT NOT NULL,
  storage_mode VARCHAR(16) NOT NULL,
  etag VARCHAR(255) NOT NULL,
  size_bytes BIGINT NOT NULL,
  estimated_landing_bytes BIGINT NOT NULL DEFAULT 0,
  last_modified TIMESTAMP NOT NULL,
  plaintext_sha256 VARCHAR(64),
  data_key_id UUID,
  pack_path TEXT,
  pack_index_path TEXT,
  pack_offset_bytes BIGINT,
  pack_ciphertext_size_bytes BIGINT,
  pack_nonce VARCHAR(255),
  pack_tag VARCHAR(255),
  pack_aad TEXT,
  pack_ciphertext_sha256 VARCHAR(64),
  frame_total INTEGER NOT NULL DEFAULT 0,
  partial_path TEXT,
  relative_meta_path TEXT,
  status VARCHAR(32) NOT NULL,
  error_code VARCHAR(64),
  error_message TEXT,
  CONSTRAINT ck_export_object_status CHECK (status IN ('PENDING', 'ASSIGNED', 'COPYING', 'EXPORTED', 'FAILED', 'SOURCE_CHANGED', 'SKIPPED')),
  CONSTRAINT ck_export_object_storage_mode CHECK (storage_mode IN ('PACK', 'FRAMES')),
  CONSTRAINT ck_export_object_frame_total CHECK (
    (storage_mode = 'PACK' AND frame_total = 0)
    OR (storage_mode = 'FRAMES' AND frame_total > 0)
  )
);

COMMENT ON TABLE export_object IS '边缘端导出对象任务表；多盘并行对象分配锁、对象状态和断点恢复核心表。';
COMMENT ON COLUMN export_object.id IS '数据库自增主键。';
COMMENT ON COLUMN export_object.object_id IS '导出对象业务 ID；PACK 和 FRAMES 明细均使用该 ID 聚合。';
COMMENT ON COLUMN export_object.export_job_id IS '所属导出任务业务 ID。';
COMMENT ON COLUMN export_object.disk_id IS 'PACK 对象当前分配写入的运输盘逻辑 ID；FRAMES 对象目标盘记录在 export_object_frame.disk_id。';
COMMENT ON COLUMN export_object.bucket IS '源 RustFS bucket 名称。';
COMMENT ON COLUMN export_object.object_key IS '源 RustFS object key；写 manifest 时映射为 objects[].key。';
COMMENT ON COLUMN export_object.storage_mode IS 'v2 存储模式；PACK 表示小对象进入 pack 文件，FRAMES 表示对象按 frame 文件写盘。';
COMMENT ON COLUMN export_object.etag IS '源对象 ETag，用于对象身份和导出前后稳定性校验。';
COMMENT ON COLUMN export_object.size_bytes IS '源对象明文总字节数。';
COMMENT ON COLUMN export_object.estimated_landing_bytes IS '该对象预计落盘总量；容量分配必须按该字段，不得按 size_bytes。';
COMMENT ON COLUMN export_object.last_modified IS '源对象 last_modified，归一化为 UTC。';
COMMENT ON COLUMN export_object.plaintext_sha256 IS '源对象整体明文 SHA256。';
COMMENT ON COLUMN export_object.data_key_id IS '本对象使用的数据密钥编号；运输盘和 manifest 只保存该编号，不保存明文密钥。';
COMMENT ON COLUMN export_object.pack_path IS 'PACK 文件相对 /rustfs-transfer/ 的路径；FRAMES 对象为空。';
COMMENT ON COLUMN export_object.pack_index_path IS 'PACK 索引文件相对 /rustfs-transfer/ 的路径；FRAMES 对象为空。';
COMMENT ON COLUMN export_object.pack_offset_bytes IS '对象密文在 PACK 文件内的起始偏移。';
COMMENT ON COLUMN export_object.pack_ciphertext_size_bytes IS 'PACK 对象密文字节数，不包含 AES-GCM tag。';
COMMENT ON COLUMN export_object.pack_nonce IS 'PACK 对象 AES-GCM nonce；同一 data_key_id 下必须唯一。';
COMMENT ON COLUMN export_object.pack_tag IS 'PACK 对象 AES-GCM 认证标签。';
COMMENT ON COLUMN export_object.pack_aad IS 'PACK 对象 Common canonical_json AAD。';
COMMENT ON COLUMN export_object.pack_ciphertext_sha256 IS 'PACK 对象密文 SHA256。';
COMMENT ON COLUMN export_object.frame_total IS 'FRAMES 对象 frame 总数；PACK 固定为 0。';
COMMENT ON COLUMN export_object.partial_path IS '.partial 临时密文路径；不得写入 manifest。';
COMMENT ON COLUMN export_object.relative_meta_path IS 'metadata sidecar 相对 /rustfs-transfer/ 的路径，必须位于 meta/ 下。';
COMMENT ON COLUMN export_object.status IS '对象任务状态；PENDING 表示待分配，ASSIGNED 表示已分配到运输盘，COPYING 表示复制/加密写盘中，EXPORTED 表示已导出且可进入 manifest，FAILED 表示失败，SOURCE_CHANGED 表示源对象导出前后变化，SKIPPED 表示已跳过；不包含 IMPORTED，序列化为 object_status。';
COMMENT ON COLUMN export_object.error_code IS '对象任务失败、源变化或跳过时的标准错误码。';
COMMENT ON COLUMN export_object.error_message IS '对象任务失败、源变化或跳过时的错误说明。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_plain_active_source
  ON export_object(export_job_id, bucket, object_key, etag, size_bytes, last_modified)
  WHERE status IN ('PENDING', 'ASSIGNED', 'COPYING', 'EXPORTED');

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_data_key_nonce
  ON export_object(data_key_id, pack_nonce)
  WHERE data_key_id IS NOT NULL AND pack_nonce IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_export_object_assignment
  ON export_object(export_job_id, status, disk_id, id);

CREATE TABLE IF NOT EXISTS export_object_frame (
  id BIGSERIAL PRIMARY KEY,
  object_id UUID NOT NULL,
  export_job_id UUID NOT NULL,
  disk_id UUID,
  frame_index INTEGER NOT NULL,
  frame_total INTEGER NOT NULL,
  frame_offset_bytes BIGINT NOT NULL,
  frame_size_bytes BIGINT NOT NULL,
  estimated_landing_bytes BIGINT NOT NULL DEFAULT 0,
  relative_frame_path TEXT,
  ciphertext_size_bytes BIGINT,
  ciphertext_sha256 VARCHAR(64),
  data_key_id UUID,
  nonce VARCHAR(255),
  tag VARCHAR(255),
  aad TEXT,
  partial_path TEXT,
  status VARCHAR(32) NOT NULL,
  error_code VARCHAR(64),
  error_message TEXT,
  CONSTRAINT ck_export_object_frame_status CHECK (status IN ('PENDING', 'ASSIGNED', 'COPYING', 'EXPORTED', 'FAILED', 'SKIPPED')),
  CONSTRAINT ck_export_object_frame_index CHECK (frame_index >= 0 AND frame_index < frame_total)
);

COMMENT ON TABLE export_object_frame IS '边缘端 FRAMES 存储模式的 frame 明细表；frame 是跨盘容量分配单元。';
COMMENT ON COLUMN export_object_frame.object_id IS '所属导出对象业务 ID。';
COMMENT ON COLUMN export_object_frame.disk_id IS '该 frame 分配写入的运输盘逻辑 ID。';
COMMENT ON COLUMN export_object_frame.estimated_landing_bytes IS '该 frame 预计落盘总量；容量分配必须按该字段。';
COMMENT ON COLUMN export_object_frame.relative_frame_path IS 'frame 密文文件相对 /rustfs-transfer/ 的路径，必须位于 frames/ 下。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_frame_object_index
  ON export_object_frame(export_job_id, object_id, frame_index);
CREATE UNIQUE INDEX IF NOT EXISTS uq_export_object_frame_data_key_nonce
  ON export_object_frame(data_key_id, nonce)
  WHERE data_key_id IS NOT NULL AND nonce IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_export_object_frame_assignment
  ON export_object_frame(export_job_id, status, disk_id, id);

CREATE TABLE IF NOT EXISTS export_nonce_ledger (
  id BIGSERIAL PRIMARY KEY,
  data_key_id UUID NOT NULL,
  nonce VARCHAR(255) NOT NULL,
  export_job_id UUID NOT NULL,
  disk_id UUID NOT NULL,
  object_id UUID NOT NULL,
  storage_mode VARCHAR(16) NOT NULL,
  frame_index INTEGER,
  created_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_export_nonce_ledger_data_key_nonce
  ON export_nonce_ledger(data_key_id, nonce);

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

COMMENT ON TABLE disk_runtime IS '边缘端运输盘运行状态表；记录热插拔、容量检测、低频 checkpoint 和恢复判断信息。';
COMMENT ON COLUMN disk_runtime.id IS '数据库自增主键。';
COMMENT ON COLUMN disk_runtime.sn IS '运输盘硬件序列号；从 Linux 设备信息读取，仅作辅助识别。';
COMMENT ON COLUMN disk_runtime.disk_id IS '运输盘逻辑 ID；从 disk_info.json 或中控接口获取。';
COMMENT ON COLUMN disk_runtime.device_path IS 'Linux 设备路径，例如 /dev/sdb。';
COMMENT ON COLUMN disk_runtime.mount_path IS '运输盘挂载路径，例如 /mnt/rustfs-transfer/disk-xxx。';
COMMENT ON COLUMN disk_runtime.capacity_bytes IS '当前识别到的运输盘总容量字节数。';
COMMENT ON COLUMN disk_runtime.free_bytes IS '当前文件系统剩余容量字节数。';
COMMENT ON COLUMN disk_runtime.reserve_bytes IS '当前预留容量字节数；按协议公式计算，用于协议文件、元数据、日志和安全余量。';
COMMENT ON COLUMN disk_runtime.object_budget_bytes IS '当前对象可分配容量字节数；调度对象或分块时只能使用该容量。';
COMMENT ON COLUMN disk_runtime.status IS '运输盘运行态；DETECTED 表示已检测到，CHECKING 表示校验中，READY 表示可用于任务，COPYING 表示复制中，CLEANING 表示中控清理运行态，REINITIALIZING 表示中控重新初始化运行态，DONE 表示任务完成，REJECTED 表示拒绝使用，REMOVED 表示已拔出，ERROR 表示异常；API/WS 序列化为 runtime_status，不表示盘内生命周期，CLEANING/REINITIALIZING 不得写入盘内 lifecycle status_code。';
COMMENT ON COLUMN disk_runtime.last_error_code IS '最近一次盘级标准错误码，例如 FILESYSTEM_UNSUPPORTED 或 RECOVERY_REQUIRED。';
COMMENT ON COLUMN disk_runtime.error_message IS '最近一次盘级错误说明；任务级错误写入 export_job 或 export_object。';
COMMENT ON COLUMN disk_runtime.partial_residue_count IS '.partial 残留文件数量；恢复扫描时统计。';
COMMENT ON COLUMN disk_runtime.partial_residue_bytes IS '.partial 残留文件总字节数；用于空间释放判断和前端提示。';
COMMENT ON COLUMN disk_runtime.last_seen_at IS '最近一次检测到该运输盘的 UTC 时间。';

CREATE INDEX IF NOT EXISTS idx_disk_runtime_disk_id ON disk_runtime(disk_id);
CREATE INDEX IF NOT EXISTS idx_disk_runtime_status ON disk_runtime(status);
CREATE INDEX IF NOT EXISTS idx_disk_runtime_last_seen ON disk_runtime(last_seen_at);
