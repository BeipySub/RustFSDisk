# Edge WebSocket v2 实时通讯协议

本文是 Edge 前后端实时通讯补充协议，用于替代当前“每秒推送 Dashboard 快照”的做法。冻结文档仍是 v1.0 基线；本文只描述当前联调后确认的 v2 调整方案，后续实现按本文执行。

## 目标

Edge WebSocket 只按事件推送，不再空闲时每秒推空的业务快照。

前端展示只关心三类业务变化：

```text
插盘
拔盘
拷贝进度
```

其中“拷贝进度”覆盖完整导出流水线：

```text
扫描 RustFS
-> 按硬盘容量分配任务
-> 多盘并行拷贝
-> 封盘
-> 完成或失败
```

## 通讯边界

HTTP 只负责完整状态快照：

```text
页面首次打开
手动刷新
WebSocket 断联兜底
必要的统计校准
```

WebSocket 只负责实时事件：

```text
硬盘插入或状态变化
硬盘拔出
导出流水线进度
```

前端不通过 WebSocket 发业务命令。前端触发扫描、刷新、恢复等命令仍走 HTTP API。

## 推送原则

后端必须按事件推送：

```text
插盘/盘状态变化 -> 立即推送 DISK_PLUGGED
拔盘 -> 立即推送 DISK_UNPLUGGED
扫描开始/完成 -> 立即推送 COPY_PROGRESS
扫描进度 -> 可按 1 秒节流推送 COPY_PROGRESS
任务分配开始/完成 -> 立即推送 COPY_PROGRESS
拷贝开始 -> 立即推送 COPY_PROGRESS
拷贝进度 -> 按 1 秒节流推送 COPY_PROGRESS
封盘开始/完成 -> 立即推送 COPY_PROGRESS
失败 -> 立即推送 COPY_PROGRESS
空闲 -> 不推业务消息
```

后端不得再发送空闲 `COPY_PROGRESS` 去表达“当前没有任务”。

## 统一消息结构

所有 Edge WS v2 消息使用统一外壳：

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "7e9c1d2e-4bd8-4f32-92f6-2e544c22a7a1",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:20:10Z",
  "stage": "COPYING",
  "message": "正在拷贝",
  "scan": null,
  "export_job": null,
  "global_progress": null,
  "disks": []
}
```

字段说明：

| 字段 | 必填 | 说明 |
|---|---|---|
| `protocol_version` | 是 | 固定 `edge-ws-v2`。 |
| `source` | 是 | 固定 `edge`。 |
| `edge_code` | 是 | Edge 站点编码。 |
| `event_id` | 是 | 单条事件 ID，用于前端去重和排查。 |
| `event_type` | 是 | 只允许 `DISK_PLUGGED`、`DISK_UNPLUGGED`、`COPY_PROGRESS`。 |
| `event_time` | 是 | 事件发生时间，ISO8601 UTC。 |
| `stage` | 否 | `COPY_PROGRESS` 的阶段；插拔盘事件为空。 |
| `message` | 否 | 前端可展示的人类可读提示。 |
| `scan` | 否 | RustFS 扫描状态。 |
| `export_job` | 否 | 导出任务整体状态。 |
| `global_progress` | 否 | 当前导出批次全局总进度。 |
| `disks` | 否 | 多盘状态数组，前端按 `disk_id` 合并。 |

## 事件类型

v2 只保留三种事件：

| `event_type` | 含义 | 前端更新 |
|---|---|---|
| `DISK_PLUGGED` | 插盘、检测中、可用、拒绝等盘状态变化。 | 只更新 `disks[]`。 |
| `DISK_UNPLUGGED` | 运输盘拔出。 | 只更新对应 `disk_id` 的盘为 `REMOVED`。 |
| `COPY_PROGRESS` | 扫描、分配、拷贝、封盘、失败全过程。 | 按 `stage` 更新扫描、任务、全局进度和每盘进度。 |

## 阶段类型

`COPY_PROGRESS.stage` 允许：

| `stage` | 含义 | 前端展示 |
|---|---|---|
| `SCANNING_RUSTFS` | 正在扫描 Edge RustFS。 | 全局显示正在扫描，更新对象统计。 |
| `PLANNING` | 正在按硬盘容量分配对象任务。 | 全局显示正在分配，每盘可展示分配量。 |
| `COPYING` | 正在拷贝、加密、写盘。 | 展示全局进度、每盘进度、当前对象。 |
| `SEALING` | 正在写 manifest 并封盘。 | 展示封盘中。 |
| `SEALED` | 封盘完成。 | 展示已封盘、可拔盘。 |
| `FAILED` | 导出失败。 | 展示失败原因和失败盘。 |

## 磁盘状态结构

`disks[]` 中每块盘必须以 `disk_id` 作为业务主身份。未读取到 `disk_id` 的预校验阶段，可临时使用 `stable_hardware_id` 和 `mount_path` 辅助展示，但进入可用、拷贝、封盘流程前必须有 `disk_id`。

```json
{
  "disk_id": "25eb1e1a-2824-4d6d-914f-cbdc10c3da8a",
  "disk_sn": "SN-A",
  "stable_hardware_id": "fs-uuid-or-serial",
  "device_path": "/dev/sdb1",
  "mount_path": "/media/edge/RFS-A",
  "filesystem": "ext4",
  "disk_status_code": "INITIALIZED",
  "runtime_status": "READY",
  "total_bytes": 983349043200,
  "free_bytes": 933322080256,
  "object_budget_bytes": 924665036800,
  "assigned_object_count": 0,
  "done_object_count": 0,
  "done_bytes": 0,
  "remaining_bytes": 0,
  "speed_bytes_per_sec": 0,
  "current_object": null,
  "last_error_code": null,
  "error_message": null,
  "message": "硬盘可用"
}
```

字段命名必须继续遵守 v1.0 规则：

```text
盘内生命周期：disk_status_code
运行态：runtime_status
导出任务态：export_job_status
对象态：object_status
```

不得使用裸 `status`。

## 插盘事件

单盘或多盘插入、检测中、可用、拒绝，统一推 `DISK_PLUGGED`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-001",
  "event_type": "DISK_PLUGGED",
  "event_time": "2026-08-13T06:19:37Z",
  "stage": null,
  "message": "检测到 2 块运输盘",
  "disks": [
    {
      "disk_id": "disk-a",
      "disk_sn": "SN-A",
      "mount_path": "/media/edge/RFS-A",
      "disk_status_code": "INITIALIZED",
      "runtime_status": "READY",
      "total_bytes": 4000,
      "free_bytes": 3900,
      "object_budget_bytes": 3600,
      "message": "硬盘可用"
    },
    {
      "disk_id": "disk-b",
      "disk_sn": "SN-B",
      "mount_path": "/media/edge/RFS-B",
      "disk_status_code": "INITIALIZED",
      "runtime_status": "READY",
      "total_bytes": 5000,
      "free_bytes": 4900,
      "object_budget_bytes": 4600,
      "message": "硬盘可用"
    }
  ]
}
```

前端处理：

```text
遍历 disks[]
按 disk_id 新增或更新硬盘卡片
不修改 export_job
不修改 global_progress
```

## 拔盘事件

拔出单块盘时推 `DISK_UNPLUGGED`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-002",
  "event_type": "DISK_UNPLUGGED",
  "event_time": "2026-08-13T06:25:00Z",
  "stage": null,
  "message": "运输盘已拔出",
  "disks": [
    {
      "disk_id": "disk-b",
      "runtime_status": "REMOVED",
      "last_error_code": "DISK_REMOVED",
      "message": "硬盘已拔出"
    }
  ]
}
```

前端处理：

```text
只更新 disk-b
其他盘不变
export_job 是否失败，以后端后续 COPY_PROGRESS stage=FAILED 为准
```

## 扫描阶段

扫描 RustFS 时推 `COPY_PROGRESS`，`stage = SCANNING_RUSTFS`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-003",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:20:00Z",
  "stage": "SCANNING_RUSTFS",
  "message": "正在扫描 RustFS",
  "scan": {
    "scan_run_id": "scan-001",
    "scan_status": "SCANNING",
    "bucket_count": 2,
    "object_seen": 80,
    "stable_object_count": 70,
    "source_changed_count": 0,
    "total_bytes": 8000
  }
}
```

前端处理：

```text
全局状态显示正在扫描
更新对象数量和容量
不修改硬盘拷贝状态
```

## 分配阶段

根据硬盘容量分配对象时推 `COPY_PROGRESS`，`stage = PLANNING`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-004",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:20:05Z",
  "stage": "PLANNING",
  "message": "正在根据硬盘容量分配任务",
  "export_job": {
    "export_job_id": "job-001",
    "export_job_status": "PENDING",
    "object_count": 120,
    "copied_count": 0,
    "total_bytes": 9000,
    "copied_bytes": 0
  },
  "disks": [
    {
      "disk_id": "disk-a",
      "runtime_status": "READY",
      "assigned_object_count": 50,
      "assigned_bytes": 4000
    },
    {
      "disk_id": "disk-b",
      "runtime_status": "READY",
      "assigned_object_count": 70,
      "assigned_bytes": 5000
    }
  ]
}
```

前端处理：

```text
全局状态显示正在分配任务
每块盘显示已分配对象数和字节数
```

## 拷贝阶段

拷贝中推 `COPY_PROGRESS`，`stage = COPYING`。该事件可按 1 秒节流推送。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-005",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:21:10Z",
  "stage": "COPYING",
  "message": "正在拷贝",
  "export_job": {
    "export_job_id": "job-001",
    "export_job_status": "COPYING",
    "object_count": 120,
    "copied_count": 45,
    "total_bytes": 9000,
    "copied_bytes": 3500
  },
  "global_progress": {
    "total_bytes": 9000,
    "done_bytes": 3500,
    "remaining_bytes": 5500,
    "speed_bytes_per_sec": 10485760,
    "object_total": 120,
    "object_done": 45,
    "object_remaining": 75
  },
  "disks": [
    {
      "disk_id": "disk-a",
      "runtime_status": "COPYING",
      "disk_status_code": "EDGE_COPYING",
      "assigned_object_count": 50,
      "done_object_count": 25,
      "total_bytes": 4000,
      "done_bytes": 2000,
      "remaining_bytes": 2000,
      "speed_bytes_per_sec": 5242880,
      "current_object": {
        "bucket": "bucket-a",
        "key": "a.bin",
        "display_name": "a.bin",
        "size_bytes": 1000,
        "done_bytes": 800,
        "remaining_bytes": 200,
        "object_status": "COPYING"
      }
    }
  ]
}
```

前端处理：

```text
全局区域显示 global_progress
硬盘卡片显示各自 progress
点击某块盘，只展示该盘 current_object
```

## 封盘阶段

封盘中推 `COPY_PROGRESS`，`stage = SEALING`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-006",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:39:00Z",
  "stage": "SEALING",
  "message": "正在写 manifest 并封盘",
  "export_job": {
    "export_job_id": "job-001",
    "export_job_status": "COPYING"
  },
  "disks": [
    {
      "disk_id": "disk-a",
      "runtime_status": "DONE",
      "message": "正在封盘"
    }
  ]
}
```

封盘完成推 `COPY_PROGRESS`，`stage = SEALED`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-007",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:40:00Z",
  "stage": "SEALED",
  "message": "已封盘，可拔盘",
  "export_job": {
    "export_job_id": "job-001",
    "export_job_status": "SEALED",
    "object_count": 120,
    "copied_count": 120,
    "total_bytes": 9000,
    "copied_bytes": 9000
  },
  "global_progress": {
    "total_bytes": 9000,
    "done_bytes": 9000,
    "remaining_bytes": 0,
    "speed_bytes_per_sec": 0,
    "object_total": 120,
    "object_done": 120,
    "object_remaining": 0
  },
  "disks": [
    {
      "disk_id": "disk-a",
      "disk_status_code": "SEALED",
      "runtime_status": "DONE",
      "message": "已封盘，可拔盘"
    }
  ]
}
```

前端处理：

```text
全局显示已封盘
每块完成盘显示可拔盘
```

## 失败阶段

失败统一推 `COPY_PROGRESS`，`stage = FAILED`。

```json
{
  "protocol_version": "edge-ws-v2",
  "source": "edge",
  "edge_code": "edge-demo",
  "event_id": "event-008",
  "event_type": "COPY_PROGRESS",
  "event_time": "2026-08-13T06:21:30Z",
  "stage": "FAILED",
  "message": "导出失败",
  "export_job": {
    "export_job_id": "job-001",
    "export_job_status": "FAILED",
    "error_code": "WRITE_BEFORE_PERMISSION_DENIED",
    "error_message": "one or more DiskWorker instances failed"
  },
  "disks": [
    {
      "disk_id": "disk-b",
      "runtime_status": "ERROR",
      "last_error_code": "MANIFEST_INVALID",
      "error_message": "Permission denied while creating manifest directory"
    }
  ]
}
```

前端处理：

```text
全局显示失败
失败盘显示错误原因
未失败盘保持自己的最后状态
```

## 前端合并规则

前端收到 WS 事件后，只按事件更新局部状态：

```ts
function onEdgeWsV2Event(event: EdgeWsV2Event) {
  if (event.protocol_version !== "edge-ws-v2") return;

  wsConnected.value = true;
  wsMessage.value = event.message ?? "";

  if (event.event_type === "DISK_PLUGGED") {
    mergeDisksByDiskId(event.disks);
    return;
  }

  if (event.event_type === "DISK_UNPLUGGED") {
    mergeDisksByDiskId(event.disks);
    return;
  }

  if (event.event_type === "COPY_PROGRESS") {
    currentStage.value = event.stage;
    if (event.scan) updateScan(event.scan);
    if (event.export_job) updateExportJob(event.export_job);
    if (event.global_progress) updateGlobalProgress(event.global_progress);
    mergeDisksByDiskId(event.disks);
  }
}
```

多盘合并必须按 `disk_id`：

```ts
function mergeDisksByDiskId(nextDisks: EdgeDiskState[] = []) {
  for (const next of nextDisks) {
    const index = state.disks.findIndex((disk) => disk.disk_id === next.disk_id);
    if (index >= 0) {
      state.disks[index] = { ...state.disks[index], ...next };
    } else {
      state.disks.push(next);
    }
  }
}
```

点击硬盘详情：

```ts
const selectedDisk = computed(() =>
  state.disks.find((disk) => disk.disk_id === selectedDiskId.value) ?? null
);
```

## 页面展示规则

前端页面分三块：

```text
全局导出区：看 stage、export_job、global_progress
多盘卡片区：看 disks[] 中每块盘自己的状态和进度
选中盘详情区：看 selectedDiskId 对应的那块盘
```

状态展示：

| 条件 | 前端显示 |
|---|---|
| `event_type = DISK_PLUGGED` 且 `runtime_status = READY` | 硬盘可用。 |
| `event_type = DISK_UNPLUGGED` | 硬盘已拔出。 |
| `stage = SCANNING_RUSTFS` | 正在扫描 RustFS。 |
| `stage = PLANNING` | 正在分配任务。 |
| `stage = COPYING` | 拷贝中，展示进度。 |
| `stage = SEALING` | 封盘中。 |
| `stage = SEALED` | 已封盘，可拔盘。 |
| `stage = FAILED` | 导出失败，展示错误。 |

## 后端改造范围

需要改造：

```text
crates/edge-backend/src/realtime.rs
crates/edge-backend/src/server.rs
crates/edge-backend/src/disk_detection.rs
crates/edge-backend/src/scanner/progress.rs
crates/edge-backend/src/control.rs
crates/edge-backend/src/export_runtime.rs
crates/edge-backend/src/disk_worker.rs
web/edge-web/src/ws/edgeCopyProgress.ts
web/edge-web/src/views/DashboardView.vue
web/edge-web/src/__tests__/edgeDashboard.test.ts
```

后端实现目标：

```text
EdgeRealtimeHub 提供事件广播。
WebSocket handler 订阅事件广播，有事件才发送。
拷贝/扫描进度由对应聚合器按 1 秒节流发布。
空闲时不发送业务事件。
```

前端实现目标：

```text
只接受 protocol_version = edge-ws-v2 的事件。
不再把 WS event 当完整 Dashboard summary。
所有 disks 更新按 disk_id 合并。
HTTP summary 仍负责页面首次完整基线和断联兜底。
```

## 迁移策略

本方案不做旧 WS 协议兼容。实施时要求：

```text
后端 WS payload 和前端解析同一批提交切换到 edge-ws-v2。
旧 CopyProgressEvent extends EdgeDashboardSummary 的前端模型删除或停用。
旧空闲 COPY_PROGRESS 删除。
现有 WS 相关测试整体改为 v2 事件。
冻结文档不修改；本文作为补充协议记录 v2 调整。
```

## 盘在位身份补充

`disks[]` 新增可选字段 `disk_presence_id`。它表示一次物理插入从检测到拔出的在位周期，后端在 `DETECTED`、`CHECKING`、`READY`、`COPYING`、`DONE` 和 `REMOVED` 事件中保持同一值。

```text
disk_presence_id：前端卡片和实时事件的合并身份
disk_id：运输盘业务身份；读取 disk_info.json 后才可获得
disk_runtime.id：数据库单条运行态快照的自增主键
```

前端优先按 `disk_presence_id` 合并；仅在旧事件缺少该字段时，才回退到真实 `disk_id` 或现有辅助识别规则。`disk_presence_id` 不得用于对象分配、导出任务、封盘或盘内协议身份判断。
