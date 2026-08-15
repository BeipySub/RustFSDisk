# RustFS 离线运输盘同步 2 天 Sprint 任务列表

本文档用于 2 天敏捷冲刺期间拆分任务、领取任务和追踪状态。需求、协议、数据库和架构基线以 `docs/v1.0冻结/` 为准，本文件只做实施拆分，不替代冻结文档。

## 状态约定

- `[ ]` 未开始
- `[~]` 开发中
- `[x]` 开发完成

## Sprint 目标

2 天内完成可部署、可联调、可演示的 v1.0 主闭环：

```text
中控注册/初始化运输盘
-> 边缘识别已初始化盘并通过中控校验
-> 边缘扫描 RustFS 并多盘分配对象
-> 边缘加密写盘、生成 manifest、封盘
-> 中控识别 SEALED 盘、校验、解密、上传归档 bucket、写 object_ledger
-> 中控清理并重新初始化运输盘
-> 双端 Dashboard 展示实时进度和异常
```

## 修正后的拆分原则

- 上游任务必须薄切，避免 Common、Center Day 1、Edge Day 1 形成串行瓶颈。
- 中控导入不等待边缘真实封盘，使用 manifest fixture 和模拟运输盘目录并行开发。
- 测试与部署从 Day 1 开始准备 fixture、脚本和联调环境，不等 Day 2 收尾。
- Web 按中控端和边缘端拆分，双端独立部署、独立联调。
- 跨盘分块在 2 天主闭环内至少做到字段校验、分块登记、缺块不可入账；完整大对象合并如未完成，必须明确记录为 v1.0 风险项，不能宣称全量完成。

## Day 1 / Day 2 排期

| Day | 必须产出 | 验收线 |
|---|---|---|
| Day 1 上午 | Common 协议/加密/文件工具分拆完成，双端 DDL 冻结，测试 fixture 初始化 | Common 三个 crate 测试通过；迁移可在空库执行；fixture 可生成模拟盘和 manifest |
| Day 1 下午 | 中控基础 API、边缘磁盘检测、RustFS 扫描、多盘分配、双端 Web mock 页面并行推进 | 中控可注册/初始化/验签/发密钥；边缘可校验盘、扫描对象、创建导出任务和分配记录；Web 可用 mock 展示 |
| Day 2 上午 | 边缘写盘封盘、中控基于 fixture 导入、双端 WS 对接 | 边缘能生成 SEALED 盘；中控能导入 fixture 盘；WS 高频进度来自 ProgressAggregator |
| Day 2 下午 | 主闭环联调、清理重新初始化、重复导入去重、故障恢复验收 | 跑通“初始化 -> 导出 -> 封盘 -> 导入 -> 去重 -> 复用”主闭环 |

## 优先级与阻塞解除顺序

优先级定义：

- `P0 解阻塞`：其他任务的接口、DDL、fixture 或基础 adapter 依赖。P0 未完成时，相关下游只能做 mock、测试草案或局部实现。
- `P1 主闭环`：直接构成“初始化 -> 导出 -> 封盘 -> 导入 -> 去重 -> 复用”的业务路径。
- `P2 交付增强`：部署、Web 对接、故障展示、最终联调和风险登记；不允许反向阻塞 P0/P1。

阻塞解除顺序：

1. 先完成 `TASK-S1-COMMON-001` 和 `TASK-S1-DB-001`，冻结协议类型和 DDL。
2. 并行完成 `TASK-S1-COMMON-002`、`TASK-S1-COMMON-003`、`TASK-S1-TEST-001`，给 HMAC、封盘、导入和 Web mock 提供输入。
3. Center/Edge 基础服务先落 adapter 和 health check，再接真实业务。
4. 中控导入 Worker 优先基于 fixture 开发，不等待边缘真实封盘。
5. Web 优先基于 fixture 开发页面和状态映射，Day 2 再对接真实 HTTP/WS。
6. 集成负责人只在 P0 全部完成且 P1 主路径可启动后进入最终联调。

## 并行泳道

| 泳道 | 建议负责人 | Day 1 | Day 2 |
|---|---|---|---|
| Common Protocol | Agent A1 | TASK-S1-COMMON-001 | 支援字段兼容 |
| Common Crypto/FS | Agent A2 | TASK-S1-COMMON-002, TASK-S1-COMMON-003 | 支援封盘/导入校验 |
| DB | Agent B | TASK-S1-DB-001 | 支援 SQL 调优 |
| Center | Agent C1/C2 | TASK-S1-CENTER-001, TASK-S1-CENTER-002, TASK-S1-CENTER-003 | TASK-S1-CENTER-004, TASK-S1-CENTER-005 |
| Edge | Agent D1/D2 | TASK-S1-EDGE-001, TASK-S1-EDGE-002, TASK-S1-EDGE-003, TASK-S1-EDGE-004 | TASK-S1-EDGE-005 |
| Web | Agent E1/E2 | TASK-S1-WEB-EDGE-001, TASK-S1-WEB-CENTER-001 mock | 对接真实 HTTP/WS |
| Deploy / QA | Agent F | TASK-S1-TEST-001, TASK-S1-DEPLOY-001 | TASK-S1-INTEGRATION-001 |

## Edge 实时可视化上线补充并行安排

本组任务用于补齐 `docs/上线要求.md` 中新增的 Edge 端“插盘后自动流转、前端实时可视化反馈”上线要求。现有 Edge 已具备受控扫描、导出、写盘和封盘能力；本组任务只把现有能力串成可控自动流程，并补齐前端真实状态展示，不放宽冻结协议、安全和状态机约束。

并行原则：

- 后端自动编排只调用既有受控能力，不复制扫描、Planner 或 DiskWorker 逻辑。
- 插盘/扫描/导出事件统一从后端权威状态发布，前端只消费，不本地推断完成态。
- 浏览器仍只读，不接触控制 token、`disk_data_key`、`edge_auth_secret`、nonce、tag 或 `data_key_id`。
- 自动导出必须可配置关闭；默认上线可灰度启用，失败时能回退到现有受控 API。
- udev 仍只触发 rescan，不直接导出；自动编排由 Edge 常驻服务在准入通过后启动。
- RustFS 成功扫描快照复用窗口由 Edge 配置 `scan.reuse_window_minutes` 控制，单位分钟；设为 `0` 时每次自动编排都扫描 RustFS。
- 导出计划只允许使用最近一次成功扫描窗口内确认上传完成的 `STABLE` 对象；正在上传、覆盖或扫描期间变化的对象不得计入统计和导出队列。

| 泳道 | 建议负责人 | 任务 | 可并行条件 | 交付物 |
|---|---|---|---|---|
| Edge Orchestrator | Agent D3 | TASK-S1-EDGE-AUTO-001 | 基于现有 scan/create/start 接口内部模块 | `READY -> scan -> create -> start -> seal` 自动编排与幂等 |
| Edge Event Stream | Agent D4 | TASK-S1-EDGE-WS-BOOT-001 | 可与自动编排并行 | 插盘、校验、扫描、导出、封盘统一 WS 事件 |
| Edge Web Production | Agent E3 | TASK-S1-WEB-EDGE-PROD-001 | 可先基于 fixture/contract 并行 | 去预览兜底、去写操作入口、真实空态/错误态 |
| Edge Web Timeline | Agent E4 | TASK-S1-WEB-EDGE-TIMELINE-001 | 等 WS 字段冻结后对接 | 每盘时间线、插盘即时反馈、多盘进度 |
| Deploy / Toggle | Agent F1 | TASK-S1-EDGE-AUTO-DEPLOY-001 | 跟自动编排并行 | 配置开关、systemd/udev 静态检查、回滚说明 |
| QA / Integration | Agent F2 | TASK-S1-EDGE-REALTIME-QA-001 | 依赖前四项可用 | 浏览器真实联调、插盘 1 秒反馈、端到端验收记录 |

## 不可降级底线

- WebSocket 只用于本端后端向本端前端推送状态；边缘端和中控端在线交互只走 HTTP API。
- 对象数据只通过运输盘离线交接。
- 运输盘生命周期以盘内 `disk_info.json.status.code` 为准。
- `CLEANING`、`REINITIALIZING` 只能作为运行态，不得写入盘内生命周期。
- API、WebSocket、manifest 和跨模块结构不得使用裸 `status` 混用语义。
- 边缘端不得删除、覆盖或修改 RustFS 源对象。
- 边缘端不得格式化、清盘或重新初始化运输盘。
- 边缘端不得保存明文 `disk_data_key` 到数据库或运输盘。
- 多盘对象分配必须使用 PostgreSQL 事务、`SELECT ... FOR UPDATE SKIP LOCKED` 和 `UPDATE ... RETURNING`。
- 高频进度必须来自内存 `ProgressAggregator`，不得每秒扫描数据库聚合。
- 导入失败不得把运输盘标记为 `IMPORTED`。
- 导入完成后的清理或重新初始化失败不得回滚 `import_job.status = DONE`。

## 任务总览

| 任务 ID | 任务名称 | 优先级 | Track / 模块 | 状态 | 计划 | 依赖 |
|---|---|---|---|---|---|---|
| TASK-S1-COMMON-001 | 协议类型、状态枚举、HTTP/WS payload 和 golden JSON | P0 解阻塞 | Common | [x] | Day 1 上午 | 无 |
| TASK-S1-DB-001 | 双端 PostgreSQL 迁移补齐并冻结 DDL | P0 解阻塞 | DB | [x] | Day 1 上午 | 无 |
| TASK-S1-COMMON-002 | HMAC、SHA256、AES-GCM 和 nonce 工具 | P0 解阻塞 | Common | [x] | Day 1 上午 | TASK-S1-COMMON-001 可并行 |
| TASK-S1-COMMON-003 | 运输盘协议文件、原子写、fsync、安全路径和 `.partial` 扫描 | P0 解阻塞 | Common | [x] | Day 1 上午 | TASK-S1-COMMON-001 |
| TASK-S1-TEST-001 | Fixture、模拟运输盘、manifest 样例和 HMAC 请求样例 | P0 解阻塞 | Tests | [x] | Day 1 上午 | TASK-S1-COMMON-001 可并行 |
| TASK-S1-CENTER-001 | 中控服务骨架、配置、DB/S3 adapter、health check | P0 解阻塞 | Center | [x] | Day 1 上午 | TASK-S1-DB-001 |
| TASK-S1-EDGE-001 | 边缘服务骨架、配置、中控 HMAC 客户端、DB/S3 adapter | P0 解阻塞 | Edge | [x] | Day 1 上午 | TASK-S1-COMMON-002, TASK-S1-DB-001 |
| TASK-S1-CENTER-002 | 中控 HMAC 鉴权、nonce 防重放和 `/api/edge/auth` | P1 主闭环 | Center | [x] | Day 1 下午 | TASK-S1-COMMON-002, TASK-S1-CENTER-001 |
| TASK-S1-CENTER-003 | 中控运输盘注册初始化、`/api/disk/verify`、`/api/disk/export-key` | P1 主闭环 | Center | [x] | Day 1 下午 | TASK-S1-COMMON-003, TASK-S1-CENTER-002 |
| TASK-S1-CENTER-HOTFIX-001 | Center 初始化失败不得遗留可发放 ACTIVE data_key | P0 解阻塞 | Center | [x] | Day 2 联调修补 | TASK-S1-CENTER-003 |
| TASK-S1-CENTER-HOTFIX-002 | Center verify/export-key 强制 Edge HMAC 鉴权 | P0 解阻塞 | Center / Edge | [x] | Day 2 审计修补 | TASK-S1-CENTER-002, TASK-S1-CENTER-003, TASK-S1-EDGE-001 |
| TASK-S1-CENTER-HOTFIX-003 | Center 真实 data_key 包裹与 disk_info 签名验签 | P0 解阻塞 | Center / Security | [x] | Day 2 审计修补 | TASK-S1-CENTER-003, TASK-S1-CENTER-004, TASK-S1-CENTER-005 |
| TASK-S1-P0-WORKER-RUNTIME-001 | 生产路径接入 DiskWorker / ImportWorker 受控触发 | P0 解阻塞 | Edge / Center | [x] | Day 2 审计修补 | TASK-S1-EDGE-005, TASK-S1-CENTER-004 |
| TASK-S1-P0-EXPORT-RECOVERY-001 | Edge 失败导出任务受控零写入恢复 | P0 解阻塞 | Edge / Center | [x] | Day 2 联调修补 | TASK-S1-P0-WORKER-RUNTIME-001 |
| TASK-S1-P0-EXPORT-RECOVERY-002 | Edge 恢复失败根因可机读审计与旧任务确认 | P0 解阻塞 | Edge | [x] | Day 2 VM 验收修补 | TASK-S1-P0-EXPORT-RECOVERY-001 |
| TASK-S1-EDGE-002 | 边缘磁盘检测、ext4 校验、disk_info 校验和恢复入口 | P1 主闭环 | Edge | [x] | Day 1 下午 | TASK-S1-COMMON-003, TASK-S1-EDGE-001 |
| TASK-S1-EDGE-HOTFIX-001 | Edge 真实挂载盘硬件 SN 探测修补 | P0 解阻塞 | Edge | [x] | Day 2 联调修补 | TASK-S1-EDGE-002, TASK-S1-CENTER-003 |
| TASK-S1-EDGE-HOTFIX-002 | Edge RustFS S3 凭据显式注入修补 | P0 解阻塞 | Edge | [x] | Day 2 联调修补 | TASK-S1-EDGE-003 |
| TASK-S1-EDGE-HOTFIX-003 | Edge disk_runtime 可重建准入修补 | P0 解阻塞 | Edge | [x] | Day 3 VM 验收修补 | TASK-S1-EDGE-002, TASK-S1-CENTER-003 |
| TASK-S1-EDGE-003 | RustFS 全 bucket 扫描、对象稳定性判断和快照入库 | P1 主闭环 | Edge | [x] | Day 1 下午 | TASK-S1-EDGE-001 |
| TASK-S1-EDGE-004 | 导出任务 Planner、容量预算、多盘分配事务和分块登记 | P1 主闭环 | Edge | [x] | Day 1 下午 | TASK-S1-EDGE-002, TASK-S1-EDGE-003 |
| TASK-S1-EDGE-005 | DiskWorker、加密写盘、metadata、manifest、封盘和导出 WS | P1 主闭环 | Edge | [x] | Day 2 上午 | TASK-S1-EDGE-004, TASK-S1-CENTER-003 |
| TASK-S1-CENTER-004 | 导入 Worker、manifest 校验、解密上传、去重账本和导入 WS | P1 主闭环 | Center | [x] | Day 2 上午 | TASK-S1-CENTER-003, TASK-S1-TEST-001 |
| TASK-S1-CENTER-005 | 导入后清理、重新初始化、密钥退役和失败恢复 | P1 主闭环 | Center | [x] | Day 2 下午 | TASK-S1-CENTER-004 |
| TASK-S1-P1-CENTER-REINIT-API-001 | Center 受控清理重初始化生产 API | P1 主闭环 | Center | [x] | Day 2 VM 验收修补 | TASK-S1-CENTER-005 |
| TASK-S1-P1-CENTER-REINIT-FSTYPE-002 | Center reinitialize findmnt 多行 ext4 解析修补 | P1 主闭环 | Center | [x] | Day 2 VM 热修补 | TASK-S1-P1-CENTER-REINIT-API-001 |
| TASK-S1-P1-CENTER-REINIT-UPDATED-AT-003 | Center reinitialize 兼容旧盘缺 updated_at | P1 主闭环 | Center | [x] | Day 2 VM 热修补 | TASK-S1-P1-CENTER-REINIT-FSTYPE-002 |
| TASK-S1-P1-CENTER-REINIT-SIGNATURE-004 | Center reinitialize 兼容旧 IMPORTED 签名 canonical | P1 主闭环 | Center | [x] | Day 2 VM 热修补 | TASK-S1-P1-CENTER-REINIT-UPDATED-AT-003 |
| TASK-S1-P1-CENTER-REINIT-SIGNATURE-005 | Center reinitialize 统一内外层 center_signature 验签 | P1 主闭环 | Center | [x] | Day 2 VM 热修补 | TASK-S1-P1-CENTER-REINIT-SIGNATURE-004 |
| TASK-S1-P1-CENTER-REINIT-ADMISSION-006 | Center reinitialize 最小生命周期准入 | P1 主闭环 | Center | [x] | Day 2 VM 热修补 | TASK-S1-P1-CENTER-REINIT-SIGNATURE-005 |
| TASK-S1-CENTER-HOTFIX-004 | Center reinitialize disk_info 失败原子性修补 | P0 解阻塞 | Center / Security | [x] | Day 2 VM 故障修补 | TASK-S1-P1-CENTER-REINIT-ADMISSION-006 |
| TASK-S1-CENTER-HOTFIX-005 | Center 导入成功后 data_key 封盘生命周期绑定 | P0 解阻塞 | Center / DB | [x] | Day 2 真实导入修补 | TASK-S1-CENTER-004, TASK-S1-CENTER-005 |
| TASK-S1-SECURITY-HOTFIX-006 | AES-GCM AAD 同源构造与全加密面审计 | P0 解阻塞 | Common / Edge / Center / Security | [x] | Day 3 审计修补 | TASK-S1-COMMON-002, TASK-S1-EDGE-005, TASK-S1-CENTER-004 |
| TASK-S1-SECURITY-HOTFIX-007 | center_signature 同源签名/验签 API 与 reinitialize 分叉修复 | P0 解阻塞 | Common / Center / Edge / Security | [x] | Day 3 真实闭环阻塞修补 | TASK-S1-SECURITY-HOTFIX-006, TASK-S1-CENTER-005 |
| TASK-S1-CENTER-HOTFIX-006 | Center import API 空 import_job_status 错误包装修补 | P0 解阻塞 | Center / Import | [x] | Day 3 真实导入阻塞修补 | TASK-S1-CENTER-004, TASK-S1-SECURITY-HOTFIX-007 |
| TASK-S1-CENTER-HOTFIX-007 | Center 初始化与重新初始化 disk_info 生成同源修补 | P0 解阻塞 | Center / Protocol | [x] | Day 3 真实导入阻塞修补 | TASK-S1-CENTER-HOTFIX-006, TASK-S1-CENTER-005 |
| TASK-S1-CENTER-HOTFIX-008 | Center 受控丢弃 SEALED 测试导出并重置 | P0 解阻塞 | Center / Reinitialize | [x] | Day 3 从零闭环阻塞修补 | TASK-S1-CENTER-HOTFIX-007 |
| TASK-S1-WEB-EDGE-001 | 边缘端 DashboardView、HTTP 汇总和 WS 进度展示 | P2 交付增强 | Web / Edge | [x] | Day 1-2 | TASK-S1-TEST-001 |
| TASK-S1-WEB-CENTER-001 | 中控端 DashboardView、HTTP 汇总和 WS 进度展示 | P2 交付增强 | Web / Center | [x] | Day 1-2 | TASK-S1-TEST-001 |
| TASK-S1-DASHBOARD-REALTIME-001 | 双端真实 Dashboard HTTP summary 与本端 WebSocket 推送 | P0 解阻塞 | Center / Edge / Web | [x] | Day 2 联调修补 | TASK-S1-WEB-EDGE-001, TASK-S1-WEB-CENTER-001 |
| TASK-S1-P1-EDGE-WEB-CONTRACT-001 | Edge Web 浏览器安全只读契约补齐 | P1 主闭环 | Edge / Web contract | [x] | Day 2 审计修补 | TASK-S1-DASHBOARD-REALTIME-001, TASK-S1-WEB-EDGE-001 |
| TASK-S1-DEPLOY-001 | systemd、udev、配置示例和本地联调脚本 | P2 交付增强 | Deploy | [x] | Day 1-2 | TASK-S1-CENTER-001, TASK-S1-EDGE-001 |
| TASK-S1-EDGE-AUTO-001 | Edge 插盘后自动扫描、自动建任务和自动启动导出编排 | P0 解阻塞 | Edge | [x] | Day 3 上午 | TASK-S1-EDGE-002, TASK-S1-EDGE-003, TASK-S1-EDGE-004, TASK-S1-EDGE-005 |
| TASK-S1-EDGE-WS-BOOT-001 | Edge 插盘、校验、扫描和封盘阶段 WebSocket 事件补齐 | P0 解阻塞 | Edge / Web contract | [x] | Day 3 上午 | TASK-S1-DASHBOARD-REALTIME-001, TASK-S1-EDGE-002 |
| TASK-S1-WEB-EDGE-PROD-001 | Edge Dashboard 生产态清理与浏览器只读收口 | P1 主闭环 | Web / Edge | [x] | Day 3 上午 | TASK-S1-P1-EDGE-WEB-CONTRACT-001 |
| TASK-S1-WEB-EDGE-TIMELINE-001 | Edge Dashboard 插盘即时反馈、每盘时间线和扫描态展示 | P1 主闭环 | Web / Edge | [x] | Day 3 下午 | TASK-S1-EDGE-WS-BOOT-001, TASK-S1-WEB-EDGE-PROD-001 |
| TASK-S1-EDGE-AUTO-DEPLOY-001 | Edge 自动流程配置开关、部署检查和回滚说明 | P1 主闭环 | Deploy / Edge | [x] | Day 3 下午 | TASK-S1-EDGE-AUTO-001 |
| TASK-S1-P0-EDGE-OFFLINE-PACK-001 | Edge 离线打包：删除运行期 Center 校验和 export-key，复用 edge_auth_secret 派生封盘 disk_data_key | P0 解阻塞 | Edge / Center / Deploy / QA | [x] | Day 4 需求调整 | TASK-S1-SECURITY-HOTFIX-007, TASK-S1-EDGE-005, TASK-S1-CENTER-004 |
| TASK-S1-P0-STREAM-STORAGE-V2-001 | 运输盘流式存储 v2 破坏性升级：PACK/FRAMES、容量预算和前端契约替换 | P0 解阻塞 | Common / DB / Edge / Center / Web / Tests | [x] | Day 4 需求调整 | TASK-S1-P0-EDGE-OFFLINE-PACK-001 |
| TASK-S1-EDGE-REALTIME-QA-001 | Edge 前后端实时可视化端到端联调验收 | P0 解阻塞 | Integration / QA | [x] | Day 3 下午 | TASK-S1-P0-EDGE-OFFLINE-PACK-001, TASK-S1-EDGE-AUTO-001, TASK-S1-EDGE-WS-BOOT-001, TASK-S1-WEB-EDGE-TIMELINE-001, TASK-S1-EDGE-AUTO-DEPLOY-001 |
| TASK-S1-INTEGRATION-001 | 主闭环联调、验收、风险登记和交付检查 | P2 交付增强 | Integration | [~] | Day 2 下午 | Day 2 主闭环任务 |

---

# 开发任务卡片：TASK-S1-P0-STREAM-STORAGE-V2-001

### 任务基本信息

- **任务 ID**：TASK-S1-P0-STREAM-STORAGE-V2-001
- **任务名称**：运输盘流式存储 v2 破坏性升级：PACK/FRAMES、容量预算和前端契约替换
- **所属 Track / 模块**：
  - [x] Track 1: Common
  - [x] Track 2: Edge
  - [x] Track 3: Center
  - [x] Track 4: Web
  - [x] Track 5: Tests / DB
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex
- **计划时间**：Day 4 需求调整
- **依赖任务**：TASK-S1-P0-EDGE-OFFLINE-PACK-001

### 任务目标与范围

- **核心目标**：按 `docs/02-方案设计/运输盘流式存储整改方案.md` 直接替换运输盘存储协议为 v2，删除 v1 manifest、对象级单文件密文模型、chunk 通讯字段和前端兼容 parser。
- **对应代码位置**：`crates/common/`、`sql/edge/`、`sql/center/`、`crates/edge-backend/`、`crates/center-backend/`、`web/edge-web/`、`web/center-web/`、测试 fixture。

### 协议与数据结构约束

- Edge 只生成 `manifest_version = 2.0.0`。
- Center 只导入 `manifest_version = 2.0.0`。
- `PACK` 对象固定 `frame_total = 0`。
- `FRAMES` 对象固定 `frame_total > 0`，frame 可以跨盘导入后合并。
- HTTP / WebSocket / 前端类型删除 v1 `relative_data_path/chunk_*/nonce/tag/aad/ciphertext_sha256` 展示字段。
- Dashboard 汇总只使用 `global_progress`，HTTP 分页只使用 `page/page_size/total/items`。
- 容量分配按 `estimated_landing_bytes`，不得按源对象 `size_bytes`。

### 安全与状态机边界

- 保留 AES-256-GCM。
- 运输盘和前端响应不得暴露明文 `disk_data_key`。
- `RECOVERY_REQUIRED` 只能作为 `last_error_code`，不得写入盘内生命周期或 `runtime_status`。
- 不修改 `docs/v1.0冻结/`。

### 验收与检查清单

- [ ] Common v2 manifest、AAD、KDF 和 golden 测试通过。
- [ ] Edge/Center SQL 迁移改为 v2 表结构和约束。
- [ ] Edge 按 PACK/FRAMES 写盘、封盘和容量预算分配。
- [ ] Center 按 PACK/FRAMES 校验、解密、导入和 frame 合并。
- [ ] HTTP/WebSocket DTO 删除 v1 字段和兼容别名。
- [ ] 双端前端类型和展示删除 v1 字段、fallback parser。
- [ ] fixture 和测试更新到 v2。
- [ ] 必跑验证按改动范围执行并记录结果。

---

# 开发任务卡片：TASK-S1-P0-EDGE-OFFLINE-PACK-001

### 任务基本信息

- **任务 ID**：TASK-S1-P0-EDGE-OFFLINE-PACK-001
- **任务名称**：Edge 离线打包：删除运行期 Center 校验和 export-key，复用 edge_auth_secret 派生封盘 disk_data_key
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
  - [x] Track 5: Deploy / QA
- **任务状态**：[x] 开发完成
- **负责人 / Role**：首席系统架构师主会话协调；Edge 后端、Center 后端、部署配置、测试窗口并行执行
- **计划时间**：Day 4 需求调整
- **依赖任务**：TASK-S1-SECURITY-HOTFIX-007, TASK-S1-EDGE-005, TASK-S1-CENTER-004

### 任务目标与范围

- **核心目标**：按 `docs/Edge离线打包需求调整方案.md` 将 Edge 导出主流程改为运行期完全离线：本地准入、本地派生 `disk_data_key`、离线封盘；Center 在导入阶段按同一派生规范解密并执行最终权威判断。
- **对应代码位置**：`crates/edge-backend/`、`crates/center-backend/`、`deploy/config/edge.example.toml`、相关测试与 `work_sprint/dev_log/`。
- **不在范围**：不新增数据库字段，不新增 `disk_info.json` 字段，不修改 `docs/v1.0冻结/`，不执行真实部署、真实插盘、初始化、导出、导入、清盘或重初始化。

### 责任边界

- **Edge 后端**：删除/清空导出主流程中的 `/api/edge/auth`、`/api/disk/verify`、`/api/disk/export-key`、对象账本联网查询；插盘后只做 ext4、`disk_info.json`、协议版本、`center_signature`、`status.code = INITIALIZED`、`.partial` 恢复检查；导出时按规范用 `center.edge_auth_secret` 派生本次封盘 `disk_data_key`，只在内存使用。
- **Center 后端**：导入 Worker 根据 `edge_code + disk_id + data_key_id + export_job_id + seal_id + edge_auth_secret` 派生解密密钥；在导入阶段最终判断 disk 注册/启用、edge ACTIVE、重复导入、manifest 合法性、校验和解密；对应联网验证接口不作为 Edge 离线导出兼容主路径保留。
- **部署配置**：复用 `center.edge_code`、`center.edge_auth_secret` / `center.edge_auth_secret_env`；`center.base_url` 和 `center.auth_key_id` 不再作为 Edge 导出必需项；示例配置不得包含真实密钥。
- **测试验证**：覆盖断网导出、不发起 Center HTTP、密钥派生一致性、错误授权 key 解密失败、缺少 `edge_auth_secret` 拒绝导出、Center 禁用 Edge/运输盘后导入拒绝、敏感信息不入库/不落盘/不进日志或 WS/HTTP。

### 协议与数据结构约束

- 不新增数据库字段。
- 不新增 `disk_info.json` 盘内字段。
- 继续使用 `center.edge_code`、`center.edge_auth_secret`、`center.edge_auth_secret_env`、`disk_id`、`data_key_id`、`export_job_id`、`seal_id`。
- 派生规范固定为 HMAC-SHA256：

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

### 安全与状态机边界

- Edge 运行期完全不访问 Center，不因 Center 不可达阻塞导出。
- Edge 不再实时判断 Center 侧 disk 是否注册/启用、edge 是否 ACTIVE；这些判断后移到 Center 导入阶段。
- `center.edge_auth_secret` 不直接作为 AES-256-GCM 密钥，只作为 HMAC 派生输入。
- Edge 不保存明文 `disk_data_key` 到数据库或运输盘，不在日志、WebSocket、HTTP summary 中暴露 `center.edge_auth_secret` 或 `disk_data_key`。
- Edge 不修改 `disk_info.json` 中 Center 签名覆盖的身份和安全基础字段。
- 非 ext4、协议版本不匹配、`status.code != INITIALIZED`、`.partial` 未恢复通过时仍拒绝导出。

### 验收与检查清单

- [x] Edge 导出路径不发起 `/api/edge/auth`、`/api/disk/verify`、`/api/disk/export-key` 或对象账本联网查询。
- [x] Center 不可达不阻塞 Edge 插盘、扫描、导出、封盘。
- [x] Edge 配置缺少 `center.edge_auth_secret` 时拒绝进入导出。
- [x] Edge 和 Center 对同一输入派生出的 32 字节 `disk_data_key` 一致。
- [x] 错误 Edge 授权 key 会导致 Center 解密或校验失败，不写入 `object_ledger`。
- [x] Center 禁用 Edge 后，对应封盘在导入阶段拒绝入库。
- [x] Center 禁用运输盘后，对应封盘在导入阶段拒绝入库。
- [x] 重复插入同一 `disk_id + seal_id` 不重复导入。
- [x] Edge 不落库、不落盘、不输出日志/WS/HTTP 明文 `disk_data_key` 或 `center.edge_auth_secret`。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] 如修改前端展示或配置文档，执行对应 `npm run typecheck` 和 `npm run build` 或静态部署检查。

---

# 开发任务卡片：TASK-S1-COMMON-001

### 任务基本信息

- **任务 ID**：TASK-S1-COMMON-001
- **任务名称**：协议类型、状态枚举、HTTP/WS payload 和 golden JSON
- **所属 Track / 模块**：
  - [x] Track 1: Common (`crates/common`)
  - [ ] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web (`web/`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent A1
- **计划时间**：Day 1 上午
- **依赖任务**：无

### 任务目标与范围

- **核心目标**：建立双端共享的协议类型、状态枚举、错误码、HTTP API payload、WebSocket event payload 和 golden JSON 样例。
- **对应代码位置**：`crates/common/src/protocol/`、`crates/common/src/error.rs`、`crates/common/tests/`

### 协议与数据结构约束

- 覆盖 `disk_info.json`、`export_manifest.json`、`POST /api/edge/auth`、`POST /api/disk/verify`、`POST /api/disk/export-key`、`GET /api/disk/{disk_id}`、`COPY_PROGRESS`、`IMPORT_PROGRESS`。
- 状态字段必须使用 `status_code`、`disk_status_code`、`runtime_status`、`export_job_status`、`import_job_status`、`object_status`、`edge_status`、`disk_enabled`。
- 盘内 `disk_info.json` 保留嵌套 `status.code`，外部 API/WS 不得简化为裸 `status`。

### 安全与状态机边界

- `export_object.status` 不包含 `IMPORTED`。
- `CLEANING`、`REINITIALIZING` 只能属于运行态。

### 验收与检查清单

- [ ] 所有冻结枚举均有 Rust 类型。
- [ ] golden JSON 能序列化和反序列化。
- [ ] API/WS/manifest 结构无裸 `status` 混用。

---

# 开发任务卡片：TASK-S1-COMMON-002

### 任务基本信息

- **任务 ID**：TASK-S1-COMMON-002
- **任务名称**：HMAC、SHA256、AES-GCM 和 nonce 工具
- **所属 Track / 模块**：
  - [x] Track 1: Common (`crates/common`)
  - [ ] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web (`web/`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent A2
- **计划时间**：Day 1 上午
- **依赖任务**：TASK-S1-COMMON-001 可并行

### 任务目标与范围

- **核心目标**：提供 HMAC canonical request、SHA256、AES-256-GCM 加解密、nonce 生成和 Base64/hex 编码工具。
- **对应代码位置**：`crates/common/src/crypto/`、`crates/common/tests/`

### 协议与数据结构约束

- HMAC 签名串为 `METHOD + "\n" + canonical_path_with_query + "\n" + X-Timestamp + "\n" + X-Nonce + "\n" + X-Body-SHA256`。
- query 参数按冻结契约排序和 percent-encoding。
- AES-GCM AAD 由 `disk_id`、`seal_id`、`export_job_id`、`bucket`、`object_key`、`chunk_group_id`、`chunk_index`、`chunk_total`、`chunk_offset_bytes` 等协议字段组成。

### 安全与状态机边界

- `edge_auth_secret` 不用于文件加密。
- `disk_data_key` 不用于 HTTP API 鉴权。
- 同一 `data_key_id + nonce` 不得重复，重复时调用方必须重新生成。

### 验收与检查清单

- [ ] HMAC 成功、签名错误、空 body、query 排序有测试。
- [ ] AES-GCM 解密失败和 tag 验证失败有测试。
- [ ] SHA256 输出 lowercase hex。

---

# 开发任务卡片：TASK-S1-COMMON-003

### 任务基本信息

- **任务 ID**：TASK-S1-COMMON-003
- **任务名称**：运输盘协议文件、原子写、fsync、安全路径和 `.partial` 扫描
- **所属 Track / 模块**：
  - [x] Track 1: Common (`crates/common`)
  - [ ] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web (`web/`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent A2
- **计划时间**：Day 1 上午
- **依赖任务**：TASK-S1-COMMON-001

### 任务目标与范围

- **核心目标**：封装运输盘协议根、`disk_info.json`、manifest、metadata 的读写、SHA256 校验、临时文件、`fsync`、原子 rename、安全路径检查和 `.partial` 扫描。
- **对应代码位置**：`crates/common/src/protocol/`、`crates/common/tests/`

### 协议与数据结构约束

- 协议根固定为 `/rustfs-transfer/`。
- `data/`、`meta/`、`manifests/`、`logs/`、`quarantine/partial/` 路径语义固定。
- `.partial` 不得进入有效 manifest。

### 安全与状态机边界

- 相对路径不得以 `/` 开头，不得包含 `..`。
- 写对象、metadata、manifest、`disk_info.json` 必须使用临时文件、`fsync` 和原子重命名。

### 验收与检查清单

- [ ] manifest SHA256 不一致返回 `CHECKSUM_MISMATCH`。
- [ ] 路径穿越被拒绝。
- [ ] `.partial` 扫描返回数量、字节数和路径列表。

---

# 开发任务卡片：TASK-S1-DB-001

### 任务基本信息

- **任务 ID**：TASK-S1-DB-001
- **任务名称**：双端 PostgreSQL 迁移补齐并冻结 DDL
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`sql/edge`)
  - [x] Track 3: Center (`sql/center`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent B
- **计划时间**：Day 1 上午
- **依赖任务**：无

### 任务目标与范围

- **核心目标**：补齐中控端和边缘端 PostgreSQL 迁移，形成 Day 1 中午前可供后端并行使用的 DDL。
- **对应代码位置**：`sql/center/`、`sql/edge/`

### 协议与数据结构约束

- 中控表：`disk_list`、`edge_site`、`signature_key`、`data_key`、`import_job`、`chunk_import_group`、`chunk_import_part`、`object_ledger`。
- 边缘表：`local_object_snapshot`、`export_job`、`export_object`、`disk_runtime`。
- 所有 timestamp 保存 UTC。

### 安全与状态机边界

- `disk_list.status` 只表示启用状态。
- `edge_site.auth_secret_ciphertext`、`data_key.encrypted_key` 不保存明文。
- 多盘分配唯一约束和导入并发唯一约束必须落库。

### 验收与检查清单

- [x] 双端迁移可在空库执行。
- [x] `import_job(disk_id, seal_id)` 活动/完成唯一约束存在。
- [x] `export_object` 活动态普通对象和分块唯一约束存在。
- [x] `export_object(data_key_id, nonce)` 唯一约束存在。

---

# 开发任务卡片：TASK-S1-TEST-001

### 任务基本信息

- **任务 ID**：TASK-S1-TEST-001
- **任务名称**：Fixture、模拟运输盘、manifest 样例和 HMAC 请求样例
- **所属 Track / 模块**：
  - [x] Track 1: Common
  - [x] Track 2: Edge
  - [x] Track 3: Center
  - [x] Track 4: Tests (`tests/`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent F
- **计划时间**：Day 1 上午启动，Day 2 持续补齐
- **依赖任务**：TASK-S1-COMMON-001 可并行

### 任务目标与范围

- **核心目标**：提前建立联调 fixture，让中控导入、前端 mock、部署脚本不等待真实边缘封盘。
- **对应代码位置**：`tests/`、`crates/*/tests/`、`web/*/src/__tests__/`

### 协议与数据结构约束

- 提供 `disk_info.json`、`export_manifest.json`、manifest SHA256、HMAC 请求样例、模拟 `/rustfs-transfer/` 目录。
- 样例字段必须与 common golden JSON 保持一致。

### 安全与状态机边界

- Fixture 不包含真实密钥。
- 清理和重新初始化测试只使用临时目录或模拟盘。

### 验收与检查清单

- [ ] 可一键生成模拟 INITIALIZED 盘和 SEALED 盘目录。
- [ ] HMAC 正确和错误请求样例可复用。
- [ ] 缺失分块、非法 manifest、`.partial` 残留样例存在。

---

# 开发任务卡片：TASK-S1-CENTER-001

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-001
- **任务名称**：中控服务骨架、配置、DB/S3 adapter、health check
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent C1
- **计划时间**：Day 1 上午
- **依赖任务**：TASK-S1-DB-001

### 任务目标与范围

- **核心目标**：建立中控端 axum 服务、配置加载、数据库连接、S3 adapter、Disk adapter、Clock/ID adapter 和 health check。
- **对应代码位置**：`crates/center-backend/src/`、`deploy/config/center.example.toml`

### 协议与数据结构约束

- 配置支持 TOML 和 `RUSTFS_TRANSFER__` 环境变量覆盖。
- 日志字段包含 `request_id`、`disk_id`、`seal_id`、`edge_code`、`error_code`。

### 安全与状态机边界

- 示例配置不得包含真实密钥。
- Adapter seam 用于测试，不绕过冻结协议。

### 验收与检查清单

- [ ] 中控服务可启动并提供 health check。
- [ ] DB/S3/Disk/Clock/ID adapter 可在测试中替换。
- [ ] 配置错误有清晰日志。

---

# 开发任务卡片：TASK-S1-CENTER-002

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-002
- **任务名称**：中控 HMAC 鉴权、nonce 防重放和 `/api/edge/auth`
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent C1
- **计划时间**：Day 1 下午
- **依赖任务**：TASK-S1-COMMON-002, TASK-S1-CENTER-001

### 任务目标与范围

- **核心目标**：实现中控统一 HMAC 鉴权层、进程内 nonce TTL 缓存和 `/api/edge/auth`。
- **对应代码位置**：`crates/center-backend/src/`

### 协议与数据结构约束

- 必填 Header：`X-Edge-Code`、`X-Auth-Key-Id`、`X-Timestamp`、`X-Nonce`、`X-Body-SHA256`、`X-Signature`。
- `/api/edge/auth` 响应必须使用 `edge_status`。

### 安全与状态机边界

- 鉴权失败返回 `401 Unauthorized` 和 `UNAUTHORIZED`。
- body `edge_code` 与 Header 不一致返回 `400 Bad Request` 和 `INVALID_REQUEST`。
- nonce TTL 不少于 10 分钟，不新增数据库表。

### 验收与检查清单

- [ ] 正确签名通过。
- [ ] 签名错误、时间偏差、重复 nonce、禁用边缘站点均被拒绝。
- [ ] 日志包含 request_id、edge_code、auth_key_id 和拒绝原因。

---

# 开发任务卡片：TASK-S1-CENTER-003

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-003
- **任务名称**：中控运输盘注册初始化、`/api/disk/verify`、`/api/disk/export-key`
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent C2
- **计划时间**：Day 1 下午
- **依赖任务**：TASK-S1-COMMON-003, TASK-S1-CENTER-002

### 任务目标与范围

- **核心目标**：实现运输盘注册、初始化、协议签名、数据密钥生成/保存、盘校验和密钥发放。
- **对应代码位置**：`crates/center-backend/src/`

### 协议与数据结构约束

- 业务主身份是 `disk_id`，SN 只作辅助校验。
- `/api/disk/verify` 响应使用 `disk_enabled`、`expected_status`、`action`。
- `/api/disk/export-key` 不返回 `expires_at`。

### 安全与状态机边界

- 初始化最后写 `disk_info.json.status.code = INITIALIZED`。
- 只有 `ACTIVE` 或同一 `edge_code + export_job_id` 的 `ISSUED` 密钥可发放。
- `SEALED_READONLY`、`RETIRED`、`REVOKED` 不得发给边缘端写盘。

### 验收与检查清单

- [ ] 未注册盘可登记并生成稳定 `disk_id`。
- [ ] 初始化写入的 `disk_info.json` 可被 common 校验。
- [ ] 未注册、禁用、协议不兼容和状态不允许的盘会被拒绝。
- [ ] 同一导出任务重复领取密钥幂等。

---

# 开发任务卡片：TASK-S1-EDGE-001

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-001
- **任务名称**：边缘服务骨架、配置、中控 HMAC 客户端、DB/S3 adapter
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D1
- **计划时间**：Day 1 上午
- **依赖任务**：TASK-S1-COMMON-002, TASK-S1-DB-001

### 任务目标与范围

- **核心目标**：建立边缘端 axum 服务、配置加载、数据库连接、RustFS S3 adapter、中控 HTTP HMAC client、Disk adapter、Clock/ID adapter 和 health check。
- **对应代码位置**：`crates/edge-backend/src/`、`deploy/config/edge.example.toml`

### 协议与数据结构约束

- 调用中控 API 必须携带 HMAC Header。
- 请求体中 `edge_code` 必须与 `X-Edge-Code` 一致。

### 安全与状态机边界

- `edge_auth_secret` 只用于 HTTP API 鉴权。
- 示例配置不得包含真实密钥。

### 验收与检查清单

- [ ] 边缘服务可启动并提供 health check。
- [ ] 中控 HMAC client 可复用 common 签名工具。
- [ ] DB/S3/Disk/Clock/ID adapter 可在测试中替换。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-001

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-001
- **任务名称**：Center 初始化失败不得遗留可发放 ACTIVE data_key
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 独立编码执行窗口
- **计划时间**：Day 2 联调修补
- **依赖任务**：TASK-S1-CENTER-003

### 任务目标与范围

- **核心目标**：修补 `/api/disk/initialize` 首次写盘失败后遗留 `ACTIVE` data_key 的一致性缺陷，确保重试成功后同盘只有盘内引用的新 key 可发放。
- **对应代码位置**：`crates/center-backend/src/lib.rs`

### 协议与数据结构约束

- `disk_info.json.status.code` 仍以盘内生命周期为准，成功初始化写入 `INITIALIZED`。
- `data_key.status` 仍只使用冻结枚举：`ACTIVE`、`ISSUED`、`SEALED_READONLY`、`RETIRED`、`REVOKED`。
- 不新增协议字段，不修改 `docs/v1.0冻结/`。

### 安全与状态机边界

- 盘内目录、`disk_info.json`、原子 rename 或 `fsync` 失败时，新生成 key 不得处于可发放 `ACTIVE`。
- 成功激活新初始化 key 时，仅撤销同盘未发放、未封盘的旧 `ACTIVE` key，不触碰 `ISSUED`、`SEALED_READONLY`、`RETIRED` 历史数据。
- 明文 `disk_data_key` 只在初始化进程内存中短暂生成并包裹保存，不写入运输盘或边缘端数据库。

### 验收与检查清单

- [x] 回归测试覆盖写盘失败后没有新增可发放 `ACTIVE` key。
- [x] 回归测试覆盖重试成功后同盘只有盘内引用的新 key 为 `ACTIVE`。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-002

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-002
- **任务名称**：Center verify/export-key 强制 Edge HMAC 鉴权
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 审计修补
- **依赖任务**：TASK-S1-CENTER-002, TASK-S1-CENTER-003, TASK-S1-EDGE-001

### 任务目标与范围

- **核心目标**：强制 `/api/disk/verify` 与 `/api/disk/export-key` 使用 Edge HMAC 请求鉴权，避免敏感磁盘校验与密钥发放接口被未签名请求调用。
- **对应代码位置**：`crates/center-backend/src/center_auth.rs`、`crates/center-backend/src/lib.rs`、`crates/center-backend/tests/edge_auth.rs`、`crates/edge-backend/src/center_client.rs`

### 协议与数据结构约束

- 继续使用 common canonical HMAC：method、canonical path/query、timestamp、nonce、body SHA256 参与签名。
- HMAC header 名大小写兼容 HTTP 行为；`edge_code` 身份来自鉴权 header，body 内若携带不一致 `edge_code` 必须拒绝。
- 不新增 WebSocket 或跨端同步通道，不修改 `docs/v1.0冻结/`。

### 安全与状态机边界

- 缺失、错误、过期、重放、body hash/method/path 不匹配的签名请求返回 401，且不得进入业务逻辑。
- 禁用 Edge 在业务逻辑前拒绝。
- `/api/disk/export-key` 鉴权失败时不得泄露 `disk_data_key` 或 key 包裹材料。
- Dashboard summary/WS 路由保持可用，管理类健康检查路由不纳入本 hotfix 范围。

### 验收与检查清单

- [x] `/api/disk/verify` 未签名请求被拒绝，正确签名请求通过。
- [x] `/api/disk/export-key` 未签名请求被拒绝，正确签名请求通过且不序列化 `expires_at`。
- [x] 回归测试覆盖 header 小写、重放 nonce、错误 timestamp、错误 body、错误 method/path、禁用 Edge。
- [x] Edge client 签名与 Center shared canonical HMAC 验证保持一致。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] `cargo test --workspace` 通过。
- [x] 部署静态检查通过。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-003

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-003
- **任务名称**：Center 真实 data_key 包裹与 disk_info 签名验签
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [x] Track 4: Deploy / Security
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 审计修补
- **依赖任务**：TASK-S1-CENTER-003, TASK-S1-CENTER-004, TASK-S1-CENTER-005

### 任务目标与范围

- **核心目标**：替换 Center data_key 明文等价 mock 包裹和 `disk_info.json` mock 签名，实现上线前必须具备的真实密钥保护与签名验签。
- **对应代码位置**：`crates/center-backend/src/center_security.rs`、`crates/center-backend/src/lib.rs`、`crates/center-backend/src/import_worker.rs`、`crates/center-backend/src/reinitializer.rs`、`deploy/`、`scripts/`

### 协议与数据结构约束

- `data_key.encrypted_key` 使用受控环境变量注入的本机主密钥执行 AES-256-GCM 包裹，`key_wrap_alg = LOCAL-MASTER-KEY`。
- `disk_info.json.security.center_signature` 使用 HMAC-SHA256 和 canonical JSON 覆盖 `protocol`、`disk`、`security.center_key_id`、`security.signature_alg`、`security.data_key_id`；按用户已更新冻结协议，`center` 不属于签名覆盖字段。
- 运输盘和 manifest 只保存 `data_key_id`、nonce、tag、校验值等非明文元数据。

### 安全与状态机边界

- 旧 `mock:v1:*`、`local-wrap:*` 或 mock signature 测试数据不得静默降级；必须拒绝发放、导入或重新初始化，并要求由 Center 重新初始化生成新 `data_key_id + disk_info.json`。
- `RUSTFS_TRANSFER__SECURITY__LOCAL_MASTER_KEY` 和 `RUSTFS_TRANSFER__SECURITY__CENTER_SIGNATURE_KEY` 缺失、占位或非 32-byte 时 Center 启动失败。
- `disk_info.json` 签名篡改、错误主密钥解包、盘内出现明文 key 等情况必须有回归测试覆盖。

### 验收与检查清单

- [x] data_key 包裹密文不包含明文 `disk_data_key` 等价内容。
- [x] 错误主密钥无法解包旧密文。
- [x] 初始化、导入、重新初始化路径写入或校验真实 `center_signature`。
- [x] 篡改 `disk_info.json` 被拒绝。
- [x] 启动安全配置缺失或占位值失败路径有测试。
- [x] 旧 mock key wrap 被拒绝，真实盘需重新初始化迁移。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-common` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。

---

# 开发任务卡片：TASK-S1-P0-WORKER-RUNTIME-001

### 任务基本信息

- **任务 ID**：TASK-S1-P0-WORKER-RUNTIME-001
- **任务名称**：生产路径接入 DiskWorker / ImportWorker 受控触发
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 审计修补
- **依赖任务**：TASK-S1-EDGE-005, TASK-S1-CENTER-004

### 任务目标与范围

- **核心目标**：修补生产主闭环断点，将已有 `DiskWorker` / `ImportWorker` 接入受控 HTTP API 与生产服务后台。
- **对应代码位置**：`crates/edge-backend/src/control.rs`、`crates/edge-backend/src/export_runtime.rs`、`crates/center-backend/src/lib.rs`、`crates/center-backend/src/import_runtime.rs`

### 协议与数据结构约束

- Edge `POST /api/edge/export-jobs/{export_job_id}/start` 只在本地 control token 通过后分配 READY 盘对象并启动 `DiskWorker`。
- `DiskWorker` 启动前必须通过 HMAC `CenterHmacClient` 调用 `/api/disk/export-key` 领取内存态 `disk_data_key`。
- Center `POST /api/center/import-jobs/start` 使用 `X-Center-Control-Token`，请求必须显式传入 `mount_path`。
- API 响应继续使用 `export_job_status`、`import_job_status` 等语义字段，不新增裸 `status`。

### 安全与状态机边界

- 服务启动扫描、udev rescan 和 READY 运行态不得自动写盘、自动导出或自动导入。
- 明文 `disk_data_key` 不得来自配置、请求或数据库，不得写入日志、数据库或运输盘。
- Center/Edge 在线交互仍只走 HTTP HMAC API；WebSocket 只用于本端前端进度推送。
- ImportWorker 失败路径不得写 `IMPORTED`，不得写入 `object_ledger`。

### 验收与检查清单

- [x] Edge start API 分配后启动 `DiskWorker`，且只处理本次分配到对象的盘。
- [x] Edge export key 通过 HMAC client 获取，明文 key 只在内存中传入 worker。
- [x] Center 导入必须通过受控 token API 显式触发。
- [x] 生产 `ImportRepository` 解包 Center 安全包裹 data_key，并沿用 disk_info 签名验签。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。

---

# 开发任务卡片：TASK-S1-P0-EXPORT-RECOVERY-001

### 任务基本信息

- **任务 ID**：TASK-S1-P0-EXPORT-RECOVERY-001
- **任务名称**：Edge 失败导出任务受控零写入恢复
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 联调修补
- **依赖任务**：TASK-S1-P0-WORKER-RUNTIME-001

### 任务目标与范围

- **核心目标**：为写盘前失败的原导出任务提供受控恢复入口，复用原 `export_job_id`、原 `ASSIGNED` 对象和原盘 assignment，不创建新 job、不绕过 Center key 状态机。
- **对应代码位置**：`crates/edge-backend/src/control.rs`、`crates/edge-backend/src/server.rs`、`crates/edge-backend/src/export_runtime.rs`、`crates/center-backend/src/lib.rs`

### 协议与数据结构约束

- Edge 新增本端受控 API：`POST /api/edge/export-jobs/{export_job_id}/recover`，必须使用 `X-Edge-Control-Token`。
- 恢复只允许 `export_job.status = FAILED` 且 `copied_count = 0`、`copied_bytes = 0`。
- 所有 `export_object` 必须仍为 `ASSIGNED`，且不得已有 hash、nonce、tag、partial/data/meta 路径等写盘字段。
- API 响应使用 `export_job_status`、`recovered_disk_count`、`worker_started_count`、`worker_failed_count`，不新增裸 `status`。

### 安全与状态机边界

- 恢复前必须确认原盘最新 `disk_runtime.status = READY`、无 partial residue、盘内 `disk_info.json.status.code = INITIALIZED`、`disk_id` 匹配、`data/`、`meta/`、`manifests/` 无残留文件。
- 只允许写前失败分类恢复；`SOURCE_CHANGED`、checksum/decrypt、sealed、manifest invalid、partial、disk full、disk removed 等失败不允许自动恢复。
- 恢复会向 `export_job.error_message` 追加 `export_recovery_requested_at` 与 `recovery_reason`，保留原失败信息。
- Center `/api/disk/export-key` 继续保持同 `export_job_id` 幂等；不同 job 不能接管已 `ISSUED` 的 data_key。

### 验收与检查清单

- [x] 无 `X-Edge-Control-Token` 的 recover 请求被拒绝。
- [x] recover 响应无裸 `status`。
- [x] 非 `FAILED` job 会拒绝恢复。
- [x] 对象已有写盘字段或非 `ASSIGNED` 会拒绝恢复。
- [x] 干净 `INITIALIZED` 原盘通过，data/meta/manifest/partial 残留拒绝。
- [x] 失败原因分类只允许写前阶段。
- [x] Center 同 job export-key 幂等、不同 job 拒绝有测试覆盖。

---

# 开发任务卡片：TASK-S1-P0-EXPORT-RECOVERY-002

### 任务基本信息

- **任务 ID**：TASK-S1-P0-EXPORT-RECOVERY-002
- **任务名称**：Edge 恢复失败根因可机读审计与旧任务确认
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 VM 验收修补
- **依赖任务**：TASK-S1-P0-EXPORT-RECOVERY-001

### 任务目标与范围

- **核心目标**：修补真实 VM 验收发现的恢复入口缺陷：生产 worker 失败聚合必须保留可机读失败代码和阶段，recover guard 不能把任意 generic worker 失败判为可恢复。
- **对应代码位置**：`crates/edge-backend/src/export_runtime.rs`、`crates/edge-backend/src/control.rs`、`crates/edge-backend/src/server.rs`

### 协议与状态机约束

- worker 失败会追加到 `export_job.error_message`，包含 `export_failure_code`、`export_failure_stage`、`worker_error_code` 和 `worker_error_message`。
- `recover` 请求新增受控字段：`admin_confirm_write_before_zero_copy` 和 `write_before_failure_code`，用于旧 generic failed job 的管理员确认。
- 新失败优先通过 `export_failure_stage=WRITE_BEFORE` 加白名单 `export_failure_code` 判定可恢复。
- 旧 failed job 只有在零写入证据全部通过，且管理员确认字段为 true 并提供白名单 `write_before_failure_code` 时才允许恢复。

### 安全与审计边界

- 未知 generic 聚合错误默认拒绝。
- `SOURCE_CHANGED`、校验失败、解密失败、partial、磁盘满、拔盘和写后阶段失败均拒绝恢复。
- 已写对象字段、manifest/data/meta/`.partial` 残留拒绝逻辑不放宽。
- 不新增 job、不重新分配对象、不直接 SQL 绕过状态机。

### 验收与检查清单

- [x] `MANIFEST_INVALID` 权限类写前失败会持久化为可机读 `WRITE_BEFORE` 审计。
- [x] 带可机读写前 marker 的失败允许恢复。
- [x] 未知 generic worker 失败拒绝恢复。
- [x] 旧 generic worker 失败必须管理员确认加白名单代码才允许恢复。
- [x] 已写/partial/manifest 拒绝规则保持。
- [x] Edge recover route 测试覆盖新 DTO 字段且响应无裸 `status`。

---

# 开发任务卡片：TASK-S1-EDGE-002

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-002
- **任务名称**：边缘磁盘检测、ext4 校验、disk_info 校验和恢复入口
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D1
- **计划时间**：Day 1 下午
- **依赖任务**：TASK-S1-COMMON-003, TASK-S1-EDGE-001

### 任务目标与范围

- **核心目标**：服务启动扫描已有运输盘，接入 udev 变化入口，读取 SN、容量、挂载点、文件系统、`disk_info.json`，调用中控 `/api/disk/verify`。
- **对应代码位置**：`crates/edge-backend/src/`

### 协议与数据结构约束

- `/api/disk/verify` 请求字段包含 `edge_code`、`disk_id`、`sn`、`capacity_bytes`、`free_bytes`、`status_code`、`protocol_version`。
- 运行态写入 `disk_runtime.status`，WS 输出为 `runtime_status`。

### 安全与状态机边界

- 非 ext4 写入 `REJECTED` 和 `FILESYSTEM_UNSUPPORTED`。
- `EDGE_COPYING` 残留或 `.partial` 残留必须先记录 `RECOVERY_REQUIRED`。
- 未注册、禁用或未导入封存数据的盘不得进入导出任务池。

### 验收与检查清单

- [ ] 插入非 ext4 盘时拒绝并记录 `FILESYSTEM_UNSUPPORTED`。
- [ ] 未注册盘被中控拒绝并阻断导出。
- [ ] 通过校验后写入 `READY` 并可进入任务池。

---

# 开发任务卡片：TASK-S1-EDGE-HOTFIX-001

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-HOTFIX-001
- **任务名称**：Edge 真实挂载盘硬件 SN 探测修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 独立编码执行窗口
- **计划时间**：Day 2 联调修补
- **依赖任务**：TASK-S1-EDGE-002, TASK-S1-CENTER-003

### 任务目标与范围

- **核心目标**：修补真实 Edge 重新扫描时 `ConfiguredMountProbe` 将挂载目录名或 label 当作 `sn` 发送给 Center 的缺陷，确保 `/api/disk/verify` payload 使用硬件 SN。
- **对应代码位置**：`crates/edge-backend/src/disk_detection.rs`

### 协议与数据结构约束

- 业务主身份仍为盘内 `disk_info.json.disk.disk_id` 和协议生命周期，硬件 SN 只作为 Center 注册记录的辅助校验。
- Edge verify 请求继续使用 `edge_code`、`disk_id`、`sn`、`capacity_bytes`、`free_bytes`、`status_code`、`protocol_version`，不得新增裸 `status`。
- 探测层读取并保留设备路径、FS UUID、label、`ID_SERIAL_SHORT` 和 `ID_SERIAL`；`sn` 优先使用 `ID_SERIAL_SHORT`，其次 `ID_SERIAL`。

### 安全与状态机边界

- 非 ext4 仍在本地拒绝并记录 `FILESYSTEM_UNSUPPORTED`，不得进入 Center verify。
- ext4 盘无法读取硬件 SN 时，本地拒绝并记录 `HARDWARE_SN_UNAVAILABLE`，不得向 Center 发送 `sn = null` 后进入任务池。
- `FUSTFS-TST-A` 等挂载目录名或文件系统 label 不得替代硬件 SN。

### 验收与检查清单

- [x] 回归测试覆盖 `ConfiguredMountProbe` 不使用挂载目录名作为硬件 SN。
- [x] 回归测试覆盖 verify payload 使用硬件 SN，且缺失硬件 SN 时不会调用 Center。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] 部署静态检查通过。

---

# 开发任务卡片：TASK-S1-EDGE-HOTFIX-002

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-HOTFIX-002
- **任务名称**：Edge RustFS S3 凭据显式注入修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 独立编码执行窗口
- **计划时间**：Day 2 联调修补
- **依赖任务**：TASK-S1-EDGE-003

### 任务目标与范围

- **核心目标**：修补 Edge RustFS/S3 adapter 未显式使用 `edge.toml` 中 `rustfs.access_key_id` / `rustfs.secret_access_key` 的问题，避免系统环境没有 AWS SDK 凭据变量时 `/api/edge/scan` 返回 `rustfs operation failed: dispatch failure`。
- **对应代码位置**：`crates/edge-backend/src/adapters.rs`、`crates/edge-backend/src/config.rs`、`crates/edge-backend/src/server.rs`

### 协议与数据结构约束

- 不新增 API、WebSocket、manifest 字段。
- 保留 RustFS endpoint、region 和既有 AWS SDK endpoint/TLS 行为。
- 配置环境变量覆盖继续使用 `RUSTFS_TRANSFER__` 前缀。

### 安全与状态机边界

- 不日志输出 `secret_access_key`，不把 RustFS S3 凭据写入数据库或运输盘。
- `edge_auth_secret` 仍只用于 Edge 调用 Center HTTP API 鉴权，不复用于 RustFS/S3。
- Edge scan 仍只走 List/HEAD 读路径，不删除、覆盖或修改 RustFS 源对象。

### 验收与检查清单

- [x] Edge S3 adapter 使用配置提供的静态凭据构造 `aws-sdk-s3` client。
- [x] 缺少 `rustfs.access_key_id` 或 `rustfs.secret_access_key` 时返回明确配置错误。
- [x] 回归测试覆盖无 AWS 系统环境变量时凭据来自 Edge 配置。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] 部署静态检查通过。

---

# 开发任务卡片：TASK-S1-EDGE-HOTFIX-003

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-HOTFIX-003
- **任务名称**：Edge disk_runtime 可重建准入修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 独立编码执行窗口
- **计划时间**：Day 3 VM 验收修补
- **依赖任务**：TASK-S1-EDGE-002, TASK-S1-CENTER-003

### 任务目标与范围

- **核心目标**：修补真实 Edge 上旧 `disk_runtime` 缓存阻断当前可信运输盘进入 scan/export 前置的问题，明确 `disk_runtime` 只代表当前插入、当前可运行的临时运行态。
- **对应代码位置**：`crates/edge-backend/src/disk_detection.rs`、`crates/edge-backend/src/rescan.rs`、`crates/edge-backend/src/server.rs`、`crates/edge-backend/src/control.rs`、`crates/edge-backend/src/disk_worker.rs`、`crates/edge-backend/src/export_runtime.rs`

### 协议与数据结构约束

- Edge 运行态不得以旧 `disk_runtime.disk_id` 是否匹配盘内 `disk_info.disk_id` 作为新盘准入条件；旧缓存存在或不匹配不得拒绝当前可信盘。
- 盘内 `disk_info.disk_id` 仍必须经 Center HMAC `/api/disk/verify` 与登记状态校验，且盘内生命周期必须为 `INITIALIZED`。
- Edge 不删除历史 `export_job`、`export_object`、manifest 或 seal 产物作为新盘准入前置。

### 安全与状态机边界

- 控制入口在 scan、创建导出任务、启动导出任务前同步刷新一次本地运输盘发现，失败则写任务前拒绝。
- 当前盘发现成功时仅按当前 `device_path`/`mount_path` 和当前盘 `disk_id` 替换同盘旧 `disk_runtime` 快照，再创建本次运行态。
- 导出成功、manifest 与封盘写入完成后，DiskWorker 在同一受控完成流程中删除该盘对应 `disk_runtime`，导出任务和对象历史继续保留。
- 物理槽位只用于当前发现写入和同盘旧 runtime 清理定位，不作为盘业务身份验收权威。
- 非 ext4、缺硬件 SN、`.partial` 残留、`EDGE_COPYING`、Center 拒绝或非 `INITIALIZED` 仍不得进入任务池。

### 验收与检查清单

- [x] 回归测试覆盖当前盘内 `disk_info.disk_id` 直接成为新 READY 运行态身份，不依赖旧缓存。
- [x] 回归测试覆盖 scan/create/start 控制入口先刷新运输盘发现再进入控制工作流。
- [x] 回归测试覆盖当前盘发现只替换同盘 runtime，不删除导出历史表。
- [x] 回归测试覆盖封盘成功后删除该盘 runtime，且导出对象历史和 manifest 保留。
- [x] 回归测试覆盖 Center 签名拒绝、非 ext4、`.partial` 残留仍阻断 READY。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] 部署静态检查通过。
- [ ] 真实 Edge VM 尚未部署本补丁，需部署窗口复验只读 rescan 后再触发 scan/export。

---

# 开发任务卡片：TASK-S1-EDGE-003

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-003
- **任务名称**：RustFS 全 bucket 扫描、对象稳定性判断和快照入库
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D2
- **计划时间**：Day 1 下午
- **依赖任务**：TASK-S1-EDGE-001

### 任务目标与范围

- **核心目标**：通过 RustFS S3 API 扫描全部 bucket，写入 `local_object_snapshot`，通过 HEAD 前后比对过滤变化对象。
- **对应代码位置**：`crates/edge-backend/src/`

### 协议与数据结构约束

- 保存 bucket、object key、size、ETag、last_modified、metadata。
- `last_modified` 归一化为 UTC。

### 安全与状态机边界

- 边缘端只允许 List、HEAD、GET。
- 不得删除、覆盖或修改 RustFS 源对象或 metadata。
- 变化对象记录 `SOURCE_CHANGED`。

### 验收与检查清单

- [ ] 能统计对象数量和容量。
- [ ] HEAD 前后不一致对象不进入导出。
- [ ] 扫描进度可供 WS/HTTP 汇总使用。

---

# 开发任务卡片：TASK-S1-EDGE-004

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-004
- **任务名称**：导出任务 Planner、容量预算、多盘分配事务和分块登记
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D2
- **计划时间**：Day 1 下午
- **依赖任务**：TASK-S1-EDGE-002, TASK-S1-EDGE-003

### 任务目标与范围

- **核心目标**：创建导出任务，计算容量预算，生成普通对象或分块记录，并用数据库事务完成多盘并发分配。
- **对应代码位置**：`crates/edge-backend/src/`

### 协议与数据结构约束

- `reserve_bytes = max(1 GiB, min(8 GiB, free_bytes * 2%))`。
- 默认分块大小 `10,737,418,240 bytes`。
- `chunk_total` 最大 `1,000,000`。

### 安全与状态机边界

- 分配必须使用 `SELECT ... FOR UPDATE SKIP LOCKED` 和 `UPDATE ... RETURNING`。
- Worker 只能处理 `RETURNING` 返回的对象或分块。
- 空间不足只停止该盘继续分配，不影响其他盘。

### 验收与检查清单

- [ ] 并发分配不会重复分配同一对象。
- [ ] 单事务分配总量不超过 `object_budget_bytes`。
- [ ] 超过分块上限记录 `CHUNK_INDEX_OVERFLOW`。

---

# 开发任务卡片：TASK-S1-EDGE-005

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-005
- **任务名称**：DiskWorker、加密写盘、metadata、manifest、封盘和导出 WS
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [x] Track 4: Web contract
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D1/D2
- **计划时间**：Day 2 上午
- **依赖任务**：TASK-S1-EDGE-004, TASK-S1-CENTER-003

### 任务目标与范围

- **核心目标**：实现每盘 DiskWorker，流式下载、AES-256-GCM 写 `.partial`、校验、`fsync`、原子 rename、写 metadata、manifest、manifest SHA256，封盘为 `SEALED`，并用 `ProgressAggregator` 推送进度。
- **对应代码位置**：`crates/edge-backend/src/`

### 协议与数据结构约束

- 只有 `EXPORTED` 对象进入 manifest。
- manifest 顶层 `seal_id` 与 `disk_info.json.edge.seal_id` 一致。
- WS 使用 `COPY_PROGRESS`、`COPY_DONE`、`SEAL_DONE`。

### 安全与状态机边界

- 明文 `disk_data_key` 只在内存使用。
- `.partial` 清理或隔离失败不得封盘。
- 写盘 `ENOSPC` 记录 `DISK_FULL`。

### 验收与检查清单

- [ ] 成功对象具备密文 SHA256、明文 SHA256、nonce、tag、AAD 和 metadata。
- [ ] 拔盘记录 `DISK_REMOVED`。
- [ ] 高频进度来自 `ProgressAggregator`。

---

# 开发任务卡片：TASK-S1-CENTER-004

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-004
- **任务名称**：导入 Worker、manifest 校验、解密上传、去重账本和导入 WS
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [x] Track 4: Web contract
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent C1/C2
- **计划时间**：Day 2 上午
- **依赖任务**：TASK-S1-CENTER-003, TASK-S1-TEST-001

### 任务目标与范围

- **核心目标**：基于 fixture 和真实封盘目录实现导入 Worker，校验 manifest、解密对象、上传 `archive-{edge_code}`、写 `object_ledger`、处理重复导入，并通过 `ProgressAggregator` 推送导入进度。
- **对应代码位置**：`crates/center-backend/src/`

### 协议与数据结构约束

- 归档 key 为 `{source_bucket}/{source_object_key}`。
- 去重身份为 `edge_code + source_bucket + source_key + source_etag + source_size_bytes + source_last_modified`。
- 导入并发锁单位为 `disk_id + seal_id`。

### 安全与状态机边界

- 导入失败不得写 `IMPORTED`。
- `REVOKED` 密钥默认不得自动解密历史数据。
- 跨盘分块缺块不得写 `object_ledger`。

### 验收与检查清单

- [ ] fixture SEALED 盘可导入。
- [ ] manifest 非法、校验失败、解密失败分别记录标准错误码。
- [ ] 重复插入已导入盘不会重复上传。
- [ ] 高频进度来自 `ProgressAggregator`。

---

# 开发任务卡片：TASK-S1-CENTER-005

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-005
- **任务名称**：导入后清理、重新初始化、密钥退役和失败恢复
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent C2
- **计划时间**：Day 2 下午
- **依赖任务**：TASK-S1-CENTER-004

### 任务目标与范围

- **核心目标**：导入完成后写 `IMPORTED`，执行清理封存数据、生成新密钥、重新写协议目录并最终写 `INITIALIZED`。
- **对应代码位置**：`crates/center-backend/src/`

### 协议与数据结构约束

- 清理前运行态 `CLEANING`。
- 重新初始化期间运行态 `REINITIALIZING`。
- 最后一步才写 `disk_info.json.status.code = INITIALIZED`。

### 安全与状态机边界

- 清理或重新初始化失败不得回滚 `import_job.status = DONE`。
- 失败时盘内保持 `IMPORTED`，运行态 `ERROR`，错误码 `REINIT_FAILED`。
- 旧密钥只在重新初始化成功后进入 `RETIRED`。

### 验收与检查清单

- [ ] 清理失败不会重复导入同一 `disk_id + seal_id`。
- [ ] 重新初始化失败时新密钥不可发放。
- [ ] 成功后运输盘可再次交付边缘端。

---

# 开发任务卡片：TASK-S1-P1-CENTER-REINIT-API-001

### 任务基本信息

- **任务 ID**：TASK-S1-P1-CENTER-REINIT-API-001
- **任务名称**：Center 受控清理重初始化生产 API
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 验收修补
- **依赖任务**：TASK-S1-CENTER-005

### 任务目标与范围

- **核心目标**：修补真实 VM 验收发现的 Center 部署态缺口，提供合法的受控 cleanup/reinitialize API，使已成功导入的 `IMPORTED` 运输盘可由 Center 清理封存数据并重新初始化为 `INITIALIZED`。
- **对应代码位置**：`crates/center-backend/src/reinitialize_runtime.rs`、`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/lib.rs`、`crates/center-backend/src/main.rs`

### 协议与状态机约束

- 新增 `POST /api/center/disks/{disk_id}/reinitialize`，继续使用 `X-Center-Control-Token`，请求必须包含 `mount_path`、`seal_id`、`expected_status_code=IMPORTED`、`operator_reason`、`confirm_reinitialize=true`。
- 写盘前 guard 必须确认 ext4、盘内 `disk_info.status.code=IMPORTED`、disk/seal 身份匹配、Center 签名验签通过、manifest 和 sha256 sidecar 校验通过、`.partial` 数量为 0。
- DB guard 必须确认 `import_job.status='DONE'`、目标盘仍启用、盘内 old `data_key_id` 属于同盘且状态为 `ISSUED` 或 `SEALED_READONLY`。
- `CLEANING`、`REINITIALIZING` 仅作为运行态日志/响应语义，不写入盘内生命周期；最终盘内生命周期写回 `INITIALIZED`。
- 成功后清理 `data/`、`meta/`、`manifests/` 等协议封存区，创建新 data key 并激活，旧 data key 仅在成功后进入 `RETIRED`。

### 安全与审计边界

- 不调用 Edge，不修改 RustFS 源对象，不删除或覆盖归档对象，不写 `object_ledger`。
- 不允许通过 `/api/disk/initialize` 覆盖式绕过清理；重初始化必须走受控 API 和状态机 guard。
- 任一 guard 失败时拒绝且盘维持原状；`.partial`、签名失败、manifest 失败、非 `IMPORTED`、非 ext4、其他 disk_id 均不得进入清理。
- 明文 data key 不写日志、DB 或运输盘；新 key 沿用当前 Center `LOCAL-MASTER-KEY` 包裹和 HMAC disk_info 签名。
- 本提交只完成代码合入和本地验证；真实 VM 尚未部署/验收，不得据此宣称目标盘已重新初始化完成。

### 验收与检查清单

- [x] 缺失或错误 `X-Center-Control-Token` 会拒绝且不触发服务。
- [x] 非 `IMPORTED`、签名缺失/验签失败、manifest sha256 不匹配、`.partial` 残留、非 ext4、其他 disk_id 均拒绝且不清理。
- [x] 成功路径清理封存 payload，盘内状态变为 `INITIALIZED`，响应使用 `disk_status_code`、`runtime_status`，无裸 `status`。
- [x] 成功后新 data key 激活，旧 data key 退役；失败时旧 key 和盘内 `IMPORTED` 边界保留。
- [x] `cargo fmt --all -- --check`、`cargo test -p rustfs-transfer-center`、`cargo test --workspace`、`scripts/check-deploy.ps1` 通过后独立提交。

---

# 开发任务卡片：TASK-S1-P1-CENTER-REINIT-FSTYPE-002

### 任务基本信息

- **任务 ID**：TASK-S1-P1-CENTER-REINIT-FSTYPE-002
- **任务名称**：Center reinitialize findmnt 多行 ext4 解析修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 热修补
- **依赖任务**：TASK-S1-P1-CENTER-REINIT-API-001

### 任务目标与范围

- **核心目标**：修补真实 VM 中 Center 受控 reinitialize API 在服务 mount namespace 内 `findmnt` 返回重复 `ext4` 行时被严格整串比较拒绝的问题。
- **现场结论**：此前两次真实 API 调用均在写盘前被 HTTP 400 安全拒绝，未写运输盘、未写 DB。
- **对应代码位置**：`crates/center-backend/src/reinitialize_runtime.rs`

### 协议与状态机约束

- `findmnt` FSTYPE 输出必须结构化解析；空白、多行、重复 `ext4` 允许规范化为 `ext4`。
- 空输出、混合文件系统、任一非 `ext4`、命令失败必须拒绝。
- ext4 安全边界不得放宽；reinitializer 写盘仍只能在 preflight 全部通过后发生。

### 验收与检查清单

- [x] 单测覆盖单行 `ext4`、重复 `ext4`、多行空白 `ext4`。
- [x] 单测覆盖混合 `ext4+xfs`、空输出、命令失败拒绝。
- [x] 拒绝路径仍在 `PostImportReinitializer` 写盘前。
- [x] `cargo fmt --all -- --check`、`cargo test -p rustfs-transfer-center`、`cargo test --workspace`、`scripts/check-deploy.ps1` 通过后独立提交。

---

# 开发任务卡片：TASK-S1-P1-CENTER-REINIT-UPDATED-AT-003

### 任务基本信息

- **任务 ID**：TASK-S1-P1-CENTER-REINIT-UPDATED-AT-003
- **任务名称**：Center reinitialize 兼容旧盘缺 updated_at
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 热修补
- **依赖任务**：TASK-S1-P1-CENTER-REINIT-FSTYPE-002

### 任务目标与范围

- **核心目标**：修补真实 VM 中旧导入路径生成的 `IMPORTED` 盘缺少顶层 `updated_at`，导致受控 reinitialize 在任何写入前反序列化失败的问题。
- **现场结论**：此前真实 API 调用在写盘前 HTTP 400 安全拒绝，已核验零写盘、零 DB 写入。
- **对应代码位置**：`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`、`crates/center-backend/src/import_worker.rs`、`crates/center-backend/tests/import_worker.rs`

### 协议与状态机约束

- 兼容范围限于 Center reinitialize 读取旧 `IMPORTED` 盘 `disk_info.json` 时缺少 `updated_at`；缺失时使用 Unix epoch sentinel 作为可审计兼容语义。
- 空字符串、非法 timestamp、缺少其他关键字段、非 `IMPORTED` 生命周期缺少 `updated_at` 仍拒绝。
- 新写出的 `INITIALIZED` disk_info 必须包含标准 `updated_at`。
- 新 ImportWorker 成功标记 `IMPORTED` 时同步写顶层 `updated_at`，避免继续产生旧格式盘。

### 验收与检查清单

- [x] 旧 `IMPORTED` disk_info 缺 `updated_at` 可解析为 sentinel。
- [x] 非 `IMPORTED` 盘缺 `updated_at` 仍拒绝。
- [x] 缺 `updated_at` 能进入后续签名/manifest/status/identity guard，而不是在 JSON 解析层失败。
- [x] 空/非法 `updated_at`、缺少 `security` 等关键字段仍拒绝。
- [x] 成功 reinitialize 写回 `INITIALIZED` disk_info 含新 `updated_at`。
- [x] ImportWorker 成功导入写回 `IMPORTED` disk_info 含 `updated_at`。
- [x] `cargo fmt --all -- --check`、`cargo test -p rustfs-transfer-center`、`cargo test --workspace`、`scripts/check-deploy.ps1` 通过后独立提交。

---

# 开发任务卡片：TASK-S1-P1-CENTER-REINIT-SIGNATURE-004

### 任务基本信息

- **任务 ID**：TASK-S1-P1-CENTER-REINIT-SIGNATURE-004
- **任务名称**：Center reinitialize 兼容旧 IMPORTED 签名 canonical
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 热修补
- **依赖任务**：TASK-S1-P1-CENTER-REINIT-UPDATED-AT-003

### 任务目标与范围

- **核心目标**：修补真实 VM 中旧 `IMPORTED` 盘因历史 `disk_info.json` 原始字段 canonical 与当前反序列化结构 canonical 不一致，导致受控 reinitialize 在写盘前 HMAC 验签失败的问题。
- **现场结论**：此前真实 API 调用在写盘前 HTTP 400 安全拒绝，已核验零写盘、零 DB 写入。
- **对应代码位置**：`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`、`crates/center-backend/src/lib.rs`

### 协议与状态机约束

- 当前 canonical HMAC 验签继续优先执行且必须成功才放行当前格式。
- 历史兼容仅限原始盘内 JSON 为 `IMPORTED` 且顶层 `updated_at` 缺失的旧形态；兼容路径使用盘上原始 JSON 和同一 Center HMAC key 重新验签。
- 无签名、错误签名、错 key、非 `IMPORTED`、或带 `updated_at` 的历史签名均拒绝。
- 新 initialize 写出的 `disk_info.json` 必须继续包含 `updated_at`，并写当前 canonical HMAC 签名，不得写 mock signature。

### 验收与检查清单

- [x] 当前 canonical 签名通过 reinitialize preflight。
- [x] 旧 `IMPORTED`、缺 `updated_at`、原始 JSON canonical 签名通过 reinitialize preflight。
- [x] 无签名、错误签名、错 key 拒绝，且仍停在写盘前。
- [x] 旧签名兼容只限缺 `updated_at` 的 `IMPORTED` 历史形态。
- [x] 新 initialize 写回 `updated_at` 和当前 HMAC 签名，且不写 mock signature。
- [x] `cargo fmt --all -- --check`、`cargo test -p rustfs-transfer-center`、`cargo test --workspace`、`scripts/check-deploy.ps1` 通过后独立提交。

---

# 开发任务卡片：TASK-S1-P1-CENTER-REINIT-SIGNATURE-005

### 任务基本信息

- **任务 ID**：TASK-S1-P1-CENTER-REINIT-SIGNATURE-005
- **任务名称**：Center reinitialize 统一内外层 center_signature 验签
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web / Deploy
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 热修补
- **依赖任务**：TASK-S1-P1-CENTER-REINIT-SIGNATURE-004

### 任务目标与范围

- **核心目标**：修补真实 VM 第四次受控 reinitialize 发现的内外层验签不一致：外层 preflight 已按旧盘 raw canonical 验证缺 `updated_at` 的 `IMPORTED` 盘，但 `PostImportReinitializer` 内部重新读取补值后的结构并按 current-only canonical 再验签，导致写盘前 HTTP 400。
- **对应代码位置**：`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`

### 协议与安全边界

- 不修改 `docs/v1.0冻结/`，不新增迁移，不修改协议语义。
- `validate_reinitialize_preflight` 返回同一份 `DiskInfoDocument`，runtime 和 `PostImportReinitializer` 都使用该 raw-context document。
- 外层与核心层共用 `validate_center_signature_for_reinitialize`；不得用裸布尔值跳过核心层验签。
- 用户已更新冻结协议，`center_signature` 覆盖字段不再包含 `center`；当前 canonical 严格覆盖 `protocol`、`disk`、`security.center_key_id`、`security.signature_alg`、`security.data_key_id`。
- 旧 `IMPORTED`/缺 top-level `updated_at` raw canonical 仍必须通过真实 Center HMAC 验签；错误签名、错 key、非 `IMPORTED`、带 `updated_at` 的历史签名仍在写入前拒绝。
- 核心 reinitializer 仍保留 identity、`IMPORTED`、seal、DONE import、old data key、`.partial=0`、key staging/activation/retire 等防线。

### 验收与检查清单

- [x] `legacy_missing_updated_at_signature_reinitializes_through_runtime_and_core` 覆盖旧 `IMPORTED` 盘从 runtime 到核心层成功清理并写回 `INITIALIZED`。
- [x] `imported_status_change_without_resigning_passes_outer_and_inner_verifiers` 覆盖导入仅改 `IMPORTED`/edge/manifest/updated_at 且不重签时，外层 preflight 与核心层使用同一真实 HMAC canonical 放行。
- [x] `imported_disk_info_keeps_signature_valid_after_center_import_mark_without_compat_path` 覆盖 `center` 导入元数据不在签名范围内，且 `data_key_id` 等签名字段篡改仍拒绝。
- [x] `runtime_bad_signature_rejects_before_core_writes_or_repo_updates` 覆盖错误签名在写盘和 repo 更新前拒绝，保留原盘内文件和 payload。
- [x] 既有 current 签名、历史签名范围、错签名/错 key、manifest、partial、非 ext4、非 `IMPORTED` 测试继续通过。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] `cargo test --workspace` 通过。
- [x] `powershell -ExecutionPolicy Bypass -File scripts\check-deploy.ps1` 通过。
- [x] 该补丁尚未部署真实 VM 验收，不得宣称 `FUSTFS-TST-A` 已完成 cleanup/reinitialize。

---

# 开发任务卡片：TASK-S1-P1-CENTER-REINIT-ADMISSION-006

### 任务基本信息

- **任务 ID**：TASK-S1-P1-CENTER-REINIT-ADMISSION-006
- **任务名称**：Center reinitialize 最小生命周期准入
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web / Deploy
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 热修补
- **依赖任务**：TASK-S1-P1-CENTER-REINIT-SIGNATURE-005

### 任务目标与范围

- **核心目标**：按用户确认的最小生命周期准入口径，移除 manifest、object_ledger、import_job DONE、old data_key 绑定作为 cleanup/reinitialize 启动门槛，避免重复验证导入完成事实阻塞 Center 对已导入盘的受控清理重初始化。
- **对应代码位置**：`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`

### 准入与拒绝边界

- 准入仅保留 Center 已登记且启用的目标盘身份、ext4、`.partial` 恢复检查、盘内 `disk_info.status.code=IMPORTED`、真实 `center_signature` 验签。
- Center 登记身份以 `disk_id` 为主，SN 仅作辅助核验；登记 SN 与盘内 SN 同时存在且不一致时写入前拒绝。
- 错误签名、非 `IMPORTED`、身份不匹配、非 ext4、`.partial` 残留仍在写入前拒绝。
- manifest、object_ledger、import_job DONE、old data_key 绑定不再作为启动前置；本任务不改 archive/source/object_ledger。
- 成功路径必须激活新 key；旧 key 退役缺失或失败只记录 warn，不回滚已成功的受控清理重初始化。

### 验收与检查清单

- [x] `minimal_admission_reinitializes_without_manifest_or_import_job_gate` 覆盖无 manifest/import_job 门槛时满足五项准入即可进入清理并写回 `INITIALIZED`。
- [x] `old_data_key_binding_mismatch_is_not_an_admission_gate` 覆盖 old data key 绑定不再阻断准入。
- [x] `old_key_retirement_failure_warns_without_blocking_new_key_activation` 与 `runtime_old_key_retirement_failure_does_not_block_reinitialize` 覆盖旧 key 退役失败只 warn，新 key 激活成功仍完成。
- [x] 非 ext4、错误签名、非 `IMPORTED`、身份不匹配、`.partial` 残留仍写入前拒绝。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] `cargo test --workspace` 通过。
- [x] `powershell -ExecutionPolicy Bypass -File scripts\check-deploy.ps1` 通过。
- [x] 该最小准入补丁尚未部署真实 VM 验收，不得宣称 `FUSTFS-TST-A` 已完成 cleanup/reinitialize。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-004

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-004
- **任务名称**：Center reinitialize disk_info 失败原子性修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [x] Track 4: Security / Tests
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 VM 故障修补
- **依赖任务**：TASK-S1-P1-CENTER-REINIT-ADMISSION-006

### 任务目标与范围

- **核心目标**：修补真实 VM 失败暴露的 reinitialize 原子性缺陷，确保前置兼容解析、验签、清理失败、目录重建失败、新 key 激活失败和最终写盘失败不会把旧 `disk_info.json` 重写成变异格式。
- **对应代码位置**：`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`

### 协议与数据结构约束

- 前置解析、legacy `updated_at` 补值与验签必须完全在内存中完成。
- 失败路径不得为了“恢复”而写回旧 `disk_info.json`；盘内生命周期仍保持旧 `IMPORTED` 字节。
- 对旧 bug 已写入的精确 `1970-01-01T00:00:00Z` sentinel，仅在 `IMPORTED` 且移除该字段后真实 HMAC 验签通过时兼容。

### 安全与状态机边界

- 清理失败或 key 激活失败不得改变盘内 `disk_info.json` 字节，也不得激活/退役 DB key。
- 成功路径最后写入新的 `INITIALIZED`、标准 `updated_at` 和当前 `center_signature`。
- 最终 `disk_info.json` 原子写失败时，尽力把刚激活的新 key 回滚为不可发放状态；旧 key 退役仍只在写盘成功后执行。
- 错误签名、错误 key、非 `IMPORTED`、身份不匹配、非 ext4、`.partial` 残留仍在写入前拒绝。

### 验收与检查清单

- [x] 清理失败时 `disk_info.json` 原始字节不变、签名仍有效、无 staged/active/retired key 变化。
- [x] key 激活失败时不写盘、不退役旧 key。
- [x] 精确 sentinel 过渡盘通过真实 HMAC 兼容，但失败路径不写盘。
- [x] sentinel 过渡盘任意签名覆盖字段篡改仍拒绝。
- [x] 成功路径写入非 sentinel `updated_at` 和当前有效签名。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-common` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] `cargo test --workspace` 通过。
- [x] `powershell -ExecutionPolicy Bypass -File scripts\check-deploy.ps1` 通过。
- [x] 该 failure-atomicity 补丁尚未部署真实 VM 复验，不得宣称 `FUSTFS-TST-A` 已完成 cleanup/reinitialize。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-005

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-005
- **任务名称**：Center 导入成功后 data_key 封盘生命周期绑定
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [x] Track 5: DB / Security
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 2 真实导入修补
- **依赖任务**：TASK-S1-CENTER-004, TASK-S1-CENTER-005

### 任务目标与范围

- **核心目标**：修补真实导入后当前 `data_key` 仍停留在 `ISSUED` 且缺少 `seal_id` 绑定的缺口，确保本次导入完成与同一 `data_key` 封为只读在同一完成动作内提交。
- **对应代码位置**：`crates/center-backend/src/import_worker.rs`、`crates/center-backend/src/import_runtime.rs`、`crates/center-backend/src/lib.rs`、`crates/center-backend/tests/import_worker.rs`

### 协议与数据结构约束

- `disk_data_key` 仍只允许中控导入解密使用，不写入运输盘、前端响应或日志。
- 导入成功后仅把本次 `disk_id + data_key_id + export_job_id + seal_id` 对应 key 置为 `SEALED_READONLY` 并补齐绑定。
- 旧 key 退役仍属于重新初始化成功路径，本任务不得在导入完成时把 key 置为 `RETIRED`。

### 安全与状态机边界

- 导入失败、解密失败、manifest 与 `disk_info.security.data_key_id` 不一致时，不得更新任意 `data_key` 生命周期。
- 重复插入已 DONE 的同一封盘批次时，仅允许幂等补齐同一导入绑定，不得更新其他盘、其他 key 或其他导出任务。
- 生产 PG 路径必须在事务中完成 `import_job` 完成状态与 `data_key` 封盘绑定；绑定失败则不得把任务误标为 DONE。
- 本任务只补运行时事务和测试，不新增迁移；既有中心迁移已包含 `data_key.seal_id`、`sealed_time` 与 `SEALED_READONLY` 状态。

### 验收与检查清单

- [x] 导入成功后内存仓库 key 状态变为 `SEALED_READONLY`，并绑定同一 `export_job_id + seal_id`。
- [x] 重复 DONE 导入可幂等补齐旧 `ISSUED` key 的 seal 绑定。
- [x] 目标 key 缺失时不会误更新其他盘同名 key。
- [x] PG SQL 使用 `disk_id + data_key_id + export_job_id + seal_id` 严格谓词。
- [x] 受控导入路由覆盖缺失 token 不调用服务、有 token 调用服务、服务失败不返回成功。
- [x] f86c 来源任务原 ID 与主线 reinitialize 原子性任务冲突，本合入改用 `TASK-S1-CENTER-HOTFIX-005` 记录。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] 未修改 `docs/v1.0冻结/`，未部署、未调用真实 API、未操作硬盘或生产 SQL。

---

# 开发任务卡片：TASK-S1-SECURITY-HOTFIX-006

### 任务基本信息

- **任务 ID**：TASK-S1-SECURITY-HOTFIX-006
- **任务名称**：AES-GCM AAD 同源构造与全加密面审计
- **所属 Track / 模块**：
  - [x] Track 1: Common (`crates/common`)
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入/提交窗口
- **计划时间**：Day 3 审计修补
- **依赖任务**：TASK-S1-COMMON-002, TASK-S1-EDGE-005, TASK-S1-CENTER-004

### 任务目标与范围

- **核心目标**：按用户已更新冻结协议做全加密面审计，确认 `center` 对象不进入 `center_signature` 或其它签名/加密输入，并修补 AES-GCM AAD 公共接口、Edge 写盘构造和 Center 导入复算校验的一致性缺口。
- **对应代码位置**：`crates/common/src/crypto/mod.rs`、`crates/edge-backend/src/disk_worker.rs`、`crates/center-backend/src/import_worker.rs`、相关测试和 fixture。

### 协议与数据结构约束

- `center_signature` canonical 严格覆盖 `protocol`、`disk`、`security.center_key_id`、`security.signature_alg`、`security.data_key_id`，不覆盖整个 `center` 对象。
- AES-GCM AAD 必须绑定 `disk_id`、`seal_id`、`export_job_id`、`bucket`、`key`、`chunk_group_id`、`chunk_index`、`chunk_total`、`chunk_offset_bytes`。
- Center `disk_data_key` 包裹/解包仍使用本机主密钥和 `disk_id + data_key_id` AAD；不得误删签名密钥配置或数据加密逻辑。

### 安全与状态机边界

- 本任务只调整签名/加密输入和导入前校验，不新增导出、导入、清理、重初始化、格式化、SQL 或真实 API 操作。
- Edge 写盘与 Center 导入使用 Common 同源 AAD 构造；Center 在 AES-GCM 解密前复算 AAD，不匹配则拒绝 manifest。
- 全仓审计确认 Web、deploy、scripts 不保存或暴露明文 `disk_data_key`、控制 token 或签名密钥；示例配置仅保留占位值。

### 验收与检查清单

- [x] Common `ObjectAad` 覆盖冻结协议完整 AAD 字段。
- [x] Edge DiskWorker 使用 Common AAD 构造并由测试断言 manifest AAD。
- [x] Center ImportWorker 复算 manifest 对象 AAD，篡改 AAD 在解密前拒绝。
- [x] 全加密面审计未发现整个 `center` 对象进入 `center_signature` 或其它签名/加密输入。
- [x] 冻结文档未修改、未暂存。

---

# 开发任务卡片：TASK-S1-SECURITY-HOTFIX-007

### 任务基本信息

- **任务 ID**：TASK-S1-SECURITY-HOTFIX-007
- **任务名称**：center_signature 同源签名/验签 API 与 reinitialize 分叉修复
- **所属 Track / 模块**：
  - [x] Track 1: Common (`crates/common`)
  - [ ] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 真实闭环阻塞修补窗口
- **计划时间**：Day 3 真实闭环阻塞修补
- **依赖任务**：TASK-S1-SECURITY-HOTFIX-006, TASK-S1-CENTER-005

### 任务目标与范围

- **核心目标**：修复真实闭环中同一 `IMPORTED` 盘在 Center 导入路径验签通过、但受控 reinitialize 前置验签失败的路径分叉；在 Common 抽出唯一 `center_signature` payload/canonicalize/sign/verify API，并迁移 Center 初始化、导入验签、清理/重初始化前验签和重新初始化签名路径复用该 API。
- **对应代码位置**：`crates/common/src/crypto/mod.rs`、`crates/center-backend/src/center_security.rs`、`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`、相关测试。

### 协议与数据结构约束

- `center_signature` payload 只能覆盖 `protocol`、`disk`、`security.center_key_id`、`security.signature_alg`、`security.data_key_id`。
- `center`、`status`、`edge`、`manifest`、`security.center_signature`、`updated_at` 均不属于签名覆盖字段。
- Canonical JSON 使用 RFC 8785 语义，UTF-8 编码，签名为 HMAC-SHA256 标准 Base64。
- 禁止各流程自行拼接 JSON、各自做局部过滤或跳过验签。

### 安全与状态机边界

- 本任务仅修改代码、测试、TASKS 与当天 dev log；不修改冻结文档，不部署 VM，不调用真实 API，不操作硬盘、数据库、RustFS 源对象、archive 或 object_ledger。
- reinitialize 仍必须执行真实 `center_signature` 验签；修补目标是消除签名输入口径分叉，不是放宽或绕过验签。

### 验收与检查清单

- [x] Common 暴露唯一 `center_signature` payload/canonicalize/sign/verify API。
- [x] Center 初始化、导入验签、清理/重初始化前验签、重新初始化签名路径均经 `CenterSecurity` 复用 Common API。
- [x] 同一 `disk_info` 生成后可被导入路径与 reinitialize 前置验签共同验证。
- [x] 修改 `center`、`status`、`edge`、`manifest` 等非覆盖字段不影响验签。
- [x] 修改任一覆盖字段会导致验签失败。
- [x] `center` 对象变化不影响验签。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-common` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] 未修改 `docs/v1.0冻结/`，未部署、未调用真实 API、未操作硬盘或生产 SQL。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-006

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-006
- **任务名称**：Center import API 空 import_job_status 错误包装修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 真实导入阻塞修补窗口
- **计划时间**：Day 3 真实导入阻塞修补
- **依赖任务**：TASK-S1-CENTER-004, TASK-S1-SECURITY-HOTFIX-007

### 任务目标与范围

- **核心目标**：修补真实 Center 导入 API 在 worker 尚未进入 `IMPORTING` 进度前失败时，把默认空进度状态包装成最外层错误 `import_job_status=`，导致真实错误码/错误信息被 API 响应遮蔽的问题。
- **对应代码位置**：`crates/center-backend/src/import_runtime.rs`。

### 协议与数据结构约束

- `import_job_status` 只表示导入任务状态，允许值为 `PENDING`、`IMPORTING`、`DONE`、`FAILED`、`CANCELLED`。
- 运输盘生命周期 `INITIALIZED`、`SEALED`、`IMPORTED` 不得混入 `import_job_status`。
- `POST /api/center/import-jobs/start` 请求 DTO 仍只需要 `mount_path`；不得新增请求协议字段或要求客户端传入 `import_job_status`。

### 安全与状态机边界

- 本任务仅修复错误包装和测试；不修改冻结文档，不部署 VM，不调用真实 API，不操作硬盘、数据库、RustFS 源对象、archive 或 object_ledger。
- 导入状态机和 repo 对非法任务状态的拒绝逻辑保持不放宽；修补目标是使失败响应保留真实 `ImportErrorCode` 与错误说明。

### 验收与检查清单

- [x] worker 未创建 import job 前失败时，API 错误消息归一为 `import_job_status=PENDING; <IMPORT_ERROR_CODE>: <message>`，不再出现空的 `import_job_status=`。
- [x] 回归测试覆盖空进度状态不会遮蔽真实错误码。
- [x] 回归测试覆盖 `import_job_status` 与运输盘生命周期状态语义隔离。
- [x] 既有受控导入路由测试确认请求缺省 `import_job_status` 仍可进入导入服务。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-common` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] 未修改 `docs/v1.0冻结/`，未部署、未调用真实 API、未操作硬盘或生产 SQL。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-007

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-007
- **任务名称**：Center 初始化与重新初始化 disk_info 生成同源修补
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [x] Track 3: Center (`crates/center-backend`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 真实导入阻塞修补窗口
- **计划时间**：Day 3 真实导入阻塞修补
- **依赖任务**：TASK-S1-CENTER-HOTFIX-006, TASK-S1-CENTER-005

### 任务目标与范围

- **核心目标**：修补 Center 注册初始化和导入后重新初始化各自拼装 `disk_info.json` 的分叉，避免 reinitialize 生成缺少 `protocol.name`、`disk.last_init_time`、`disk.initialized_by` 等冻结协议必填/签名覆盖字段的窄版协议文件。
- **对应代码位置**：`crates/center-backend/src/disk_info_document.rs`、`crates/center-backend/src/lib.rs`、`crates/center-backend/src/reinitializer.rs`、`crates/center-backend/src/reinitialize_runtime.rs`。

### 协议与数据结构约束

- `INITIALIZED` 状态的 `disk_info.json` 只能通过 Center 共享构造器生成，包含完整 `protocol`、`disk`、`edge`、`center`、`manifest`、`security`、`status` 结构。
- `center_signature` 仍按冻结协议只覆盖 `protocol`、`disk`、`security.center_key_id`、`security.data_key_id`、`security.signature_alg`；不得通过跳过验签绕过错误。
- `edge.seal_id`、`edge.export_job_id` 等盘内协议字段保持字符串形态；准入需要 UUID 时单独解析，避免 DTO 重序列化改变 canonical 输入。

### 安全与状态机边界

- 本任务仅修复本地代码和测试；不修改冻结文档，不部署 VM，不调用真实 API，不操作硬盘、数据库、RustFS 源对象、archive 或 object_ledger。
- 导入解析仍要求冻结协议必填字段；不放宽 import DTO 来兼容错误生成物。

### 验收与检查清单

- [x] Center 初始化和重新初始化成功路径共用同一套 `INITIALIZED` disk_info 构造与原子写入 helper。
- [x] 回归测试覆盖 reinitialize 后的 `disk_info.json` 具有完整 edge/manifest 空对象、`protocol.name` 和签名覆盖 disk 字段。
- [x] 回归测试覆盖重新初始化输出读回后 `center_signature` 仍可验证。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-common` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] 未修改 `docs/v1.0冻结/`，未部署、未调用真实 API、未操作硬盘或生产 SQL。

---

# 开发任务卡片：TASK-S1-CENTER-HOTFIX-008

### 任务基本信息

- **任务 ID**：TASK-S1-CENTER-HOTFIX-008
- **任务名称**：Center 受控丢弃 SEALED 测试导出并重置
- **所属 Track / 模块**：Center / Reinitialize
- **优先级**：P0 解阻塞
- **计划时间**：Day 3 从零闭环阻塞修补
- **当前状态**：[x]
- **依赖任务**：TASK-S1-CENTER-HOTFIX-007

### 任务目标与范围

- **核心目标**：新增最小受控能力，在用户明确放弃单个测试盘已封盘导出数据时，允许 Center 对满足严格门禁的 `SEALED` 盘清理测试导出载荷并重新初始化为 `INITIALIZED`，用于从零重新扫描导出。
- **对应代码位置**：`crates/center-backend/src/reinitialize_runtime.rs`、`crates/center-backend/src/reinitializer.rs`、相关 Center 测试。
- **不在范围内**：不修改冻结文档；不部署 VM；不触碰真实盘、源对象、archive、object_ledger 或无关 DB 记录；不放宽普通 `IMPORTED` 清理重新初始化路径。

### 验收标准

- [x] 仅当请求显式确认丢弃 `SEALED` 测试导出且 `expected_status_code=SEALED` 时进入新分支。
- [x] 进入写入前完成 ext4、协议文件、目标身份、`.partial=0`、Center 签名、封盘 seal、manifest/data/meta 自洽门禁。
- [x] 成功后清理测试导出 payload，写回完整 `INITIALIZED` disk_info，生成并激活新 data_key。
- [x] 失败时保持盘内 `SEALED` 边界，不手工 SQL，不影响源对象、其他盘、archive 或 object_ledger。
- [x] `cargo fmt --all -- --check`、`cargo test -p rustfs-transfer-common`、`cargo test -p rustfs-transfer-center` 通过。

# 开发任务卡片：TASK-S1-WEB-EDGE-001

### 任务基本信息

- **任务 ID**：TASK-S1-WEB-EDGE-001
- **任务名称**：边缘端 DashboardView、HTTP 汇总和 WS 进度展示
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [ ] Track 3: Center
  - [x] Track 4: Web (`web/edge-web`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent E1
- **计划时间**：Day 1-2
- **依赖任务**：TASK-S1-TEST-001

### 任务目标与范围

- **核心目标**：实现边缘端 Vue 3 `DashboardView`，Day 1 用 mock fixture，Day 2 对接真实 HTTP 汇总和 WS。
- **对应代码位置**：`web/edge-web/src/`

### 协议与数据结构约束

- 多盘并行展示以 `disks[]` 为每块盘独立进度来源。
- 运行中事件使用 `COPY_PROGRESS`。

### 安全与状态机边界

- 前端不得自行创造状态枚举。
- 不把裸 `status` 当作生命周期或任务态。

### 验收与检查清单

- [x] 展示扫描状态、盘列表、拷贝进度、当前对象、速度、错误。
- [x] WS 断开和页面刷新能恢复 HTTP 汇总状态。
- [x] 非 ext4、未注册、拔盘、空间不足和恢复要求均有展示。
- [x] Edge 前端保留 `/dashboard`、`/sync-records` 两页，Dashboard 为默认入口；未恢复 `/first-access`。
- [x] 浏览器仅使用本地 Edge 公开只读 Dashboard summary、导出记录列表/详情和 Edge progress WS 契约，不读取、不展示、不发送 Center 地址、`authorization_key` 或控制 token。
- [x] Edge 页面不展示 `IMPORTED`，不提供初始化、清理、格式化、导入或取消操作入口。
- [x] 按 UI/UE 交接方案完成 Dashboard 与同步记录页 16:9 主视觉候选实现，并保留 `web/edge-web/design-qa.md` 视觉 QA 记录。
- [x] 按用户最新决定将粒子传输效果排除出当前阻塞项；同步记录统计卡精确图标与稳定 1440x810 截图仍作为后续视觉复核残留。

---

# 开发任务卡片：TASK-S1-WEB-CENTER-001

### 任务基本信息

- **任务 ID**：TASK-S1-WEB-CENTER-001
- **任务名称**：中控端 DashboardView、HTTP 汇总和 WS 进度展示
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [ ] Track 3: Center
  - [x] Track 4: Web (`web/center-web`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent E2
- **计划时间**：Day 1-2
- **依赖任务**：TASK-S1-TEST-001

### 任务目标与范围

- **核心目标**：实现中控端 Vue 3 `DashboardView`，Day 1 用 mock fixture，Day 2 对接真实 HTTP 汇总和 WS。
- **对应代码位置**：`web/center-web/src/`

### 协议与数据结构约束

- 导入事件使用 `IMPORT_PROGRESS`。
- 导入任务状态字段使用 `import_job_status`。

### 安全与状态机边界

- `CLEANING`、`REINITIALIZING` 只作为运行态展示。
- 已导入但重新初始化失败的盘必须展示不可复用。

### 验收与检查清单

- [x] 展示注册/初始化状态、待导入盘、导入进度、当前对象、速度、错误、是否可复用。
- [x] 重复插入已导入硬盘不会显示为待导入。
- [x] `REINIT_FAILED` 有明确展示。

---

# 开发任务卡片：TASK-S1-DEPLOY-001

### 任务基本信息

- **任务 ID**：TASK-S1-DEPLOY-001
- **任务名称**：systemd、udev、配置示例和本地联调脚本
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`deploy/`, `scripts/`)
  - [x] Track 3: Center (`deploy/`, `scripts/`)
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent F
- **计划时间**：Day 1-2
- **依赖任务**：TASK-S1-CENTER-001, TASK-S1-EDGE-001

### 任务目标与范围

- **核心目标**：完善 systemd、udev、配置示例和本地联调脚本，支持快速启动双端、执行迁移和模拟运输盘目录。
- **对应代码位置**：`deploy/systemd/`、`deploy/udev/`、`deploy/config/`、`scripts/`

### 协议与数据结构约束

- 目录约定：`/opt/rustfs-transfer/`、`/etc/rustfs-transfer/`、`/var/lib/rustfs-transfer/`、`/var/log/rustfs-transfer/`。
- 环境变量前缀：`RUSTFS_TRANSFER__`。

### 安全与状态机边界

- udev 只触发磁盘变化识别，不直接执行业务导出或导入。
- 示例配置不得包含真实密钥。

### 验收与检查清单

- [ ] systemd 服务文件能启动 center 和 edge。
- [ ] udev 规则不会直接执行导出或导入业务。
- [ ] 脚本能创建本地联调用模拟运输盘目录。

---

# 开发任务卡片：TASK-S1-DASHBOARD-REALTIME-001

### 任务基本信息

- **任务 ID**：TASK-S1-DASHBOARD-REALTIME-001
- **任务名称**：双端真实 Dashboard HTTP summary 与本端 WebSocket 推送
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [x] Track 3: Center (`crates/center-backend`)
  - [x] Track 4: Web contract
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 联调修补
- **依赖任务**：TASK-S1-WEB-EDGE-001, TASK-S1-WEB-CENTER-001

### 任务目标与范围

- **核心目标**：补齐双端 Dashboard 刷新后的真实 HTTP summary 和运行中本端 WebSocket 推送入口，替换前端 fixture/mock 数据源。
- **对应代码位置**：`crates/center-backend/src/lib.rs`、`crates/edge-backend/src/control.rs`、`crates/edge-backend/src/server.rs`

### 协议与数据结构约束

- Center HTTP：`GET /api/center/summary`。
- Center WS：`GET /ws/center/import-progress`，兼容前端默认 `/ws/center/progress`，事件使用 `IMPORT_PROGRESS`。
- Edge HTTP：`GET /api/edge/summary`。
- Edge WS：`GET /ws/edge/copy-progress`，兼容前端默认 `/ws/edge/progress`，事件使用 `COPY_PROGRESS`。
- 输出继续使用 `disk_status_code`、`runtime_status`、`export_job_status`、`import_job_status`、`disk_enabled` 等语义字段。

### 安全与状态机边界

- WebSocket 只服务本端前端展示，不用于 Center/Edge 之间同步。
- 不新增导出、导入、清理或初始化写路径。
- Edge HTTP control token 保护保持不变；前端部署代理继续负责本地注入读取 summary 所需 token。

### 验收与检查清单

- [x] Edge Dashboard 页面刷新后调用真实 `/api/edge/summary`。
- [x] Center Dashboard 页面刷新后调用真实 `/api/center/summary`。
- [x] 双端 WS 输出事件无裸 `status` 字段。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-center` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] 双端前端 typecheck/build 通过。
- [x] 部署静态检查通过。

---

# 开发任务卡片：TASK-S1-P1-EDGE-WEB-CONTRACT-001

### 任务基本信息

- **任务 ID**：TASK-S1-P1-EDGE-WEB-CONTRACT-001
- **任务名称**：Edge Web 浏览器安全只读契约补齐
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [x] Track 4: Web contract
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Codex 串行合入执行窗口
- **计划时间**：Day 2 审计修补
- **依赖任务**：TASK-S1-DASHBOARD-REALTIME-001, TASK-S1-WEB-EDGE-001

### 任务目标与范围

- **核心目标**：补齐 Edge Web 页面可直接调用的浏览器公开只读接口，避免 Dashboard 和导出记录页依赖 `X-Edge-Control-Token`。
- **对应代码位置**：`crates/edge-backend/src/control.rs`、`crates/edge-backend/src/server.rs`

### 协议与数据结构约束

- 新增公开只读路径：`GET /api/edge/dashboard/summary`、`GET /api/edge/dashboard/export-jobs`、`GET /api/edge/dashboard/export-jobs/{export_job_id}`。
- 原受控运维路径保持不变：`GET /api/edge/summary`、scan、导出创建、启动、恢复等控制接口继续要求 `X-Edge-Control-Token`。
- 浏览器只读响应不得暴露控制 token、`disk_data_key`、`authorization_key`、nonce、tag 或 `data_key_id`。
- 浏览器 summary 不展示 `IMPORTED` 运输盘生命周期；如底层出现该状态，面向浏览器降级为错误展示。

### 安全与状态机边界

- 用户已明确授权 Edge Dashboard/导出记录浏览器只读接口对所有可访问浏览器公开，不做本机限制、登录、代理认证或 token。
- 本任务不新增扫描、导出创建、启动、恢复、初始化、清理、格式化、导入或任意写盘/写库路径。
- WebSocket 仍只用于本端前端展示，不用于 Center/Edge 在线同步。

### 验收与检查清单

- [x] `/api/edge/dashboard/summary` 无控制 token 可读。
- [x] `/api/edge/dashboard/export-jobs` 与详情接口无控制 token 可读。
- [x] `/api/edge/summary` 与控制 POST 路径仍要求控制 token。
- [x] 响应字段无裸 `status` 混用。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。
- [x] 未修改、未暂存 `docs/v1.0冻结/`。

---

# 开发任务卡片：TASK-S1-INTEGRATION-001

### 任务基本信息

- **任务 ID**：TASK-S1-INTEGRATION-001
- **任务名称**：主闭环联调、验收、风险登记和交付检查
- **所属 Track / 模块**：
  - [x] Track 1: Common
  - [x] Track 2: Edge
  - [x] Track 3: Center
  - [x] Track 4: Web / Deploy / Tests
- **任务状态**：[~] 开发中
- **负责人 / Role**：首席系统架构师 / Agent F
- **计划时间**：Day 2 下午
- **依赖任务**：Day 2 主闭环任务

### 任务目标与范围

- **核心目标**：合并各泳道成果，跑通最终演示路径，确认冻结文档约束未被破坏，登记未完成能力和剩余风险。
- **对应代码位置**：全仓库

### 协议与数据结构约束

- API、WebSocket、manifest、日志字段不得出现裸 `status` 混用。
- 跨盘分块如未完成完整合并，必须登记为风险项。

### 安全与状态机边界

- 不允许为了演示跳过 HMAC、加密、去重、数据库锁或恢复检查。
- 不允许修改冻结文档来适配实现。

### 验收与检查清单

- [ ] 后端测试或等价检查通过。
- [x] 前端构建或类型检查通过。
- [ ] 迁移脚本可在空库执行。
- [ ] 主闭环联调记录写入当天 dev log。
- [ ] 未完成项已回写对应任务卡和 dev log。

---

# 开发任务卡片：TASK-S1-EDGE-AUTO-001

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-AUTO-001
- **任务名称**：Edge 插盘后自动扫描、自动建任务和自动启动导出编排
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D3
- **计划时间**：Day 3 上午
- **依赖任务**：TASK-S1-EDGE-002, TASK-S1-EDGE-003, TASK-S1-EDGE-004, TASK-S1-EDGE-005

### 任务目标与范围

- **核心目标**：新增 Edge 自动编排模块，在运输盘准入为 `READY` 后自动串起扫描 RustFS、创建导出任务、启动导出 Worker，并保证重复 udev / 启动扫描不会重复创建同一轮导出。
- **建议代码位置**：`crates/edge-backend/src/auto_export.rs`、`crates/edge-backend/src/server.rs`、`crates/edge-backend/src/rescan.rs`、`crates/edge-backend/src/control.rs`。

### Interface 设计

- 新增深模块 `AutoExportOrchestrator`，外部 Interface 尽量保持一个入口：`on_transport_disks_refreshed(trigger)`。
- 该模块内部可复用 `EdgeControlService::scan_once`、`create_export_job`、`start_export_job`，不得复制扫描、Planner、分配或 DiskWorker 实现。
- 编排器必须接受配置开关，例如 `auto_export.enabled`、`auto_export.start_on_ready`、`auto_export.min_ready_disk_count`、`auto_export.cooldown_seconds`。
- 编排器需要返回结构化结果：未启用、无 READY 盘、已有活动任务、已启动任务、启动失败。

### 协议与数据结构约束

- 自动流程只能在 `disk_status_code = INITIALIZED` 且 `runtime_status = READY` 后启动。
- 同一批 READY 盘已有 `PENDING`、`SCANNING`、`COPYING`、`SEALING` 或未完成 `export_job` 时，不得创建新的自动任务。
- RustFS 扫描复用窗口由 `scan.reuse_window_minutes` 控制；窗口内已有 `DONE` 记录时复用最近扫描快照，设为 `0` 时不复用。
- 导出计划只能读取最近一次成功扫描窗口内的 `stable_status = STABLE` 快照。
- 自动任务创建后仍使用现有 `export_job_status`、`object_status` 和 `disk_runtime.status`。
- 不新增裸 `status` 字段。

### 安全与状态机边界

- udev 仍只触发 rescan；自动导出由 Edge 常驻服务执行。
- 自动编排不得绕过本地 ext4、`disk_info.json`、`center_signature`、`disk_status_code = INITIALIZED`、`.partial` 恢复检查和活动任务门禁。
- 自动编排不得依赖 Center `/api/disk/verify` 或 `/api/disk/export-key`；本次封盘 `disk_data_key` 必须由部署阶段 `edge_auth_secret` 本地派生。
- 缺少 `center.edge_auth_secret` 或派生失败时不得开始加密导出；中控不可达不得阻塞 Edge 离线导出主流程。
- Edge 不得初始化、清理、格式化、重新初始化或导入运输盘。
- 不得删除、覆盖或修改 RustFS 源对象。

### 验收与检查清单

- [x] 配置关闭时，插盘只进入 READY，不自动 scan/export。
- [x] 配置开启时，插入单块 `INITIALIZED` 盘后自动完成 scan、create export_job、start DiskWorker。
- [x] 服务启动时已有 READY 盘也能自动进入流程。
- [x] 扫描复用按 `scan.reuse_window_minutes` 执行；设为 `0` 时重复自动流程会重新扫描 RustFS。
- [x] 自动导出只包含最近一次成功扫描确认上传完成的 `STABLE` 对象。
- [x] 连续 udev 重复事件不会重复创建多个活动 export_job。
- [x] 已存在活动 export_job 时不会启动第二个自动任务。
- [x] 缺少本地 `edge_auth_secret` 或密钥派生失败时不启动导出，并留下可机读错误。
- [x] `cargo fmt --all -- --check` 通过。
- [x] `cargo test -p rustfs-transfer-edge` 通过。

---

# 开发任务卡片：TASK-S1-EDGE-WS-BOOT-001

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-WS-BOOT-001
- **任务名称**：Edge 插盘、校验、扫描和封盘阶段 WebSocket 事件补齐
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [x] Track 4: Web contract
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent D4
- **计划时间**：Day 3 上午
- **依赖任务**：TASK-S1-DASHBOARD-REALTIME-001, TASK-S1-EDGE-002

### 任务目标与范围

- **核心目标**：补齐 Edge 从插盘到封盘的全阶段 WebSocket 事件，让前端不用等到 `COPY_PROGRESS` 才知道后端发生了什么。
- **建议代码位置**：`crates/edge-backend/src/progress.rs`、`crates/edge-backend/src/scanner/progress.rs`、`crates/edge-backend/src/disk_detection.rs`、`crates/edge-backend/src/server.rs`。

### Interface 设计

- 新增或扩展一个小 Interface：`EdgeRealtimePublisher`，由 disk detection、scanner、auto orchestrator 和 DiskWorker 发布状态。
- WebSocket Publisher 统一消费该 Interface 的快照或广播通道。
- 不让前端轮询多个接口拼状态；后端提供单一事件流。

### 协议与数据结构约束

- 必须支持事件：`DISK_DETECTED`、`DISK_CHECKING`、`DISK_READY`、`DISK_REJECTED`、`DISK_REMOVED`、`SCAN_STARTED`、`SCAN_PROGRESS`、`SCAN_DONE`、`COPY_STARTED`、`COPY_PROGRESS`、`COPY_DONE`、`SEAL_DONE`、`ERROR`。
- 事件字段必须使用 `disk_status_code`、`runtime_status`、`export_job_status`、`object_status`。
- `COPY_PROGRESS` 的高频字节进度仍来自内存 `ProgressAggregator`。
- 扫描事件可以来自 scanner progress snapshot，但不得每秒扫描数据库聚合。

### 安全与状态机边界

- WebSocket 只服务 Edge 后端到 Edge 前端展示，不用于 Center/Edge 同步。
- 事件不得包含控制 token、`disk_data_key`、`edge_auth_secret`、nonce、tag 或 `data_key_id`。
- `SEAL_DONE` 只能在实际封盘和 `disk_info.json.status.code = SEALED` 成功后发布。

### 验收与检查清单

- [x] 插盘后 1 秒内 WebSocket 可收到 `DISK_DETECTED` 或 HTTP summary 可观察到对应新盘。
- [x] 准入校验期间可收到 `DISK_CHECKING`。
- [x] 校验失败可收到 `DISK_REJECTED` 或 `ERROR`，包含标准错误码。
- [x] 扫描期间可收到 `SCAN_STARTED`、`SCAN_PROGRESS`、`SCAN_DONE`。
- [x] 导出启动时可收到 `COPY_STARTED`。
- [x] 封盘成功时可收到 `SEAL_DONE`。
- [x] 所有事件 payload 无裸 `status` 混用。
- [x] `cargo test -p rustfs-transfer-edge` 通过。

---

# 开发任务卡片：TASK-S1-WEB-EDGE-PROD-001

### 任务基本信息

- **任务 ID**：TASK-S1-WEB-EDGE-PROD-001
- **任务名称**：Edge Dashboard 生产态清理与浏览器只读收口
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [ ] Track 3: Center
  - [x] Track 4: Web (`web/edge-web`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent E3
- **计划时间**：Day 3 上午
- **依赖任务**：TASK-S1-P1-EDGE-WEB-CONTRACT-001

### 任务目标与范围

- **核心目标**：把 Edge Dashboard 从演示/预览态收口为上线生产态，避免 HTTP 失败时展示假数据，移除浏览器写操作入口。
- **对应代码位置**：`web/edge-web/src/views/DashboardView.vue`、`web/edge-web/src/api/edgeDashboard.ts`、`web/edge-web/src/ws/edgeCopyProgress.ts`、`web/edge-web/src/style.css`。

### 协议与数据结构约束

- 页面首次加载只调用 `/api/edge/dashboard/summary`。
- 运行中只连接 `/ws/edge/copy-progress` 或兼容 `/ws/edge/progress`。
- 导出记录只使用 `/api/edge/dashboard/export-jobs` 和详情接口。
- 不调用 `/api/edge/scan`、`/api/edge/export-jobs`、`/api/edge/export-jobs/{id}/start`、`/recover` 等受控写路径。

### 安全与状态机边界

- 删除或隔离 `previewSummary` 上线展示兜底；HTTP 失败必须展示真实空态/错误态。
- 页面不得提供初始化、清理、格式化、重新初始化、导入、启动导出、恢复导出等写操作按钮。
- 浏览器代码不得读取、展示或发送控制 token、Center 地址、`authorization_key`、`disk_data_key`、nonce、tag 或 `data_key_id`。
- Edge 页面不得展示 `IMPORTED` 作为正常盘状态。

### 验收与检查清单

- [x] HTTP summary 失败时不展示假进度、假盘位或假对象。
- [x] Dashboard 无受控写接口调用。
- [x] 页面无“执行恢复检查”等生产不允许操作入口。
- [x] 空态、断线态、错误态都有真实展示。
- [x] `npm run typecheck` 通过。
- [x] `npm run build` 通过。
- [x] 浏览器接口响应字段映射无裸 `status` 混用。
- [x] Dashboard Edge 节点名仅在顶部标题右侧展示，左侧源服务器框不重复显示节点名。
- [x] 同步记录页统计卡仅展示总览不触发筛选，返回入口、列表和详情抽屉按 16:9 首屏对齐，底部提示条移除。
- [x] 同步记录统计逐项容错，列表全量在当前页时兜底计算；详情面板默认展示首条记录，无记录时显示暂无内容。
- [x] Dashboard 在 HTTP summary 已确认终态且 `disks[]` 为空时，不再因旧 WebSocket 非终态快照恢复已拔出的盘位列表。

---

# 开发任务卡片：TASK-S1-WEB-EDGE-TIMELINE-001

### 任务基本信息

- **任务 ID**：TASK-S1-WEB-EDGE-TIMELINE-001
- **任务名称**：Edge Dashboard 插盘即时反馈、每盘时间线和扫描态展示
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [ ] Track 3: Center
  - [x] Track 4: Web (`web/edge-web`)
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent E4
- **计划时间**：Day 3 下午
- **依赖任务**：TASK-S1-EDGE-WS-BOOT-001, TASK-S1-WEB-EDGE-PROD-001

### 任务目标与范围

- **核心目标**：实现 Edge Dashboard 全流程视觉反馈：插盘即时出现、校验中、扫描中、导出中、封盘完成、可拔盘和异常处理。
- **对应代码位置**：`web/edge-web/src/views/DashboardView.vue`、`web/edge-web/src/ws/edgeCopyProgress.ts`、`web/edge-web/src/api/edgeDashboard.ts`、`web/edge-web/src/style.css`。

### 协议与数据结构约束

- 每块盘以 WebSocket `disks[]` 为独立进度来源。
- 时间线节点固定为：已检测、校验中、已授权、扫描 RustFS、分配对象、写盘中、生成清单、封盘完成、可拔盘。
- 只有 `SEAL_DONE` 或 HTTP summary 确认 `disk_status_code = SEALED` 且 `runtime_status = DONE` 时，才展示可拔盘。
- 扫描态使用 `SCAN_STARTED`、`SCAN_PROGRESS`、`SCAN_DONE`，不得硬编码固定扫描百分比。

### 安全与状态机边界

- 前端不得本地推断封盘完成。
- 前端不得自行创造状态枚举；未知事件进入“未知/等待后端更新”展示，不转成成功态。
- 错误态必须展示 `last_error_code` 和后端 `message`。

### 验收与检查清单

- [x] 插盘后出现检测视觉反馈。
- [x] 每盘时间线能随事件推进。
- [x] 扫描期间显示 bucket、对象数、稳定对象数和容量。
- [x] 导出期间显示全局进度、每盘进度、当前对象、速度和剩余字节。
- [x] WebSocket 断开后显示重连态；重连后继续更新。
- [x] 页面刷新后 HTTP summary 能恢复当前状态。
- [x] 封盘前不显示可拔盘；封盘后显示可拔盘。
- [x] 移动端和 1440x810 桌面视口不出现文字重叠。
- [x] `npm run typecheck` 和 `npm run build` 通过。

---

# 开发任务卡片：TASK-S1-EDGE-AUTO-DEPLOY-001

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-AUTO-DEPLOY-001
- **任务名称**：Edge 自动流程配置开关、部署检查和回滚说明
- **所属 Track / 模块**：
- [ ] Track 1: Common
- [x] Track 2: Edge (`deploy/`, `crates/edge-backend`)
- [ ] Track 3: Center
- [ ] Track 4: Web
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent F1
- **计划时间**：Day 3 下午
- **依赖任务**：TASK-S1-EDGE-AUTO-001

### 任务目标与范围

- **核心目标**：让 Edge 自动流程可部署、可灰度、可回滚，并把上线检查写进静态验证和运行手册。
- **对应代码位置**：`deploy/config/edge.example.toml`、`deploy/systemd/`、`deploy/udev/`、`scripts/check-deploy.ps1`、`docs/上线要求.md`。

### 协议与数据结构约束

- 新增配置只能控制是否自动触发 scan/export，不改变协议字段、状态码或数据库语义。
- 默认值必须保守；如默认不开启，部署说明必须写清如何开启。
- udev 规则不得直接包含 export、import、cleanup、reinitialize、mkfs 或 format 业务动作。

### 安全与状态机边界

- 回滚方式必须能恢复到现有受控 API 模式。
- 示例配置不得包含真实密钥、token 或生产地址。
- 不得修改冻结文档。

### 验收与检查清单

- [x] 示例配置包含自动流程开关和说明。
- [x] 静态检查能确认 udev 仍只触发 rescan。
- [x] 部署说明包含开启、关闭、回滚和排障步骤。
- [x] `scripts/check-deploy.ps1` 通过。
- [x] 自动流程关闭后原受控 scan/export/start 仍可用。

---

# 开发任务卡片：TASK-S1-EDGE-REALTIME-QA-001

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-REALTIME-QA-001
- **任务名称**：Edge 前后端实时可视化端到端联调验收
- **所属 Track / 模块**：
  - [x] Track 1: Common
  - [x] Track 2: Edge
  - [ ] Track 3: Center
  - [x] Track 4: Web / Deploy / QA
- **任务状态**：[x] 开发完成
- **负责人 / Role**：Agent F2
- **计划时间**：Day 3 下午
- **依赖任务**：TASK-S1-P0-EDGE-OFFLINE-PACK-001, TASK-S1-EDGE-AUTO-001, TASK-S1-EDGE-WS-BOOT-001, TASK-S1-WEB-EDGE-TIMELINE-001, TASK-S1-EDGE-AUTO-DEPLOY-001

### 任务目标与范围

- **核心目标**：在真实 Edge 前后端环境跑通插盘自动导出可视化链路，并形成可交付验收记录。
- **对应位置**：`work_sprint/dev_log/`、`web/edge-web/design-qa.md`、必要时新增 `work_sprint/edge_realtime_acceptance.md`。

### 验收路径

```text
Center 初始化运输盘
-> 运输盘插入 Edge
-> Edge Dashboard 1 秒内出现 DISK_DETECTED 视觉反馈
-> Edge 自动校验并进入 READY
-> Edge 自动扫描 RustFS
-> Edge 自动创建并启动导出
-> Dashboard 展示 SCAN_PROGRESS 和 COPY_PROGRESS
-> Edge 封盘到 SEALED
-> Dashboard 展示 SEAL_DONE 和可拔盘
```

### 安全与状态机边界

- 不允许为演示跳过本地密钥派生、ext4 检查、`.partial` 恢复检查、AES-GCM 或数据库分配锁；Center 注册/启用权威判断必须在导入阶段验收。
- 不允许手工写数据库、手工改盘内协议文件或手工标记任务成功。
- 不允许修改冻结文档。

### 验收与检查清单

- [x] 插盘反馈已用 Edge HTTP summary、journald 和只读 DB 证据覆盖；本轮未采集浏览器截图。
- [x] WebSocket/实时事件序列以 HTTP/日志脱敏证据覆盖 `DISK_DETECTED -> DISK_CHECKING -> DISK_READY -> SCAN_* -> COPY_* -> SEAL_DONE`。
- [x] RustFS 扫描形成 59 个稳定对象导出任务；重复插盘/重启后复用现有任务与已完成状态。
- [x] 正在上传或扫描期间变化的对象不计入稳定对象统计，不进入导出队列；本轮未额外构造变化对象。
- [x] 页面刷新后 summary 恢复当前状态；Edge 重启后 summary 顶层恢复 `SEALED`。
- [x] WebSocket/HTTP 重连不造成假完成态；以任务记录、DB 和盘内状态交叉验证。
- [x] 盘内最终 `disk_info.json.status.code = SEALED`，Center 导入后为 `IMPORTED`。
- [x] `.partial=0`。
- [x] `export_job_status = SEALED`，Edge 对象状态无 `IMPORTED`。
- [x] HTTP/summary/日志未暴露敏感字段；未记录密钥、token、密码、私钥或完整连接串。
- [x] 后端测试、前端既有 typecheck/build、部署与 VM 服务级检查通过；本轮新增 Edge/Center 后端测试通过。
- [x] 验收记录写入当天 dev log 和 `work_sprint/edge_realtime_acceptance.md`。
