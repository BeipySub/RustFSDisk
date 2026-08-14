-- Center 签名密钥已收口到部署环境变量，运行时代码不再读取 signature_key 表。
DROP TABLE IF EXISTS signature_key;
