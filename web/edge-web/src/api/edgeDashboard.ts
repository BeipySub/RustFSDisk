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

export interface EdgeCurrentObject {
  object_id: string;
  bucket: string;
  key: string;
  display_name: string;
  storage_mode: "PACK" | "FRAMES";
  frame_index: number;
  frame_total: number;
  size_bytes: number;
  done_bytes: number;
  remaining_bytes: number;
  speed_bytes_per_sec: number;
  object_status: ObjectStatus;
}

export interface EdgeObjectInventory {
  total_bytes: number;
  exported_bytes: number;
  total_count: number;
  exported_count: number;
}

export interface EdgeDiskProgressSnapshot extends EdgeGlobalProgress {
  percent: number;
}

export interface EdgeDiskProgress {
  disk_presence_id?: string;
  disk_id: string;
  disk_sn: string;
  hardware_serial?: string;
  id_serial?: string;
  stable_hardware_id?: string;
  mount_path: string;
  device_path?: string;
  filesystem?: string;
  filesystem_type?: string;
  filesystem_uuid?: string;
  fs_uuid?: string;
  partition_uuid?: string;
  model?: string;
  vendor?: string;
  transport?: string;
  disk_status_code?: EdgeVisibleDiskStatusCode;
  runtime_status: RuntimeStatus;
  capacity_bytes: number;
  reserve_bytes: number;
  object_budget_bytes: number;
  task_pool_eligible?: boolean;
  export_job_id?: string;
  progress?: EdgeDiskProgressSnapshot;
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

export interface EdgeExportJobSnapshot extends EdgeGlobalProgress {
  export_job_id: string;
  export_job_status: ExportJobStatus;
  start_time?: string | null;
  finish_time?: string | null;
}

export interface EdgeDashboardSummary {
  source: "edge";
  edge_code: string;
  edge_name: string;
  edge_status?: EdgeStatus;
  object_inventory?: EdgeObjectInventory;
  export_job?: EdgeExportJobSnapshot | null;
  global_progress: EdgeGlobalProgress;
  disks: EdgeDiskProgress[];
  ws_connected: boolean;
  last_http_refresh_at: string;
  message: string;
}

export function diskFilesystemDisplay(
  disk: Pick<EdgeDiskProgress, "filesystem" | "filesystem_type"> | null | undefined,
): string {
  return disk?.filesystem_type ?? disk?.filesystem ?? "未返回";
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
  items?: EdgeExportJobRecord[];
}

export interface EdgeReadiness {
  ok: boolean;
  service: string;
  edge_code: string;
  database_ok?: boolean;
  rustfs_ok?: boolean;
  disk_mount_roots?: string[];
}

interface EdgeWireObjectInventory {
  total_bytes?: number;
  exported_bytes?: number;
  total_count?: number;
  exported_count?: number;
}

interface EdgeWireExportJob extends Partial<EdgeGlobalProgress> {
  export_job_id?: string | null;
  edge_code?: string;
  export_job_status?: string | null;
  start_time?: string | null;
  finish_time?: string | null;
}

interface EdgeControlDiskRuntime {
  disk_presence_id?: string | null;
  disk_id?: string | null;
  disk_sn?: string | null;
  hardware_serial?: string | null;
  id_serial?: string | null;
  stable_hardware_id?: string | null;
  mount_path?: string | null;
  device_path?: string | null;
  filesystem?: string | null;
  filesystem_type?: string | null;
  filesystem_uuid?: string | null;
  fs_uuid?: string | null;
  partition_uuid?: string | null;
  model?: string | null;
  vendor?: string | null;
  transport?: string | null;
  disk_status_code?: string | null;
  runtime_status?: string | null;
  task_pool_eligible?: boolean;
  export_job_id?: string | null;
  progress?: Partial<EdgeDiskProgressSnapshot> | null;
  capacity_bytes?: number;
  free_bytes?: number;
  reserve_bytes?: number;
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
  object_inventory?: EdgeWireObjectInventory;
  export_job?: EdgeWireExportJob | null;
  global_progress?: Partial<EdgeGlobalProgress>;
  disks?: EdgeControlDiskRuntime[];
  message?: string;
  ws_connected?: boolean;
  last_http_refresh_at?: string;
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
  const controlPayload = payload as EdgeControlSummary & Partial<EdgeDashboardSummary>;
  const exportJob = normalizeExportJobSnapshot(controlPayload.export_job);
  const globalProgress = normalizeGlobalProgress(controlPayload.global_progress, exportJob ?? emptyGlobalProgress());
  const objectInventory = normalizeObjectInventory(
    controlPayload.object_inventory,
    globalProgress,
    exportJob,
  );

  return {
    source: "edge",
    edge_code: controlPayload.edge_code ?? "",
    edge_name: controlPayload.edge_name ?? controlPayload.edge_code ?? "Edge",
    edge_status: edgeStatus(controlPayload.edge_status),
    object_inventory: objectInventory,
    export_job: exportJob,
    global_progress: globalProgress,
    disks: (controlPayload.disks ?? []).map((disk, index) => normalizeDiskProgress(disk, index)),
    ws_connected: Boolean(controlPayload.ws_connected),
    last_http_refresh_at: controlPayload.last_http_refresh_at ?? new Date().toISOString(),
    message: controlPayload.message ?? "HTTP summary loaded; waiting for COPY_PROGRESS WebSocket",
  };
}

export function normalizeDiskProgress(
  disk: EdgeDiskProgress | EdgeControlDiskRuntime,
  index = 0,
): EdgeDiskProgress {
  const filesystem = nullableString(disk.filesystem_type ?? disk.filesystem);
  const filesystemUuid = nullableString(disk.fs_uuid ?? disk.filesystem_uuid);
  const capacityBytes = numberValue(disk.capacity_bytes);
  const budgetBytes = numberValue(disk.object_budget_bytes);
  const progressDefault = {
    total_bytes: numberValue((disk.progress?.total_bytes ?? disk.total_bytes ?? budgetBytes) || capacityBytes),
    done_bytes: numberValue(disk.progress?.done_bytes ?? disk.done_bytes),
    remaining_bytes: numberValue(disk.progress?.remaining_bytes ?? disk.remaining_bytes),
    speed_bytes_per_sec: numberValue(disk.progress?.speed_bytes_per_sec ?? disk.speed_bytes_per_sec),
    object_total: numberValue(disk.progress?.object_total ?? disk.object_total),
    object_done: numberValue(disk.progress?.object_done ?? disk.object_done),
    object_remaining: numberValue(disk.progress?.object_remaining ?? disk.object_remaining),
  };
  const progress = normalizeDiskProgressSnapshot(disk.progress, progressDefault);
  const diskId = disk.disk_id || `unidentified-disk-${index + 1}`;

  return {
    ...disk,
    disk_presence_id: nullableString(disk.disk_presence_id),
    disk_id: diskId,
    disk_sn: disk.disk_sn ?? disk.hardware_serial ?? "",
    hardware_serial: nullableString(disk.hardware_serial),
    id_serial: nullableString(disk.id_serial),
    stable_hardware_id: nullableString(disk.stable_hardware_id),
    mount_path: disk.mount_path ?? "",
    device_path: nullableString(disk.device_path),
    filesystem,
    filesystem_type: filesystem,
    filesystem_uuid: filesystemUuid,
    fs_uuid: filesystemUuid,
    partition_uuid: nullableString(disk.partition_uuid),
    model: nullableString(disk.model),
    vendor: nullableString(disk.vendor),
    transport: nullableString(disk.transport),
    disk_status_code: visibleDiskStatusCode(disk.disk_status_code),
    runtime_status: runtimeStatus(disk.runtime_status, "DETECTED"),
    capacity_bytes: capacityBytes,
    reserve_bytes: numberValue(disk.reserve_bytes),
    object_budget_bytes: budgetBytes,
    task_pool_eligible: disk.task_pool_eligible,
    export_job_id: nullableString(disk.export_job_id),
    progress,
    total_bytes: progress.total_bytes || budgetBytes || capacityBytes,
    done_bytes: progress.done_bytes,
    remaining_bytes: progress.remaining_bytes,
    free_bytes: numberValue(disk.free_bytes),
    speed_bytes_per_sec: progress.speed_bytes_per_sec,
    object_total: progress.object_total,
    object_done: progress.object_done,
    object_remaining: progress.object_remaining,
    current_object: normalizeCurrentObject(disk.current_object),
    last_error_code: nullableString(disk.last_error_code),
    error_message: nullableString(disk.error_message),
    message: disk.message ?? disk.error_message ?? "",
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
    total: numberValue(payload.total),
    items: (payload.items ?? []).map(normalizeExportJobRecord),
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

function emptyGlobalProgress(): EdgeGlobalProgress {
  return {
    total_bytes: 0,
    done_bytes: 0,
    remaining_bytes: 0,
    speed_bytes_per_sec: 0,
    object_total: 0,
    object_done: 0,
    object_remaining: 0,
  };
}

function normalizeDiskProgressSnapshot(
  value: Partial<EdgeDiskProgressSnapshot> | null | undefined,
  defaultValue: EdgeGlobalProgress,
): EdgeDiskProgressSnapshot {
  const progress = normalizeGlobalProgress(value ?? undefined, defaultValue);
  return {
    ...progress,
    percent: numberValue(value?.percent, percentValue(progress.done_bytes, progress.total_bytes)),
  };
}

function normalizeExportJobSnapshot(
  value: EdgeWireExportJob | EdgeExportJobSnapshot | null | undefined,
): EdgeExportJobSnapshot | null {
  const progressValue = value as (Partial<EdgeWireExportJob> & Partial<EdgeExportJobSnapshot>) | null | undefined;
  if (!progressValue?.export_job_id && !progressValue?.export_job_status) return null;
  const totalBytes = numberValue(progressValue.total_bytes);
  const doneBytes = numberValue(progressValue.done_bytes);
  const objectTotal = numberValue(progressValue.object_total);
  const objectDone = numberValue(progressValue.object_done);
  const progress = normalizeGlobalProgress(progressValue, {
    total_bytes: totalBytes,
    done_bytes: doneBytes,
    remaining_bytes: Math.max(0, totalBytes - doneBytes),
    speed_bytes_per_sec: numberValue(progressValue.speed_bytes_per_sec),
    object_total: objectTotal,
    object_done: objectDone,
    object_remaining: Math.max(0, objectTotal - objectDone),
  });
  return {
    ...progress,
    export_job_id: progressValue.export_job_id ?? "",
    export_job_status: exportJobStatus(progressValue.export_job_status ?? undefined, "PENDING"),
    start_time: progressValue.start_time,
    finish_time: progressValue.finish_time,
  };
}

function normalizeObjectInventory(
  value: EdgeWireObjectInventory | EdgeObjectInventory | undefined,
  globalProgress: EdgeGlobalProgress,
  exportJob: EdgeExportJobSnapshot | null | undefined,
): EdgeObjectInventory {
  return {
    total_bytes: numberValue(value?.total_bytes, globalProgress.total_bytes),
    exported_bytes: numberValue(value?.exported_bytes, exportJob?.done_bytes ?? globalProgress.done_bytes),
    total_count: numberValue(value?.total_count, globalProgress.object_total),
    exported_count: numberValue(value?.exported_count, exportJob?.object_done ?? globalProgress.object_done),
  };
}

function percentValue(doneBytes: number, totalBytes: number): number {
  if (totalBytes <= 0) return 0;
  return Math.min(100, Math.max(0, (doneBytes / totalBytes) * 100));
}

function normalizeCurrentObject(
  value: EdgeCurrentObject | Partial<EdgeCurrentObject> | null | undefined,
): EdgeCurrentObject | null {
  if (!value) return null;
  return {
    object_id: value.object_id ?? "",
    bucket: value.bucket ?? "",
    key: value.key ?? "",
    display_name: value.display_name ?? lastPathSegment(value.key) ?? "",
    storage_mode: value.storage_mode ?? "PACK",
    frame_index: numberValue(value.frame_index),
    frame_total: numberValue(value.frame_total),
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

function nullableString(value: string | null | undefined): string | undefined {
  return value ?? undefined;
}

function numberValue(value: unknown, defaultValue = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : defaultValue;
}
