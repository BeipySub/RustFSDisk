-- Edge 站点授权 KEY 收口：去掉 auth_key_id，保留一站点一把当前有效 edge_key。

ALTER TABLE edge_site
    ADD COLUMN IF NOT EXISTS edge_key_ciphertext TEXT;

UPDATE edge_site
SET edge_key_ciphertext = auth_secret_ciphertext
WHERE edge_key_ciphertext IS NULL
  AND auth_secret_ciphertext IS NOT NULL;

ALTER TABLE edge_site
    ALTER COLUMN edge_key_ciphertext SET NOT NULL;

ALTER TABLE edge_site
    ADD COLUMN IF NOT EXISTS key_updated_time TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'UTC');

DROP INDEX IF EXISTS uq_edge_site_auth_key_id;

ALTER TABLE edge_site
    DROP COLUMN IF EXISTS auth_key_id,
    DROP COLUMN IF EXISTS auth_secret_ciphertext;

COMMENT ON COLUMN edge_site.edge_key_ciphertext IS 'Edge 站点当前有效 KEY 的本机主密钥包裹密文';
COMMENT ON COLUMN edge_site.key_updated_time IS 'Edge 站点 KEY 最近生成或重置时间';
