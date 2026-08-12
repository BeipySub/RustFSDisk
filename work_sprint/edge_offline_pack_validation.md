# TASK-S1-P0-EDGE-OFFLINE-PACK-001 非破坏性验证记录

执行时间：2026-08-12 09:10 +08:00

## 验证边界

- 不部署、不连接真实 Edge / Center 环境。
- 不触碰真实运输盘，不执行插盘、初始化、导出、导入、清盘或重新初始化。
- 不手工改数据库伪造成功态。
- 未读取或记录任何密钥、token、密码、私钥或完整连接串。

## 已执行检查

| 检查项 | 结果 | 说明 |
|---|---|---|
| `git status --short` | 已执行 | 工作区已有多项未提交改动，包含本任务相关代码、任务表、dev_log、冻结文档目录改动和 `.codex-vmware-*` 临时文件；本验证窗口不回退、不覆盖。 |
| `docs/Edge离线打包需求调整方案.md` | 已阅读 | 明确 Edge 运行期完全不访问 Center，使用 `edge_auth_secret + edge_code + disk_id + data_key_id + export_job_id + seal_id` HMAC-SHA256 派生 32 字节 `disk_data_key`。 |
| `work_sprint/TASKS.md` 对应任务卡 | 已阅读 | `TASK-S1-P0-EDGE-OFFLINE-PACK-001` 当前为 `[~]`，验收项尚未勾选。 |
| `work_sprint/dev_log/2026-08-12.md` | 已阅读 | 已有架构协调领取记录，后续需要 Edge / Center / QA 窗口落地。 |
| `cargo fmt --all -- --check` | 通过 | 无格式差异。 |
| `cargo test -p rustfs-transfer-edge` | 通过 | 65 个 lib/main 测试、1 个 `disk_worker_tests`、2 个 `scanner` 测试和 doc tests 通过；仅有 Windows linker stdout warning。 |
| `cargo test -p rustfs-transfer-center` | 通过 | 73 个 lib/main 测试、14 个 `edge_auth` 测试、11 个 `import_worker` 测试和 doc tests 通过；仅有 Windows linker stdout warning。 |

> 首次并行运行 Edge / Center 测试时出现 build directory file lock 和首次编译超时；已改为顺序重跑并通过。

## 当前实现缺口

这些是测试窗口通过代码阅读确认的阻塞，不应在本窗口直接改主实现：

1. Edge 导出启动器仍依赖 Center export-key：
   - `crates/edge-backend/src/export_runtime.rs` 中 `ProductionExportWorkerLauncher` 仍调用 `center_client.export_key(...)`。
   - 同文件仍通过 `decode_disk_data_key(response.disk_data_key.as_deref())` 取 Center 返回的明文导出 key。
   - Edge 当前测试仍包含 `center_client::tests::*export_key*`、`disk_detection::*center_verify*` 和 `export_runtime::tests::decodes_center_export_key_only_in_memory_shape`，说明现有绿灯覆盖的是旧联网主流程，不是离线派生主流程。

2. Center 导入 Worker 仍依赖仓储中的 `data_key` 明文：
   - `crates/center-backend/src/import_worker.rs` 的 `decrypt_object` 仍调用 `repo.data_key(disk_id, object.data_key_id)`。
   - `MemoryRepository` / `SealedDiskFixture` 仍以固定 `key_bytes = vec![7_u8; 32]` 构造导入 fixture。
   - 现有 `edge_auth.rs` 仍覆盖 `/api/edge/auth`、`/api/disk/verify`、`/api/disk/export-key`，需要由后端窗口区分“保留非主流程兼容测试”还是“改写为不属于 Edge 导出主路径”。

3. 新派生规范尚缺公共可复用入口：
   - 建议在 common 或明确共享模块提供 `derive_offline_disk_data_key(...) -> [u8; 32]`。
   - Edge 和 Center 必须调用同一实现，避免两端各自拼接 HMAC message 造成漂移。

## 非破坏性验收设计

| 验收目标 | 推荐实现方式 | 当前状态 |
|---|---|---|
| Edge 导出路径不发起 Center HTTP | 用 fake `CenterHmacClient` / trait spy 或将导出启动器改为不持有 center client；测试导出 worker 启动后 HTTP 调用计数为 0。 | 阻塞：生产路径仍调用 `export_key`。 |
| Center 不可达不阻塞 Edge 导出 | 用不可达 `base_url` + 本地磁盘 fixture + memory S3/repo，确认 scan/create/start/seal 不依赖 Center。 | 阻塞：当前导出前会请求 Center export-key。 |
| 缺 `edge_auth_secret` 拒绝导出 | 配置层或 worker launcher 单测：`center.edge_auth_secret = None` 时返回标准错误，不启动 DiskWorker。 | 待补测试。 |
| Edge / Center 派生向量一致 | 新增 golden vector：固定 secret、edge_code、disk_id、data_key_id、export_job_id、seal_id，断言输出 32 字节 hex/base64；Edge/Center 均调用同一函数。 | 阻塞：共享派生函数未落地。 |
| 错误 key 解密失败且不写 ledger | 用新派生 key 生成 sealed disk fixture，再将 Center repo 中 edge secret 替换为 wrong secret，导入应 `DECRYPT_FAILED` 且 ledger 为空。 | 阻塞：Center import 尚未按 edge secret 派生。 |
| Center 禁用 Edge 后导入拒绝 | `ImportRepository` 增加 `edge_enabled(edge_code)` 或等价查询，memory repo 提供 disabled edge fixture，导入前拒绝且 ledger 为空。 | 阻塞：当前 import trait 只判断 disk 注册/启用，不判断 Edge 启用。 |
| Center 禁用运输盘后导入拒绝 | 现有 `disk_enabled` 可覆盖；补单测将目标 disk 标为 disabled，期待拒绝且 ledger 为空。 | 待补测试，当前基础接口具备。 |
| 重复 `disk_id + seal_id` 不重复导入 | 现有 `skips_same_seal_when_done_with_same_manifest_sha256` 已覆盖同 seal 跳过，ledger 仍为 1。 | 已覆盖旧 key 模型；需在新派生 fixture 下复测。 |
| 敏感信息不落盘/日志/WS/HTTP | 扫描协议目录文本、manifest、HTTP summary、WS payload；断言不包含 secret 明文和派生 key base64/hex。 | Edge DiskWorker 已覆盖 disk key base64 不落盘；需扩展到 `edge_auth_secret`、HTTP/WS 和日志。 |

## 建议给实现窗口的 fixture

1. 在 common 测试中创建 golden vector：

```text
edge_auth_secret = "edge-secret-for-offline-pack"
edge_code = "edge-a"
disk_id = "11111111-1111-4111-8111-111111111111"
data_key_id = "22222222-2222-4222-8222-222222222222"
export_job_id = "33333333-3333-4333-8333-333333333333"
seal_id = "44444444-4444-4444-8444-444444444444"
```

2. 将 `SealedDiskFixture` 改为支持两种 key 来源：
   - `StaticKey([u8; 32])`：保留旧测试最小改动。
   - `DerivedFromEdgeSecret { edge_auth_secret, edge_code }`：生成新离线封盘 fixture。

3. Center `MemoryRepository` 建议补充：
   - `register_edge(edge_code, edge_auth_secret)`
   - `disable_edge(edge_code)`
   - `edge_auth_secret(edge_code) -> Option<String>`

4. Edge 导出路径建议补 spy：
   - `NoopCenterClient` 或移除生产 launcher 的 Center client 依赖。
   - 测试中将 `base_url` 设置为不可达地址，断言仍能构造 worker config 并封盘。

## 结论

当前 Rust 基线测试通过，但尚不能说明 `TASK-S1-P0-EDGE-OFFLINE-PACK-001` 已满足验收。现有通过的测试仍覆盖旧的 Center verify/export-key 联网主流程；离线派生、Center 导入阶段 Edge/盘禁用拒绝、错误授权 key 解密失败等关键验收仍需要实现窗口补齐后再复测。

---

## 集成后复测记录

执行时间：2026-08-12 集成窗口

### 已集成实现

- Edge 后端主流程已移除 `CenterHmacClient`、`/api/edge/auth`、`/api/disk/verify`、`/api/disk/export-key` 依赖；旧 Edge `center_client.rs` 已删除，crate 不再导出该客户端。
- Edge 插盘准入改为本地离线准入：ext4、协议文件、协议版本、`center_signature` 存在、`status.code = INITIALIZED` 和恢复检查。
- Edge 导出 Worker 使用 `center.edge_auth_secret + edge_code + disk_id + data_key_id + export_job_id + seal_id` HMAC-SHA256 派生 32 字节 `disk_data_key`，只在内存传入 `DiskWorkerConfig`。
- Center 导入 Worker 通过 ACTIVE `edge_site` 授权 secret 按同一规范派生解密 key，并在导入阶段执行 disk 注册/启用、Edge ACTIVE、data_key 绑定、重复导入、manifest、AAD、nonce、密文/明文校验。
- 部署示例和 `scripts/check-deploy.ps1` 已更新离线导出语义；`center.base_url` 与 `center.auth_key_id` 仅作为兼容/审计字段，不再是 Edge 导出必需项。

### 复测结果

| 检查项 | 结果 |
|---|---|
| `cargo fmt --all -- --check` | 通过 |
| `cargo test -p rustfs-transfer-edge` | 通过，63 个库/单元测试、1 个 disk worker 集成测试、2 个 scanner 集成测试和 doc tests 通过 |
| `cargo test -p rustfs-transfer-center` | 通过，73 个库/单元测试、14 个 edge_auth 测试、15 个 import_worker 测试和 doc tests 通过 |
| `powershell -ExecutionPolicy Bypass -File scripts/check-deploy.ps1` | 通过 |
| Edge 主流程静态搜索 | `rg -n "CenterHmacClient|export_key\(|/api/edge/auth|/api/disk/verify|/api/disk/export-key" crates/edge-backend/src crates/edge-backend/tests -S` 无结果 |
| 敏感字段静态审计 | 命中仅限配置读取、内存派生/加密、测试断言和文档说明；WS/HTTP summary 路径中为“不返回”测试断言 |

### 仍不在本次范围

- 未部署服务。
- 未连接真实 Edge / Center 环境。
- 未触碰真实运输盘。
- 未执行真实插盘、初始化、导出、导入、清盘或重新初始化。
