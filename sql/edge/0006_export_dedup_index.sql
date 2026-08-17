-- Edge 本地历史导出对象版本索引：创建导出计划时跳过已经随 SEALED 任务导出的同一 RustFS 对象版本。

CREATE INDEX IF NOT EXISTS idx_export_object_exported_source_version
  ON export_object(bucket, object_key, etag, size_bytes, last_modified)
  WHERE status = 'EXPORTED';

COMMENT ON INDEX idx_export_object_exported_source_version IS 'Edge 创建导出计划时用于排除历史已经随 SEALED 任务导出的同一 RustFS 对象版本，避免重复导出。';
