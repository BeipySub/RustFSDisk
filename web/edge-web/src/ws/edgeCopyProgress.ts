import {
  normalizeDiskProgress,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
  type EdgeGlobalProgress,
  type ScanEventType,
  localEdgePath,
  visibleDiskStatusCode,
} from "../api/edgeDashboard.ts";

export type EdgeProgressEventType =
  | "SCAN_STARTED"
  | "SCAN_PROGRESS"
  | "SCAN_DONE"
  | "COPY_STARTED"
  | "COPY_PROGRESS"
  | "COPY_DONE"
  | "SEAL_DONE"
  | "DISK_DETECTED"
  | "DISK_REMOVED"
  | "DISK_CHECKING"
  | "DISK_READY"
  | "DISK_REJECTED"
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
  "SCAN_STARTED",
  "SCAN_PROGRESS",
  "SCAN_DONE",
  "COPY_STARTED",
  "COPY_PROGRESS",
  "COPY_DONE",
  "SEAL_DONE",
  "DISK_DETECTED",
  "DISK_REMOVED",
  "DISK_CHECKING",
  "DISK_READY",
  "DISK_REJECTED",
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

  if (summaryIsTerminal && summary.disks.length === 0 && !eventIsTerminal) {
    return {
      ...summary,
      ws_connected: true,
      message: summary.message,
    };
  }

  if (sameJob && summaryIsTerminal && !eventIsTerminal) {
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
    scan: mergeScanSummary(summary, event),
    global_progress: event.global_progress,
    disks: mergeDiskProgress(summary.disks, event),
    ws_connected: true,
    message: event.message,
  };
}

function mergeScanSummary(
  summary: EdgeDashboardSummary,
  event: CopyProgressEvent,
): EdgeDashboardSummary["scan"] {
  if (!isScanEventType(event.event_type)) return summary.scan;

  return {
    ...summary.scan,
    scan_event_type: event.event_type,
    scanned_object_count: event.global_progress.object_total,
    scanned_bytes: event.global_progress.total_bytes,
    stable_object_count: event.global_progress.object_done,
    skipped_object_count: event.global_progress.object_remaining,
    last_scan_at: event.event_time,
    message: event.message,
  };
}

function isScanEventType(eventType: EdgeProgressEventType): eventType is ScanEventType {
  return eventType === "SCAN_STARTED" || eventType === "SCAN_PROGRESS" || eventType === "SCAN_DONE";
}

function mergeDiskProgress(
  currentDisks: EdgeDiskProgress[],
  event: CopyProgressEvent,
): EdgeDiskProgress[] {
  const eventByDiskId = new Map(
    event.disks.map((disk) => [
      diskIdentity(disk),
      normalizeDiskProgress({
        ...disk,
        last_event_type: event.event_type,
        last_event_time: event.event_time,
      }),
    ]),
  );
  const merged = currentDisks.map((disk) => eventByDiskId.get(diskIdentity(disk)) ?? disk);
  const knownDiskIds = new Set(currentDisks.map(diskIdentity));
  for (const disk of eventByDiskId.values()) {
    if (!knownDiskIds.has(diskIdentity(disk))) merged.push(disk);
  }
  return merged;
}

function diskIdentity(disk: EdgeDiskProgress): string {
  return disk.disk_id || disk.mount_path || disk.disk_sn;
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
