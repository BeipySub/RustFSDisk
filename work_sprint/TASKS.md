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
| TASK-S1-COMMON-001 | 协议类型、状态枚举、HTTP/WS payload 和 golden JSON | P0 解阻塞 | Common | [ ] | Day 1 上午 | 无 |
| TASK-S1-DB-001 | 双端 PostgreSQL 迁移补齐并冻结 DDL | P0 解阻塞 | DB | [x] | Day 1 上午 | 无 |
| TASK-S1-COMMON-002 | HMAC、SHA256、AES-GCM 和 nonce 工具 | P0 解阻塞 | Common | [ ] | Day 1 上午 | TASK-S1-COMMON-001 可并行 |
| TASK-S1-COMMON-003 | 运输盘协议文件、原子写、fsync、安全路径和 `.partial` 扫描 | P0 解阻塞 | Common | [ ] | Day 1 上午 | TASK-S1-COMMON-001 |
| TASK-S1-TEST-001 | Fixture、模拟运输盘、manifest 样例和 HMAC 请求样例 | P0 解阻塞 | Tests | [ ] | Day 1 上午 | TASK-S1-COMMON-001 可并行 |
| TASK-S1-CENTER-001 | 中控服务骨架、配置、DB/S3 adapter、health check | P0 解阻塞 | Center | [ ] | Day 1 上午 | TASK-S1-DB-001 |
| TASK-S1-EDGE-001 | 边缘服务骨架、配置、中控 HMAC 客户端、DB/S3 adapter | P0 解阻塞 | Edge | [ ] | Day 1 上午 | TASK-S1-COMMON-002, TASK-S1-DB-001 |
| TASK-S1-CENTER-002 | 中控 HMAC 鉴权、nonce 防重放和 `/api/edge/auth` | P1 主闭环 | Center | [ ] | Day 1 下午 | TASK-S1-COMMON-002, TASK-S1-CENTER-001 |
| TASK-S1-CENTER-003 | 中控运输盘注册初始化、`/api/disk/verify`、`/api/disk/export-key` | P1 主闭环 | Center | [ ] | Day 1 下午 | TASK-S1-COMMON-003, TASK-S1-CENTER-002 |
| TASK-S1-EDGE-002 | 边缘磁盘检测、ext4 校验、disk_info 校验和恢复入口 | P1 主闭环 | Edge | [ ] | Day 1 下午 | TASK-S1-COMMON-003, TASK-S1-EDGE-001 |
| TASK-S1-EDGE-003 | RustFS 全 bucket 扫描、对象稳定性判断和快照入库 | P1 主闭环 | Edge | [ ] | Day 1 下午 | TASK-S1-EDGE-001 |
| TASK-S1-EDGE-004 | 导出任务 Planner、容量预算、多盘分配事务和分块登记 | P1 主闭环 | Edge | [ ] | Day 1 下午 | TASK-S1-EDGE-002, TASK-S1-EDGE-003 |
| TASK-S1-EDGE-005 | DiskWorker、加密写盘、metadata、manifest、封盘和导出 WS | P1 主闭环 | Edge | [ ] | Day 2 上午 | TASK-S1-EDGE-004, TASK-S1-CENTER-003 |
| TASK-S1-CENTER-004 | 导入 Worker、manifest 校验、解密上传、去重账本和导入 WS | P1 主闭环 | Center | [ ] | Day 2 上午 | TASK-S1-CENTER-003, TASK-S1-TEST-001 |
| TASK-S1-CENTER-005 | 导入后清理、重新初始化、密钥退役和失败恢复 | P1 主闭环 | Center | [ ] | Day 2 下午 | TASK-S1-CENTER-004 |
| TASK-S1-WEB-EDGE-001 | 边缘端 DashboardView、HTTP 汇总和 WS 进度展示 | P2 交付增强 | Web / Edge | [ ] | Day 1-2 | TASK-S1-TEST-001 |
| TASK-S1-WEB-CENTER-001 | 中控端 DashboardView、HTTP 汇总和 WS 进度展示 | P2 交付增强 | Web / Center | [ ] | Day 1-2 | TASK-S1-TEST-001 |
| TASK-S1-DEPLOY-001 | systemd、udev、配置示例和本地联调脚本 | P2 交付增强 | Deploy | [ ] | Day 1-2 | TASK-S1-CENTER-001, TASK-S1-EDGE-001 |
| TASK-S1-INTEGRATION-001 | 主闭环联调、验收、风险登记和交付检查 | P2 交付增强 | Integration | [ ] | Day 2 下午 | Day 2 主闭环任务 |

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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
- **负责人 / Role**：Agent A2
- **计划时间**：Day 1 上午
- **依赖任务**：TASK-S1-COMMON-001 可并行

### 任务目标与范围

- **核心目标**：提供 HMAC canonical request、SHA256、AES-256-GCM 加解密、nonce 生成和 Base64/hex 编码工具。
- **对应代码位置**：`crates/common/src/crypto/`、`crates/common/tests/`

### 协议与数据结构约束

- HMAC 签名串为 `METHOD + "\n" + canonical_path_with_query + "\n" + X-Timestamp + "\n" + X-Nonce + "\n" + X-Body-SHA256`。
- query 参数按冻结契约排序和 percent-encoding。
- AES-GCM AAD 由 `disk_id/export_job_id/bucket/object_key/chunk_index/chunk_total` 等协议字段组成。

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
- **任务状态**：[ ] 未开始
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

- 中控表：`disk_list`、`edge_site`、`center_config`、`signature_key`、`data_key`、`import_job`、`chunk_import_group`、`chunk_import_part`、`object_ledger`。
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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

# 开发任务卡片：TASK-S1-EDGE-002

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-002
- **任务名称**：边缘磁盘检测、ext4 校验、disk_info 校验和恢复入口
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[ ] 未开始
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

# 开发任务卡片：TASK-S1-EDGE-003

### 任务基本信息

- **任务 ID**：TASK-S1-EDGE-003
- **任务名称**：RustFS 全 bucket 扫描、对象稳定性判断和快照入库
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [x] Track 2: Edge (`crates/edge-backend`)
  - [ ] Track 3: Center
  - [ ] Track 4: Web
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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
- **任务状态**：[ ] 未开始
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

# 开发任务卡片：TASK-S1-WEB-EDGE-001

### 任务基本信息

- **任务 ID**：TASK-S1-WEB-EDGE-001
- **任务名称**：边缘端 DashboardView、HTTP 汇总和 WS 进度展示
- **所属 Track / 模块**：
  - [ ] Track 1: Common
  - [ ] Track 2: Edge
  - [ ] Track 3: Center
  - [x] Track 4: Web (`web/edge-web`)
- **任务状态**：[ ] 未开始
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

- [ ] 展示扫描状态、盘列表、拷贝进度、当前对象、速度、错误。
- [ ] WS 断开和页面刷新能恢复 HTTP 汇总状态。
- [ ] 非 ext4、未注册、拔盘、空间不足和恢复要求均有展示。

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
- **任务状态**：[ ] 未开始
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

- [ ] 展示注册/初始化状态、待导入盘、导入进度、当前对象、速度、错误、是否可复用。
- [ ] 重复插入已导入硬盘不会显示为待导入。
- [ ] `REINIT_FAILED` 有明确展示。

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
- **任务状态**：[ ] 未开始
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

# 开发任务卡片：TASK-S1-INTEGRATION-001

### 任务基本信息

- **任务 ID**：TASK-S1-INTEGRATION-001
- **任务名称**：主闭环联调、验收、风险登记和交付检查
- **所属 Track / 模块**：
  - [x] Track 1: Common
  - [x] Track 2: Edge
  - [x] Track 3: Center
  - [x] Track 4: Web / Deploy / Tests
- **任务状态**：[ ] 未开始
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
- [ ] 前端构建或类型检查通过。
- [ ] 迁移脚本可在空库执行。
- [ ] 主闭环联调记录写入当天 dev log。
- [ ] 未完成项已回写对应任务卡和 dev log。
