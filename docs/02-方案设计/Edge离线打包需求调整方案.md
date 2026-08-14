# Edge 离线打包需求调整方案

## 1. 调整背景

当前实现中，Edge 运行期依赖 Center HTTP 接口完成运输盘准入和导出密钥领取。项目实际代码中的 Edge 配置为：

```toml
[center]
base_url = "http://center.example.com:8080"
edge_code = "edge-demo"
auth_key_id = "CHANGE_ME_AUTH_KEY_ID"
edge_auth_secret_env = "RUSTFS_TRANSFER__CENTER__EDGE_AUTH_SECRET"
```

新需求明确要求 Edge 端不允许联网，因此 Edge 运行期不能再调用 Center 接口判断硬盘信息，也不能通过 Center 接口领取 `disk_data_key`。

本方案作为 v1.0 冻结文档的补充实施方案，不修改 `docs/v1.0冻结/` 下任何文件。

## 2. 调整目标

- Edge 运行期完全不访问 Center。
- 删除 Edge 到 Center 的联网验证和取密钥主流程。
- 不新增数据库字段。
- 不新增 `disk_info.json` 字段。
- 复用当前 Edge 配置中的 `center.edge_code`、`center.edge_auth_secret` / `center.edge_auth_secret_env`。
- `center.edge_auth_secret` 作为部署阶段写入 Edge 的固定授权 key，后期不轮换。
- Edge 离线完成扫描、导出、加密、封盘。
- Center 在运输盘回插后完成最终校验、密钥派生、解密、导入和去重。

## 3. 现有字段复用

Edge 侧继续使用当前已有配置字段：

| 字段 | 新流程用途 |
|---|---|
| `center.edge_code` | Edge 本机站点编码，写入现有 `disk_info.json.edge.edge_code` 和 manifest 来源字段。 |
| `center.edge_auth_secret` | Edge 部署阶段写入的固定授权 key，用于本地派生本次封盘 `disk_data_key`。 |
| `center.edge_auth_secret_env` | 从环境变量读取 `center.edge_auth_secret` 的现有配置方式。 |
| `center.auth_key_id` | 保留为现有配置字段和审计标识，不参与 Edge 运行期联网签名。 |
| `center.base_url` | 删除联网主流程后，不再参与 Edge 导出路径。 |

Center 侧继续复用当前已有 `edge_site` 记录保存同一份授权 key。Center 导入时按 `edge_code` 找到对应 Edge 记录，取回 Center 侧保存的授权 key 明文，用于派生解密密钥。

运输盘继续使用当前已有协议字段：

| 字段 | 新流程用途 |
|---|---|
| `disk_info.json.disk.disk_id` | 运输盘业务身份。 |
| `disk_info.json.security.data_key_id` | 本次初始化生成的数据密钥编号，参与派生。 |
| `disk_info.json.security.center_key_id` | Center 签名密钥编号。 |
| `disk_info.json.security.center_signature` | Center 写入盘身份和安全基础字段后的签名。 |
| `disk_info.json.edge.edge_code` | Edge 封盘时写入来源站点编码。 |
| `disk_info.json.edge.export_job_id` | Edge 封盘时写入导出任务 ID。 |
| `disk_info.json.edge.seal_id` | Edge 封盘批次 ID，参与派生。 |
| `disk_info.json.status.code` | 运输盘生命周期状态。 |

## 4. 删除的联网主流程

Edge 运行期删除以下 Center 调用：

```text
POST /api/edge/auth
POST /api/disk/verify
POST /api/disk/export-key
对象账本联网查询接口
```

Edge 不再依赖以下 HTTP 鉴权字段：

```text
X-Auth-Key-Id
X-Timestamp
X-Nonce
X-Body-SHA256
X-Signature
```

Center 端对应路由从新主流程中清空，不作为 Edge 离线导出的兼容路径保留。

## 5. Center 初始化运输盘流程

Center 初始化运输盘仍按当前职责执行：

```text
Center 插入运输盘
-> 检查 ext4
-> 注册或识别 disk_id
-> 生成 data_key_id
-> 写入 disk_info.json
-> 写入 status.code = INITIALIZED
-> 写入 center_signature
-> 交付 Edge
```

Center 初始化阶段不把 Edge 授权 key 写入运输盘。

## 6. Edge 插盘离线准入流程

Edge 插入运输盘后只执行本地准入：

```text
检查文件系统为 ext4
-> 读取 disk_info.json
-> 检查协议版本
-> 校验 center_signature
-> 检查 status.code = INITIALIZED
-> 检查 .partial 残留和恢复状态
-> 通过后进入 READY
```

Edge 离线准入阶段不判断以下 Center 权威状态：

```text
disk_id 是否仍在 Center 注册
运输盘是否仍启用
edge_code 是否仍启用
```

这些判断统一后移到 Center 导入阶段。

## 7. Edge 离线导出和密钥派生

Edge 创建导出任务后执行：

```text
扫描本地 RustFS
-> 创建 export_job
-> 生成 seal_id
-> 写入现有 edge_code / export_job_id / seal_id
-> 读取 disk_id
-> 读取 data_key_id
-> 读取 center.edge_auth_secret
-> 派生本次 disk_data_key
-> 使用 disk_data_key 执行 AES-256-GCM 加密写盘
-> 写 metadata
-> 写 export_manifest.json
-> fsync 和原子重命名
-> 写入 status.code = SEALED
-> 清理内存中的 disk_data_key
```

`center.edge_auth_secret` 不直接作为 AES-256-GCM 密钥使用，只作为派生输入。派生输入固定为：

```text
edge_auth_secret
edge_code
disk_id
data_key_id
export_job_id
seal_id
```

派生规范：

```text
disk_data_key = HMAC-SHA256(
  key = UTF-8 bytes of center.edge_auth_secret,
  message =
    "rustfs-transfer:offline-disk-data-key:v1\n" +
    "edge_code=" + edge_code + "\n" +
    "disk_id=" + disk_id + "\n" +
    "data_key_id=" + data_key_id + "\n" +
    "export_job_id=" + export_job_id + "\n" +
    "seal_id=" + seal_id
)
```

HMAC-SHA256 输出 32 字节，直接作为本次封盘的 AES-256-GCM `disk_data_key`。

安全约束：

- Edge 不保存明文 `disk_data_key` 到数据库。
- Edge 不写明文 `disk_data_key` 到运输盘。
- Edge 不把 `center.edge_auth_secret` 写入运输盘、manifest、日志或 WebSocket。
- Edge 不修改 `disk_info.json` 中 Center 签名覆盖的身份和安全基础字段。

## 8. Center 导入流程

Center 插入 `SEALED` 盘后执行：

```text
检查 ext4
-> 读取 disk_info.json
-> 校验 center_signature
-> 校验 status.code = SEALED
-> 校验 disk_id 在 Center 已注册且启用
-> 读取 edge_code / export_job_id / seal_id / data_key_id
-> 校验 edge_code 在 Center 已注册且启用
-> 读取 Center 侧保存的同一份 Edge 授权 key
-> 按同一派生规范生成 disk_data_key
-> 校验 manifest SHA256
-> 校验 manifest 字段和分块连续性
-> 校验密文 SHA256
-> AES-GCM 解密对象或分块
-> 校验明文 SHA256
-> 上传 archive-{edge_code}
-> 写 object_ledger
-> 标记 IMPORTED
-> 清理并重新初始化
```

Center 导入阶段是最终权威判断点。运输盘禁用、Edge 禁用、重复导入、manifest 非法、无法派生正确解密密钥、校验失败，都在导入阶段拒绝入库。

## 9. 字段和接口影响

继续使用的字段：

```text
center.edge_code
center.edge_auth_secret
center.edge_auth_secret_env
disk_info.json.disk.disk_id
disk_info.json.security.data_key_id
disk_info.json.security.center_key_id
disk_info.json.security.center_signature
disk_info.json.edge.edge_code
disk_info.json.edge.export_job_id
disk_info.json.edge.seal_id
disk_info.json.status.code
```

不再参与 Edge 导出主流程的字段和能力：

```text
center.base_url
center.auth_key_id 的 HTTP 签名用途
HTTP HMAC canonical request
请求 nonce 防重放缓存
POST /api/edge/auth
POST /api/disk/verify
POST /api/disk/export-key
ExportKeyResponse.disk_data_key
VerifyDiskResponse.allowed
VerifyDiskResponse.disk_enabled
VerifyDiskResponse.expected_status
VerifyDiskResponse.action
```

Center `data_key` 表中的 `data_key_id` 继续作为盘内密钥编号和派生输入。新流程不再通过 `/api/disk/export-key` 向 Edge 发放明文 `disk_data_key`。

## 10. 权威判断调整

调整前：

```text
Edge 导出前通过 Center 在线接口确认 Edge、运输盘和 data_key 均允许使用。
```

调整后：

```text
Edge 只负责离线准入和封盘。
Center 在导入时最终确认是否允许入库。
```

因此 Edge 离线期间无法实时感知 Center 侧禁用 Edge 或禁用运输盘。该取舍由 Center 导入阶段拒绝入库来兜底。

## 11. 实施任务拆分

建议新增 P0 任务：

```text
TASK-S1-P0-EDGE-OFFLINE-PACK-001
Edge 离线打包：删除运行期 Center 校验和 export-key，复用 edge_auth_secret 派生封盘 disk_data_key
```

实施范围：

- Edge 删除 Center verify/export-key 调用路径。
- Edge 插盘准入改为本地协议校验。
- Edge 导出运行时用 `center.edge_auth_secret` 派生 `disk_data_key`。
- Center 导入 Worker 用同一派生规范生成解密密钥。
- Center 清空 Edge 运行期联网验证主流程对应路由。
- 部署示例保留 `center.edge_code` 和 `center.edge_auth_secret_env`，移除 Edge 导出对 `center.base_url` 的必需校验。
- 测试覆盖断网导出、密钥派生一致性、错误授权 key 解密失败、Center 禁用 Edge 后导入拒绝。

## 12. 验收标准

- Edge 断网状态下完成插盘、扫描、导出、封盘。
- Edge 导出路径不发起任何 Center HTTP 请求。
- Center 不可达不阻塞 Edge 导出。
- Edge 配置缺少 `center.edge_auth_secret` 时拒绝进入导出。
- 非 ext4 运输盘仍拒绝。
- `status.code != INITIALIZED` 的运输盘仍拒绝 Edge 导出。
- `.partial` 残留未通过恢复检查时仍拒绝导出。
- Edge 不落库、不落盘保存明文 `disk_data_key`。
- Edge 不在日志、WebSocket、HTTP summary 中暴露 `center.edge_auth_secret` 或 `disk_data_key`。
- Center 使用同一派生规范成功解密并导入合法封盘。
- Center 禁用 Edge 后，对应封盘在导入阶段拒绝入库。
- Center 禁用运输盘后，对应封盘在导入阶段拒绝入库。
- 重复插入同一 `disk_id + seal_id` 不重复导入。

## 13. 一句话结论

Edge 不再联网校验和领取密钥；复用现有 `center.edge_auth_secret` 作为部署阶段固定授权 key；Edge 与 Center 分别用同一授权 key 和现有盘内字段派生本次 `disk_data_key`；Edge 离线封盘，Center 回收后最终验收和导入；整个方案不新增字段。

## 14. 配置与安全审计补充

- `deploy/config/edge.example.toml` 和 `edge.env.example` 可以保留 `center.base_url` 与 `center.auth_key_id`，但必须标注为兼容或审计字段，不参与 Edge 离线导出主流程，也不得作为导出前置校验。
- `center.edge_auth_secret_env` 是读取部署阶段授权密钥的来源；示例只能写环境变量名或 `CHANGE_ME` 占位值，不得写真实密钥、token、数据库密码、RustFS secret 或完整生产连接串。
- `center.edge_auth_secret` 只允许作为 HMAC-SHA256 派生输入，生成本次封盘 `disk_data_key`；不得直接作为 AES-256-GCM 密钥。
- `edge_auth_secret` 和明文 `disk_data_key` 不得进入日志、WebSocket、HTTP summary、浏览器只读接口、manifest、`disk_info.json` 或边缘端数据库。
- 当前 AGENTS.md 与冻结文档仍写明 `edge_auth_secret` 只用于 HTTP API 鉴权；本方案是新增补充语义。是否需要更新 AGENTS.md 或新增更高优先级补充约束，需要主会话决策，不能在本审计窗口直接修改 AGENTS.md。
