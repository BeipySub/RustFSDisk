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

COMMENT ON TABLE disk_list IS '中控端运输盘注册表；只记录运输盘是否登记、是否启用，不记录盘内生命周期状态。';
COMMENT ON COLUMN disk_list.id IS '数据库自增主键。';
COMMENT ON COLUMN disk_list.sn IS '运输盘硬件序列号；可辅助识别，但不得作为协议业务主身份。';
COMMENT ON COLUMN disk_list.disk_id IS '中控生成的运输盘逻辑 ID；两端协议使用的稳定业务主身份。';
COMMENT ON COLUMN disk_list.capacity_bytes IS '最近一次初始化识别到的运输盘总容量字节数。';
COMMENT ON COLUMN disk_list.status IS '运输盘启用状态；TRUE 表示已启用、边缘端校验后可继续使用，FALSE 表示禁用、边缘端必须拒绝；API 序列化为 disk_enabled 或 enabled，不表示盘内生命周期。';
COMMENT ON COLUMN disk_list.create_time IS '运输盘首次录入中控的 UTC 时间。';
COMMENT ON COLUMN disk_list.last_init_time IS '中控最近一次初始化并写入 disk_info.json 的 UTC 时间。';
COMMENT ON COLUMN disk_list.remark IS '人工备注，例如硬盘编号、位置或负责人。';

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

COMMENT ON TABLE edge_site IS '边缘站点注册表；用于 HTTP 鉴权、归档 bucket 命名和来源追踪。';
COMMENT ON COLUMN edge_site.id IS '数据库自增主键。';
COMMENT ON COLUMN edge_site.edge_code IS '边缘站点编码；全局唯一，并用于生成 archive-{edge_code} 归档 bucket。';
COMMENT ON COLUMN edge_site.edge_name IS '边缘站点展示名称。';
COMMENT ON COLUMN edge_site.auth_key_id IS 'HTTP API 鉴权密钥编号；不得复用为运输盘签名密钥或数据加密密钥。';
COMMENT ON COLUMN edge_site.auth_secret_ciphertext IS '边缘端调用中控 API 的 edge_auth_secret 加密值；只用于 HTTP HMAC 鉴权，不用于文件加密。';
COMMENT ON COLUMN edge_site.status IS '边缘站点状态；ACTIVE 表示启用并允许调用中控接口，DISABLED 表示禁用并拒绝接入，ERROR 表示异常且默认拒绝接入；API 序列化为 edge_status。';
COMMENT ON COLUMN edge_site.create_time IS '站点首次注册的 UTC 时间。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_edge_site_edge_code ON edge_site(edge_code);
CREATE UNIQUE INDEX IF NOT EXISTS uq_edge_site_auth_key_id ON edge_site(auth_key_id);

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

COMMENT ON TABLE signature_key IS '中控协议签名密钥元数据表；只用于 disk_info.json 签名和验签，不下发边缘端，不用于文件加密。';
COMMENT ON COLUMN signature_key.id IS '数据库自增主键。';
COMMENT ON COLUMN signature_key.key_id IS '中控签名密钥编号；写入 disk_info.json.security.center_key_id。';
COMMENT ON COLUMN signature_key.owner_type IS '密钥所属方类型；固定为 CENTER。';
COMMENT ON COLUMN signature_key.owner_code IS '密钥所属方编码；对应部署配置中的 center_id / 中控系统 ID。';
COMMENT ON COLUMN signature_key.signature_alg IS '签名算法；固定为 HMAC-SHA256。';
COMMENT ON COLUMN signature_key.encrypted_key IS '加密后的签名密钥；不得明文落库。';
COMMENT ON COLUMN signature_key.status IS '签名密钥状态；ACTIVE 表示可用于签名和验签，DISABLED 表示停用，REVOKED 表示吊销且默认不可用。';
COMMENT ON COLUMN signature_key.create_time IS '签名密钥生成的 UTC 时间。';
COMMENT ON COLUMN signature_key.retire_time IS '签名密钥停用或轮换后的 UTC 时间。';
COMMENT ON COLUMN signature_key.last_use_time IS '签名或验签最近一次使用该密钥的 UTC 时间。';

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

COMMENT ON TABLE data_key IS '运输盘对象数据加密密钥表；中控保存密文密钥，不保存明文 disk_data_key。';
COMMENT ON COLUMN data_key.id IS '数据库自增主键。';
COMMENT ON COLUMN data_key.data_key_id IS '数据加密密钥编号；写入 disk_info.json.security.data_key_id 和 manifest。';
COMMENT ON COLUMN data_key.disk_id IS '该数据密钥当前绑定的运输盘逻辑 ID。';
COMMENT ON COLUMN data_key.edge_code IS '允许领取和使用该密钥的边缘站点编码。';
COMMENT ON COLUMN data_key.export_job_id IS '边缘端开始导出后绑定的导出任务 ID。';
COMMENT ON COLUMN data_key.seal_id IS '边缘端封盘后绑定的封盘批次 ID。';
COMMENT ON COLUMN data_key.encryption_alg IS '对象数据加密算法；固定为 AES-256-GCM。';
COMMENT ON COLUMN data_key.encrypted_key IS '加密后的 256-bit 数据密钥；运输盘和 manifest 只保存 data_key_id。';
COMMENT ON COLUMN data_key.key_wrap_alg IS '密钥包裹算法或保护方式，例如 LOCAL-MASTER-KEY 或后续 KMS 算法。';
COMMENT ON COLUMN data_key.status IS '数据密钥生命周期状态；ACTIVE 表示可发放，ISSUED 表示已发放给绑定边缘任务，SEALED_READONLY 表示已封盘只允许中控导入读取，RETIRED 表示导入完成后历史保留，REVOKED 表示疑似泄露或人工吊销且默认不可自动发放/解密。';
COMMENT ON COLUMN data_key.create_time IS '中控初始化运输盘并生成密钥的 UTC 时间。';
COMMENT ON COLUMN data_key.activate_time IS '密钥允许被边缘端领取的 UTC 时间。';
COMMENT ON COLUMN data_key.issued_time IS '首次向边缘端返回明文 disk_data_key 的 UTC 时间。';
COMMENT ON COLUMN data_key.sealed_time IS '密钥进入 SEALED_READONLY 的 UTC 时间。';
COMMENT ON COLUMN data_key.retire_time IS '运输盘导入完成并重新初始化后，旧密钥退役的 UTC 时间。';
COMMENT ON COLUMN data_key.last_use_time IS '中控发放密钥或导入解密时最近一次使用的 UTC 时间。';
COMMENT ON COLUMN data_key.remark IS '人工说明或异常原因。';

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

COMMENT ON TABLE import_job IS '中控端每次导入运输盘的任务表；记录进度、结果和重复导入防护。';
COMMENT ON COLUMN import_job.id IS '数据库自增主键。';
COMMENT ON COLUMN import_job.import_job_id IS '导入任务业务 ID；用于 API、日志和审计追踪。';
COMMENT ON COLUMN import_job.disk_id IS '本次导入对应的运输盘逻辑 ID。';
COMMENT ON COLUMN import_job.seal_id IS '本次导入对应的封盘批次 ID。';
COMMENT ON COLUMN import_job.export_job_id IS '边缘端产生该封盘数据的导出任务 ID。';
COMMENT ON COLUMN import_job.manifest_sha256 IS '本次导入使用的 export_manifest.json SHA256。';
COMMENT ON COLUMN import_job.edge_code IS '产生该导出数据的边缘站点编码。';
COMMENT ON COLUMN import_job.status IS '导入任务状态；PENDING 表示待导入，IMPORTING 表示导入中，DONE 表示导入完成，FAILED 表示导入失败，CANCELLED 表示已取消；API/WS 序列化为 import_job_status。';
COMMENT ON COLUMN import_job.object_count IS 'manifest 中待导入对象总数。';
COMMENT ON COLUMN import_job.imported_count IS '已成功导入对象数量。';
COMMENT ON COLUMN import_job.total_bytes IS 'manifest 中待导入对象总字节数。';
COMMENT ON COLUMN import_job.imported_bytes IS '已成功导入对象字节数。';
COMMENT ON COLUMN import_job.start_time IS '导入任务开始的 UTC 时间。';
COMMENT ON COLUMN import_job.finish_time IS '导入任务结束的 UTC 时间。';
COMMENT ON COLUMN import_job.error_message IS '导入任务失败、取消或异常时的错误说明。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_import_job_business_id ON import_job(import_job_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_import_job_seal_active_or_done
  ON import_job(disk_id, seal_id)
  WHERE status IN ('PENDING', 'IMPORTING', 'DONE');
CREATE INDEX IF NOT EXISTS idx_import_job_seal_manifest ON import_job(disk_id, seal_id, manifest_sha256);

CREATE TABLE IF NOT EXISTS object_frame_import (
  id BIGSERIAL PRIMARY KEY,
  object_id UUID NOT NULL,
  edge_code VARCHAR(255) NOT NULL,
  source_bucket VARCHAR(255) NOT NULL,
  source_key TEXT NOT NULL,
  source_etag VARCHAR(255) NOT NULL,
  source_size_bytes BIGINT NOT NULL,
  source_last_modified TIMESTAMP NOT NULL,
  plaintext_sha256 VARCHAR(64) NOT NULL,
  frame_index INTEGER NOT NULL,
  frame_total INTEGER NOT NULL,
  frame_offset_bytes BIGINT NOT NULL,
  frame_size_bytes BIGINT NOT NULL,
  relative_frame_path TEXT NOT NULL,
  ciphertext_size_bytes BIGINT NOT NULL,
  ciphertext_sha256 VARCHAR(64) NOT NULL,
  data_key_id UUID NOT NULL,
  nonce VARCHAR(255) NOT NULL,
  tag VARCHAR(255) NOT NULL,
  aad TEXT NOT NULL,
  disk_id UUID NOT NULL,
  seal_id UUID NOT NULL,
  export_job_id UUID NOT NULL,
  import_job_id UUID NOT NULL,
  staged_plaintext_path TEXT NOT NULL,
  staged_plaintext_sha256 VARCHAR(64) NOT NULL,
  status VARCHAR(32) NOT NULL DEFAULT 'VERIFIED',
  import_bucket VARCHAR(255),
  import_key TEXT,
  created_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  updated_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC'),
  verified_at TIMESTAMP,
  merged_at TIMESTAMP,
  error_code VARCHAR(64),
  error_message TEXT,
  CONSTRAINT ck_object_frame_import_status CHECK (status IN ('VERIFIED', 'MERGED', 'FAILED')),
  CONSTRAINT ck_object_frame_import_index CHECK (frame_index >= 0 AND frame_index < frame_total)
);

COMMENT ON TABLE object_frame_import IS '中控端 FRAMES 对象 frame 导入账本；全部 frame 到齐后合并上传并写入 object_ledger。';
COMMENT ON COLUMN object_frame_import.object_id IS 'FRAMES 对象业务 ID。';
COMMENT ON COLUMN object_frame_import.frame_index IS '当前 frame 序号，从 0 开始。';
COMMENT ON COLUMN object_frame_import.frame_total IS '对象 frame 总数。';
COMMENT ON COLUMN object_frame_import.staged_plaintext_path IS 'Center 本地已解密 frame 暂存路径；不写入运输盘或前端响应。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_object_frame_import_object_index
  ON object_frame_import(export_job_id, object_id, frame_index);
CREATE UNIQUE INDEX IF NOT EXISTS uq_object_frame_import_data_key_nonce
  ON object_frame_import(data_key_id, nonce);
CREATE INDEX IF NOT EXISTS idx_object_frame_import_merge
  ON object_frame_import(export_job_id, object_id, status, frame_index);

CREATE TABLE IF NOT EXISTS import_nonce_ledger (
  id BIGSERIAL PRIMARY KEY,
  data_key_id UUID NOT NULL,
  nonce VARCHAR(255) NOT NULL,
  disk_id UUID NOT NULL,
  seal_id UUID NOT NULL,
  export_job_id UUID NOT NULL,
  object_id UUID NOT NULL,
  storage_mode VARCHAR(16) NOT NULL,
  frame_index INTEGER,
  import_job_id UUID NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

COMMENT ON TABLE import_nonce_ledger IS '中控导入 nonce 账本；保证同一 data_key_id + nonce 历史唯一。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_import_nonce_ledger_data_key_nonce
  ON import_nonce_ledger(data_key_id, nonce);

CREATE TABLE IF NOT EXISTS object_ledger (
  id BIGSERIAL PRIMARY KEY,
  edge_code VARCHAR(255) NOT NULL,
  source_bucket VARCHAR(255) NOT NULL,
  source_key TEXT NOT NULL,
  source_etag VARCHAR(255) NOT NULL,
  source_size_bytes BIGINT NOT NULL,
  source_last_modified TIMESTAMP NOT NULL,
  plaintext_sha256 VARCHAR(64) NOT NULL,
  storage_mode VARCHAR(16) NOT NULL DEFAULT 'PACK',
  frame_total INTEGER NOT NULL DEFAULT 0,
  pack_ciphertext_sha256 VARCHAR(64),
  data_key_id UUID,
  pack_nonce VARCHAR(255),
  import_bucket VARCHAR(255) NOT NULL,
  import_key TEXT NOT NULL,
  export_job_id UUID NOT NULL,
  import_job_id UUID NOT NULL,
  imported_at TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC')
);

COMMENT ON TABLE object_ledger IS '中控对象导入账本和全局去重权威表；导入是否完成以该表和 import_job 为准。';
COMMENT ON COLUMN object_ledger.id IS '数据库自增主键。';
COMMENT ON COLUMN object_ledger.edge_code IS '源边缘站点编码。';
COMMENT ON COLUMN object_ledger.source_bucket IS '源 RustFS bucket 名称。';
COMMENT ON COLUMN object_ledger.source_key IS '源 RustFS object key。';
COMMENT ON COLUMN object_ledger.source_etag IS '源对象 ETag。';
COMMENT ON COLUMN object_ledger.source_size_bytes IS '源对象字节数。';
COMMENT ON COLUMN object_ledger.source_last_modified IS '源对象 last_modified，归一化为 UTC。';
COMMENT ON COLUMN object_ledger.plaintext_sha256 IS '源对象明文 SHA256；用于审计和内容完整性校验。';
COMMENT ON COLUMN object_ledger.storage_mode IS '对象导入时的 v2 存储模式，取值 PACK 或 FRAMES。';
COMMENT ON COLUMN object_ledger.frame_total IS 'FRAMES 对象 frame 总数；PACK 固定为 0。';
COMMENT ON COLUMN object_ledger.pack_ciphertext_sha256 IS 'PACK 对象密文 SHA256；FRAMES 对象逐 frame 记录在 object_frame_import。';
COMMENT ON COLUMN object_ledger.data_key_id IS 'PACK 对象使用的数据加密密钥编号；FRAMES 对象在 object_frame_import 中逐 frame 记录。';
COMMENT ON COLUMN object_ledger.pack_nonce IS 'PACK 对象 AES-GCM nonce；FRAMES 对象在 object_frame_import 中逐 frame 记录。';
COMMENT ON COLUMN object_ledger.import_bucket IS '中控 RustFS 归档 bucket 名称，例如 archive-site001。';
COMMENT ON COLUMN object_ledger.import_key IS '中控 RustFS 导入 object key。';
COMMENT ON COLUMN object_ledger.export_job_id IS '边缘端导出任务业务 ID；来自 manifest 或 disk_info.json。';
COMMENT ON COLUMN object_ledger.import_job_id IS '成功导入该对象的导入任务业务 ID。';
COMMENT ON COLUMN object_ledger.imported_at IS '对象上传成功且账本写入成功的 UTC 时间。';

CREATE UNIQUE INDEX IF NOT EXISTS uq_object_ledger_data_key_nonce
  ON object_ledger(data_key_id, pack_nonce)
  WHERE data_key_id IS NOT NULL AND pack_nonce IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_object_ledger_source_version
  ON object_ledger(edge_code, source_bucket, source_key, source_etag, source_size_bytes, source_last_modified);
CREATE INDEX IF NOT EXISTS idx_object_ledger_import_job ON object_ledger(import_job_id);
