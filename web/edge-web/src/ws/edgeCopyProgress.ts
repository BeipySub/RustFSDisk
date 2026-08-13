import {
  normalizeEdgeDashboardSummary,
  type EdgeDashboardSummary,
  localEdgePath,
} from "../api/edgeDashboard.ts";

export type EdgeProgressEventType =
  | "COPY_PROGRESS"
  | "COPY_STARTED"
  | "COPY_DONE"
  | "SEAL_DONE"
  | "DISK_DETECTED"
  | "DISK_REMOVED"
  | "DISK_CHECKING"
  | "DISK_READY"
  | "DISK_REJECTED"
  | "SCAN_STARTED"
  | "SCAN_PROGRESS"
  | "SCAN_DONE"
  | "ERROR";

export interface CopyProgressEvent extends EdgeDashboardSummary {
  event_type?: EdgeProgressEventType;
  event_time: string;
}

export interface EdgeProgressSocketHandlers {
  onEvent: (event: CopyProgressEvent) => void;
  onConnectionChange: (connected: boolean, message: string) => void;
}

export interface EdgeProgressSocket {
  close: () => void;
}

const eventTypes: readonly EdgeProgressEventType[] = [
  "COPY_PROGRESS",
  "COPY_STARTED",
  "COPY_DONE",
  "SEAL_DONE",
  "DISK_DETECTED",
  "DISK_REMOVED",
  "DISK_CHECKING",
  "DISK_READY",
  "DISK_REJECTED",
  "SCAN_STARTED",
  "SCAN_PROGRESS",
  "SCAN_DONE",
  "ERROR",
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
  const next = normalizeEdgeDashboardSummary(event);
  const objectInventory = hasUsableObjectInventory(event.object_inventory)
    ? next.object_inventory
    : summary.object_inventory;
  return {
    ...next,
    object_inventory: objectInventory,
    ws_connected: true,
    last_http_refresh_at: next.last_http_refresh_at || summary.last_http_refresh_at,
    message: next.message || event.message || summary.message,
  };
}

function hasUsableObjectInventory(
  value: EdgeDashboardSummary["object_inventory"] | undefined,
): boolean {
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
    if (event.source !== "edge" || !Array.isArray(event.disks)) return null;
    if (event.event_type && !eventTypes.includes(event.event_type)) return null;

    const summary = normalizeEdgeDashboardSummary(event as EdgeDashboardSummary);
    return {
      ...summary,
      event_type: event.event_type,
      event_time: event.event_time ?? new Date().toISOString(),
      ws_connected: true,
      message: summary.message || event.message || "",
    };
  } catch {
    return null;
  }
}
