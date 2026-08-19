-- `SEALING` is an active export-job phase. It must be accepted both by
-- database validation and the API status filter used by auto orchestration.
ALTER TABLE export_job
  DROP CONSTRAINT IF EXISTS ck_export_job_status;

ALTER TABLE export_job
  ADD CONSTRAINT ck_export_job_status
CHECK (status IN ('PENDING', 'COPYING', 'SEALING', 'SEALED', 'FAILED', 'CANCELLED'));

COMMENT ON COLUMN export_job.status IS '导出任务状态；PENDING 表示待处理，COPYING 表示拷贝中，SEALING 表示封盘中，SEALED 表示已封盘，FAILED 表示导出失败，CANCELLED 表示已取消；API/WS 序列化为 export_job_status。';
