# Edge Dashboard 实时快照 JSONC 字段契约

本文档用于约束 Edge Dashboard 的首次 HTTP 查询和 WebSocket 实时推送结构：

- `GET /api/edge/dashboard/summary`
- `WS /ws/edge/progress`

两者必须返回/推送同一套快照结构。`jsonc` 仅用于表达字段注释，实际接口返回仍为标准 JSON。

```jsonc
{
  "source": "edge",                  // 数据来源，固定为 edge，表示该快照由 Edge 后端生成。
  "edge_code": "edge-demo",          // 当前 Edge 节点编码。
  "edge_name": "edge-demo",          // 当前 Edge 节点展示名称。

  "object_inventory": {              // RustFS 源对象库存概览，来自 Edge 本地扫描结果或数据库快照，不等同于当前导出任务进度。
    "total_bytes": 2122,             // 当前扫描范围内源对象总字节数，可由 local_object_snapshot 聚合。
    "exported_bytes": 11,            // 已被 Edge 导出记录覆盖的对象字节数，可由 export_object 聚合。
    "total_count": 211,              // 当前扫描范围内源对象总数量。
    "exported_count": 12             // 已被 Edge 导出记录覆盖的对象数量。
  },

  "export_job": {                    // 当前或最近导出任务总览；没有导出任务时可为 null。
    "export_job_id": "job-uuid",     // 导出任务 ID。
    "export_job_status": "COPYING",  // 导出任务状态：PENDING、SCANNING、COPYING、SEALING、SEALED、FAILED、CANCELLED。
    "start_time": "2026-08-12T12:26:20Z", // 导出任务开始时间，UTC。
    "finish_time": null,             // 导出任务完成时间；未完成时为 null。
    "total_bytes": 1428414961,       // 本导出任务计划导出的总字节数。
    "done_bytes": 102400,            // 本导出任务已经写入运输盘的字节数。
    "remaining_bytes": 1428312561,   // 本导出任务剩余字节数。
    "speed_bytes_per_sec": 10485760, // 本导出任务当前聚合传输速度，单位 bytes/s。
    "object_total": 59,              // 本导出任务计划导出的对象数量。
    "object_done": 12,               // 本导出任务已完成导出的对象数量。
    "object_remaining": 47           // 本导出任务剩余对象数量。
  },

  "disks": [                         // 当前 Edge 端仍在位、需要展示给用户感知的磁盘视图；HTTP 和 WS 必须使用同一字段结构。
    {
      "disk_id": "disk-uuid",        // 运输盘协议 ID，来自 disk_info.json；未初始化盘可能为空或使用后端临时展示 ID，但不得作为业务主身份。
      "disk_sn": "disk-sn",          // 硬盘序列号或后端可获得的硬件标识；不能单独作为运输盘业务主身份。
      "hardware_serial": "disk-sn",  // 硬件序列号原始值或规范化值。
      "stable_hardware_id": "usb-xxx-part1", // 后端组合出的稳定硬件标识，可由 device_path、mount_path、fs_uuid、id_serial、label 等组成，用于未初始化盘去重展示。

      "device_path": "/dev/sdb1",    // 当前在 Edge VM 上识别到的设备路径。
      "mount_path": "/media/edge/RFS-ZERO-FRESH", // 当前挂载路径。
      "filesystem_type": "ext4",     // 文件系统类型；运输盘要求 ext4。
      "fs_uuid": "b4f0-xxxx",        // 文件系统 UUID，可用于去重和稳定识别。

      "capacity_bytes": 983349043200,     // 磁盘总容量字节数。
      "free_bytes": 931893178368,         // 磁盘当前可用空间字节数。
      "object_budget_bytes": 923236134912, // 后端计算出的可用于对象导出的预算容量。

      "disk_status_code": "EDGE_COPYING", // 运输盘生命周期状态，来自 disk_info.json.status.code；卡片主状态必须优先使用该字段。
                                          // 可选值：UNREGISTERED、REGISTERED、INITIALIZED、EDGE_COPYING、SEALED、CENTER_IMPORTING、IMPORTED、ERROR。
      "runtime_status": "COPYING",        // 当前本机运行态，来自 Edge 运行检测或任务过程；只作为副状态、进度或准入原因，不得覆盖 disk_status_code。
                                          // 可选值：DETECTED、CHECKING、READY、COPYING、CLEANING、REINITIALIZING、DONE、REJECTED、REMOVED、ERROR。
      "task_pool_eligible": true,          // 当前是否可进入 Edge 导出任务池；通常要求 disk_status_code=INITIALIZED 且 runtime_status=READY。

      "progress": {                   // 单盘实时传输进度；没有运行任务时保留 0 值或后端最近快照。
        "total_bytes": 800000000,     // 分配到该盘的总字节数。
        "done_bytes": 120000000,      // 该盘已完成写入字节数。
        "remaining_bytes": 680000000, // 该盘剩余写入字节数。
        "speed_bytes_per_sec": 5242880, // 该盘当前写入速度，单位 bytes/s。
        "object_total": 30,           // 分配到该盘的对象数量。
        "object_done": 5,             // 该盘已完成导出的对象数量。
        "object_remaining": 25,       // 该盘剩余对象数量。
        "percent": 15.0               // 该盘完成百分比；可由后端提供，也可由前端按 done_bytes/total_bytes 计算。
      },

      "current_object": {             // 该盘当前正在传输的对象；没有正在传输对象时为 null。
        "bucket": "photos",           // 源 RustFS bucket。
        "key": "2026/08/a.jpg",       // 源对象 key。
        "display_name": "a.jpg",      // 前端展示文件名。
        "relative_data_path": "data/photos/2026/08/a.jpg.enc", // 写入运输盘内的数据相对路径；不得包含敏感密钥。
        "size_bytes": 104857600,      // 当前对象总大小。
        "done_bytes": 52428800,       // 当前对象已写入字节数。
        "remaining_bytes": 52428800,  // 当前对象剩余字节数。
        "speed_bytes_per_sec": 5242880, // 当前对象写入速度，单位 bytes/s。
        "object_status": "COPYING"    // Edge 本地对象导出状态；不得出现 IMPORTED。
      },

      "last_error_code": null,        // 最近一次盘级错误码；无错误时为 null。
      "error_message": null,          // 最近一次盘级错误说明；不得包含密钥、token、完整连接串。
      "message": "正在拷贝"            // 面向前端的简短状态说明。
    }
  ],

  "ws_connected": false,              // HTTP 返回时默认为 false；浏览器实际 WebSocket 连接状态由前端运行时更新。
  "last_http_refresh_at": "2026-08-12T12:26:20Z", // 后端生成 summary 的时间，UTC。
  "message": "edge dashboard summary" // 后端对本次快照的说明。
}
```

## 展示优先级

```text
卡片主状态 = disks[].disk_status_code
卡片副状态 = disks[].runtime_status
单盘进度 = disks[].progress
当前传输文件 = disks[].current_object
```

`runtime_status` 不得覆盖 `disk_status_code`。例如：

- `disk_status_code=IMPORTED` 且 `runtime_status=REJECTED`：主状态显示“已导入”，副状态显示“当前不可用于 Edge 导出，请在 Center 重新初始化”。
- `disk_status_code=SEALED` 且 `runtime_status=REJECTED`：主状态显示“已封盘”，副状态显示“可拔盘送 Center 导入，不可继续导出”。
- 无有效 `disk_status_code` 且缺少 `disk_info.json`：才显示“未初始化/缺少运输协议文件”。

