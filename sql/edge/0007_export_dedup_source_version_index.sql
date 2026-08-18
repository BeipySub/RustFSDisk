-- Edge 本地历史导出对象版本索引：按 S3 对象身份和内容版本排除已随 SEALED 任务导出的对象。

CREATE INDEX IF NOT EXISTS idx_export_object_exported_source_identity
  ON export_object(bucket, object_key, etag, size_bytes)
  WHERE status = 'EXPORTED';

COMMENT ON INDEX idx_export_object_exported_source_identity IS 'Edge 创建导出计划时用于排除历史已经随 SEALED 任务导出的同一 RustFS 对象版本；判断字段固定为 bucket、object_key、etag、size_bytes，last_modified 只做审计展示，不参与去重命中。';
