import {
  normalizeEdgeDashboardSummary,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
  type EdgeExportJobSnapshot,
  type EdgeGlobalProgress,
  type EdgeObjectInventory,
  localEdgePath,
} from "../api/edgeDashboard.ts";

export type EdgeWsV2EventType = "DISK_PLUGGED" | "DISK_UNPLUGGED" | "COPY_PROGRESS";

export type EdgeCopyStage =
  | "SCANNING_RUSTFS"
  | "PLANNING"
  | "COPYING"
  | "SEALING"
  | "SEALED"
  | "FAILED";

export interface EdgeScanProgress {
  scan_status?: string;
  bucket_count?: number;
  object_seen?: number;
  stable_object_count?: number;
  source_changed_count?: number;
  total_bytes?: number;
  current_bucket?: string | null;
  current_object_key?: string | null;
  last_error_code?: string | null;
}

export interface CopyProgressEvent {
  protocol_version: "edge-ws-v2";
  event_id: string;
  event_type: EdgeWsV2EventType;
  event_time: string;
  source: "edge";
  edge_code: string;
  edge_name?: string;
  stage?: EdgeCopyStage | null;
  scan?: EdgeScanProgress | null;
  object_inventory?: EdgeObjectInventory;
  export_job?: EdgeExportJobSnapshot | null;
  global?: EdgeGlobalProgress;
  global_progress?: EdgeGlobalProgress;
  disks?: Partial<EdgeDiskProgress>[];
  ws_connected?: boolean;
  last_http_refresh_at?: string;
  message?: string;
}

export interface EdgeProgressSocketHandlers {
  onEvent: (event: CopyProgressEvent) => void;
  onConnectionChange: (connected: boolean, message: string) => void;
}

export interface EdgeProgressSocket {
  close: () => void;
}

const eventTypes: readonly EdgeWsV2EventType[] = [
  "DISK_PLUGGED",
  "DISK_UNPLUGGED",
  "COPY_PROGRESS",
];

const copyStages: readonly EdgeCopyStage[] = [
  "SCANNING_RUSTFS",
  "PLANNING",
  "COPYING",
  "SEALING",
  "SEALED",
  "FAILED",
];

export function edgeDashboardWsPath(): string {
  return localEdgePath(
    (import.meta as unknown as { env?: Record<string, string | undefined> }).env
      ?.VITE_EDGE_DASHBOARD_WS_PATH,
    "/ws/edge/progress",
  );
}

export function connectEdgeProgressSocket(
  handlers: EdgeProgressSocketHandlers,
  path = edgeDashboardWsPath(),
): EdgeProgressSocket {
  let socket: WebSocket | null = null;
  let stopped = false;
  let retryAttempt = 0;
  let retryTimer: number | undefined;

  const connect = () => {
    if (stopped) return;

    socket = new WebSocket(toWebSocketUrl(path));
    socket.addEventListener("open", () => {
      retryAttempt = 0;
      handlers.onConnectionChange(true, "WebSocket 已连接");
    });
    socket.addEventListener("message", (message) => {
      const event = parseCopyProgressEvent(message.data);
      if (event) handlers.onEvent(event);
    });
    socket.addEventListener("close", () => {
      if (stopped) return;
      handlers.onConnectionChange(false, "WebSocket 已断开，正在重连");
      scheduleReconnect();
    });
    socket.addEventListener("error", () => {
      handlers.onConnectionChange(false, "WebSocket 连接异常，等待重连");
      socket?.close();
    });
  };

  const scheduleReconnect = () => {
    window.clearTimeout(retryTimer);
    retryAttempt += 1;
    const delayMs = Math.min(10_000, 1000 + retryAttempt * 1000);
    retryTimer = window.setTimeout(connect, delayMs);
  };

  connect();

  return {
    close() {
      stopped = true;
      window.clearTimeout(retryTimer);
      socket?.close();
    },
  };
}

export function applyCopyProgressEvent(
  summary: EdgeDashboardSummary,
  event: CopyProgressEvent,
): EdgeDashboardSummary {
  const normalizedEvent = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: event.edge_code || summary.edge_code,
    edge_name: event.edge_name || summary.edge_name,
    object_inventory: event.object_inventory,
    export_job: event.export_job ?? summary.export_job,
    global: event.global,
    global_progress: event.global_progress ?? summary.global_progress,
    disks: event.disks ?? [],
    ws_connected: true,
    last_http_refresh_at: summary.last_http_refresh_at,
    message: event.message || summary.message,
  });

  const nextDisks = mergeDisksByDiskId(summary.disks, normalizedEvent.disks);
  const objectInventory = hasUsableObjectInventory(event.object_inventory)
    ? normalizedEvent.object_inventory
    : summary.object_inventory;

  return {
    ...summary,
    edge_code: event.edge_code || summary.edge_code,
    edge_name: event.edge_name || summary.edge_name,
    object_inventory: objectInventory,
    export_job:
      event.event_type === "COPY_PROGRESS" && event.export_job !== undefined
        ? normalizedEvent.export_job
        : summary.export_job,
    global: event.event_type === "COPY_PROGRESS" ? normalizedEvent.global : summary.global,
    global_progress:
      event.event_type === "COPY_PROGRESS"
        ? normalizedEvent.global_progress
        : summary.global_progress,
    disks: nextDisks,
    ws_connected: true,
    last_http_refresh_at: summary.last_http_refresh_at,
    message: event.message || summary.message,
  };
}

function mergeDisksByDiskId(
  current: EdgeDiskProgress[],
  incoming: EdgeDiskProgress[],
): EdgeDiskProgress[] {
  const disks = [...current];
  for (const next of incoming) {
    const index = disks.findIndex((disk) => sameDisk(disk, next));
    if (index >= 0) {
      disks[index] = { ...disks[index], ...next };
    } else {
      disks.push(next);
    }
  }
  return disks;
}

function sameDisk(left: EdgeDiskProgress, right: EdgeDiskProgress): boolean {
  if (left.disk_presence_id && right.disk_presence_id) {
    return left.disk_presence_id === right.disk_presence_id;
  }
  const leftHasRealDiskId = left.disk_id && !left.disk_id.startsWith("unidentified-disk-");
  const rightHasRealDiskId = right.disk_id && !right.disk_id.startsWith("unidentified-disk-");
  if (leftHasRealDiskId && rightHasRealDiskId) return left.disk_id === right.disk_id;
  if (left.stable_hardware_id && right.stable_hardware_id) {
    return left.stable_hardware_id === right.stable_hardware_id;
  }
  return Boolean(left.mount_path && right.mount_path && left.mount_path === right.mount_path);
}

function hasUsableObjectInventory(value: EdgeObjectInventory | undefined): boolean {
  if (!value) return false;
  return (
    value.total_bytes > 0 ||
    value.exported_bytes > 0 ||
    value.total_count > 0 ||
    value.exported_count > 0
  );
}

export function toWebSocketUrl(path: string): string {
  const safePath = localEdgePath(path, "/ws/edge/progress");
  const url = new URL(safePath, window.location.origin);
  url.protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

export function parseCopyProgressEvent(data: unknown): CopyProgressEvent | null {
  if (typeof data !== "string") return null;

  try {
    const event = JSON.parse(data) as Partial<CopyProgressEvent>;
    if (event.protocol_version !== "edge-ws-v2") return null;
    if (event.source !== "edge") return null;
    if (!event.event_type || !eventTypes.includes(event.event_type)) return null;
    if (event.stage && !copyStages.includes(event.stage)) return null;

    return {
      protocol_version: "edge-ws-v2",
      event_id: event.event_id || `${event.event_type}-${event.event_time || Date.now()}`,
      event_type: event.event_type,
      event_time: event.event_time ?? new Date().toISOString(),
      source: "edge",
      edge_code: event.edge_code || "",
      edge_name: event.edge_name,
      stage: event.stage ?? null,
      scan: event.scan ?? null,
      object_inventory: event.object_inventory,
      export_job: event.export_job ?? null,
      global: event.global,
      global_progress: event.global_progress,
      disks: Array.isArray(event.disks) ? event.disks : [],
      ws_connected: true,
      last_http_refresh_at: event.last_http_refresh_at,
      message: event.message || "",
    };
  } catch {
    return null;
  }
}
