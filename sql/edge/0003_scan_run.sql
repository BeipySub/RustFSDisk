CREATE TABLE IF NOT EXISTS edge_scan_run (
  id BIGSERIAL PRIMARY KEY,
  scan_run_id UUID NOT NULL,
  scan_status VARCHAR(32) NOT NULL,
  scan_started_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  scan_finished_at TIMESTAMP,
  bucket_count BIGINT NOT NULL DEFAULT 0,
  object_seen BIGINT NOT NULL DEFAULT 0,
  stable_object_count BIGINT NOT NULL DEFAULT 0,
  source_changed_count BIGINT NOT NULL DEFAULT 0,
  total_bytes BIGINT NOT NULL DEFAULT 0,
  error_code VARCHAR(64),
  error_message TEXT,
  CONSTRAINT uq_edge_scan_run_business_id UNIQUE (scan_run_id),
  CONSTRAINT ck_edge_scan_run_status CHECK (scan_status IN ('SCANNING', 'DONE', 'FAILED'))
);

COMMENT ON TABLE edge_scan_run IS '边缘端 RustFS 扫描运行记录表；用于一天一次扫描门禁和扫描完成审计。';
COMMENT ON COLUMN edge_scan_run.scan_run_id IS '扫描运行业务 ID；用于日志和审计。';
COMMENT ON COLUMN edge_scan_run.scan_status IS '扫描运行状态；SCANNING 表示扫描中，DONE 表示成功完成，FAILED 表示失败。';
COMMENT ON COLUMN edge_scan_run.scan_started_at IS '扫描开始 UTC 时间。';
COMMENT ON COLUMN edge_scan_run.scan_finished_at IS '扫描结束 UTC 时间；只有 DONE 或 FAILED 时写入。';
COMMENT ON COLUMN edge_scan_run.bucket_count IS '本次扫描 bucket 数量。';
COMMENT ON COLUMN edge_scan_run.object_seen IS '本次扫描看到的对象数量；只作为候选数量，不表示可导出完整对象。';
COMMENT ON COLUMN edge_scan_run.stable_object_count IS '本次扫描确认上传完成且稳定的对象数量。';
COMMENT ON COLUMN edge_scan_run.source_changed_count IS '本次扫描发现正在变化或不稳定的对象数量，不进入导出统计。';
COMMENT ON COLUMN edge_scan_run.total_bytes IS '本次扫描确认稳定对象的总字节数。';
COMMENT ON COLUMN edge_scan_run.error_code IS '扫描失败错误码。';
COMMENT ON COLUMN edge_scan_run.error_message IS '扫描失败原因。';

CREATE INDEX IF NOT EXISTS idx_edge_scan_run_finished
  ON edge_scan_run(scan_finished_at);

CREATE INDEX IF NOT EXISTS idx_edge_scan_run_done_finished
  ON edge_scan_run(scan_finished_at)
  WHERE scan_status = 'DONE';
