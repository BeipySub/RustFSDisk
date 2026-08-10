export type DiskStatusCode =
  | "UNREGISTERED"
  | "REGISTERED"
  | "INITIALIZED"
  | "EDGE_COPYING"
  | "SEALED"
  | "CENTER_IMPORTING"
  | "IMPORTED"
  | "ERROR";

export type RuntimeStatus =
  | "DETECTED"
  | "CHECKING"
  | "READY"
  | "COPYING"
  | "CLEANING"
  | "REINITIALIZING"
  | "DONE"
  | "REJECTED"
  | "REMOVED"
  | "ERROR";

export type ExportJobStatus =
  | "PENDING"
  | "SCANNING"
  | "COPYING"
  | "SEALED"
  | "FAILED"
  | "CANCELLED";

export type ObjectStatus =
  | "PENDING"
  | "ASSIGNED"
  | "COPYING"
  | "EXPORTED"
  | "FAILED"
  | "SOURCE_CHANGED"
  | "SKIPPED";

export type ScanEventType = "SCAN_STARTED" | "SCAN_PROGRESS" | "SCAN_DONE" | "ERROR";

export interface EdgeCurrentObject {
  bucket: string;
  key: string;
  display_name: string;
  relative_data_path: string;
  size_bytes: number;
  done_bytes: number;
  remaining_bytes: number;
  speed_bytes_per_sec: number;
  object_status: ObjectStatus;
}

export interface EdgeDiskProgress {
  disk_id: string;
  disk_sn: string;
  hardware_serial?: string;
  id_serial?: string;
  stable_hardware_id?: string;
  mount_path: string;
  device_path?: string;
  filesystem?: string;
  filesystem_uuid?: string;
  partition_uuid?: string;
  model?: string;
  vendor?: string;
  transport?: string;
  disk_status_code?: DiskStatusCode;
  runtime_status: RuntimeStatus;
  total_bytes: number;
  done_bytes: number;
  remaining_bytes: number;
  free_bytes: number;
  speed_bytes_per_sec: number;
  object_total: number;
  object_done: number;
  object_remaining: number;
  current_object: EdgeCurrentObject | null;
  last_error_code?: string;
  error_message?: string;
  message: string;
}

export interface EdgeGlobalProgress {
  total_bytes: number;
  done_bytes: number;
  remaining_bytes: number;
  speed_bytes_per_sec: number;
  object_total: number;
  object_done: number;
  object_remaining: number;
}

export interface EdgeScanSummary {
  scan_event_type: ScanEventType;
  scanned_bucket_count: number;
  scanned_object_count: number;
  scanned_bytes: number;
  stable_object_count: number;
  skipped_object_count: number;
  current_bucket: string;
  current_key: string;
  last_scan_at: string;
  message: string;
}

export interface EdgeDashboardSummary {
  source: "edge";
  edge_code: string;
  edge_name: string;
  export_job_id: string;
  export_job_status: ExportJobStatus;
  disk_status_code?: DiskStatusCode;
  scan: EdgeScanSummary;
  global_progress: EdgeGlobalProgress;
  disks: EdgeDiskProgress[];
  ws_connected: boolean;
  last_http_refresh_at: string;
  message: string;
}

interface EdgeControlScanSnapshot {
  event_type?: ScanEventType;
  event_time?: string;
  scan_phase?: string;
  bucket_total?: number;
  bucket_done?: number;
  object_seen?: number;
  stable_object_count?: number;
  source_changed_count?: number;
  total_bytes?: number;
  current_bucket?: string | null;
  current_object_key?: string | null;
  last_error_code?: string | null;
  message?: string | null;
}

interface EdgeControlExportJob {
  export_job_id: string;
  edge_code: string;
  export_job_status: ExportJobStatus;
  object_count: number;
  copied_count: number;
  total_bytes: number;
  copied_bytes: number;
  error_message?: string | null;
}

interface EdgeControlDiskRuntime {
  disk_id?: string | null;
  disk_sn?: string | null;
  hardware_serial?: string | null;
  id_serial?: string | null;
  stable_hardware_id?: string | null;
  mount_path?: string | null;
  device_path?: string | null;
  filesystem?: string | null;
  filesystem_uuid?: string | null;
  partition_uuid?: string | null;
  model?: string | null;
  vendor?: string | null;
  transport?: string | null;
  disk_status_code?: DiskStatusCode | null;
  runtime_status: RuntimeStatus;
  capacity_bytes?: number;
  free_bytes: number;
  object_budget_bytes?: number;
  last_error_code?: string | null;
  error_message?: string | null;
}

interface EdgeControlSummary {
  source: "edge";
  edge_code: string;
  edge_name?: string;
  scan?: EdgeControlScanSnapshot;
  latest_export_job?: EdgeControlExportJob | null;
  disks?: EdgeControlDiskRuntime[];
  message?: string;
}

export class DashboardHttpError extends Error {
  constructor(
    public readonly error_code: string,
    message: string,
    public readonly http_status?: number,
  ) {
    super(message);
  }
}

export const edgeDashboardMockSummary: EdgeDashboardSummary = {
  source: "edge",
  edge_code: "edge-hz-01",
  edge_name: "杭州边缘站点 01",
  export_job_id: "export-job-fixture-20260809-001",
  export_job_status: "COPYING",
  disk_status_code: "EDGE_COPYING",
  scan: {
    scan_event_type: "SCAN_DONE",
    scanned_bucket_count: 8,
    scanned_object_count: 12846,
    scanned_bytes: 1_956_421_623_808,
    stable_object_count: 12821,
    skipped_object_count: 25,
    current_bucket: "medical-archive",
    current_key: "2026/08/ct-study/index.json",
    last_scan_at: "2026-08-09T08:45:13Z",
    message: "扫描完成，25 个对象因 SOURCE_CHANGED 跳过本轮导出",
  },
  global_progress: {
    total_bytes: 1_481_036_337_152,
    done_bytes: 832_417_439_744,
    remaining_bytes: 648_618_897_408,
    speed_bytes_per_sec: 264_241_152,
    object_total: 9360,
    object_done: 5214,
    object_remaining: 4146,
  },
  disks: [
    {
      disk_id: "disk-2b601d2f",
      disk_sn: "323533383650343031383331",
      hardware_serial: "323533383650343031383331",
      id_serial: "SANDISK_ELE_323533383650343031383331-0:0",
      stable_hardware_id: "SANDISK_ELE_323533383650343031383331-0:0",
      mount_path: "/media/edge/FUSTFS-A-DRILL1",
      device_path: "/dev/sdb1",
      filesystem: "ext4",
      filesystem_uuid: "47a293a6-7b0d-487a-8f20-492fe5cc4d74",
      partition_uuid: "dae871ea-1da4-4719-bace-1e9479cc7fe4",
      model: "ELE",
      vendor: "SANDISK",
      transport: "usb",
      disk_status_code: "EDGE_COPYING",
      runtime_status: "COPYING",
      total_bytes: 658_119_753_728,
      done_bytes: 421_477_646_336,
      remaining_bytes: 236_642_107_392,
      free_bytes: 1_842_093_096_960,
      speed_bytes_per_sec: 132_112_384,
      object_total: 4120,
      object_done: 2782,
      object_remaining: 1338,
      current_object: {
        bucket: "medical-archive",
        key: "2026/08/ct-study/series-18/slice-0942.dcm",
        display_name: "slice-0942.dcm",
        relative_data_path: "data/medical-archive/2026/08/ct-study/series-18/slice-0942.dcm.enc",
        size_bytes: 734_003_200,
        done_bytes: 401_604_608,
        remaining_bytes: 332_398_592,
        speed_bytes_per_sec: 66_584_576,
        object_status: "COPYING",
      },
      message: "正在写入对象密文",
    },
    {
      disk_id: "disk-7f4cb309",
      disk_sn: "SN-RFS-EDGE-A002",
      hardware_serial: "SN-RFS-EDGE-A002",
      stable_hardware_id: "USB-DISK-SN-RFS-EDGE-A002",
      mount_path: "/mnt/rustfs-transfer/disk-b",
      device_path: "/dev/sdc1",
      filesystem: "ext4",
      filesystem_uuid: "72c50601-4da4-4d57-a98f-61fbdf83b00a",
      partition_uuid: "9d22c0d0-1898-49d7-8421-9c594df3e1cf",
      model: "PortableSSD",
      vendor: "DemoVendor",
      transport: "usb",
      disk_status_code: "EDGE_COPYING",
      runtime_status: "COPYING",
      total_bytes: 601_295_421_440,
      done_bytes: 368_050_176_000,
      remaining_bytes: 233_245_245_440,
      free_bytes: 1_612_910_845_952,
      speed_bytes_per_sec: 118_489_088,
      object_total: 3778,
      object_done: 2320,
      object_remaining: 1458,
      current_object: {
        bucket: "logs-prod",
        key: "edge-hz-01/2026/08/09/part-00384.parquet",
        display_name: "part-00384.parquet",
        relative_data_path: "data/logs-prod/edge-hz-01/2026/08/09/part-00384.parquet.enc",
        size_bytes: 1_073_741_824,
        done_bytes: 766_509_056,
        remaining_bytes: 307_232_768,
        speed_bytes_per_sec: 59_244_544,
        object_status: "COPYING",
      },
      message: "多盘并行拷贝中",
    },
    {
      disk_id: "disk-0fe891ab",
      disk_sn: "SN-RFS-EDGE-A003",
      hardware_serial: "SN-RFS-EDGE-A003",
      mount_path: "/mnt/rustfs-transfer/disk-c",
      device_path: "/dev/sdd1",
      filesystem: "ext4",
      disk_status_code: "INITIALIZED",
      runtime_status: "REJECTED",
      total_bytes: 221_621_161_984,
      done_bytes: 42_889_617_408,
      remaining_bytes: 178_731_544_576,
      free_bytes: 402_653_184,
      speed_bytes_per_sec: 0,
      object_total: 1462,
      object_done: 112,
      object_remaining: 1350,
      current_object: null,
      last_error_code: "INSUFFICIENT_SPACE",
      error_message: "对象预算容量不足，该盘停止继续分配；其他运输盘继续拷贝",
      message: "空间不足，等待人工处理或更换运输盘",
    },
    {
      disk_id: "disk-unregistered",
      disk_sn: "SN-RFS-LAB-000",
      hardware_serial: "SN-RFS-LAB-000",
      mount_path: "/mnt/rustfs-transfer/lab-fat32",
      device_path: "/dev/sde1",
      filesystem: "vfat",
      disk_status_code: "UNREGISTERED",
      runtime_status: "REJECTED",
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      free_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
      current_object: null,
      last_error_code: "FILESYSTEM_UNSUPPORTED",
      error_message: "检测到非 ext4 文件系统，拒绝进入导出任务池",
      message: "未注册或文件系统不符合协议",
    },
    {
      disk_id: "disk-recovery-required",
      disk_sn: "SN-RFS-EDGE-A004",
      hardware_serial: "SN-RFS-EDGE-A004",
      mount_path: "/mnt/rustfs-transfer/disk-d",
      device_path: "/dev/sdf1",
      filesystem: "ext4",
      disk_status_code: "EDGE_COPYING",
      runtime_status: "ERROR",
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      free_bytes: 820_338_753_536,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
      current_object: null,
      last_error_code: "RECOVERY_REQUIRED",
      error_message: "发现 EDGE_COPYING 中间态和 .partial 残留，恢复检查通过前不得继续导出",
      message: "需要恢复检查",
    },
    {
      disk_id: "disk-removed",
      disk_sn: "SN-RFS-EDGE-A005",
      hardware_serial: "SN-RFS-EDGE-A005",
      mount_path: "/mnt/rustfs-transfer/disk-e",
      device_path: "/dev/sdg1",
      filesystem: "ext4",
      disk_status_code: "EDGE_COPYING",
      runtime_status: "REMOVED",
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      free_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
      current_object: null,
      last_error_code: "DISK_REMOVED",
      error_message: "拷贝过程中检测到运输盘拔出",
      message: "运输盘已拔出",
    },
  ],
  ws_connected: false,
  last_http_refresh_at: "2026-08-09T08:46:02Z",
  message: "Day 1 fixture：真实 HTTP 汇总和 WebSocket 将在 Day 2 接入",
};

export function edgeDashboardSummaryPath(): string {
  return import.meta.env.VITE_EDGE_DASHBOARD_SUMMARY_PATH ?? "/api/edge/summary";
}

export async function fetchEdgeDashboardSummary(
  path = edgeDashboardSummaryPath(),
): Promise<EdgeDashboardSummary> {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 6000);

  try {
    const response = await fetch(path, {
      headers: { Accept: "application/json" },
      signal: controller.signal,
    });

    if (!response.ok) {
      throw new DashboardHttpError(
        response.status === 404 ? "SUMMARY_ENDPOINT_NOT_READY" : "SUMMARY_HTTP_ERROR",
        `HTTP ${response.status} while loading ${path}`,
        response.status,
      );
    }

    const payload = (await response.json()) as EdgeDashboardSummary | EdgeControlSummary;
    return normalizeEdgeDashboardSummary(payload);
  } catch (error) {
    if (error instanceof DashboardHttpError) {
      throw error;
    }
    if (error instanceof DOMException && error.name === "AbortError") {
      throw new DashboardHttpError("SUMMARY_TIMEOUT", `Timed out while loading ${path}`);
    }
    throw new DashboardHttpError(
      "SUMMARY_UNAVAILABLE",
      error instanceof Error ? error.message : `Unable to load ${path}`,
    );
  } finally {
    window.clearTimeout(timeout);
  }
}

function normalizeEdgeDashboardSummary(
  payload: EdgeDashboardSummary | EdgeControlSummary,
): EdgeDashboardSummary {
  if ("global_progress" in payload && Array.isArray(payload.disks)) {
    return {
      ...payload,
      ws_connected: false,
      last_http_refresh_at: new Date().toISOString(),
    };
  }

  const controlPayload = payload as EdgeControlSummary;
  const latestExportJob = controlPayload.latest_export_job ?? null;
  const copiedBytes = latestExportJob?.copied_bytes ?? 0;
  const totalBytes = latestExportJob?.total_bytes ?? 0;
  const copiedCount = latestExportJob?.copied_count ?? 0;
  const objectCount = latestExportJob?.object_count ?? 0;
  const scan = controlPayload.scan ?? {};

  return {
    source: "edge",
    edge_code: controlPayload.edge_code,
    edge_name: controlPayload.edge_name ?? controlPayload.edge_code,
    export_job_id: latestExportJob?.export_job_id ?? "",
    export_job_status: latestExportJob?.export_job_status ?? "PENDING",
    scan: {
      scan_event_type: scan.event_type ?? "SCAN_PROGRESS",
      scanned_bucket_count: scan.bucket_done ?? 0,
      scanned_object_count: scan.object_seen ?? 0,
      scanned_bytes: scan.total_bytes ?? 0,
      stable_object_count: scan.stable_object_count ?? 0,
      skipped_object_count: scan.source_changed_count ?? 0,
      current_bucket: scan.current_bucket ?? "",
      current_key: scan.current_object_key ?? "",
      last_scan_at: scan.event_time ?? new Date().toISOString(),
      message: scan.message ?? scan.scan_phase ?? "等待 RustFS 扫描状态",
    },
    global_progress: {
      total_bytes: totalBytes,
      done_bytes: copiedBytes,
      remaining_bytes: Math.max(0, totalBytes - copiedBytes),
      speed_bytes_per_sec: 0,
      object_total: objectCount,
      object_done: copiedCount,
      object_remaining: Math.max(0, objectCount - copiedCount),
    },
    disks: (controlPayload.disks ?? []).map((disk, index) => {
      const diskId = disk.disk_id ?? `unidentified-disk-${index + 1}`;
      return {
        disk_id: diskId,
        disk_sn: disk.disk_sn ?? disk.hardware_serial ?? "待后端补充",
        hardware_serial: disk.hardware_serial ?? undefined,
        id_serial: disk.id_serial ?? undefined,
        stable_hardware_id: disk.stable_hardware_id ?? undefined,
        mount_path: disk.mount_path ?? "待后端补充",
        device_path: disk.device_path ?? undefined,
        filesystem: disk.filesystem ?? undefined,
        filesystem_uuid: disk.filesystem_uuid ?? undefined,
        partition_uuid: disk.partition_uuid ?? undefined,
        model: disk.model ?? undefined,
        vendor: disk.vendor ?? undefined,
        transport: disk.transport ?? undefined,
        disk_status_code: disk.disk_status_code ?? undefined,
        runtime_status: disk.runtime_status,
        total_bytes: disk.object_budget_bytes ?? disk.capacity_bytes ?? 0,
        done_bytes: 0,
        remaining_bytes: disk.object_budget_bytes ?? 0,
        free_bytes: disk.free_bytes,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
        current_object: null,
        last_error_code: disk.last_error_code ?? undefined,
        error_message: disk.error_message ?? undefined,
        message: disk.error_message ?? "等待 COPY_PROGRESS WebSocket 补充分盘实时进度",
      } satisfies EdgeDiskProgress;
    }),
    ws_connected: false,
    last_http_refresh_at: new Date().toISOString(),
    message: controlPayload.message ?? "HTTP summary loaded; waiting for COPY_PROGRESS WebSocket",
  };
}
