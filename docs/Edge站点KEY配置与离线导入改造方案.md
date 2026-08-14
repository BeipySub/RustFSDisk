# Edge 站点 KEY 配置与离线导入改造方案

## 1. 目标

本方案用于把 Edge 站点授权、Edge 本地配置、导出加密和 Center 导入解密收敛成一条可理解、可实施的链路：

```text
Center 创建 Edge 站点
-> Center 生成 Edge KEY
-> Center 页面可查看 Edge KEY
-> 运维把 Edge KEY 写入 Edge 配置文件
-> Edge 导出时用 Edge KEY 派生本次 disk_data_key
-> Center 导入时用同一把 Edge KEY 派生 disk_data_key 并解密导入
```

本方案是 v1.0 冻结文档之后的补充改造方案，不修改 `docs/v1.0冻结/` 下任何文件。

## 2. 核心原则

- 对外只保留一个站点授权概念：`edge_key`。
- `edge_code` 负责识别站点，`edge_key` 负责证明该站点可信。
- Edge 配置文件中直接写 `edge_code` 和 `edge_key`，不再通过环境变量指针间接查找。
- Center 数据库仍加密保存 `edge_key`，但 Center 管理接口和页面允许查看明文 `edge_key`。
- `edge_key` 只用于站点互认和派生本次导出的 `disk_data_key`，不得直接作为 AES-256-GCM 数据加密密钥。
- `disk_data_key` 只存在于 Edge 导出和 Center 导入的内存中，不得写入 Edge 数据库、运输盘、manifest、WebSocket 或日志。

## 3. 去除冗余字段

以下字段和配置需要从新实现中剔除，不保留兼容分支：

```text
center.base_url
center.auth_key_id
center.edge_auth_secret
center.edge_auth_secret_env
server.control_api_token
auth_key_id
edge_auth_secret
```

原因：

- 当前离线导出主流程不依赖 Edge 运行期访问 Center。
- 一条 Edge 站点只保留一把当前有效 KEY，`edge_code` 已经可以唯一定位站点，不需要 `auth_key_id`。
- `edge_auth_secret_env` 只是“去哪个环境变量读密钥”的间接层，会增加配置理解成本。
- 同时保留 `edge_auth_secret` 和 `edge_key` 会造成两套授权语义并存，后续阅读和维护更难。
- Edge 主流程采用插盘自动识别、自动扫描、自动导出，浏览器前端只读展示，不交付手动控制接口，因此不保留 `control_api_token`。

Edge 配置结构必须一次性收口，避免继续使用 `[center]` 承载旧字段。新配置使用独立 `[edge]` 段。

## 4. Center 数据库改造

当前 `edge_site` 建议从：

```text
edge_code
edge_name
auth_key_id
auth_secret_ciphertext
status
create_time
```

改为：

```text
edge_code
edge_name
edge_key_ciphertext
status
create_time
key_updated_time
```

字段含义：

| 字段 | 含义 |
|---|---|
| `edge_code` | Edge 站点唯一编码，用于站点识别、归档 bucket 命名和 manifest 来源追踪。 |
| `edge_name` | 展示名称。 |
| `edge_key_ciphertext` | 使用 Center 本机主密钥加密后的 Edge KEY。 |
| `status` | 数据库内部状态字段，API 序列化为 `edge_status`。可取 `ACTIVE`、`DISABLED`、`ERROR`。 |
| `create_time` | 站点创建 UTC 时间。 |
| `key_updated_time` | Edge KEY 最近生成或重置的 UTC 时间。 |

迁移要求：

- 新增迁移文件，不改写已落地迁移。
- 删除 `auth_key_id` 唯一索引和字段。
- 将 `auth_secret_ciphertext` 重命名为 `edge_key_ciphertext`。
- 为 `key_updated_time` 填充当前 UTC 时间或沿用 `create_time`。
- 保持 `edge_code` 唯一约束。

## 5. Center API 改造

### 5.1 新建 Edge 站点

请求：

```json
{
  "edge_code": "edge-a",
  "edge_name": "边缘节点A"
}
```

后端处理：

```text
校验 edge_code 唯一
-> 自动生成 edge_key
-> 加密 edge_key 为 edge_key_ciphertext
-> 写入 edge_site
-> 返回明文 edge_key
```

响应：

```json
{
  "edge_code": "edge-a",
  "edge_name": "边缘节点A",
  "edge_key": "Center生成的明文KEY",
  "edge_status": "ACTIVE",
  "create_time": "2026-08-14T00:00:00Z",
  "key_updated_time": "2026-08-14T00:00:00Z"
}
```

### 5.2 Edge 站点列表

按当前运维诉求，列表接口允许返回可查看的 `edge_key`。

响应：

```json
{
  "items": [
    {
      "edge_code": "edge-a",
      "edge_name": "边缘节点A",
      "edge_key": "Center解密后的明文KEY",
      "edge_status": "ACTIVE",
      "object_count": 0,
      "create_time": "2026-08-14T00:00:00Z",
      "key_updated_time": "2026-08-14T00:00:00Z"
    }
  ]
}
```

安全边界：

- `edge_key` 可以返回给 Center 管理页面，但不得写入服务端日志、浏览器 localStorage、运输盘、manifest 或 WebSocket。
- Center 管理页面必须视为受信任运维入口，不得开放到不受控网络。
- 后续如增加登录权限，查看 `edge_key` 应作为高权限操作。

### 5.3 更新 Edge 站点

只允许更新：

```json
{
  "edge_name": "边缘节点A",
  "edge_status": "ACTIVE"
}
```

不允许通过普通更新接口改 `edge_code` 或 `edge_key`。

### 5.4 重置 Edge KEY

接口：

```text
POST /api/center/edge-sites/{edge_code}/key/reset
```

处理：

```text
生成新 edge_key
-> 加密覆盖 edge_key_ciphertext
-> 更新 key_updated_time
-> 返回明文 edge_key
```

响应：

```json
{
  "edge_code": "edge-a",
  "edge_name": "边缘节点A",
  "edge_key": "新的明文KEY",
  "edge_status": "ACTIVE",
  "key_updated_time": "2026-08-14T00:00:00Z"
}
```

运维动作：

```text
把新 edge_key 写入 Edge 配置文件
-> 重启 rustfs-transfer-edge
```

## 6. Edge 配置文件改造

Edge 配置文件使用：

```text
/etc/rustfs-transfer/edge.toml
```

站点配置改为：

```toml
[edge]
edge_code = "edge-a"
edge_key = "Center页面显示的KEY"
```

保留其他业务配置，例如：

```toml
[server]
bind = "0.0.0.0:8081"

[database]
url = "postgres://rustfs_transfer_edge:密码@127.0.0.1:5432/rustfs_transfer_edge"

[rustfs]
endpoint = "http://127.0.0.1:9000"
region = "us-east-1"
access_key_id = "RustFS访问KEY"
secret_access_key = "RustFS访问SECRET"

[paths]
data_dir = "/var/lib/rustfs-transfer/edge"
log_dir = "/var/log/rustfs-transfer/edge"
transport_mount_root = "/mnt/rustfs-transfer"

[rescan]
token = "本机udev重扫令牌"

[scan]
reuse_window_minutes = 0

[auto_export]
enabled = true
start_on_ready = true
min_ready_disk_count = 1
cooldown_seconds = 5
```

配置校验要求：

- `edge.edge_code` 非空。
- `edge.edge_key` 非空。
- 不再接受 `center.edge_auth_secret_env` 作为替代来源。
- 不再接受 `center.auth_key_id`。
- 不再要求 `center.base_url`。
- 示例配置不得包含真实密钥，只能使用 `CHANGE_ME` 占位。

## 7. 导出加密逻辑

Edge 导出时：

```text
读取 edge.edge_code
读取 edge.edge_key
读取 disk_id
读取 data_key_id
生成 export_job_id
生成 seal_id
派生 disk_data_key
使用 disk_data_key 执行 AES-256-GCM 加密写盘
```

派生规则：

```text
disk_data_key = HMAC-SHA256(
  key = edge_key,
  message =
    "rustfs-transfer:offline-disk-data-key:v1\n" +
    "edge_code=" + edge_code + "\n" +
    "disk_id=" + disk_id + "\n" +
    "data_key_id=" + data_key_id + "\n" +
    "export_job_id=" + export_job_id + "\n" +
    "seal_id=" + seal_id
)
```

输出取 HMAC 结果的 32 字节作为 AES-256-GCM key。

约束：

- AES-256-GCM 算法不变。
- 只改密钥来源和命名：从 `edge_auth_secret` 改为 `edge_key`。
- `edge_key` 不直接参与对象文件加密，只作为 HMAC 派生输入。
- `disk_data_key` 不落库、不落盘、不进 manifest、不进日志、不进 WebSocket。

## 8. 盘内协议输出

运输盘内允许保存：

```text
edge_code
disk_id
data_key_id
export_job_id
seal_id
nonce
tag
ciphertext_sha256
plaintext_sha256
manifest
```

运输盘内禁止保存：

```text
edge_key
edge_key_ciphertext
disk_data_key
Center 数据库连接串
控制 token
RustFS secret_access_key
```

`disk_info.json.status.code` 仍然是运输盘生命周期状态的权威来源。

## 9. Center 导入解密逻辑

Center 导入时：

```text
读取 disk_info.json 和 manifest
-> 校验 center_signature
-> 校验 manifest SHA256
-> 读取 edge_code
-> 查询 edge_site
-> 确认 edge_status = ACTIVE
-> 解密 edge_key_ciphertext 得到 edge_key
-> 使用 edge_key + edge_code + disk_id + data_key_id + export_job_id + seal_id 派生 disk_data_key
-> 使用 disk_data_key 执行 AES-256-GCM 解密
-> 校验 plaintext_sha256
-> 上传 archive-{edge_code}
-> 写 object_ledger
```

对应关系：

```text
Center edge_site.edge_code
= Edge 配置 edge.edge_code
= manifest.edge_code

Center edge_site.edge_key_ciphertext 解密后
= Edge 配置 edge.edge_key
```

如果任一项不一致：

- 找不到 `edge_code`：拒绝导入。
- `edge_status != ACTIVE`：拒绝导入。
- `edge_key` 不一致：AES-GCM 解密失败，记录 `DECRYPT_FAILED`。
- `disk_id / data_key_id / export_job_id / seal_id` 不一致：派生结果不一致，解密失败或 manifest 校验失败。

## 10. 前端改造

Center Edge 管理页：

- 新建表单只保留 `edge_code`、`edge_name`。
- 新建成功后显示 `edge_key`。
- 列表显示 `edge_code`、`edge_name`、`edge_key`、`edge_status`、`object_count`、`create_time`、`key_updated_time`。
- 提供复制 `edge_key` 按钮。
- 提供重置 `edge_key` 按钮。
- 更新站点只允许改 `edge_name` 和 `edge_status`。
- 删除 `auth_key_id` 输入、显示、校验和 API 字段。

Edge 前端：

- 不展示 `edge_key`。
- 不读取 Edge 配置文件。
- 不通过 WebSocket 或 HTTP summary 暴露 `edge_key`。

## 11. 后端代码改造清单

Center：

- `CreateEdgeSiteRequest` 删除 `auth_key_id`、`edge_auth_secret`。
- `CreateEdgeSiteRequest` 只保留 `edge_code`、`edge_name`。
- `ManagedEdgeSite` 删除 `auth_key_id`，新增 `edge_key`、`key_updated_time`。
- `NewEdgeSite` 使用自动生成的 `edge_key`。
- `wrap_edge_auth_secret` 改名为 `wrap_edge_key`。
- `unwrap_edge_auth_secret` 改名为 `unwrap_edge_key`。
- 新增 `generate_edge_key`。
- 新增 `reset_edge_key` service 和 route。
- 导入 Worker 使用 `unwrap_edge_key`，不再使用 `auth_key_id`。
- 任何日志不得输出 `edge_key`。

Edge：

- `EdgeConfig` 新增 `edge: EdgeIdentityConfig`。
- `ServerConfig` 删除 `control_api_token` 和 `control_api_token_env`。
- 删除 `CenterConfig` 或至少删除其中所有旧授权字段。
- 所有读取 `config.center.edge_code` 的地方改为 `config.edge.edge_code`。
- 所有读取 `config.center.edge_auth_secret` 的地方改为 `config.edge.edge_key`。
- 删除 `RUSTFS_TRANSFER__CENTER__EDGE_AUTH_SECRET` 覆盖逻辑。
- 删除 `RUSTFS_TRANSFER__CENTER__AUTH_KEY_ID` 覆盖逻辑。
- 删除 `RUSTFS_TRANSFER__CENTER__BASE_URL` 对导出主流程的影响。
- 删除 `RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN` 覆盖逻辑。
- 删除或禁用需要 `X-Edge-Control-Token` 的手动写接口；Edge 自动导出主流程不得依赖本机控制 token。
- 导出 Worker 缺少 `edge_key` 时拒绝启动，错误码保持可机读。

Common / security：

- `derive_offline_disk_data_key(edge_auth_secret, ...)` 参数改名为 `edge_key`。
- 函数名可保留，也可改为 `derive_disk_data_key_from_edge_key`。
- 算法和 HMAC message 字段不变。
- 增加 Edge/Center 共用 golden vector 测试。

## 12. 测试要求

Center 后端：

- 新建 Edge 自动生成 `edge_key`。
- 数据库不明文保存 `edge_key`。
- 列表接口可以返回解密后的 `edge_key`。
- 重置 KEY 后旧 KEY 不能再解密新封盘数据。
- `edge_status = DISABLED` 时导入拒绝。
- 日志不包含 `edge_key`。

Edge 后端：

- 缺少 `[edge].edge_key` 时启动或导出拒绝。
- 使用 `[edge].edge_key` 能成功派生 `disk_data_key`。
- 不再需要 `center.base_url`、`center.auth_key_id`、`center.edge_auth_secret_env`。
- 不再需要 `server.control_api_token`。
- 浏览器前端不调用 Edge 手动写接口。
- 导出过程不访问 Center `/api/disk/verify` 或 `/api/disk/export-key`。
- manifest 和 `disk_info.json` 不包含 `edge_key`。

端到端：

```text
Center 创建 Edge 并显示 KEY
-> 将 KEY 写入 Edge 配置
-> Edge 自动导出并封盘
-> Center 读取 SEALED 盘
-> Center 按 edge_code 查到 edge_key
-> Center 派生 disk_data_key
-> 导入成功并写 object_ledger
```

## 13. 实施顺序

1. 新增 Center 数据库迁移，调整 `edge_site` 字段。
2. 改 Center security 命名和 KEY 生成/解密能力。
3. 改 Center Edge 管理 API 和前端页面。
4. 改 Edge 配置结构，删除旧 `[center]` 授权配置。
5. 改 Edge 导出 Worker 的密钥来源。
6. 改 Center 导入 Worker 的密钥来源。
7. 更新部署示例和上线说明。
8. 补齐单元测试、集成测试和一次真实闭环验证。

## 14. 明确不做

- 不引入注册文件。
- 不引入 KEY ID。
- 不保留旧 `edge_auth_secret_env` 兼容路径。
- 不保留 `control_api_token` 和 Edge 手动控制接口作为交付配置。
- 不把 `edge_key` 写入运输盘。
- 不把 `disk_data_key` 写入数据库、运输盘或日志。
- 不修改冻结文档。
