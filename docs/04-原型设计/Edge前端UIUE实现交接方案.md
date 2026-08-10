# Edge 前端 UI/UE 实现交接方案

本文档用于交接 Edge 前端页面的 UI/UE 设计与前端实现范围。当前版本已删除“Edge 首次接入”页面，本轮只实现 Dashboard 与同步记录两个页面。

本文档不是冻结文档，不替代 `docs/v1.0冻结/` 中的需求、协议、数据库和架构基线。若本文档与冻结文档冲突，以冻结文档为准。

## 1. 目标

根据最终产品图和素材目录，安排 UI 设计、UE 设计与前端开发工程师实现 Edge 前端页面。

核心目标：

- 先梳理并复用现有接口，避免前端重复造接口。
- 只补齐当前范围内缺失的只读接口。
- 拆分清楚页面模块、交互点、状态来源和组件边界。
- 以最终产品图做 16:9 桌面大屏主版本 100% 视觉还原。
- 严格复用指定素材目录，不换风格，不使用占位图。

## 2. 页面范围

本轮只保留两个页面：

| 页面 | 路由建议 | 说明 |
|---|---|---|
| Edge Dashboard 首页 | `/dashboard` | 主控制台，展示源服务器、运输盘位、导出进度、异常汇总、选中盘详情和当前对象。 |
| 同步记录 / 导出记录页 | `/sync-records` | 展示 Edge 本地导出历史、筛选、任务表格和详情抽屉。 |

已删除并排除：

- 不做 `Edge 首次接入` 页面。
- 不做 `first-access` 路由。
- 不做中控端 URL 输入。
- 不做授权 Key 输入。
- 不做注册包、接入向导、站点身份确认、bootstrap 接口和相关交互。
- 不做设置页。
- 不做源端状态 Tab。
- 不做连接状态 / 诊断 Tab。
- 不拆独立运输盘详情页，Dashboard 已包含选中盘详情。
- 不拆独立异常恢复页，Dashboard 已包含异常提示和恢复检查入口。

## 3. 视觉基准与素材目录

最终产品图是桌面 16:9 主版本的视觉还原基准：

- `C:/Users/Beipy/AppData/Local/Temp/codex-clipboard-99c9e886-e501-40b6-9173-fa0b7f2547c8.png`：Edge Dashboard。
- `C:/Users/Beipy/AppData/Local/Temp/codex-clipboard-041d3a8b-2692-40fb-a36c-34acbb591a3f.png`：同步记录。

已删除页面的参考图不再作为实现范围：

- `C:/Users/Beipy/AppData/Local/Temp/codex-clipboard-cbc95c13-0534-417c-b58c-dcb11f406379.png`：Edge 首次接入，仅作废弃参考，不进入实现文档。

指定素材目录：

```text
F:/Project/RustFSDisk/docs/原型参考/web/apps/web-antd/public/assets/fustfs-baseline
```

前端必须优先复用该目录中的设备、背景、光效和状态素材。若素材缺失，必须先标注缺口并反馈，不允许擅自替换为通用插画、占位图或其他视觉风格。

可用素材映射：

| 素材 | 用途 |
|---|---|
| `factory-environment-v4.webp` | 暗色工业背景，Dashboard 与记录页底层环境。 |
| `source-rack-cutout-v3.webp` | Dashboard 左侧源服务器实体。 |
| `transport-nas-cutout-v3.webp` | Dashboard 右侧运输盘位设备实体。 |
| `transport-disk-cutout-v1.png` | 单盘详情、小型磁盘辅助图。 |
| `task-database-sync-v1.png` | 同步任务辅助视觉，谨慎用于空态或说明，不得替代主设备图。 |
| `a04-failed-lock-v1.png` | 异常、安全失败、锁定状态辅助图标。 |
| `a04-failed-lock-small-v1.png` | 小尺寸异常状态图标。 |
| `a04-packed-shield-v1.png` | 封盘、安全确认状态图标。 |
| `icons/task-confidence-shield.svg` | 可信度、安全说明图标。 |
| `icons/task-confirmed-database.svg` | 数据确认图标。 |
| `icons/task-eta-clock.svg` | ETA / 时间图标。 |

素材缺口：

- 蓝色粒子数据流需要工程用 CSS 或 canvas 实现。
- RustFS 顶部品牌字效需要前端用文字样式模拟，或补正式 logo 素材。
- 盘位设备内部灯带、选中描边、卡片状态光效需要 CSS 叠加实现。
- 同步记录页右侧机房背景如现有素材不完全匹配，可从 `factory-environment-v4.webp` 裁切、暗化、叠加蒙层处理。

## 4. 现有代码与数据层

当前 Edge 前端现状：

- `web/edge-web/src/App.vue`：直接渲染 `DashboardView`，尚无页面路由。
- `web/edge-web/src/views/DashboardView.vue`：已有 Dashboard 数据读取和 WebSocket 接入逻辑，但当前视觉未对齐目标图。
- `web/edge-web/src/api/edgeDashboard.ts`：已有 Dashboard 类型、mock、HTTP summary 请求和归一化逻辑。
- `web/edge-web/src/ws/edgeCopyProgress.ts`：已有 `/ws/edge/progress` 连接与 `COPY_PROGRESS` 归并逻辑。

实现原则：

- Dashboard 优先复用现有 `edgeDashboard.ts` 和 `edgeCopyProgress.ts`。
- 不新增一套与现有 summary 重复的数据模型。
- 同步记录页先盘点后端已有导出任务详情接口，再补最小列表 / 详情缺口。
- 前端不得自行创造状态枚举，状态码以冻结协议文档为准。

## 5. 已有接口优先复用

Edge 本端接口优先复用：

| 接口 | 用途 | 页面 |
|---|---|---|
| `GET /api/edge/summary` | Dashboard 首屏 HTTP 汇总。 | Dashboard |
| `GET /ws/edge/progress` 或 `/ws/edge/copy-progress` | Edge 本端实时进度推送。 | Dashboard |
| `POST /api/edge/scan` | 触发扫描。 | Dashboard |
| `POST /api/edge/export-jobs` | 创建导出任务。 | Dashboard |
| `GET /api/edge/export-jobs/{export_job_id}` | 查询单个导出任务详情。 | 同步记录 / 详情抽屉 |
| `POST /api/edge/export-jobs/{export_job_id}/start` | 启动导出任务。 | Dashboard |
| `POST /api/edge/export-jobs/{export_job_id}/recover` | 恢复检查后继续导出。 | Dashboard |

接口注意事项：

- `GET /api/edge/summary` 当前可能需要 `X-Edge-Control-Token`。若浏览器前端无法安全携带该 token，应由后端提供只读同源 Dashboard summary 或服务端注入机制，不允许前端硬编码敏感 token。
- WebSocket 只用于 Edge 后端向 Edge 前端推送本端状态，不用于 Edge 与 Center 数据同步。

## 6. 明确接口缺口

本轮仅保留同步记录相关缺口：

### 6.1 导出记录列表接口

当前单任务详情接口不足以支撑记录页，需要补只读列表接口。

建议能力：

- 分页。
- 最近 30 天 / 时间范围筛选。
- `export_job_status` 筛选。
- 批次号 / 导出任务 ID 搜索。
- 返回统计计数：全部、进行中、已封盘、失败。

数据来源优先复用：

- 边缘端 `export_job`。
- 边缘端 `export_object`。
- 必要时关联 `disk_runtime` 或导出任务盘分配记录。

### 6.2 导出记录详情接口

详情抽屉需要只读详情数据：

- 导出任务 ID。
- 导出批次。
- 开始时间。
- 结束时间。
- 总对象数。
- 已导出对象数。
- 跳过对象数。
- 失败对象数。
- 参与运输盘列表。
- 每块运输盘的容量、进度、状态。
- 错误码 / 失败原因。
- manifest 信息。
- seal 信息。

不需要的缺口：

- 不需要首次接入接口。
- 不需要 bootstrap 接口。
- 不需要提交 `center_url`。
- 不需要提交 `authorization_key`。

## 7. 状态字段约束

API、WebSocket、前端类型和组件 props 不得使用裸 `status` 表达多种含义。

必须显式区分：

| 语义 | 字段 |
|---|---|
| 运输盘生命周期 | `disk_status_code` 或 `status_code` |
| 运输盘启用状态 | `disk_enabled` 或 `enabled` |
| 边缘站点状态 | `edge_status` |
| 运输盘运行态 | `runtime_status` |
| 导出任务状态 | `export_job_status` |
| 导入任务状态 | `import_job_status` |
| 对象状态 | `object_status` |

Edge 前端不得展示或写入：

- `IMPORTED`。
- Center 导入结果。
- Center 清理 / 重新初始化状态作为 Edge 操作。

## 8. Dashboard 页面规格

### 8.1 页面结构

Dashboard 是主页面，不再拆出盘详情页或异常恢复页。

模块：

- 顶部状态栏。
- 源服务器视觉区。
- 蓝色数据流 / 粒子动画。
- 运输盘位矩阵。
- 选中盘详情条。
- 全局导出进度。
- 扫描与导出概览。
- 导出前置检查。
- 异常盘汇总。
- 运输盘运行状态表。
- 当前对象面板。
- 底部提示与操作区。

### 8.2 顶部状态栏

展示：

- RustFS 离线同步中心。
- Edge 工厂名称。
- HTTP 服务状态胶囊。
- WebSocket 连接状态胶囊。
- 最后心跳。
- 最后更新时间。
- 刷新图标。

视觉：

- 状态胶囊贴近参考图，小尺寸、低透明背景、细描边。
- HTTP 正常使用绿色点。
- WebSocket 已连接使用蓝色点。
- 异常使用红色或橙色点。

### 8.3 运输盘位矩阵

数据来源：

- `summary.disks[]`。
- WebSocket `COPY_PROGRESS` 中的 `disks[]`。

要求：

- 盘位数量由 `disks[]` 动态渲染，不写死 8 或 16。
- 视觉密度、卡片比例、选中描边和设备位置贴近 Dashboard 最终图。
- 没有接入的空盘位可以显示为空槽，但空槽数量不得误导为固定总盘位。

状态颜色：

| 条件 | 颜色 |
|---|---|
| `runtime_status = COPYING` | cyan / blue |
| 就绪或可参与导出 | green |
| `runtime_status = REJECTED` | orange |
| `runtime_status = ERROR` 或写入错误 | red |
| `runtime_status = REMOVED` | gray |

### 8.4 选中盘联动

前端本地维护：

- `selectedDiskId`。

交互：

```text
点击盘位
-> 更新 selectedDiskId
-> 选中盘详情条刷新
-> 当前对象面板刷新
-> 运输盘运行状态表高亮对应行
```

选中盘详情条展示：

- `disk_id`。
- `disk_sn`。
- 文件系统。
- `disk_status_code`。
- `runtime_status`。
- 容量。
- 已用容量。
- 剩余容量。
- 当前速度。

### 8.5 粒子动画

触发条件：

```ts
const showParticleStream = disks.some((disk) => disk.runtime_status === "COPYING");
```

要求：

- 有传输时显示蓝色粒子流。
- 没有传输时隐藏粒子，不显示解释性文字。
- 粒子流只表达源服务器到运输盘位的本机导出过程，不表达 Edge 与 Center 在线传输。

### 8.6 全局导出进度

数据来源：

- `summary.global_progress`。

展示：

- 总进度百分比。
- 已完成容量。
- 总容量。
- 剩余容量。
- 实时速度。
- 文件数。
- 对象数。
- 批次。
- 开始时间。
- 预计完成。

### 8.7 扫描与导出概览

数据来源：

- `summary.scan`。
- `summary.global_progress`。

展示：

- 扫描完成度。
- 已发现对象。
- 已导出对象。
- 预计完成时间。

### 8.8 导出前置检查

展示 Edge 允许展示的只读检查项：

- 源端扫描完成。
- 运输盘已注册。
- 加密写入可用。
- 校验链路正常。
- 断点续传可用。
- 封盘前检查通过。

注意：

- 不出现初始化运输盘。
- 不出现清理运输盘。
- 不出现导入运输盘。

### 8.9 异常盘汇总

数据来源：

- `disks[]` 前端聚合。
- `last_error_code`。
- `runtime_status`。

展示：

- 需恢复。
- 已移除。
- 被拒绝。
- 错误。
- 其他告警。

异常映射示例：

| 异常 | 说明 |
|---|---|
| `RECOVERY_REQUIRED` | 需要恢复检查。 |
| `DISK_REMOVED` | 运输盘已移除。 |
| `FILESYSTEM_UNSUPPORTED` | 文件系统不支持，非 ext4 拒绝进入任务池。 |
| `INSUFFICIENT_SPACE` | 剩余空间不足。 |

### 8.10 当前对象面板

优先展示选中盘的 `current_object`。

字段：

- 对象路径。
- `object_status`。
- 剩余大小。
- 传输速度。
- 目标盘。
- 对象标识。
- 加密状态。
- 写入阶段。
- 校验状态。
- 当前进度。

## 9. 同步记录页面规格

### 9.1 页面结构

模块：

- 顶部状态栏。
- 标题行。
- 返回 Dashboard 按钮。
- 状态统计。
- 筛选区。
- 导出任务表格。
- 详情抽屉。
- 底部提示。

标题与返回按钮要求：

- 返回 Dashboard 按钮与“同步记录 / 导出记录”标题在同一行。
- 不使用左侧大图标。
- 页面右侧背景可保留暗色机房/工业空间氛围，但不得喧宾夺主。

### 9.2 状态统计

统计卡：

- 全部。
- 进行中。
- 已封盘。
- 失败。

不做：

- 不做已取消主统计卡。
- 不提供人工取消入口。

如果历史数据中存在 `CANCELLED`，产品建议：

- 不作为主状态展示。
- 可归入历史遗留或异常类记录。
- 不提供任何取消操作按钮。

### 9.3 筛选区

筛选：

- 最近 30 天。
- 状态筛选。
- 批次号 / 导出任务 ID 搜索。

状态筛选只允许冻结协议和现有后端支持的 `export_job_status`。

### 9.4 记录表格

列：

- 时间。
- 导出批次。
- 导出任务状态。
- 数据量。
- 对象数。
- 运输盘。
- 结果。
- 操作。

操作：

- 查看详情。

禁止：

- 取消。
- 清理。
- 初始化。
- 导入。
- 重新初始化。

### 9.5 详情抽屉

展示：

- 导出任务 ID。
- 开始时间。
- 结束时间。
- 总对象数。
- 已导出对象。
- 跳过对象。
- 失败对象。
- 参与运输盘列表。
- 错误码 / 失败原因。
- manifest 信息。
- seal 信息。
- 本机历史独立保存提示。

说明文案：

- 同步记录仅保存在当前 Edge 端。
- Edge 不写入中控导入结果。
- 重新部署或清理本地数据可能导致历史缺失。

## 10. 公共组件拆分建议

建议前端拆分：

| 组件 | 用途 |
|---|---|
| `EdgeShell` | 暗色工业背景、页面基础布局。 |
| `TopStatusBar` | 顶部品牌、HTTP、WebSocket、心跳、更新时间。 |
| `GlassPanel` | 毛玻璃面板容器。 |
| `StatusBadge` | 状态点、状态胶囊、颜色映射。 |
| `ProgressBar` | 全局、盘级、对象级进度条。 |
| `DiskSlotMatrix` | 运输盘位矩阵容器。 |
| `DiskSlotCard` | 单个盘位卡。 |
| `ParticleStream` | 蓝色粒子动画。 |
| `MetricStrip` | 指标组。 |
| `WarningSummary` | 异常盘汇总。 |
| `CurrentObjectPanel` | 当前对象面板。 |
| `ExportRecordTable` | 同步记录表格。 |
| `ExportRecordDrawer` | 导出任务详情抽屉。 |

不再需要：

- `FirstAccessForm`。
- 注册向导组件。
- 中控 URL / 授权 Key 表单组件。

## 11. 视觉还原验收

必须贴近最终产品图：

- 暗色工业背景。
- 源服务器和运输盘设备实体的位置、比例、明暗。
- 蓝色数据流和粒子方向。
- 毛玻璃面板透明度、边框、阴影。
- cyan 高亮。
- 橙色警告。
- 红色异常。
- 顶部状态胶囊。
- Dashboard 底部操作区。
- 同步记录表格与详情抽屉。

布局要求：

- 先保证 16:9 桌面大屏主版本。
- 主版本不得被移动端适配破坏。
- 响应式只做降级：面板堆叠、表格横向滚动、设备区按比例缩放。
- 所有文字不得重叠、溢出或遮挡关键设备视觉。

## 12. 禁止项

Edge 前端和后端都不得提供：

- 初始化运输盘。
- 清理运输盘。
- 格式化运输盘。
- 重新初始化运输盘。
- 导入运输盘数据。
- 人工取消导出。
- 写入 Center 导入状态。
- 展示或写入 `IMPORTED`。
- 删除、覆盖或修改 RustFS 源对象。

## 13. 建议实现顺序

1. 系统架构师确认当前 `GET /api/edge/summary`、WebSocket payload 和导出任务详情接口字段。
2. 前端建立最小路由：`/dashboard`、`/sync-records`。
3. Dashboard 复用 `edgeDashboard.ts` 和 `edgeCopyProgress.ts`，先恢复正确中文文案并接现有 summary。
4. UI 先实现 `EdgeShell`、`TopStatusBar`、`GlassPanel`、`StatusBadge`、`ProgressBar`。
5. 实现 Dashboard 设备主视觉、盘位矩阵、选中盘联动和粒子动画。
6. 实现 Dashboard 下半区指标、异常汇总、运行状态表、当前对象和底部操作。
7. 后端或接口负责人补导出记录只读列表 / 详情接口。
8. 前端实现同步记录页面、表格筛选和详情抽屉。
9. 用两张最终产品图逐屏验收视觉还原。
10. 运行 `web/edge-web/` 的类型检查和构建。

## 14. 待确认事项

- `GET /api/edge/summary` 是否可以提供浏览器安全访问方式，避免前端暴露控制 token。
- 导出记录列表接口的最终路径、分页参数和响应结构。
- 导出记录详情是否复用 `GET /api/edge/export-jobs/{export_job_id}`，或新增只读详情聚合接口。
- 素材目录当前如被迁移或删除，需要先恢复到可被 `web/edge-web` 构建引用的位置。
