import {
  normalizeDiskProgress,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
  type EdgeGlobalProgress,
  localEdgePath,
  visibleDiskStatusCode,
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

export interface CopyProgressEvent {
  event_type: EdgeProgressEventType;
  event_time: string;
  source: "edge";
  edge_code: string;
  export_job_id: string;
  disk_status_code?: EdgeDashboardSummary["disk_status_code"];
  export_job_status: EdgeDashboardSummary["export_job_status"];
  global_progress: EdgeGlobalProgress;
  disks: EdgeDiskProgress[];
  message: string;
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
  const summaryIsTerminal = isTerminalExportJobStatus(summary.export_job_status);
  const eventIsTerminal = isTerminalExportJobStatus(event.export_job_status);
  const sameJob = summary.export_job_id === event.export_job_id;

  if (event.event_type === "DISK_REMOVED" && summary.disks.length === 0) {
    return {
      ...summary,
      ws_connected: true,
      message: event.message || summary.message,
    };
  }

  if (summaryIsTerminal && summary.disks.length === 0 && !eventIsTerminal && !event.event_type.startsWith("DISK_")) {
    return {
      ...summary,
      ws_connected: true,
      message: summary.message,
    };
  }

  if (sameJob && summaryIsTerminal && !eventIsTerminal && !event.event_type.startsWith("DISK_")) {
    return {
      ...summary,
      ws_connected: true,
      message: summary.message,
    };
  }

  return {
    ...summary,
    edge_code: event.edge_code,
    export_job_id: event.export_job_id,
    export_job_status: event.export_job_status,
    disk_status_code: visibleDiskStatusCode(event.disk_status_code),
    global_progress: event.global_progress,
    disks: mergeDiskProgress(summary.disks, event.disks),
    ws_connected: true,
    message: event.message,
  };
}

function mergeDiskProgress(
  currentDisks: EdgeDiskProgress[],
  eventDisks: EdgeDiskProgress[],
): EdgeDiskProgress[] {
  if (eventDisks.length === 0) return [];

  const currentByKey = new Map(currentDisks.map((disk) => [diskIdentityKey(disk), disk]));
  return eventDisks.map((disk) => {
    const normalized = normalizeDiskProgress(disk);
    const current = currentByKey.get(diskIdentityKey(normalized));
    if (!current) return normalized;

    return normalizeDiskProgress({
      ...current,
      ...normalized,
      disk_status_code: normalized.disk_status_code ?? current.disk_status_code,
      filesystem: normalized.filesystem ?? current.filesystem,
      filesystem_uuid: normalized.filesystem_uuid ?? current.filesystem_uuid,
      partition_uuid: normalized.partition_uuid ?? current.partition_uuid,
      device_path: normalized.device_path ?? current.device_path,
      model: normalized.model ?? current.model,
      vendor: normalized.vendor ?? current.vendor,
      transport: normalized.transport ?? current.transport,
      hardware_serial: normalized.hardware_serial ?? current.hardware_serial,
      id_serial: normalized.id_serial ?? current.id_serial,
      stable_hardware_id: normalized.stable_hardware_id ?? current.stable_hardware_id,
      total_bytes: normalized.total_bytes || current.total_bytes,
      free_bytes: normalized.free_bytes || current.free_bytes,
    });
  });
}

function diskIdentityKey(disk: EdgeDiskProgress): string {
  return disk.disk_id || disk.mount_path || disk.device_path || disk.disk_sn || disk.stable_hardware_id || "";
}

function isTerminalExportJobStatus(
  value: EdgeDashboardSummary["export_job_status"],
): boolean {
  return value === "SEALED" || value === "FAILED" || value === "CANCELLED";
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
    if (
      event.source !== "edge" ||
      !event.event_type ||
      !eventTypes.includes(event.event_type) ||
      !Array.isArray(event.disks) ||
      !event.global_progress
    ) {
      return null;
    }

    return {
      event_type: event.event_type,
      event_time: event.event_time ?? new Date().toISOString(),
      source: "edge",
      edge_code: event.edge_code ?? "",
      export_job_id: event.export_job_id ?? "",
      disk_status_code: visibleDiskStatusCode(event.disk_status_code),
      export_job_status: event.export_job_status ?? "COPYING",
      global_progress: event.global_progress,
      disks: event.disks.map(normalizeDiskProgress),
      message: event.message ?? "",
    };
  } catch {
    return null;
  }
}
