# AGENTS.md

本文件约束本仓库内所有后续 AI 代理（AI Agents）、研发人员和自动化改动。执行任何代码生成、重构或开发任务前，必须先阅读本文件，并严格按本文约束工作。

## 1. 项目定位

本项目是 RustFS 离线运输盘同步系统，包含：

- 中控端后端和前端。
- 边缘端后端和前端。
- 双端 PostgreSQL 数据库迁移。
- Linux systemd、udev 和部署配置。
- 运输盘离线协议、加密、导出、导入、去重和恢复逻辑。

## 2. 冻结文档

`docs/v1.0冻结/` 下所有文档已经冻结，是 v1.0 版本的业务、协议、数据库和架构基线：

- `docs/v1.0冻结/需求文档.md`
- `docs/v1.0冻结/开发协议契约.md`
- `docs/v1.0冻结/数据库设计.md`
- `docs/v1.0冻结/技术架构与技术选型.md`

硬约束：

- 禁止修改、移动、重命名、删除 `docs/v1.0冻结/` 下任何文件。
- 禁止为了“修正格式、错别字、排版、编码、链接”修改冻结文档。
- 冻结文档与实现冲突时，以冻结文档为准，调整实现。
- 需求变更必须新建补充文档或任务记录，不得直接改冻结文档。
- 如用户明确要求修改冻结文档，必须先提醒该目录已冻结，并等待用户再次明确授权。

## 3. 文档读取顺序

涉及需求、协议、数据库或架构判断时，按以下顺序读取：

1. `docs/v1.0冻结/需求文档.md`
2. `docs/v1.0冻结/开发协议契约.md`
3. `docs/v1.0冻结/数据库设计.md`
4. `docs/v1.0冻结/技术架构与技术选型.md`

读取完成标准：

- 能说清当前任务对应的端：中控端、边缘端、前端、数据库、部署或运输盘协议。
- 能说清当前任务涉及的状态字段、权威来源和禁止混用点。
- 能说清当前任务是否触碰运输盘生命周期、密钥、对象账本、导出任务或导入任务。

## 4. 开发任务记录

开发任务记录目录：

```text
work_sprint/dev_log/
```

规则：

- 每天一个 Markdown 文件，文件名格式为 `YYYY-MM-DD.md`。
- 开始实质开发前，确认当天日志文件存在。
- 任务完成后，在当天日志中记录完成内容、验证方式和待跟进项。
- 任务记录不得替代冻结文档；它只记录执行过程和补充事项。

## 5. 架构边界

必须保持以下边界：

- WebSocket 只用于后端向本端前端推送实时状态。
- 边缘端和中控端之间的在线交互只走 HTTP API。
- 对象数据只通过运输硬盘离线交接。
- 中控端数据库是运输盘注册、边缘站点、密钥、导入任务和对象账本的权威库。
- 边缘端数据库是本地扫描、导出任务、对象状态和运输盘运行状态账本。
- 运输盘生命周期状态以盘内 `disk_info.json.status.code` 为准。
- `CLEANING`、`REINITIALIZING` 只能作为运行态，不得写入盘内生命周期状态。

## 6. 状态字段命名

API、WebSocket、manifest 和跨模块结构中不得使用裸 `status` 表示多种含义。

必须使用：

- 运输盘生命周期：`status_code` 或 `disk_status_code`。
- 运输盘启用状态：`disk_enabled` 或 `enabled`。
- 边缘站点状态：`edge_status`。
- 运输盘运行态：`runtime_status`。
- 导出任务状态：`export_job_status`。
- 导入任务状态：`import_job_status`。
- 对象状态：`object_status`。

数据库表内部可以保留字段名 `status`，但序列化到 API、WebSocket、manifest 或日志语义字段时必须带前缀。

## 7. 后端实现约束

后端使用 Rust：

- 异步运行时：Tokio。
- HTTP / WebSocket：axum。
- 数据库访问：sqlx + 显式 SQL。
- 数据库：PostgreSQL。
- S3 访问：S3 兼容 SDK，优先 `aws-sdk-s3`。
- 序列化：serde / serde_json。
- 日志：tracing。
- 加密：AES-256-GCM。
- 校验：SHA256。

约束：

- 不引入 ORM。
- 不引入复杂任务队列或额外消息中间件。
- 不用内存锁替代数据库唯一约束和行级锁。
- 多盘对象分配必须使用 PostgreSQL 事务、`SELECT ... FOR UPDATE SKIP LOCKED` 和 `UPDATE ... RETURNING`。
- 运行中高频进度必须来自内存 `ProgressAggregator`，不得每秒扫描数据库聚合实时进度。

## 8. 数据和安全约束

必须遵守：

- 边缘端禁止删除、覆盖或修改 RustFS 源对象。
- 边缘端不得自动清理未导入封存数据。
- 边缘端不得保存明文 `disk_data_key` 到数据库或运输盘。
- `edge_auth_secret` 只用于 HTTP API 鉴权，不用于文件加密。
- `disk_data_key` 只用于运输盘对象数据 AES-256-GCM 加密。
- 运输盘和 manifest 只保存 `data_key_id`、nonce、tag、校验值等非明文密钥信息。
- 中控端导入去重以 `object_ledger` 为准。
- 边缘端 `export_object.status` 不包含 `IMPORTED`。

## 9. 运输盘和 Linux 约束

必须遵守：

- 运输盘文件系统固定为 ext4。
- 运行时检测到非 ext4 必须拒绝进入任务池。
- 边缘端不得格式化、清盘或重新初始化运输盘。
- 中控端负责初始化、导入完成后的清理和重新初始化。
- 写对象、metadata、manifest、`disk_info.json` 必须使用临时文件、`fsync` 和原子重命名。
- `.partial` 残留必须进入恢复检查；清理或隔离失败不得封盘或导入。
- udev 只触发磁盘变化识别，不直接执行业务导出或导入。
- 服务启动时必须主动扫描已存在运输盘。

## 10. 前端约束

前端使用：

- Vue 3。
- TypeScript。
- Vite。
- WebSocket。

规则：

- 中控端前端和边缘端前端独立部署。
- 主页面使用 `DashboardView`。
- 页面刷新后通过 HTTP 查询任务汇总状态。
- 运行中状态通过 WebSocket 更新。
- 多盘并行展示以 WebSocket `disks[]` 为每块盘独立进度来源。
- 前端不得自行创造状态枚举；状态码以冻结协议文档为准。

## 11. 数据库和迁移约束

数据库迁移放在：

```text
sql/center/
sql/edge/
```

规则：

- 使用 `sqlx migrate` 管理迁移。
- 中控端和边缘端迁移分开维护。
- 所有 timestamp 保存 UTC。
- 表、字段、索引、唯一约束必须与冻结数据库设计一致。
- SQL 文件应包含表、字段、索引和关键约束注释。
- 修改迁移前必须确认是否已有同名迁移被使用；已经落地的迁移不得随意改写，优先新增迁移。

## 12. 修改流程

开始改动前：

- 查看 `git status --short`，识别已有用户改动。
- 阅读当前任务涉及的冻结文档。
- 确认当天 `work_sprint/dev_log/YYYY-MM-DD.md` 存在。

改动中：

- 保持改动范围最小。
- 不重构无关代码。
- 不覆盖用户已有改动。
- 不修改冻结文档。
- 新增状态、字段、协议、表结构前先对照冻结文档。

完成后：

- 运行与改动范围匹配的格式化、类型检查或测试。
- 无法运行验证时说明原因。
- 更新当天开发日志。
- 汇报修改文件、核心变更和验证结果。

## 13. 禁止事项

- 禁止修改 `docs/v1.0冻结/`。
- 禁止引入与冻结文档冲突的协议字段、状态码或数据库语义。
- 禁止把 WebSocket 当作边缘端和中控端的数据同步通道。
- 禁止把中控端导入结果写回边缘端 `export_object.status`。
- 禁止只依赖硬件 SN 作为运输盘业务主身份。
- 禁止把 `edge_auth_secret` 复用为文件加密密钥。
- 禁止用内存锁替代多盘并行对象分配的数据库锁和唯一约束。
- 禁止在未通过恢复检查时继续导出、导入、清理或重新初始化。

## 14. 2 天 Sprint 任务协作

Sprint 任务总表位于：

```text
work_sprint/TASKS.md
```

执行规则：

- 当前开发目标是 2 天内完成可部署、可联调、可演示的 v1.0 主闭环。
- 开始任何开发任务前，先阅读 `work_sprint/TASKS.md` 的 Sprint 目标、不可降级底线和对应任务卡。
- 领取任务时，将任务状态从 `[ ]` 改为 `[~]`，并在当天 `work_sprint/dev_log/YYYY-MM-DD.md` 记录任务 ID、领取人或代理标识、开始时间和范围。
- 状态强同步规则：开发者或 Agent 在领取或完成任何任务卡片时，必须在同一 Commit 同步更新 `work_sprint/TASKS.md` 顶部“任务总览”列表与底部对应“任务卡片”的任务状态。严禁出现卡片内部标为 `[x]` 但总览行仍为 `[ ]`，或卡片与总览处于任何其他不一致状态。
- 一个代理一次只领取一个主任务；跨任务修改必须在开发日志写明原因和影响面。
- Day 1 优先完成 Common、数据库迁移、中控基础 API、边缘检测扫描和导出分配骨架。
- Day 2 优先完成边缘写盘封盘、中控导入去重、清理重新初始化、双端 Dashboard、验收测试和最终联调。
- 任务优先级按 `P0 解阻塞`、`P1 主闭环`、`P2 交付增强` 执行；同一时间优先领取 P0，P0 未完成时下游只能做 mock、测试草案或不阻塞的局部实现。
- P1 任务必须直接服务主闭环；P2 任务不得反向阻塞 P0/P1，也不得为了展示绕过安全、状态机或数据库一致性底线。
- 如完整能力会阻塞主闭环，可按 `work_sprint/TASKS.md` 的可后置项处理，但必须保留协议字段、错误码、清晰日志和待跟进记录。
- 完成任务后，将任务状态改为 `[x]`，并在当天开发日志记录完成内容、验证方式、未覆盖项和后续待跟进。
- 未完成、未验证或破坏不可降级底线的任务不得标记为 `[x]`。
- 修改任务范围、依赖、验收标准或新增任务时，只能更新 `work_sprint/TASKS.md` 或新增补充任务文档，不得修改冻结文档。

## 15. 提交前验证矩阵

按改动范围运行匹配验证；无法运行时，在最终汇报和当天 dev log 写明原因。

| 改动范围 | 必跑验证 |
|---|---|
| Rust common | `cargo fmt --all -- --check`；`cargo test -p rustfs-transfer-common` |
| Rust center | `cargo fmt --all -- --check`；`cargo test -p rustfs-transfer-center` |
| Rust edge | `cargo fmt --all -- --check`；`cargo test -p rustfs-transfer-edge` |
| SQL center | 在空 PostgreSQL 库执行 `sql/center/` 迁移；确认唯一约束和索引存在 |
| SQL edge | 在空 PostgreSQL 库执行 `sql/edge/` 迁移；确认多盘分配唯一约束存在 |
| Web center | 在 `web/center-web/` 运行 `npm run typecheck` 和 `npm run build` |
| Web edge | 在 `web/edge-web/` 运行 `npm run typecheck` 和 `npm run build` |
| Deploy / scripts | 检查 systemd `ExecStart` 路径、udev 不直接执行业务导出/导入、脚本不含真实密钥 |
| Sprint task status | 同一 Commit 内确认 `TASKS.md` 顶部任务总览和底部任务卡片状态一致 |

提交基线要求：

- Sprint 并行开发启动前，必须先形成一个干净基线 Commit，包含 `AGENTS.md`、`work_sprint/TASKS.md`、冻结文档目录状态、工程骨架、依赖清单和锁文件。
- 后续每个任务 Commit 只包含该任务必要改动、对应任务状态同步和当天 dev log 记录。
