import type {
  CenterDashboardSummary,
  CenterDiskProgress,
  CenterGlobalProgress,
} from "../api/centerDashboard";

export type CenterProgressEventType =
  | "IMPORT_STARTED"
  | "IMPORT_PROGRESS"
  | "IMPORT_DONE"
  | "DISK_DETECTED"
  | "DISK_REMOVED"
  | "DISK_CHECKING"
  | "DISK_READY"
  | "DISK_REJECTED"
  | "ERROR";

export interface ImportProgressEvent {
  event_type: CenterProgressEventType;
  event_time: string;
  source: "center";
  global_progress?: CenterGlobalProgress;
  disks: CenterDiskProgress[];
  message: string;
}

export interface CenterProgressSocketHandlers {
  onEvent: (event: ImportProgressEvent) => void;
  onConnectionChange: (connected: boolean, message: string) => void;
}

export interface CenterProgressSocket {
  close: () => void;
}

export function centerDashboardWsPath(): string {
  return import.meta.env.VITE_CENTER_DASHBOARD_WS_PATH ?? "/ws/center/progress";
}

export function connectCenterProgressSocket(
  handlers: CenterProgressSocketHandlers,
  path = centerDashboardWsPath(),
): CenterProgressSocket {
  let socket: WebSocket | null = null;
  let stopped = false;
  let retryAttempt = 0;
  let retryTimer: number | undefined;

  const connect = () => {
    if (stopped) {
      return;
    }

    socket = new WebSocket(toWebSocketUrl(path));
    socket.addEventListener("open", () => {
      retryAttempt = 0;
      handlers.onConnectionChange(true, "WebSocket 已连接");
    });
    socket.addEventListener("message", (message) => {
      const event = parseImportProgressEvent(message.data);
      if (event) {
        handlers.onEvent(event);
      }
    });
    socket.addEventListener("close", () => {
      if (stopped) {
        return;
      }
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

export function applyImportProgressEvent(
  summary: CenterDashboardSummary,
  event: ImportProgressEvent,
): CenterDashboardSummary {
  return {
    ...summary,
    event_type: event.event_type,
    event_time: event.event_time,
    global_progress: event.global_progress ?? summarizeDisks(event.disks),
    disks: event.disks,
    ws_connected: true,
    message: event.message,
  };
}

function summarizeDisks(disks: CenterDiskProgress[]): CenterGlobalProgress {
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
    ...totals,
    remaining_bytes: Math.max(0, totals.total_bytes - totals.done_bytes),
    object_remaining: Math.max(0, totals.object_total - totals.object_done),
  };
}

function toWebSocketUrl(path: string): string {
  if (path.startsWith("ws://") || path.startsWith("wss://")) {
    return path;
  }

  const url = new URL(path, window.location.origin);
  url.protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

function parseImportProgressEvent(data: unknown): ImportProgressEvent | null {
  if (typeof data !== "string") {
    return null;
  }

  try {
    const event = JSON.parse(data) as Partial<ImportProgressEvent>;
    if (event.source !== "center" || !event.event_type || !Array.isArray(event.disks)) {
      return null;
    }
    return event as ImportProgressEvent;
  } catch {
    return null;
  }
}
