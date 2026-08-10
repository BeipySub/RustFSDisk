import type {
  ReadinessState,
  RuntimeState,
  SnapshotViewState,
  ViewFreshness,
} from '#/api/local-views';

export type SemanticTone =
  | 'danger'
  | 'denied'
  | 'muted'
  | 'running'
  | 'success'
  | 'warning';

export function freshnessLabel(value: ViewFreshness) {
  return {
    FAILED_RETAINED: '采集失败 · 保留最近完整值',
    FRESH: '数据最新',
    STALE: '数据已过期',
    UPDATING: '正在更新 · 当前显示旧值',
  }[value];
}

export function freshnessTone(value: ViewFreshness): SemanticTone {
  return {
    FAILED_RETAINED: 'danger',
    FRESH: 'success',
    STALE: 'warning',
    UPDATING: 'running',
  }[value] as SemanticTone;
}

export function readinessTone(value: ReadinessState): SemanticTone {
  return {
    ERROR: 'danger',
    PERMISSION_DENIED: 'denied',
    READY: 'success',
    UNKNOWN: 'muted',
    WARNING: 'warning',
  }[value] as SemanticTone;
}

export function runtimeTone(value: RuntimeState): SemanticTone {
  return {
    COMPLETED: 'success',
    IDLE: 'muted',
    LOADING: 'running',
    NO_MEDIA: 'muted',
    PAUSED: 'warning',
    PERMISSION_DENIED: 'denied',
    RISK_LOCKED: 'danger',
    RUNNING: 'running',
  }[value] as SemanticTone;
}

export function snapshotLabel(value: SnapshotViewState) {
  return {
    COLLECTION_FAILED: '采集失败',
    LATEST: '最新',
    PARTIAL: '部分快照',
    STALE: '过期',
    UPDATING: '正在更新',
  }[value];
}

export function snapshotTone(value: SnapshotViewState): SemanticTone {
  return {
    COLLECTION_FAILED: 'danger',
    LATEST: 'success',
    PARTIAL: 'warning',
    STALE: 'warning',
    UPDATING: 'running',
  }[value] as SemanticTone;
}

export function formatCount(value: null | number | undefined) {
  return value === null || value === undefined
    ? '未知'
    : new Intl.NumberFormat('zh-CN').format(value);
}

export function formatBytes(value: null | number | undefined) {
  if (value === null || value === undefined) return '未知';
  if (value === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const exponent = Math.min(
    Math.floor(Math.log(value) / Math.log(1000)),
    units.length - 1,
  );
  return `${(value / 1000 ** exponent).toFixed(exponent >= 4 ? 1 : 2)} ${units[exponent]}`;
}

export function formatEta(seconds: null | number | undefined) {
  if (seconds === null || seconds === undefined) return '未知';
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  return [hours, minutes, rest]
    .map((value) => String(value).padStart(2, '0'))
    .join(':');
}

export function formatTimestamp(value: null | string | undefined) {
  if (!value) return '未知';
  const parsed = new Date(value);
  return Number.isNaN(parsed.valueOf())
    ? value
    : new Intl.DateTimeFormat('zh-CN', {
        day: '2-digit',
        hour: '2-digit',
        hour12: false,
        minute: '2-digit',
        month: '2-digit',
        timeZone: 'Asia/Shanghai',
      }).format(parsed);
}
