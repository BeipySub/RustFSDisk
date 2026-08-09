-- 作用：中控端基础表、唯一约束和必要索引。
-- 适用端：中控端。
-- 约束来源：docs/v1.0冻结/数据库设计.md。

CREATE TABLE IF NOT EXISTS disk_list (
  id BIGSERIAL PRIMARY KEY,
  sn VARCHAR(255) NOT NULL,
  disk_id UUID NOT NULL,
  capacity_bytes BIGINT,
  status BOOLEAN NOT NULL DEFAULT TRUE,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  last_init_time TIMESTAMP,
  remark TEXT
);

COMMENT ON TABLE disk_list IS '中控端运输盘注册表；status 只表示启用状态，不表示盘内生命周期。';
COMMENT ON COLUMN disk_list.disk_id IS '中控生成的运输盘逻辑 ID，两端协议业务主身份。';
COMMENT ON COLUMN disk_list.status IS '运输盘是否启用；API 序列化为 disk_enabled 或 enabled。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_disk_list_disk_id ON disk_list(disk_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_disk_list_sn ON disk_list(sn);

CREATE TABLE IF NOT EXISTS edge_site (
  id BIGSERIAL PRIMARY KEY,
  edge_code VARCHAR(255) NOT NULL,
  edge_name VARCHAR(255) NOT NULL,
  auth_key_id VARCHAR(255) NOT NULL,
  auth_secret_ciphertext TEXT NOT NULL,
  status VARCHAR(32) NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  CONSTRAINT ck_edge_site_status CHECK (status IN ('ACTIVE', 'DISABLED', 'ERROR'))
);

COMMENT ON TABLE edge_site IS '边缘站点表；auth_secret_ciphertext 只用于 HTTP HMAC 鉴权，不用于文件加密。';
COMMENT ON COLUMN edge_site.status IS '边缘站点状态；API 序列化为 edge_status。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_edge_site_edge_code ON edge_site(edge_code);
CREATE UNIQUE INDEX IF NOT EXISTS uq_edge_site_auth_key_id ON edge_site(auth_key_id);

CREATE TABLE IF NOT EXISTS center_config (
  id BIGSERIAL PRIMARY KEY,
  center_id UUID NOT NULL,
  center_name VARCHAR(255),
  protocol_version VARCHAR(64) NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  update_time TIMESTAMP
);

COMMENT ON TABLE center_config IS '中控系统身份和默认协议版本配置。';
CREATE UNIQUE INDEX IF NOT EXISTS uq_center_config_center_id ON center_config(center_id);

CREATE TABLE IF NOT EXISTS signature_key (
  id BIGSERIAL PRIMARY KEY,
  key_id UUID NOT NULL,
  owner_type VARCHAR(32) NOT NULL DEFAULT 'CENTER',
  owner_code VARCHAR(255) NOT NULL,
  signature_alg VARCHAR(64) NOT NULL DEFAULT 'HMAC-SHA256',
  encrypted_key TEXT NOT NULL,
  status VARCHAR(32) NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  retire_time TIMESTAMP,
  last_use_time TIMESTAMP,
  CONSTRAINT ck_signature_key_owner_type CHECK (owner_type = 'CENTER'),
  CONSTRAINT ck_signature_key_status CHECK (status IN ('ACTIVE', 'DISABLED', 'REVOKED'))
);

COMMENT ON TABLE signature_key IS '中控协议签名密钥元数据；不下发给边缘端，不用于文件加密。';
CREATE UNIQUE INDEX IF NOT EXISTS uq_signature_key_key_id ON signature_key(key_id);

CREATE TABLE IF NOT EXISTS data_key (
  id BIGSERIAL PRIMARY KEY,
  data_key_id UUID NOT NULL,
  disk_id UUID NOT NULL,
  edge_code VARCHAR(255),
  export_job_id UUID,
  seal_id UUID,
  encryption_alg VARCHAR(64) NOT NULL DEFAULT 'AES-256-GCM',
  encrypted_key TEXT NOT NULL,
  key_wrap_alg VARCHAR(64) NOT NULL,
  status VARCHAR(32) NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  activate_time TIMESTAMP,
  issued_time TIMESTAMP,
  sealed_time TIMESTAMP,
  retire_time TIMESTAMP,
  last_use_time TIMESTAMP,
  remark TEXT,
  CONSTRAINT ck_data_key_status CHECK (status IN ('ACTIVE', 'ISSUED', 'SEALED_READONLY', 'RETIRED', 'REVOKED'))
);

COMMENT ON TABLE data_key IS '运输盘对象数据加密密钥表；不保存明文 disk_data_key。';
COMMENT ON COLUMN data_key.encrypted_key IS '加密后的 256-bit 数据密钥，运输盘和 manifest 只保存 data_key_id。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_data_key_data_key_id ON data_key(data_key_id);
CREATE INDEX IF NOT EXISTS idx_data_key_disk_id ON data_key(disk_id);
CREATE INDEX IF NOT EXISTS idx_data_key_seal_id ON data_key(seal_id);

CREATE TABLE IF NOT EXISTS import_job (
  id BIGSERIAL PRIMARY KEY,
  import_job_id UUID NOT NULL,
  disk_id UUID NOT NULL,
  seal_id UUID NOT NULL,
  export_job_id UUID NOT NULL,
  manifest_sha256 VARCHAR(64) NOT NULL,
  edge_code VARCHAR(255) NOT NULL,
  status VARCHAR(32) NOT NULL,
  object_count BIGINT NOT NULL DEFAULT 0,
  imported_count BIGINT NOT NULL DEFAULT 0,
  total_bytes BIGINT NOT NULL DEFAULT 0,
  imported_bytes BIGINT NOT NULL DEFAULT 0,
  start_time TIMESTAMP,
  finish_time TIMESTAMP,
  error_message TEXT,
  CONSTRAINT ck_import_job_status CHECK (status IN ('PENDING', 'IMPORTING', 'DONE', 'FAILED', 'CANCELLED'))
);

COMMENT ON TABLE import_job IS '中控端每次导入运输盘的任务；同一 disk_id + seal_id 活动/完成任务唯一。';
COMMENT ON COLUMN import_job.status IS '导入任务状态；API/WS 序列化为 import_job_status。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_import_job_business_id ON import_job(import_job_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_import_job_seal_active_or_done
  ON import_job(disk_id, seal_id)
  WHERE status IN ('PENDING', 'IMPORTING', 'DONE');
CREATE INDEX IF NOT EXISTS idx_import_job_seal_manifest ON import_job(disk_id, seal_id, manifest_sha256);

CREATE TABLE IF NOT EXISTS chunk_import_group (
  id BIGSERIAL PRIMARY KEY,
  chunk_group_id UUID NOT NULL,
  edge_code VARCHAR(255) NOT NULL,
  source_bucket VARCHAR(255) NOT NULL,
  source_key TEXT NOT NULL,
  source_etag VARCHAR(255) NOT NULL,
  source_size_bytes BIGINT NOT NULL,
  source_last_modified TIMESTAMP NOT NULL,
  chunk_total INTEGER NOT NULL,
  received_count INTEGER NOT NULL DEFAULT 0,
  received_bytes BIGINT NOT NULL DEFAULT 0,
  status VARCHAR(32) NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  update_time TIMESTAMP,
  error_message TEXT,
  CONSTRAINT ck_chunk_import_group_status CHECK (status IN ('PENDING', 'PARTIAL', 'READY', 'IMPORTING', 'DONE', 'FAILED'))
);

COMMENT ON TABLE chunk_import_group IS '跨盘大对象导入聚合状态；全部分块到齐并校验后才能入账。';
CREATE UNIQUE INDEX IF NOT EXISTS uq_chunk_import_group_id ON chunk_import_group(chunk_group_id);

CREATE TABLE IF NOT EXISTS chunk_import_part (
  id BIGSERIAL PRIMARY KEY,
  chunk_group_id UUID NOT NULL,
  chunk_index INTEGER NOT NULL,
  chunk_total INTEGER NOT NULL,
  chunk_offset_bytes BIGINT NOT NULL,
  chunk_size_bytes BIGINT NOT NULL,
  disk_id UUID NOT NULL,
  seal_id UUID NOT NULL,
  import_job_id UUID NOT NULL,
  relative_data_path TEXT NOT NULL,
  ciphertext_sha256 VARCHAR(64) NOT NULL,
  plaintext_sha256 VARCHAR(64),
  create_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

COMMENT ON TABLE chunk_import_part IS '跨盘大对象分块明细；同一 chunk_group_id + chunk_index 只能登记一次。';
CREATE UNIQUE INDEX IF NOT EXISTS uq_chunk_import_part_group_index ON chunk_import_part(chunk_group_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_chunk_import_part_import_job ON chunk_import_part(import_job_id);

CREATE TABLE IF NOT EXISTS object_ledger (
  id BIGSERIAL PRIMARY KEY,
  ledger_id UUID NOT NULL,
  edge_code VARCHAR(255) NOT NULL,
  source_bucket VARCHAR(255) NOT NULL,
  source_key TEXT NOT NULL,
  source_etag VARCHAR(255) NOT NULL,
  source_size_bytes BIGINT NOT NULL,
  source_last_modified TIMESTAMP NOT NULL,
  archive_bucket VARCHAR(255) NOT NULL,
  archive_key TEXT NOT NULL,
  import_job_id UUID NOT NULL,
  disk_id UUID NOT NULL,
  seal_id UUID NOT NULL,
  plaintext_sha256 VARCHAR(64),
  imported_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

COMMENT ON TABLE object_ledger IS '中控端对象导入账本和全局去重权威表。';
CREATE UNIQUE INDEX IF NOT EXISTS uq_object_ledger_ledger_id ON object_ledger(ledger_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_object_ledger_source_identity
  ON object_ledger(edge_code, source_bucket, source_key, source_etag, source_size_bytes, source_last_modified);
CREATE INDEX IF NOT EXISTS idx_object_ledger_import_job ON object_ledger(import_job_id);
