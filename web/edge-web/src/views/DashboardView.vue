<script setup lang="ts">
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  diskStatusDisplay,
  fetchEdgeDashboardSummary,
  fetchEdgeReadiness,
  isActiveExportJobStatus,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
  type EdgeReadiness,
} from "../api/edgeDashboard";
import {
  applyCopyProgressEvent,
  connectEdgeProgressSocket,
  type CopyProgressEvent,
  type EdgeProgressSocket,
} from "../ws/edgeCopyProgress";

type EdgeRoute = "/dashboard" | "/sync-records";

function emptySummary(): EdgeDashboardSummary {
  const now = new Date().toISOString();
  return {
    source: "edge",
    edge_code: "",
    edge_name: "Edge",
    edge_status: undefined,
    export_job_id: "",
    export_job_status: "PENDING",
    disk_status_code: undefined,
    scan: {
      scan_event_type: "SCAN_PROGRESS",
      scanned_bucket_count: 0,
      scanned_object_count: 0,
      scanned_bytes: 0,
      stable_object_count: 0,
      skipped_object_count: 0,
      current_bucket: "",
      current_key: "",
      last_scan_at: now,
      message: "等待 Edge 后端返回扫描状态",
    },
    global_progress: {
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
    },
    disks: [],
    ws_connected: false,
    last_http_refresh_at: now,
    message: "等待 Edge Dashboard 只读接口",
  };
}

const summary = ref<EdgeDashboardSummary | null>(null);
const readiness = ref<EdgeReadiness | null>(null);
const readyError = ref<DashboardHttpError | null>(null);
const selectedDiskId = ref("");
const isRefreshing = ref(false);
const httpError = ref<DashboardHttpError | null>(null);
const wsConnected = ref(false);
const wsMessage = ref("WebSocket 尚未连接");
const navigate = inject<(path: EdgeRoute) => void>("edgeNavigate");
let progressSocket: EdgeProgressSocket | null = null;
let pendingProgressEvent: CopyProgressEvent | null = null;
const TRANSPORT_SLOT_COUNT = 9;

const viewSummary = computed(() => summary.value ?? emptySummary());
const disks = computed(() => viewSummary.value.disks);
const transportSlots = computed(() =>
  Array.from({ length: TRANSPORT_SLOT_COUNT }, (_, index) => ({
    slotNumber: index + 1,
    disk: disks.value[index] ?? null,
  })),
);
const selectedDisk = computed(() => disks.value.find((disk) => disk.disk_id === selectedDiskId.value) ?? null);
const showParticleStream = computed(() => disks.value.some((disk) => disk.runtime_status === "COPYING"));
const globalProgressPercent = computed(() =>
  progressPercent(viewSummary.value.global_progress.done_bytes, viewSummary.value.global_progress.total_bytes),
);
const runningDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "COPYING").length);
const readyDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "READY" || disk.runtime_status === "DONE").length);
const removedDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "REMOVED").length);
const rejectedDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "REJECTED").length);
const errorDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "ERROR").length);
const attentionDisks = computed(() => rejectedDisks.value + errorDisks.value);
const otherWarningDisks = computed(() => disks.value.filter((disk) => disk.last_error_code === "INSUFFICIENT_SPACE").length);
const selectedProgressPercent = computed(() =>
  progressPercent(selectedDisk.value?.done_bytes ?? 0, selectedDisk.value?.total_bytes ?? 0),
);
const selectedDiskIndex = computed(() => (selectedDisk.value ? disks.value.findIndex((disk) => disk.disk_id === selectedDisk.value?.disk_id) : -1));
const selectedSlotLabel = computed(() => (selectedDiskIndex.value >= 0 ? String(selectedDiskIndex.value + 1).padStart(2, "0") : "--"));
const selectedDiskTitle = computed(() =>
  selectedDisk.value ? `选中磁盘详情（盘位 ${selectedSlotLabel.value}）` : "选中磁盘详情（未选中）",
);
const selectedDiskFreeLabel = computed(() => {
  const disk = selectedDisk.value;
  if (!disk) return "未返回";
  const freePercent = progressPercent(disk.free_bytes, disk.total_bytes);
  return `${formatBytes(disk.free_bytes)} (${freePercent.toFixed(2)}%)`;
});
const currentObjectProgressPercent = computed(() =>
  progressPercent(selectedDisk.value?.current_object?.done_bytes ?? 0, selectedDisk.value?.current_object?.size_bytes ?? 0),
);
const exportedObjectPercent = computed(() =>
  progressPercent(
    viewSummary.value.global_progress.object_done,
    viewSummary.value.scan.scanned_object_count || viewSummary.value.global_progress.object_total,
  ),
);
const skippedObjectPercent = computed(() =>
  progressPercent(viewSummary.value.scan.skipped_object_count, viewSummary.value.scan.scanned_object_count),
);
const selectedDiskShortName = computed(() => {
  const disk = selectedDisk.value;
  if (!disk) return "未选中";
  const serial = disk.disk_sn ? `SN...${disk.disk_sn.slice(-4)}` : "SN 未返回";
  return `盘位 ${selectedSlotLabel.value}（${serial}）`;
});
const showRecoveryCheckAction = computed(() => {
  const disk = selectedDisk.value;
  if (!disk) return false;
  return (
    disk.runtime_status === "ERROR" ||
    disk.runtime_status === "REJECTED" ||
    disk.runtime_status === "REMOVED" ||
    Boolean(disk.last_error_code || disk.error_message)
  );
});
const lastHeartbeat = computed(() => (wsConnected.value ? "实时" : "等待连接"));
const lastUpdate = computed(() => formatClock(viewSummary.value.last_http_refresh_at || new Date().toISOString()));
const estimatedDone = computed(() => {
  const speed = viewSummary.value.global_progress.speed_bytes_per_sec;
  if (speed <= 0) return "等待速度";
  return formatDuration(Math.ceil(viewSummary.value.global_progress.remaining_bytes / speed));
});
const readyLabel = computed(() => {
  if (readiness.value?.ok) return "可用";
  if (readyError.value) return dashboardStatusLabel(readyError.value.error_code);
  if (readiness.value) return "不可用";
  return "检查中";
});
const httpLabel = computed(() => {
  if (httpError.value) return dashboardStatusLabel(httpError.value.error_code);
  if (isRefreshing.value) return "加载中";
  return "已连接";
});
const isEmpty = computed(() => !isRefreshing.value && !httpError.value && disks.value.length === 0);
const edgeDisplayName = computed(() => viewSummary.value.edge_name || viewSummary.value.edge_code || "Edge 本地节点");
const hasCurrentExport = computed(
  () =>
    isActiveExportJobStatus(viewSummary.value.export_job_status) &&
    Boolean(viewSummary.value.export_job_id || viewSummary.value.global_progress.total_bytes > 0),
);
const exportStatusTitle = computed(() => {
  if (httpError.value) return "只读接口不可用";
  if (hasCurrentExport.value) return "当前导出进度";
  return "当前无导出任务";
});
const exportStatusNotice = computed(() => {
  if (httpError.value) return `Edge Dashboard 只读接口不可用：${httpError.value.error_code}。当前不展示模拟数据。`;
  if (hasCurrentExport.value) return `WS：${wsMessage.value}`;
  if (isEmpty.value) return "未检测到运输盘；插入未注册盘或异常盘后会显示在盘位区。";
  return "运输盘已被探测，但当前没有运行中的导出任务。";
});

watch(
  () => disks.value.map((disk) => disk.disk_id).join("|"),
  () => {
    if (!disks.value.some((disk) => disk.disk_id === selectedDiskId.value)) {
      selectedDiskId.value = "";
    }
  },
  { immediate: true },
);

async function refreshFromHttpSummary() {
  isRefreshing.value = true;
  httpError.value = null;
  readyError.value = null;
  try {
    const [readyResult, summaryResult] = await Promise.allSettled([
      fetchEdgeReadiness(),
      fetchEdgeDashboardSummary(),
    ]);

    if (readyResult.status === "fulfilled") {
      readiness.value = readyResult.value;
    } else {
      readyError.value =
        readyResult.reason instanceof DashboardHttpError
          ? readyResult.reason
          : new DashboardHttpError("EDGE_READY_UNAVAILABLE", "Edge readiness unavailable");
    }

    if (summaryResult.status === "fulfilled") {
      const nextSummary = pendingProgressEvent
        ? applyCopyProgressEvent(summaryResult.value, pendingProgressEvent)
        : summaryResult.value;
      summary.value = nextSummary;
      publishEdgeIdentity(nextSummary);
      return;
    }

    summary.value = null;
    throw summaryResult.reason;
  } catch (error) {
    httpError.value =
      error instanceof DashboardHttpError
        ? error
        : new DashboardHttpError("DASHBOARD_UNAVAILABLE", error instanceof Error ? error.message : "HTTP summary unavailable");
  } finally {
    isRefreshing.value = false;
  }
}

function selectDisk(disk: EdgeDiskProgress) {
  selectedDiskId.value = disk.disk_id;
}

function openSyncRecords() {
  if (navigate) {
    navigate("/sync-records");
    return;
  }
  window.history.pushState({}, "", "/sync-records");
}

function publishEdgeIdentity(nextSummary: EdgeDashboardSummary) {
  window.dispatchEvent(
    new CustomEvent("edge-dashboard:identity", {
      detail: {
        edge_name: nextSummary.edge_name,
        edge_code: nextSummary.edge_code,
      },
    }),
  );
}

function diskTone(disk: EdgeDiskProgress): string {
  if (disk.runtime_status === "COPYING") return "running";
  if (disk.runtime_status === "READY" || disk.runtime_status === "DONE") return "success";
  if (disk.runtime_status === "REJECTED") return "warning";
  if (disk.runtime_status === "REMOVED") return "removed";
  if (disk.runtime_status === "ERROR") return "danger";
  return "muted";
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1000)), units.length - 1);
  return `${(bytes / 1000 ** unitIndex).toFixed(unitIndex >= 4 ? 1 : 2)} ${units[unitIndex]}`;
}

function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

function progressPercent(doneBytes: number, totalBytes: number): number {
  if (totalBytes <= 0) return 0;
  return Math.min(100, Math.max(0, (doneBytes / totalBytes) * 100));
}

function dashboardStatusLabel(value: string): string {
  const labels: Record<string, string> = {
    DASHBOARD_UNAVAILABLE: "接口不可用",
    DASHBOARD_ENDPOINT_NOT_READY: "接口未就绪",
    EDGE_READY_UNAVAILABLE: "自检不可用",
    NETWORK_ERROR: "网络异常",
  };
  return labels[value] ?? "异常";
}

function formatPercent(doneBytes: number, totalBytes: number): string {
  return `${progressPercent(doneBytes, totalBytes).toFixed(1)}%`;
}

function formatClock(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "--:--:--";
  return new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(date);
}

function formatDuration(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

onMounted(() => {
  void refreshFromHttpSummary();
  progressSocket = connectEdgeProgressSocket({
    onEvent(event) {
      pendingProgressEvent = event;
      if (summary.value) {
        summary.value = applyCopyProgressEvent(summary.value, event);
      }
      wsMessage.value = event.message || event.event_type;
      wsConnected.value = true;
    },
    onConnectionChange(connected, message) {
      wsConnected.value = connected;
      wsMessage.value = message;
      if (summary.value) summary.value = { ...summary.value, ws_connected: connected };
    },
  });
});

onBeforeUnmount(() => progressSocket?.close());
</script>

<template>
  <main class="dashboard page-panel">
    <section class="top-telemetry" aria-label="Edge 连接状态">
      <span :class="['status-pill', httpError ? 'warning' : 'ok']"><i></i> HTTP：{{ httpLabel }}</span>
      <span :class="['status-pill', readiness?.ok ? 'ok' : 'quiet']"><i></i> 本机服务：{{ readyLabel }}</span>
      <span :class="['status-pill', wsConnected ? 'live' : 'quiet']"><i></i> WebSocket：{{ wsConnected ? "已连接" : "重连中" }}</span>
      <span class="status-pill quiet">最后心跳：{{ lastHeartbeat }}</span>
      <span class="last-update">最后更新 {{ lastUpdate }}</span>
      <button aria-label="刷新 Dashboard" class="icon-refresh" :disabled="isRefreshing" type="button" @click="refreshFromHttpSummary">↻</button>
    </section>

    <section class="runtime-stage" aria-label="Edge 导出运行态">
      <div class="source-meta">
        <strong>源服务器（RustFS）<i></i> Edge</strong>
        <button class="records-shortcut" type="button" @click="openSyncRecords">同步记录</button>
      </div>
      <img alt="" class="source-rack" src="/assets/fustfs-baseline/source-rack-cutout-v3.webp" />
      <div v-if="showParticleStream" class="particle-field" aria-hidden="true">
        <i
          v-for="index in 110"
          :key="index"
          :style="{
            '--delay': `${(index % 23) * -0.16}s`,
            '--top': `${10 + ((index * 17) % 70)}%`,
            '--drift': `${((index % 11) - 5) * 7}px`,
          }"
        ></i>
      </div>
      <div class="transport-array">
        <header>
          <strong>运输盘位 · {{ disks.length }} 盘位</strong>
          <small><i></i> Edge 后端盘位状态</small>
        </header>
        <div class="nas-shell">
          <img alt="" src="/assets/fustfs-baseline/transport-bay-inner-black-clean-alpha.png" />
          <div class="disk-slot-matrix" aria-label="运输盘位列表">
            <template v-for="slot in transportSlots" :key="slot.slotNumber">
              <button
                v-if="slot.disk"
                :aria-pressed="selectedDisk?.disk_id === slot.disk.disk_id"
                :class="[`slot-${diskTone(slot.disk)}`, { selected: selectedDisk?.disk_id === slot.disk.disk_id }]"
                type="button"
                @click="selectDisk(slot.disk)"
              >
                <b>{{ String(slot.slotNumber).padStart(2, "0") }}</b>
                <strong v-if="slot.disk.runtime_status === 'COPYING'">{{ formatPercent(slot.disk.done_bytes, slot.disk.total_bytes) }}</strong>
                <strong v-else>{{ slot.disk.message }}</strong>
                <span>{{ slot.disk.runtime_status === "REMOVED" ? "已移除" : slot.disk.last_error_code ?? formatBytes(slot.disk.done_bytes) }}</span>
                <small>{{ slot.disk.runtime_status === "COPYING" ? formatBytes(slot.disk.total_bytes) : slot.disk.error_message ?? formatBytes(slot.disk.free_bytes) }}</small>
              </button>
              <div v-else class="disk-slot-cell empty" :aria-label="`empty transport slot ${slot.slotNumber}`">
                <b>{{ String(slot.slotNumber).padStart(2, "0") }}</b>
              </div>
            </template>
          </div>
        </div>
      </div>
    </section>

    <section v-if="false" :class="['global-progress', 'glass-panel', { idle: !hasCurrentExport }]">
      <div>
        <span>{{ exportStatusTitle }}</span>
        <strong v-if="hasCurrentExport">{{ globalProgressPercent.toFixed(0) }}<small>%</small></strong>
        <strong v-else>--<small></small></strong>
        <em>{{ hasCurrentExport ? viewSummary.export_job_status : "IDLE" }}</em>
      </div>
      <div v-if="hasCurrentExport" class="progress-main">
        <p>
          <span>已完成 <b>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</b> / {{ formatBytes(viewSummary.global_progress.total_bytes) }}</span>
          <span>剩余 <b>{{ formatBytes(viewSummary.global_progress.remaining_bytes) }}</b></span>
          <span>速度 <b>{{ formatSpeed(viewSummary.global_progress.speed_bytes_per_sec) }}</b></span>
        </p>
        <div class="progress-track"><b :style="{ width: `${globalProgressPercent}%` }"></b></div>
        <dl>
          <div><dt>文件</dt><dd>{{ viewSummary.global_progress.object_total.toLocaleString() }}</dd></div>
          <div><dt>对象</dt><dd>{{ viewSummary.global_progress.object_done.toLocaleString() }}</dd></div>
          <div><dt>批次</dt><dd>{{ viewSummary.export_job_id || "暂无" }}</dd></div>
          <div><dt>预计完成</dt><dd>{{ estimatedDone }}</dd></div>
        </dl>
      </div>
      <div v-else class="progress-main idle-copy">
        <p>{{ exportStatusNotice }}</p>
        <dl>
          <div><dt>现场盘位</dt><dd>{{ disks.length }} 盘位</dd></div>
          <div><dt>未注册盘</dt><dd>会展示</dd></div>
          <div><dt>历史批次</dt><dd>同步记录</dd></div>
          <div><dt>浏览器权限</dt><dd>只读</dd></div>
        </dl>
      </div>
    </section>

    <section
      v-if="selectedDisk"
      :class="['selected-disk-strip', 'glass-panel', { 'has-action': showRecoveryCheckAction }]"
      aria-label="选中磁盘详情"
    >
      <div class="selected-disk-content">
        <strong>{{ selectedDiskTitle }}</strong>
        <dl>
          <div><dt>disk_id</dt><dd>{{ selectedDisk?.disk_id ?? "未返回" }}</dd></div>
          <div><dt>mount_path</dt><dd>{{ selectedDisk?.mount_path ?? "未返回" }}</dd></div>
          <div><dt>disk_sn</dt><dd>{{ selectedDisk?.disk_sn ?? "未返回" }}</dd></div>
          <div><dt>filesystem</dt><dd>{{ selectedDisk?.filesystem ?? "未返回" }}</dd></div>
          <div><dt>runtime_status</dt><dd class="tone-running">{{ selectedDisk?.runtime_status ?? "未返回" }}</dd></div>
          <div><dt>disk_status_code</dt><dd class="tone-running">{{ diskStatusDisplay(selectedDisk?.disk_status_code) }}</dd></div>
          <div><dt>object_budget_bytes</dt><dd>{{ formatBytes(selectedDisk?.total_bytes ?? 0) }}</dd></div>
          <div><dt>free_bytes</dt><dd>{{ selectedDiskFreeLabel }}</dd></div>
        </dl>
      </div>
      <button v-if="showRecoveryCheckAction" class="readonly-action" type="button" disabled title="恢复检查由 Edge 后端受控执行，浏览器不直接调用">
        执行恢复检查
      </button>
    </section>

    <section v-if="hasCurrentExport" class="global-progress glass-panel" aria-label="导出任务总进度">
      <div class="progress-title">
        <span>导出任务总进度</span>
        <strong v-if="hasCurrentExport">{{ globalProgressPercent.toFixed(0) }}<small>%</small></strong>
        <em>所有运行中任务汇总</em>
      </div>
      <div class="progress-main">
        <p>
          <span>已完成 <b>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</b> / {{ formatBytes(viewSummary.global_progress.total_bytes) }}</span>
          <span>剩余 <b>{{ formatBytes(viewSummary.global_progress.remaining_bytes) }}</b></span>
          <span>速度 <b>{{ formatSpeed(viewSummary.global_progress.speed_bytes_per_sec) }}</b></span>
        </p>
        <div class="progress-track"><b :style="{ width: `${globalProgressPercent}%` }"></b></div>
        <dl>
          <div><dt>扫描字节</dt><dd>{{ formatBytes(viewSummary.scan.scanned_bytes) }} / {{ formatBytes(viewSummary.global_progress.total_bytes) }}</dd></div>
          <div><dt>跳过对象</dt><dd>{{ viewSummary.scan.skipped_object_count.toLocaleString() }}（{{ skippedObjectPercent.toFixed(2) }}%）</dd></div>
          <div><dt>批次</dt><dd>{{ viewSummary.export_job_id || "暂无" }}</dd></div>
          <div><dt>预计完成</dt><dd>{{ estimatedDone }}</dd></div>
        </dl>
      </div>
    </section>

    <section :class="['dashboard-lower-grid', { compact: !hasCurrentExport }]">
      <article class="overview-panel glass-panel">
        <h2>扫描与导出概览</h2>
        <dl class="overview-metrics">
          <div><dt>已发现对象</dt><dd>{{ viewSummary.scan.scanned_object_count.toLocaleString() }}</dd></div>
          <div><dt>已导出对象</dt><dd>{{ viewSummary.global_progress.object_done.toLocaleString() }}</dd></div>
          <div><dt>导出进度</dt><dd>{{ exportedObjectPercent.toFixed(2) }}%</dd></div>
          <div><dt>扫描字节</dt><dd>{{ formatBytes(viewSummary.scan.scanned_bytes) }}</dd></div>
          <div><dt>导出字节</dt><dd>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</dd></div>
          <div><dt>跳过对象</dt><dd>{{ viewSummary.scan.skipped_object_count.toLocaleString() }}（{{ skippedObjectPercent.toFixed(2) }}%）</dd></div>
        </dl>
      </article>

      <article class="object-panel object-wide-panel glass-panel">
        <h2>当前对象与异常处理</h2>
        <div v-if="selectedDisk" class="object-detail-grid">
          <dl>
            <dt>对象路径</dt><dd>{{ selectedDisk.current_object?.key ?? (viewSummary.scan.current_key || "暂无") }}</dd>
            <dt>对象状态</dt><dd class="tone-running">{{ selectedDisk.current_object?.object_status ?? "等待对象" }}</dd>
            <dt>剩余大小</dt><dd>{{ formatBytes(selectedDisk.current_object?.remaining_bytes ?? 0) }} / {{ formatBytes(selectedDisk.current_object?.size_bytes ?? 0) }}</dd>
            <dt>传输速度</dt><dd>{{ formatSpeed(selectedDisk.speed_bytes_per_sec ?? 0) }}</dd>
          </dl>
          <dl>
            <dt>文件系统</dt><dd>{{ selectedDiskShortName }}</dd>
            <dt>对象标识</dt><dd>{{ selectedDisk.current_object?.key ?? "未返回" }}</dd>
            <dt>加密状态</dt><dd>{{ selectedDisk.current_object ? "已加密" : "未返回" }}</dd>
            <dt>写入阶段</dt><dd>{{ selectedDisk.runtime_status }}</dd>
            <dt>校验状态</dt><dd>{{ selectedDisk.disk_status_code ? diskStatusDisplay(selectedDisk.disk_status_code) : "未返回" }}</dd>
          </dl>
        </div>
        <p v-else class="object-empty">
          未选中运输盘。未注册或异常盘只有在 Edge 后端检测并返回后才会显示。
        </p>
        <div class="object-progress-row">
          <div class="progress-track object-progress"><b :style="{ width: `${currentObjectProgressPercent}%` }"></b></div>
          <span>{{ currentObjectProgressPercent.toFixed(2) }}%</span>
        </div>
        <h3>异常汇总（基于当前导出任务）</h3>
        <div class="warning-cards alert-cards">
          <span><b>{{ attentionDisks }}</b><em>需恢复</em><small>硬盘</small></span>
          <span><b>{{ removedDisks }}</b><em>已移除</em><small>硬盘</small></span>
          <span class="danger"><b>{{ rejectedDisks }}</b><em>被拒绝</em><small>硬盘</small></span>
          <span class="danger"><b>{{ errorDisks }}</b><em>错误</em><small>硬盘</small></span>
          <span><b>{{ otherWarningDisks }}</b><em>其他告警</em><small>硬盘</small></span>
        </div>
      </article>
    </section>

    <section v-if="false" class="dashboard-grid">
      <article class="overview-panel glass-panel">
        <h2>扫描与导出概览</h2>
        <dl class="metric-strip">
          <div><dt>扫描字节</dt><dd>{{ formatBytes(viewSummary.scan.scanned_bytes) }}</dd><small>{{ viewSummary.scan.scan_event_type }}</small></div>
          <div><dt>已发现对象</dt><dd>{{ viewSummary.scan.scanned_object_count.toLocaleString() }}</dd><small>总计对象</small></div>
          <div><dt>已导出对象</dt><dd>{{ viewSummary.global_progress.object_done.toLocaleString() }}</dd><small>{{ globalProgressPercent.toFixed(2) }}%</small></div>
          <div><dt>预计完成</dt><dd>{{ estimatedDone }}</dd><small>剩余时间</small></div>
        </dl>
        <h2>只读接口边界</h2>
        <ul class="check-list">
          <li>浏览器仅请求本机 Edge Dashboard API</li>
          <li>浏览器不携带控制 token</li>
          <li>WebSocket 只接收本端实时状态</li>
          <li>隐藏中控导入生命周期</li>
          <li>生产页面仅保留观察入口</li>
          <li>异常处理遵循 Edge 后端受保护流程</li>
        </ul>
      </article>

      <article class="warning-panel glass-panel">
        <h2>异常盘汇总</h2>
        <div class="warning-cards">
          <span><b>{{ attentionDisks }}</b>需关注</span>
          <span><b>{{ removedDisks }}</b>已移除</span>
          <span class="danger"><b>{{ rejectedDisks }}</b>被拒绝</span>
          <span class="danger"><b>{{ errorDisks }}</b>错误</span>
          <span><b>{{ otherWarningDisks }}</b>其他告警</span>
        </div>
        <h3>盘位运行状态（{{ disks.length }} 盘位）</h3>
        <div class="runtime-table">
          <button
            v-for="(disk, index) in disks.slice(0, 6)"
            :key="disk.disk_id"
            :class="{ selected: selectedDisk?.disk_id === disk.disk_id }"
            type="button"
            @click="selectDisk(disk)"
          >
            <span>{{ String(index + 1).padStart(2, "0") }}</span>
            <span>{{ disk.disk_sn }}</span>
            <span>{{ formatBytes(disk.total_bytes) }}</span>
            <strong :class="`tone-${diskTone(disk)}`">{{ disk.message }}</strong>
            <em>{{ disk.runtime_status }}</em>
          </button>
        </div>
      </article>

      <article class="object-panel glass-panel">
        <h2>当前对象（{{ selectedDisk?.message ?? "等待状态" }}）</h2>
        <dl v-if="selectedDisk">
          <dt>对象路径</dt><dd>{{ selectedDisk?.current_object?.key ?? (viewSummary.scan.current_key || "暂无") }}</dd>
          <dt>对象状态</dt><dd class="tone-running">{{ selectedDisk?.current_object?.object_status ?? "等待对象" }}</dd>
          <dt>剩余大小</dt><dd>{{ formatBytes(selectedDisk?.current_object?.remaining_bytes ?? 0) }}</dd>
          <dt>传输速度</dt><dd>{{ formatSpeed(selectedDisk?.speed_bytes_per_sec ?? 0) }}</dd>
          <dt>文件系统</dt><dd>{{ selectedDisk?.filesystem ?? "未返回" }}</dd>
          <dt>FS UUID</dt><dd>{{ selectedDisk?.filesystem_uuid ?? "未返回" }}</dd>
          <dt>硬件 SN</dt><dd>{{ selectedDisk?.disk_sn ?? "未返回" }}</dd>
          <dt>设备路径</dt><dd>{{ selectedDisk?.device_path ?? "未返回" }}</dd>
          <dt>盘内状态</dt><dd>{{ diskStatusDisplay(selectedDisk?.disk_status_code) }}</dd>
        </dl>
        <p v-else class="object-empty">
          未选中运输盘。插入后，未注册或异常盘也会显示在右侧盘位区。
        </p>
        <div v-if="selectedDisk" class="progress-track object-progress"><b :style="{ width: `${selectedProgressPercent}%` }"></b></div>
        <span v-if="selectedDisk" class="object-percent">{{ selectedProgressPercent.toFixed(2) }}%</span>
      </article>
    </section>

  </main>
</template>
