export type DiskLifecycleCode =
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

export type ImportJobStatus = "PENDING" | "IMPORTING" | "DONE" | "FAILED" | "CANCELLED";

export interface ImportObject {
  bucket: string;
  key: string;
  display_name: string;
  size_bytes: number;
  done_bytes: number;
  remaining_bytes: number;
  speed_bytes_per_sec: number;
}

export interface CenterDiskProgress {
  disk_id: string;
  disk_sn: string;
  hardware_serial?: string;
  id_serial?: string;
  stable_hardware_id?: string;
  edge_code: string;
  mount_path: string;
  device_path?: string;
  filesystem?: string;
  filesystem_uuid?: string;
  partition_uuid?: string;
  model?: string;
  vendor?: string;
  transport?: string;
  disk_enabled: boolean;
  registered: boolean;
  can_initialize: boolean;
  reusable: boolean;
  imported_before: boolean;
  disk_status_code: DiskLifecycleCode;
  runtime_status: RuntimeStatus;
  import_job_id?: string;
  import_job_status?: ImportJobStatus;
  seal_id?: string;
  total_bytes: number;
  done_bytes: number;
  object_total: number;
  object_done: number;
  speed_bytes_per_sec: number;
  current_object?: ImportObject;
  last_error_code?: string;
  error_message?: string;
  message: string;
}

export interface CenterGlobalProgress {
  total_bytes: number;
  done_bytes: number;
  remaining_bytes: number;
  speed_bytes_per_sec: number;
  object_total: number;
  object_done: number;
  object_remaining: number;
}

export interface CenterDashboardSummary {
  source: "center";
  center_id?: string;
  center_name?: string;
  event_type?: string;
  event_time?: string;
  global_progress: CenterGlobalProgress;
  disks: CenterDiskProgress[];
  ws_connected: boolean;
  last_http_refresh_at: string;
  message: string;
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

export function centerDashboardSummaryPath(): string {
  return import.meta.env.VITE_CENTER_DASHBOARD_SUMMARY_PATH ?? "/api/center/summary";
}

export async function fetchCenterDashboardSummary(
  path = centerDashboardSummaryPath(),
): Promise<CenterDashboardSummary> {
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

    const payload = (await response.json()) as CenterDashboardSummary;
    return normalizeCenterDashboardSummary(payload);
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

export function normalizeCenterDashboardSummary(
  payload: CenterDashboardSummary,
): CenterDashboardSummary {
  const disks = Array.isArray(payload.disks) ? payload.disks : [];
  const totals = disks.reduce(
    (acc, disk) => ({
      total_bytes: acc.total_bytes + disk.total_bytes,
      done_bytes: acc.done_bytes + disk.done_bytes,
      speed_bytes_per_sec: acc.speed_bytes_per_sec + disk.speed_bytes_per_sec,
      object_total: acc.object_total + disk.object_total,
      object_done: acc.object_done + disk.object_done,
    }),
    {
      total_bytes: 0,
      done_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
    },
  );

  return {
    source: "center",
    center_id: payload.center_id,
    center_name: payload.center_name ?? "Center 控制中心",
    event_type: payload.event_type,
    event_time: payload.event_time,
    global_progress: payload.global_progress ?? {
      ...totals,
      remaining_bytes: Math.max(0, totals.total_bytes - totals.done_bytes),
      object_remaining: Math.max(0, totals.object_total - totals.object_done),
    },
    disks,
    ws_connected: false,
    last_http_refresh_at: new Date().toISOString(),
    message: payload.message ?? "HTTP summary loaded; waiting for IMPORT_PROGRESS WebSocket",
  };
}
