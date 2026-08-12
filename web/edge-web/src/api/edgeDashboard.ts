export type DiskStatusCode =
  | "UNREGISTERED"
  | "REGISTERED"
  | "INITIALIZED"
  | "EDGE_COPYING"
  | "SEALED"
  | "CENTER_IMPORTING"
  | "IMPORTED"
  | "ERROR";

export type EdgeVisibleDiskStatusCode = DiskStatusCode;

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
  | "SEALING"
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

export type EdgeStatus = "ACTIVE" | "DISABLED" | "ERROR";
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
  disk_status_code?: EdgeVisibleDiskStatusCode;
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
  edge_status?: EdgeStatus;
  export_job_id: string;
  export_job_status: ExportJobStatus;
  disk_status_code?: EdgeVisibleDiskStatusCode;
  scan: EdgeScanSummary;
  global_progress: EdgeGlobalProgress;
  disks: EdgeDiskProgress[];
  ws_connected: boolean;
  last_http_refresh_at: string;
  message: string;
}

export interface EdgeExportJobRecord {
  export_job_id: string;
  edge_code: string;
  export_job_status: ExportJobStatus;
  object_count: number;
  copied_count: number;
  total_bytes: number;
  copied_bytes: number;
  disk_count: number;
  start_time?: string;
  finish_time?: string;
  error_message?: string;
  object_status_counts: Partial<Record<ObjectStatus, number>>;
}

export interface EdgeExportJobDetail extends EdgeExportJobRecord {
  disks: EdgeDiskProgress[];
  events: EdgeExportJobEvent[];
}

export interface EdgeExportJobEvent {
  event_time: string;
  event_type: string;
  runtime_status?: RuntimeStatus;
  export_job_status?: ExportJobStatus;
  disk_id?: string;
  last_error_code?: string;
  message: string;
}

export interface EdgeExportJobsQuery {
  page: number;
  page_size: number;
  export_job_status?: ExportJobStatus | "";
  started_from?: string;
  started_to?: string;
  q?: string;
}

export interface EdgeExportJobsResponse {
  page: number;
  page_size: number;
  total: number;
  items: EdgeExportJobRecord[];
}

interface EdgeExportJobsWireResponse {
  page?: number;
  page_size?: number;
  total?: number;
  total_count?: number;
  items?: EdgeExportJobRecord[];
  records?: EdgeExportJobRecord[];
}

export interface EdgeReadiness {
  ok: boolean;
  service: string;
  edge_code: string;
  database_ok?: boolean;
  rustfs_ok?: boolean;
  disk_mount_roots?: string[];
}

interface EdgeControlScanSnapshot {
  event_type?: string;
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
  message?: string | null;
}

interface EdgeControlExportJob {
  export_job_id: string;
  edge_code: string;
  export_job_status: string;
  object_count?: number;
  copied_count?: number;
  total_bytes?: number;
  copied_bytes?: number;
  disk_count?: number;
  start_time?: string | null;
  finish_time?: string | null;
  error_message?: string | null;
  object_status_counts?: Record<string, number>;
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
  disk_status_code?: string | null;
  runtime_status?: string | null;
  capacity_bytes?: number;
  free_bytes?: number;
  object_budget_bytes?: number;
  total_bytes?: number;
  done_bytes?: number;
  remaining_bytes?: number;
  speed_bytes_per_sec?: number;
  object_total?: number;
  object_done?: number;
  object_remaining?: number;
  current_object?: Partial<EdgeCurrentObject> | null;
  last_error_code?: string | null;
  error_message?: string | null;
  message?: string | null;
}

interface EdgeControlSummary {
  source: "edge";
  edge_code: string;
  edge_name?: string;
  edge_status?: string;
  scan?: EdgeControlScanSnapshot;
  latest_export_job?: EdgeControlExportJob | null;
  export_job?: EdgeControlExportJob | null;
  export_job_id?: string;
  export_job_status?: string;
  disk_status_code?: string;
  global_progress?: Partial<EdgeGlobalProgress>;
  disks?: EdgeControlDiskRuntime[];
  message?: string;
}

const exportJobStatuses: readonly ExportJobStatus[] = [
  "PENDING",
  "SCANNING",
  "COPYING",
  "SEALING",
  "SEALED",
  "FAILED",
  "CANCELLED",
];

const runtimeStatuses: readonly RuntimeStatus[] = [
  "DETECTED",
  "CHECKING",
  "READY",
  "COPYING",
  "CLEANING",
  "REINITIALIZING",
  "DONE",
  "REJECTED",
  "REMOVED",
  "ERROR",
];

const objectStatuses: readonly ObjectStatus[] = [
  "PENDING",
  "ASSIGNED",
  "COPYING",
  "EXPORTED",
  "FAILED",
  "SOURCE_CHANGED",
  "SKIPPED",
];

const scanEventTypes: readonly ScanEventType[] = [
  "SCAN_STARTED",
  "SCAN_PROGRESS",
  "SCAN_DONE",
  "ERROR",
];

const visibleDiskStatusCodes: readonly EdgeVisibleDiskStatusCode[] = [
  "UNREGISTERED",
  "REGISTERED",
  "INITIALIZED",
  "EDGE_COPYING",
  "SEALED",
  "CENTER_IMPORTING",
  "IMPORTED",
  "ERROR",
];

export class DashboardHttpError extends Error {
  public readonly error_code: string;
  public readonly http_status?: number;

  constructor(
    errorCode: string,
    message: string,
    httpStatus?: number,
  ) {
    super(message);
    this.error_code = errorCode;
    this.http_status = httpStatus;
  }
}

export function edgeDashboardSummaryPath(): string {
  return envValue("VITE_EDGE_DASHBOARD_SUMMARY_PATH", "/api/edge/dashboard/summary");
}

export function edgeExportJobsPath(): string {
  return envValue("VITE_EDGE_EXPORT_JOBS_PATH", "/api/edge/dashboard/export-jobs");
}

export function edgeReadinessPath(): string {
  return envValue("VITE_EDGE_READINESS_PATH", "/readyz");
}

export function edgeScanPath(): string {
  return envValue("VITE_EDGE_SCAN_PATH", "/api/edge/scan");
}

export async function fetchEdgeReadiness(path = edgeReadinessPath()): Promise<EdgeReadiness> {
  return getJson<EdgeReadiness>(localEdgePath(path, "/readyz"));
}

export async function fetchEdgeDashboardSummary(
  path = edgeDashboardSummaryPath(),
): Promise<EdgeDashboardSummary> {
  const payload = await getJson<EdgeDashboardSummary | EdgeControlSummary>(
    localEdgePath(path, "/api/edge/dashboard/summary"),
  );
  return normalizeEdgeDashboardSummary(payload);
}

export async function fetchEdgeExportJobs(
  query: EdgeExportJobsQuery,
  basePath = edgeExportJobsPath(),
): Promise<EdgeExportJobsResponse> {
  const payload = await getJson<EdgeExportJobsWireResponse | EdgeExportJobRecord[]>(
    buildExportJobsUrl(basePath, query),
  );
  return normalizeExportJobsResponse(payload, query);
}

export async function fetchEdgeExportJobDetail(
  exportJobId: string,
  basePath = edgeExportJobsPath(),
): Promise<EdgeExportJobDetail> {
  const safeId = encodeURIComponent(exportJobId);
  const safeBasePath = localEdgePath(basePath, "/api/edge/dashboard/export-jobs");
  const payload = await getJson<Partial<EdgeExportJobDetail>>(`${safeBasePath}/${safeId}`);
  return normalizeExportJobDetail(payload);
}

export async function triggerEdgeRustFsScan(path = edgeScanPath()): Promise<void> {
  await postJson(localEdgePath(path, "/api/edge/scan"));
}

export function buildExportJobsUrl(basePath: string, query: EdgeExportJobsQuery): string {
  const url = new URL(localEdgePath(basePath, "/api/edge/dashboard/export-jobs"), browserOrigin());
  url.searchParams.set("page", String(query.page));
  url.searchParams.set("page_size", String(query.page_size));
  if (query.export_job_status) url.searchParams.set("export_job_status", query.export_job_status);
  if (query.started_from) url.searchParams.set("started_from", query.started_from);
  if (query.started_to) url.searchParams.set("started_to", query.started_to);
  if (query.q?.trim()) url.searchParams.set("q", query.q.trim());
  return url.pathname + url.search;
}

export function normalizeEdgeDashboardSummary(
  payload: EdgeDashboardSummary | EdgeControlSummary,
): EdgeDashboardSummary {
  if (isNormalizedDashboardSummary(payload)) {
    const summary = payload as EdgeDashboardSummary;
    return {
      ...summary,
      disk_status_code: visibleDiskStatusCode(summary.disk_status_code),
      disks: summary.disks.map(normalizeDiskProgress),
      ws_connected: false,
      last_http_refresh_at: new Date().toISOString(),
    };
  }

  const controlPayload = payload as EdgeControlSummary;
  const latestExportJob = controlPayload.latest_export_job ?? controlPayload.export_job ?? null;
  const copiedBytes = latestExportJob?.copied_bytes ?? 0;
  const totalBytes = latestExportJob?.total_bytes ?? 0;
  const copiedCount = latestExportJob?.copied_count ?? 0;
  const objectCount = latestExportJob?.object_count ?? 0;
  const scan = controlPayload.scan ?? {};

  return {
    source: "edge",
    edge_code: controlPayload.edge_code,
    edge_name: controlPayload.edge_name ?? controlPayload.edge_code,
    edge_status: edgeStatus(controlPayload.edge_status),
    export_job_id: latestExportJob?.export_job_id ?? controlPayload.export_job_id ?? "",
    export_job_status: exportJobStatus(
      latestExportJob?.export_job_status ?? controlPayload.export_job_status,
      "PENDING",
    ),
    disk_status_code: visibleDiskStatusCode(controlPayload.disk_status_code),
    scan: {
      scan_event_type: scanEventType(scan.event_type, "SCAN_PROGRESS"),
      scanned_bucket_count: numberValue(scan.bucket_done),
      scanned_object_count: numberValue(scan.object_seen),
      scanned_bytes: numberValue(scan.total_bytes),
      stable_object_count: numberValue(scan.stable_object_count),
      skipped_object_count: numberValue(scan.source_changed_count),
      current_bucket: scan.current_bucket ?? "",
      current_key: scan.current_object_key ?? "",
      last_scan_at: scan.event_time ?? new Date().toISOString(),
      message: scan.message ?? scan.scan_phase ?? "等待 RustFS 扫描状态",
    },
    global_progress: normalizeGlobalProgress(controlPayload.global_progress, {
      total_bytes: totalBytes,
      done_bytes: copiedBytes,
      remaining_bytes: Math.max(0, totalBytes - copiedBytes),
      speed_bytes_per_sec: 0,
      object_total: objectCount,
      object_done: copiedCount,
      object_remaining: Math.max(0, objectCount - copiedCount),
    }),
    disks: (controlPayload.disks ?? []).map((disk, index) =>
      normalizeDiskProgress({
        disk_id: disk.disk_id ?? `unidentified-disk-${index + 1}`,
        disk_sn: disk.disk_sn ?? disk.hardware_serial ?? "待后端补充",
        hardware_serial: nullableString(disk.hardware_serial),
        id_serial: nullableString(disk.id_serial),
        stable_hardware_id: nullableString(disk.stable_hardware_id),
        mount_path: disk.mount_path ?? "待后端补充",
        device_path: nullableString(disk.device_path),
        filesystem: nullableString(disk.filesystem),
        filesystem_uuid: nullableString(disk.filesystem_uuid),
        partition_uuid: nullableString(disk.partition_uuid),
        model: nullableString(disk.model),
        vendor: nullableString(disk.vendor),
        transport: nullableString(disk.transport),
        disk_status_code: visibleDiskStatusCode(disk.disk_status_code),
        runtime_status: runtimeStatus(disk.runtime_status, "DETECTED"),
        total_bytes: numberValue(disk.total_bytes ?? disk.object_budget_bytes ?? disk.capacity_bytes),
        done_bytes: numberValue(disk.done_bytes),
        remaining_bytes: numberValue(disk.remaining_bytes ?? disk.object_budget_bytes),
        free_bytes: numberValue(disk.free_bytes),
        speed_bytes_per_sec: numberValue(disk.speed_bytes_per_sec),
        object_total: numberValue(disk.object_total),
        object_done: numberValue(disk.object_done),
        object_remaining: numberValue(disk.object_remaining),
        current_object: normalizeCurrentObject(disk.current_object),
        last_error_code: nullableString(disk.last_error_code),
        error_message: nullableString(disk.error_message),
        message:
          disk.message ??
          disk.error_message ??
          "等待 COPY_PROGRESS WebSocket 补充分盘实时进度",
      }),
    ),
    ws_connected: false,
    last_http_refresh_at: new Date().toISOString(),
    message: controlPayload.message ?? "HTTP summary loaded; waiting for COPY_PROGRESS WebSocket",
  };
}

function isNormalizedDashboardSummary(
  payload: EdgeDashboardSummary | EdgeControlSummary,
): payload is EdgeDashboardSummary {
  const scan = (payload as { scan?: Partial<EdgeScanSummary> }).scan;
  return Array.isArray(payload.disks) && scan?.scan_event_type !== undefined;
}

export function normalizeDiskProgress(disk: EdgeDiskProgress): EdgeDiskProgress {
  return {
    ...disk,
    disk_status_code: visibleDiskStatusCode(disk.disk_status_code),
    runtime_status: runtimeStatus(disk.runtime_status, "DETECTED"),
    current_object: normalizeCurrentObject(disk.current_object),
  };
}

export function visibleDiskStatusCode(
  value: DiskStatusCode | string | null | undefined,
): EdgeVisibleDiskStatusCode | undefined {
  return visibleDiskStatusCodes.includes(value as EdgeVisibleDiskStatusCode)
    ? (value as EdgeVisibleDiskStatusCode)
    : undefined;
}

export function diskStatusDisplay(value: EdgeVisibleDiskStatusCode | undefined): string {
  if (value === "UNREGISTERED") return "未注册";
  if (value === "REGISTERED") return "已注册";
  if (value === "INITIALIZED") return "已初始化";
  if (value === "EDGE_COPYING") return "写入中";
  if (value === "SEALED") return "已封盘";
  if (value === "CENTER_IMPORTING") return "中控导入中";
  if (value === "IMPORTED") return "已导入";
  if (value === "ERROR") return "异常";
  return "未返回";
}

export function edgeRejectedDiskStatusLabel(
  disk: Pick<EdgeDiskProgress, "disk_status_code" | "last_error_code" | "error_message" | "message">,
): string {
  if (disk.disk_status_code === "SEALED") return "已封盘";
  if (disk.disk_status_code === "IMPORTED") return "已导入";
  if (disk.disk_status_code === "CENTER_IMPORTING") return "中控导入中";
  if (disk.disk_status_code === "UNREGISTERED") return "未注册";
  const reason = disk.error_message || disk.last_error_code || disk.message || "";
  if (isEdgeUninitializedDiskIssue(reason)) return "未初始化";
  if (isEdgeUnregisteredDiskIssue(reason)) return "未注册";
  if (isEdgeUnsupportedDiskIssue(reason)) return "不可导出";
  return "拒绝";
}

export function edgeDiskPrimaryStatusLabel(
  disk: Pick<EdgeDiskProgress, "disk_status_code" | "runtime_status" | "last_error_code" | "error_message" | "message">,
): string {
  if (disk.disk_status_code) return diskStatusDisplay(disk.disk_status_code);
  return "未返回";
}

export function isEdgeUninitializedDiskIssue(value: string): boolean {
  return /MISSING_DISK_INFO|NO_DISK_INFO|missing .*disk_info|disk_info\.json.*missing|UNINITIALIZED|missing .*protocol|缺少.*运输协议|缺少.*disk_info/i.test(value);
}

export function isEdgeUnregisteredDiskIssue(value: string): boolean {
  return /UNREGISTERED|not registered|unregistered disk/i.test(value);
}

export function isEdgeUnsupportedDiskIssue(value: string): boolean {
  return /FILESYSTEM_INVALID|UNSUPPORTED|non[-_ ]?protocol|not ext4|non[-_ ]?ext4|filesystem/i.test(value);
}

export function isActiveExportJobStatus(value: ExportJobStatus | undefined): boolean {
  return value === "SCANNING" || value === "COPYING" || value === "SEALING";
}

export function normalizeExportJobsResponse(
  payload: EdgeExportJobsWireResponse | EdgeExportJobRecord[],
  query: Pick<EdgeExportJobsQuery, "page" | "page_size">,
): EdgeExportJobsResponse {
  if (Array.isArray(payload)) {
    return {
      page: query.page,
      page_size: query.page_size,
      total: payload.length,
      items: payload.map(normalizeExportJobRecord),
    };
  }

  return {
    page: numberValue(payload.page, query.page),
    page_size: numberValue(payload.page_size, query.page_size),
    total: numberValue(payload.total ?? payload.total_count),
    items: (payload.items ?? payload.records ?? []).map(normalizeExportJobRecord),
  };
}

export function normalizeExportJobDetail(payload: Partial<EdgeExportJobDetail>): EdgeExportJobDetail {
  const record = normalizeExportJobRecord(payload);
  return {
    ...record,
    disks: (payload.disks ?? []).map((disk) =>
      normalizeHistoricalExportDiskProgress(disk, record.export_job_status),
    ),
    events: (payload.events ?? []).map((event) => ({
      event_time: event.event_time ?? "",
      event_type: event.event_type ?? "COPY_PROGRESS",
      runtime_status: runtimeStatus(event.runtime_status, undefined),
      export_job_status: exportJobStatus(event.export_job_status, undefined),
      disk_id: event.disk_id,
      last_error_code: event.last_error_code,
      message: event.message ?? "",
    })),
  };
}

function normalizeHistoricalExportDiskProgress(
  disk: EdgeDiskProgress,
  exportJobStatus: ExportJobStatus,
): EdgeDiskProgress {
  const normalized = normalizeDiskProgress(disk);
  if (exportJobStatus !== "SEALED") return normalized;

  return {
    ...normalized,
    disk_status_code: normalized.disk_status_code ?? "SEALED",
    runtime_status:
      normalized.runtime_status === "DETECTED" && !normalized.mount_path ? "DONE" : normalized.runtime_status,
    remaining_bytes: 0,
    message:
      normalized.message === "等待 COPY_PROGRESS WebSocket 补充分盘实时进度" || !normalized.message
        ? "已封盘，可拔盘"
        : normalized.message,
  };
}

function normalizeExportJobRecord(payload: Partial<EdgeExportJobRecord>): EdgeExportJobRecord {
  return {
    export_job_id: payload.export_job_id ?? "",
    edge_code: payload.edge_code ?? "",
    export_job_status: exportJobStatus(payload.export_job_status, "PENDING"),
    object_count: numberValue(payload.object_count),
    copied_count: numberValue(payload.copied_count),
    total_bytes: numberValue(payload.total_bytes),
    copied_bytes: numberValue(payload.copied_bytes),
    disk_count: numberValue(payload.disk_count),
    start_time: payload.start_time,
    finish_time: payload.finish_time,
    error_message: payload.error_message,
    object_status_counts: normalizeObjectStatusCounts(payload.object_status_counts),
  };
}

function normalizeObjectStatusCounts(
  counts: Partial<Record<ObjectStatus, number>> | undefined,
): Partial<Record<ObjectStatus, number>> {
  const safeCounts: Partial<Record<ObjectStatus, number>> = {};
  for (const [key, value] of Object.entries(counts ?? {})) {
    if (objectStatuses.includes(key as ObjectStatus)) {
      safeCounts[key as ObjectStatus] = numberValue(value);
    }
  }
  return safeCounts;
}

function normalizeGlobalProgress(
  value: Partial<EdgeGlobalProgress> | undefined,
  defaultValue: EdgeGlobalProgress,
): EdgeGlobalProgress {
  const totalBytes = numberValue(value?.total_bytes, defaultValue.total_bytes);
  const doneBytes = numberValue(value?.done_bytes, defaultValue.done_bytes);
  const objectTotal = numberValue(value?.object_total, defaultValue.object_total);
  const objectDone = numberValue(value?.object_done, defaultValue.object_done);
  return {
    total_bytes: totalBytes,
    done_bytes: doneBytes,
    remaining_bytes: numberValue(value?.remaining_bytes, Math.max(0, totalBytes - doneBytes)),
    speed_bytes_per_sec: numberValue(value?.speed_bytes_per_sec, defaultValue.speed_bytes_per_sec),
    object_total: objectTotal,
    object_done: objectDone,
    object_remaining: numberValue(value?.object_remaining, Math.max(0, objectTotal - objectDone)),
  };
}

function normalizeCurrentObject(
  value: EdgeCurrentObject | Partial<EdgeCurrentObject> | null | undefined,
): EdgeCurrentObject | null {
  if (!value) return null;
  return {
    bucket: value.bucket ?? "",
    key: value.key ?? "",
    display_name: value.display_name ?? lastPathSegment(value.key) ?? "",
    relative_data_path: value.relative_data_path ?? "",
    size_bytes: numberValue(value.size_bytes),
    done_bytes: numberValue(value.done_bytes),
    remaining_bytes: numberValue(value.remaining_bytes),
    speed_bytes_per_sec: numberValue(value.speed_bytes_per_sec),
    object_status: objectStatus(value.object_status, "COPYING"),
  };
}

async function getJson<T>(path: string): Promise<T> {
  const controller = new AbortController();
  const timeout = globalThis.setTimeout(() => controller.abort(), 6000);

  try {
    const response = await fetch(path, {
      headers: { Accept: "application/json" },
      signal: controller.signal,
    });

    if (!response.ok) {
      throw new DashboardHttpError(
        response.status === 404 ? "DASHBOARD_ENDPOINT_NOT_READY" : "DASHBOARD_HTTP_ERROR",
        `HTTP ${response.status} while loading ${path}`,
        response.status,
      );
    }

    return (await response.json()) as T;
  } catch (error) {
    if (error instanceof DashboardHttpError) throw error;
    if (error instanceof DOMException && error.name === "AbortError") {
      throw new DashboardHttpError("DASHBOARD_TIMEOUT", `Timed out while loading ${path}`);
    }
    throw new DashboardHttpError(
      "DASHBOARD_UNAVAILABLE",
      error instanceof Error ? error.message : `Unable to load ${path}`,
    );
  } finally {
    globalThis.clearTimeout(timeout);
  }
}

async function postJson(path: string): Promise<void> {
  const controller = new AbortController();
  const timeout = globalThis.setTimeout(() => controller.abort(), 6000);

  try {
    const response = await fetch(path, {
      method: "POST",
      headers: { Accept: "application/json" },
      signal: controller.signal,
    });

    if (!response.ok) {
      throw new DashboardHttpError(
        response.status === 404 ? "EDGE_SCAN_ENDPOINT_NOT_READY" : "EDGE_SCAN_HTTP_ERROR",
        `HTTP ${response.status} while posting ${path}`,
        response.status,
      );
    }
  } catch (error) {
    if (error instanceof DashboardHttpError) throw error;
    if (error instanceof DOMException && error.name === "AbortError") {
      throw new DashboardHttpError("EDGE_SCAN_TIMEOUT", `Timed out while posting ${path}`);
    }
    throw new DashboardHttpError(
      "EDGE_SCAN_UNAVAILABLE",
      error instanceof Error ? error.message : `Unable to post ${path}`,
    );
  } finally {
    globalThis.clearTimeout(timeout);
  }
}

function envValue(key: string, defaultPath: string): string {
  return localEdgePath(
    (import.meta as unknown as { env?: Record<string, string | undefined> }).env?.[key],
    defaultPath,
  );
}

export function localEdgePath(value: string | undefined, defaultPath: string): string {
  const trimmed = value?.trim();
  if (!trimmed || trimmed.startsWith("//")) return defaultPath;

  try {
    const url = new URL(trimmed, browserOrigin());
    if (url.origin !== browserOrigin()) return defaultPath;
    return url.pathname + url.search;
  } catch {
    return defaultPath;
  }
}

function browserOrigin(): string {
  return (globalThis as { location?: Location }).location?.origin ?? "http://localhost";
}

function lastPathSegment(value: string | undefined): string | undefined {
  if (!value) return undefined;
  const segments = value.split("/");
  return segments[segments.length - 1];
}

function edgeStatus(value: string | undefined): EdgeStatus | undefined {
  return value === "ACTIVE" || value === "DISABLED" || value === "ERROR" ? value : undefined;
}

function exportJobStatus<T extends ExportJobStatus | undefined>(
  value: string | undefined,
  defaultStatus: T,
): ExportJobStatus | T {
  return exportJobStatuses.includes(value as ExportJobStatus) ? (value as ExportJobStatus) : defaultStatus;
}

function runtimeStatus<T extends RuntimeStatus | undefined>(
  value: string | undefined | null,
  defaultStatus: T,
): RuntimeStatus | T {
  return runtimeStatuses.includes(value as RuntimeStatus) ? (value as RuntimeStatus) : defaultStatus;
}

function objectStatus(value: string | undefined, defaultStatus: ObjectStatus): ObjectStatus {
  return objectStatuses.includes(value as ObjectStatus) ? (value as ObjectStatus) : defaultStatus;
}

function scanEventType(value: string | undefined, defaultStatus: ScanEventType): ScanEventType {
  return scanEventTypes.includes(value as ScanEventType) ? (value as ScanEventType) : defaultStatus;
}

function nullableString(value: string | null | undefined): string | undefined {
  return value ?? undefined;
}

function numberValue(value: unknown, defaultValue = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : defaultValue;
}
