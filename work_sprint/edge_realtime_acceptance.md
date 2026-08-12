# TASK-S1-EDGE-REALTIME-QA-001 Edge 实时可视化验收计划

## 范围和边界

- 目标：在真实 Edge 前后端环境验证插盘后自动导出可视化链路，并形成截图或录像证据。
- 非破坏性准备阶段不得格式化、清盘、初始化、重新初始化、导入或导出运输盘。
- 正式端到端执行前，不手工改数据库、不手工改盘内 `disk_info.json` / manifest、不伪造成功态。
- 浏览器和验收记录不得暴露控制 token、`edge_auth_secret`、`disk_data_key`、nonce、tag、`data_key_id` 或完整连接串。

## 前置条件

- Center 已初始化一块合法运输盘，盘内 `disk_info.json.status.code = INITIALIZED`，文件系统为 ext4。
- Edge VM 已部署当前包含自动编排、WS boot 事件、Dashboard 时间线和自动流程开关的构建。
- Edge 配置启用自动导出，但保留可回滚开关；udev 仅触发 rescan，不直接执行业务导出。
- Edge 能通过 HMAC 调用 Center `/api/edge/auth`、`/api/disk/verify` 和 `/api/disk/export-key`。
- Edge PostgreSQL、Edge RustFS S3 API、Edge backend、Edge web 或反向代理均已启动。
- RustFS 源 bucket 中存在测试对象：至少 2 个稳定小文件、1 个会在扫描/导出期间变化的对象；变化对象不得进入导出队列。
- 浏览器录屏工具可用，且能同时展示 Dashboard、开发者工具 Network/WS 面板或后端事件采集日志。

## 服务检查

- Edge VM：确认系统时间同步、项目目录存在、`rustfs-transfer-edge` 与 rescan 二进制存在。
- systemd：确认 `rustfs-transfer-edge.service` active，rescan template 存在但未直接执行业务导出。
- PostgreSQL：只读确认 `export_job`、`export_object`、`disk_runtime` 表可查询。
- RustFS：只读 list/head 可用；不得删除、覆盖或修改源对象。
- Web：Dashboard 首屏只请求 `/api/edge/dashboard/summary`，WS 连接 `/ws/edge/copy-progress` 或兼容 `/ws/edge/progress`。

## 测试文件

- 稳定对象 A：小文本或图片文件，上传完成后等待稳定窗口，期望进入导出队列。
- 稳定对象 B：较大但可快速导出的二进制文件，期望产生可见 `COPY_PROGRESS`。
- 变化对象 C：扫描开始前创建，扫描或导出前修改 etag/size/last_modified，期望标记 `SOURCE_CHANGED` 或跳过，不进入有效 manifest。
- 证据文件：保留浏览器录像、关键截图、WS 事件导出、HTTP summary JSON 摘要、盘内最终只读检查摘要。

## 期望 WebSocket 事件序列

```text
DISK_DETECTED
-> DISK_CHECKING
-> DISK_READY
-> SCAN_STARTED
-> SCAN_PROGRESS
-> SCAN_DONE
-> COPY_STARTED
-> COPY_PROGRESS
-> COPY_DONE
-> SEAL_DONE
```

检查点：

- 插盘后 1 秒内 Dashboard 出现 `DISK_DETECTED` 或等价视觉反馈。
- `SCAN_PROGRESS` 展示 scanned/skipped/stable/source_changed 相关进度；变化对象不计入稳定对象导出。
- `COPY_PROGRESS` 至少每秒更新一次，并携带 `global_progress`、`disks[]` 和当前对象进度。
- `SEAL_DONE` 后 Dashboard 展示可拔盘，盘内生命周期为 `SEALED`。
- WS 断开重连后不得把终态回退为运行中，也不得创造假完成态。

## HTTP Summary 检查

- 首次加载和刷新都请求 `GET /api/edge/dashboard/summary`。
- 插盘后 summary 中 `disks[].runtime_status` 能恢复当前运行态。
- 扫描中 summary 包含 `scan.scan_event_type` 和扫描计数。
- 导出中 summary 包含 `export_job_id`、`export_job_status`、`global_progress`、`disks[]`。
- 封盘后 summary 展示终态，不包含裸 `status`，不展示 `IMPORTED` 作为 Edge 正常对象或盘状态。
- 浏览器请求不调用 `/api/edge/scan`、`/api/edge/export-jobs`、`/start`、`/recover` 等写路径。

## 截图和录像记录点

- T0：插盘前 Dashboard 空态、Network 面板已打开。
- T1：插盘后 1 秒内 Dashboard 出现检测反馈。
- T2：校验中或 READY 状态。
- T3：扫描中进度和变化对象跳过证据。
- T4：导出中全局进度、分盘进度、当前对象进度。
- T5：WS 断开/重连后状态恢复。
- T6：封盘完成 `SEAL_DONE` 与可拔盘提示。
- T7：只读检查 `.partial=0`、`disk_info.json.status.code = SEALED`、`export_job_status = SEALED`、对象状态无 `IMPORTED`。

## 正式执行前需要协调

- 主会话或用户确认 SSH 免密访问、Edge VM 当前部署版本和服务名。
- 主会话协调 Center 初始化真实运输盘，并明确插盘窗口。
- 用户确认允许进入真实插盘和自动导出执行阶段。
- 若前端依赖未安装，需先在项目内安装依赖后复跑 `npm run typecheck` 和 `npm run build`。
- 若需要浏览器录像，需确认可用的浏览器地址、反向代理端口和录屏方式。

## VM 从零主流程验收记录（2026-08-12）

- 来源窗口：`019ff3bc-1ee5-7a51-9698-61066ca179f4`。
- 执行边界：真实验收在专门 VM 窗口完成；本主工作区集成只同步记录和最小代码修复，不部署、不碰真实磁盘、不执行盘操作。
- Edge 导出：运输盘从 `INITIALIZED` 准入到 `READY`，自动扫描形成 59 个稳定对象导出任务，导出完成后 `export_job_status = SEALED`，盘内 `disk_info.json.status.code = SEALED`，`.partial=0`，Edge 对象状态无 `IMPORTED`。
- 运行态恢复：Edge 重启/刷新后 HTTP summary 可恢复封盘终态；WebSocket/HTTP 重连未造成假完成态。
- Center 导入：导入任务完成，盘内状态进入 `IMPORTED`；对象账本与归档对象数量/字节数一致，重复导入保持幂等跳过。
- 安全检查：验收记录、HTTP summary、日志和只读检查摘要未暴露 `edge_auth_secret`、`disk_data_key`、token、密码、私钥或完整连接串。
- 发现并修复：Edge 后端 `crates/edge-backend/src/control.rs` 的导出任务磁盘汇总 SQL 中，PostgreSQL `SUM(...)` 返回 NUMERIC，按 `i64` 解码会 panic；已将 `total_bytes` / `done_bytes` 聚合结果显式 cast 为 `::BIGINT`。
- 验证结果：VM 窗口通过 `cargo fmt --all -- --check`、`cargo test -p rustfs-transfer-edge`、`cargo test -p rustfs-transfer-center`；真实 Edge -> Center 主流程和重复导入验收通过。
- 验收结论：`TASK-S1-EDGE-REALTIME-QA-001` 可标记为完成；浏览器截图缺口已由 HTTP summary、journald 和只读 DB/S3/盘内证据替代记录。