-- v2 pack 数据面：保留 export_object 状态机，只增加 pack 定位和恢复账本。
ALTER TABLE export_object
  ADD COLUMN IF NOT EXISTS storage_layout VARCHAR(32) NOT NULL DEFAULT 'PACK_RECORDS_V2',
  ADD COLUMN IF NOT EXISTS pack_records_json JSONB NOT NULL DEFAULT '[]'::jsonb;

COMMENT ON COLUMN export_object.storage_layout IS '运输盘数据布局；v2 固定 PACK_RECORDS_V2，不改变对象任务 status 语义。';
COMMENT ON COLUMN export_object.pack_records_json IS '对象在顺序 pack 中的 AES-GCM 记录定位、nonce、tag、AAD 和摘要；封盘时写入 manifest。';
