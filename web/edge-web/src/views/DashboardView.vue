<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  diskStatusDisplay,
  fetchEdgeDashboardSummary,
  fetchEdgeReadiness,
  isActiveExportJobStatus,
  triggerEdgeRustFsScan,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
  type EdgeReadiness,
} from "../api/edgeDashboard";
import {
  applyCopyProgressEvent,
  connectEdgeProgressSocket,
  type EdgeProgressSocket,
} from "../ws/edgeCopyProgress";
import EdgeTelemetry from "../components/EdgeTelemetry.vue";
import ParticleAetherField from "../components/ParticleAetherField.vue";

type ParticlePalette = "semantic" | "electric" | "cyan" | "emerald" | "amber" | "violet";
type ParticleSceneState = "loading" | "running" | "paused" | "complete" | "error";
type ParticlePathAnchors = {
  startX: number;
  startY: number;
  endX: number;
  endY: number;
};

const particleSceneLabels: Record<ParticleSceneState, string> = {
  loading: "汇聚装载",
  running: "传输",
  paused: "暂停",
  complete: "可运输",
  error: "异常",
};
const particleSceneStates: ParticleSceneState[] = ["loading", "running", "paused", "complete", "error"];

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
const selectedDiskDetailVisible = ref(false);
const isRefreshing = ref(false);
const httpError = ref<DashboardHttpError | null>(null);
const wsConnected = ref(false);
const wsMessage = ref("WebSocket 尚未连接");
const isScanRequested = ref(false);
const scanRequestLabel = ref("");
const showParticleDevPanel = import.meta.env.DEV;
const particlePanelOpen = ref(false);
const particleSceneState = ref<ParticleSceneState>("running");
const particleSamplePlaying = ref(false);
const particleSpeed = ref(1);
const particleGlow = ref(1);
const particlePalette = ref<ParticlePalette>("semantic");
const runtimeStageRef = ref<HTMLElement | null>(null);
const sourceRackRef = ref<HTMLElement | null>(null);
const nasShellRef = ref<HTMLElement | null>(null);
const particlePathAnchors = ref<ParticlePathAnchors>({
  startX: 0.28,
  startY: 0.5,
  endX: 0.52,
  endY: 0.58,
});
let progressSocket: EdgeProgressSocket | null = null;
let particleAnchorObserver: ResizeObserver | null = null;
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
const particleStreamActive = computed(() => showParticleStream.value || (showParticleDevPanel && particleSamplePlaying.value));
const particleCanvasActive = computed(() => showParticleStream.value || (showParticleDevPanel && particleSamplePlaying.value && particleSceneState.value === "running"));
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
const selectedCurrentObjectName = computed(() => {
  const currentObject = selectedDisk.value?.current_object;
  return currentObject?.display_name || currentObject?.key || viewSummary.value.scan.current_key || "暂无";
});
const selectedCurrentObjectSizeLabel = computed(() => {
  const currentObject = selectedDisk.value?.current_object;
  return currentObject ? `${formatBytes(currentObject.done_bytes)} / ${formatBytes(currentObject.size_bytes)}` : "0 B / 0 B";
});
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
const sealedJobReadyForPickup = computed(
  () =>
    !httpError.value &&
    viewSummary.value.export_job_status === "SEALED" &&
    viewSummary.value.disks.length === 0 &&
    Boolean(viewSummary.value.export_job_id || viewSummary.value.global_progress.done_bytes > 0),
);
const estimatedDone = computed(() => {
  const speed = viewSummary.value.global_progress.speed_bytes_per_sec;
  if (speed <= 0) return "等待速度";
  return formatDuration(Math.ceil(viewSummary.value.global_progress.remaining_bytes / speed));
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
  if (isRefreshing.value) return;
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
      summary.value = summaryResult.value;
      publishEdgeIdentity(summaryResult.value);
      return;
    }

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
  selectedDiskDetailVisible.value = true;
}

function clearSelectedDisk() {
  selectedDiskDetailVisible.value = false;
}

async function requestRustFsScan() {
  if (isScanRequested.value) return;
  isScanRequested.value = true;
  scanRequestLabel.value = "正在提交扫描";
  try {
    await triggerEdgeRustFsScan();
    scanRequestLabel.value = "扫描已提交";
    await refreshFromHttpSummary();
  } catch (error) {
    scanRequestLabel.value =
      error instanceof DashboardHttpError && error.http_status === 401
        ? "待后端鉴权联调"
        : "待后端接口联调";
  } finally {
    window.setTimeout(() => {
      isScanRequested.value = false;
      scanRequestLabel.value = "";
    }, 2400);
  }
}

function clampParticleAnchor(value: number): number {
  return Math.min(0.96, Math.max(0.04, value));
}

function updateParticlePathAnchors() {
  const stageRect = runtimeStageRef.value?.getBoundingClientRect();
  const sourceRect = sourceRackRef.value?.getBoundingClientRect();
  const nasRect = nasShellRef.value?.getBoundingClientRect();
  if (!stageRect || !sourceRect || !nasRect || stageRect.width <= 0 || stageRect.height <= 0) return;

  particlePathAnchors.value = {
    startX: clampParticleAnchor((sourceRect.left - stageRect.left + sourceRect.width * 0.9) / stageRect.width),
    startY: clampParticleAnchor((sourceRect.top - stageRect.top + sourceRect.height * 0.43) / stageRect.height),
    endX: clampParticleAnchor((nasRect.left - stageRect.left + nasRect.width * 0.045) / stageRect.width),
    endY: clampParticleAnchor((nasRect.top - stageRect.top + nasRect.height * 0.52) / stageRect.height),
  };
}

function resetParticleControls() {
  particleSceneState.value = "running";
  particleSamplePlaying.value = false;
  particleSpeed.value = 1;
  particleGlow.value = 1;
  particlePalette.value = "semantic";
}

function toggleParticleSample() {
  particleSamplePlaying.value = !particleSamplePlaying.value;
  if (particleSamplePlaying.value) particleSceneState.value = "loading";
}

function openSyncRecords() {
  window.history.pushState({}, "", "/sync-records");
  window.dispatchEvent(new PopStateEvent("popstate"));
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

function diskStatusLabel(disk: EdgeDiskProgress): string {
  if (disk.runtime_status === "COPYING") return "拷贝中";
  if (disk.runtime_status === "READY") return "就绪";
  if (disk.runtime_status === "DONE") return disk.disk_status_code === "SEALED" ? "已封盘" : "完成";
  if (disk.runtime_status === "REJECTED") return rejectedDiskStatusLabel(disk);
  if (disk.runtime_status === "REMOVED") return "已移除";
  if (disk.runtime_status === "ERROR") return "错误";
  if (disk.runtime_status === "CHECKING") return "校验中";
  if (disk.runtime_status === "DETECTED") return "已检测";
  return diskStatusDisplay(disk.disk_status_code);
}

function diskCardStatusLabel(disk: EdgeDiskProgress): string {
  if (disk.runtime_status === "REJECTED") return rejectedDiskStatusLabel(disk);
  return disk.disk_status_code ? diskStatusDisplay(disk.disk_status_code) : diskStatusLabel(disk);
}

function diskStatusDetail(disk: EdgeDiskProgress): string {
  if (disk.runtime_status === "REJECTED" || disk.runtime_status === "ERROR") {
    return diskIssueRawText(disk) ? "" : "等待后端错误详情";
  }
  if (disk.runtime_status === "REMOVED") return "等待重新插入";
  if (disk.runtime_status === "COPYING") return `${formatPercent(disk.done_bytes, disk.total_bytes)} 已写入`;
  return disk.message || diskStatusDisplay(disk.disk_status_code);
}

function diskStatusTooltip(disk: EdgeDiskProgress): string {
  if (disk.runtime_status !== "REJECTED" && disk.runtime_status !== "ERROR") return "";
  return translateDiskIssue(diskIssueRawText(disk));
}

function diskIssueRawText(disk: EdgeDiskProgress): string {
  return disk.error_message || disk.last_error_code || disk.message || "";
}

function rejectedDiskStatusLabel(disk: EdgeDiskProgress): string {
  const reason = diskIssueRawText(disk);
  if (isUninitializedDiskIssue(reason)) return "未初始化";
  if (isUnregisteredDiskIssue(reason) || disk.disk_status_code === "UNREGISTERED") return "未注册";
  if (isUnsupportedDiskIssue(reason)) return "不可导出";
  return "拒绝";
}

function translateDiskIssue(value: string): string {
  if (!value) return "后端暂未返回详细原因";
  if (
    value.includes("MANIFEST_INVALID") &&
    value.includes("status_code SEALED") &&
    value.includes("expected INITIALIZED")
  ) {
    return "盘内清单无效：当前盘已封盘，不能用于 Edge 离线导出；请在中控端完成导入并重新初始化后再使用。";
  }
  if (value.includes("MANIFEST_INVALID") && value.includes("expected INITIALIZED")) {
    return "盘内清单无效：当前盘内状态不符合 Edge 离线导出要求，需要先处于“已初始化”状态。";
  }
  if (value.includes("MANIFEST_INVALID")) {
    return `盘内清单无效：${value}`;
  }
  if (isUninitializedDiskIssue(value)) {
    return "未初始化：未检测到有效的盘内初始化信息，请先到中控端初始化后再用于 Edge 离线导出。";
  }
  if (isUnregisteredDiskIssue(value)) {
    return "未注册：需要先在中控端注册并初始化后再用于 Edge 离线导出。";
  }
  if (isUnsupportedDiskIssue(value)) {
    return "不可导出：当前磁盘不满足 Edge 离线导出要求，请按中控端初始化流程处理。";
  }
  return value;
}

function isUninitializedDiskIssue(value: string): boolean {
  return /MISSING_DISK_INFO|NO_DISK_INFO|disk_info|UNINITIALIZED|not initialized|expected INITIALIZED/i.test(value);
}

function isUnregisteredDiskIssue(value: string): boolean {
  return /UNREGISTERED|not registered|unregistered disk/i.test(value);
}

function isUnsupportedDiskIssue(value: string): boolean {
  return /FILESYSTEM_INVALID|UNSUPPORTED|non[-_ ]?protocol|not ext4|non[-_ ]?ext4|filesystem/i.test(value);
}

function diskLifecycleStatusLabel(disk: EdgeDiskProgress | null): string {
  if (!disk) return "未返回";
  if (disk.disk_status_code) return diskStatusDisplay(disk.disk_status_code);
  if (disk.runtime_status === "REJECTED") return rejectedDiskStatusLabel(disk);
  return "未返回";
}

function slotSnLabel(disk: EdgeDiskProgress): string {
  return displaySerial(disk.disk_sn || disk.hardware_serial || disk.id_serial || disk.stable_hardware_id);
}

function displaySerial(value: string | undefined): string {
  if (!value) return "未返回";
  return decodeHexAscii(value) ?? value;
}

function decodeHexAscii(value: string): string | undefined {
  if (!/^(?:[0-9a-fA-F]{2})+$/.test(value) || value.length < 8) return undefined;
  const chars: string[] = [];
  for (let index = 0; index < value.length; index += 2) {
    const code = Number.parseInt(value.slice(index, index + 2), 16);
    if (code < 32 || code > 126) return undefined;
    chars.push(String.fromCharCode(code));
  }
  const decoded = chars.join("").trim();
  return decoded || undefined;
}

function slotUsedBytes(disk: EdgeDiskProgress): number {
  const total = slotTotalBytes(disk);
  if (disk.done_bytes > 0) return Math.min(total, disk.done_bytes);
  if (total > 0 && disk.free_bytes > 0) return Math.max(0, total - disk.free_bytes);
  return 0;
}

function slotTotalBytes(disk: EdgeDiskProgress): number {
  return disk.total_bytes || disk.done_bytes + disk.remaining_bytes || disk.free_bytes || 0;
}

function slotProgressPercent(disk: EdgeDiskProgress): number {
  return progressPercent(slotUsedBytes(disk), slotTotalBytes(disk));
}

function formatTbValue(bytes: number): string {
  if (bytes <= 0) return "0.00";
  return (bytes / 1000 ** 4).toFixed(2);
}

function formatTbPair(disk: EdgeDiskProgress): string {
  return `${formatTbValue(slotUsedBytes(disk))}/${formatTbValue(slotTotalBytes(disk))} TB`;
}

function formatTbNumberPair(disk: EdgeDiskProgress): string {
  return `${formatTbValue(slotUsedBytes(disk))}/${formatTbValue(slotTotalBytes(disk))}`;
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
  requestAnimationFrame(updateParticlePathAnchors);
  window.setTimeout(updateParticlePathAnchors, 250);
  window.addEventListener("resize", updateParticlePathAnchors);
  if ("ResizeObserver" in window) {
    particleAnchorObserver = new ResizeObserver(updateParticlePathAnchors);
    for (const element of [runtimeStageRef.value, sourceRackRef.value, nasShellRef.value]) {
      if (element) particleAnchorObserver.observe(element);
    }
  }
  progressSocket = connectEdgeProgressSocket({
    onEvent(event) {
      summary.value = applyCopyProgressEvent(summary.value ?? emptySummary(), event);
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

onBeforeUnmount(() => {
  particleAnchorObserver?.disconnect();
  window.removeEventListener("resize", updateParticlePathAnchors);
  progressSocket?.close();
});
</script>

<template>
  <main class="dashboard page-panel">
    <EdgeTelemetry
      :http-tone="httpError ? 'warning' : 'ok'"
      :local-tone="readiness?.ok ? 'ok' : 'quiet'"
      :ws-tone="wsConnected ? 'ok' : 'quiet'"
      refresh-label="刷新 Dashboard"
      :refresh-disabled="isRefreshing"
      :scan-label="scanRequestLabel || '扫描 RustFS'"
      :scan-disabled="isScanRequested"
      @refresh="refreshFromHttpSummary"
      @scan="requestRustFsScan"
    />

    <section ref="runtimeStageRef" class="runtime-stage" aria-label="Edge 导出运行态">
      <button class="source-rack-link" type="button" aria-label="打开同步记录" title="同步记录" @click="openSyncRecords">
        <img ref="sourceRackRef" alt="" class="source-rack" src="/assets/fustfs-baseline/source-rack-cutout-v3.webp" @load="updateParticlePathAnchors" />
      </button>
      <ParticleAetherField
        v-if="particleStreamActive"
        :active="particleCanvasActive"
        :end-x="particlePathAnchors.endX"
        :end-y="particlePathAnchors.endY"
        :glow="particleGlow"
        :palette="particlePalette"
        :speed="particleSpeed"
        :start-x="particlePathAnchors.startX"
        :start-y="particlePathAnchors.startY"
      />
      <aside v-if="showParticleDevPanel" :class="['particle-controls', { 'is-open': particlePanelOpen }]" aria-label="粒子动画调节">
        <header class="particle-controls-head">
          <button class="particle-controls-toggle" type="button" :aria-expanded="particlePanelOpen" @click="particlePanelOpen = !particlePanelOpen">
            <span><i></i>粒子调节</span>
            <em>{{ particlePanelOpen ? "收起" : "展开" }}</em>
          </button>
          <button v-if="particlePanelOpen" class="particle-controls-reset" type="button" @click="resetParticleControls">
            恢复默认
          </button>
        </header>
        <div v-if="particlePanelOpen" class="particle-controls-body">
          <section class="particle-control-section motion-sample-section">
            <div class="motion-sample-heading">
              <span>
                <strong>A 首页业务样板</strong>
                <small>扫描 -> 汇聚装载 -> 传输 -> 校验 -> 可运输</small>
              </span>
              <button type="button" :class="{ 'is-playing': particleSamplePlaying }" :aria-pressed="particleSamplePlaying" @click="toggleParticleSample">
                {{ particleSamplePlaying ? "停止" : "播放" }}
              </button>
            </div>
            <div class="motion-sample-steps" aria-label="业务动效阶段">
              <button
                v-for="(state, index) in particleSceneStates"
                :key="state"
                type="button"
                :class="{ 'is-active': particleSceneState === state }"
                :aria-pressed="particleSceneState === state"
                @click="particleSceneState = state"
              >
                <i>{{ index + 1 }}</i>
                <span>{{ particleSceneLabels[state] }}</span>
              </button>
            </div>
          </section>
          <section class="particle-control-section">
            <span class="particle-control-label">状态</span>
            <div class="particle-state-buttons">
              <button
                v-for="state in particleSceneStates"
                :key="state"
                type="button"
                :class="{ 'is-active': particleSceneState === state }"
                :aria-pressed="particleSceneState === state"
                @click="particleSceneState = state"
              >
                {{ particleSceneLabels[state] }}
              </button>
            </div>
          </section>
          <section class="particle-control-section particle-range-control">
            <label for="edge-particle-glow"><span>蓝色外发光</span><output>{{ Math.round(particleGlow * 100) }}%</output></label>
            <input id="edge-particle-glow" v-model.number="particleGlow" type="range" min="0.25" max="2" step="0.05" />
          </section>
          <section class="particle-control-section particle-range-control">
            <label for="edge-particle-speed"><span>传输速度</span><output>{{ particleSpeed.toFixed(2) }}x</output></label>
            <input id="edge-particle-speed" v-model.number="particleSpeed" type="range" min="0.25" max="2.5" step="0.05" />
          </section>
          <section class="particle-control-section">
            <span class="particle-control-label">颜色</span>
            <div class="particle-palette-buttons">
              <button type="button" :class="{ 'is-active': particlePalette === 'semantic' }" @click="particlePalette = 'semantic'"><i></i>语义</button>
              <button type="button" :class="{ 'is-active': particlePalette === 'electric' }" @click="particlePalette = 'electric'"><i></i>电蓝</button>
              <button type="button" :class="{ 'is-active': particlePalette === 'cyan' }" @click="particlePalette = 'cyan'"><i></i>青蓝</button>
              <button type="button" :class="{ 'is-active': particlePalette === 'emerald' }" @click="particlePalette = 'emerald'"><i></i>翠绿</button>
              <button type="button" :class="{ 'is-active': particlePalette === 'amber' }" @click="particlePalette = 'amber'"><i></i>琥珀</button>
              <button type="button" :class="{ 'is-active': particlePalette === 'violet' }" @click="particlePalette = 'violet'"><i></i>紫光</button>
            </div>
          </section>
        </div>
      </aside>
      <div class="transport-array">
        <div ref="nasShellRef" class="nas-shell">
          <img alt="" src="/assets/fustfs-baseline/transport-bay-inner-black-clean-alpha.png" @load="updateParticlePathAnchors" />
          <div class="disk-slot-matrix" aria-label="运输盘位列表">
            <template v-for="slot in transportSlots" :key="slot.disk ? `${slot.slotNumber}-${slot.disk.disk_id}` : `empty-${slot.slotNumber}`">
              <button
                v-if="slot.disk"
                :aria-pressed="selectedDisk?.disk_id === slot.disk.disk_id"
                :class="[`slot-${diskTone(slot.disk)}`, { selected: selectedDisk?.disk_id === slot.disk.disk_id }]"
                type="button"
                @click="selectDisk(slot.disk)"
              >
                <b>{{ String(slot.slotNumber).padStart(2, "0") }}</b>
                <strong class="slot-status" :title="diskStatusTooltip(slot.disk)">
                  {{ diskCardStatusLabel(slot.disk) }}
                </strong>
                <span class="slot-sn">SN: {{ slotSnLabel(slot.disk) }}</span>
                <small>{{ formatTbNumberPair(slot.disk) }} <span class="slot-unit">TB</span></small>
                <i class="slot-progress" aria-hidden="true">
                  <span :style="{ width: `${slotProgressPercent(slot.disk)}%` }"></span>
                </i>
              </button>
              <div v-else class="disk-slot-cell empty" :aria-label="`empty transport slot ${slot.slotNumber}`"></div>
            </template>
          </div>
        </div>
      </div>
    </section>

    <section v-if="!httpError" :class="['global-progress', 'glass-panel', { idle: !hasCurrentExport }]">
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

    <section v-if="httpError" class="selected-disk-strip glass-panel error-state" aria-label="Edge Dashboard 错误态">
      <div class="selected-disk-content">
        <strong>Edge Dashboard 只读接口不可用</strong>
        <dl>
          <div><dt>错误码</dt><dd class="tone-running">{{ httpError.error_code }}</dd></div>
          <div><dt>HTTP</dt><dd>{{ httpError.http_status ?? "未返回" }}</dd></div>
          <div><dt>展示策略</dt><dd>不展示模拟进度、模拟盘位或模拟对象</dd></div>
          <div><dt>WebSocket</dt><dd>{{ wsMessage }}</dd></div>
        </dl>
      </div>
    </section>

    <section v-else-if="sealedJobReadyForPickup" class="selected-disk-strip glass-panel" aria-label="封盘完成可拔盘">
      <div class="selected-disk-content">
        <strong>最近导出任务已封盘，可拔盘</strong>
        <dl>
          <div><dt>导出任务</dt><dd>{{ viewSummary.export_job_id || "未返回" }}</dd></div>
          <div><dt>export_job_status</dt><dd class="tone-running">SEALED</dd></div>
          <div><dt>盘内状态</dt><dd class="tone-running">已封盘</dd></div>
          <div><dt>已导出</dt><dd>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</dd></div>
        </dl>
      </div>
    </section>

    <section v-else-if="selectedDisk && selectedDiskDetailVisible" class="selected-disk-strip glass-panel" aria-label="选中磁盘详情">
      <div class="selected-disk-content">
        <strong>{{ selectedDiskTitle }}</strong>
        <dl>
          <div><dt>磁盘 ID</dt><dd>{{ selectedDisk?.disk_id ?? "未返回" }}</dd></div>
          <div><dt>挂载路径</dt><dd>{{ selectedDisk?.mount_path ?? "未返回" }}</dd></div>
          <div><dt>硬盘 SN</dt><dd>{{ selectedDisk ? slotSnLabel(selectedDisk) : "未返回" }}</dd></div>
          <div><dt>系统格式</dt><dd>{{ selectedDisk?.filesystem ?? selectedDisk?.filesystem_uuid ?? "未返回" }}</dd></div>
          <div><dt>运行状态</dt><dd class="tone-running">{{ selectedDisk ? diskStatusLabel(selectedDisk) : "未返回" }}</dd></div>
          <div><dt>盘内状态</dt><dd class="tone-running">{{ diskLifecycleStatusLabel(selectedDisk) }}</dd></div>
          <div><dt>拷贝进度</dt><dd>{{ selectedProgressPercent.toFixed(2) }}%</dd></div>
          <div><dt>当前文件</dt><dd :title="selectedCurrentObjectName">{{ selectedCurrentObjectName }}</dd></div>
          <div><dt>容量</dt><dd>{{ selectedDisk ? formatTbPair(selectedDisk) : "0.00/0.00 TB" }}</dd></div>
        </dl>
      </div>
      <button
        class="selected-disk-close"
        type="button"
        aria-label="关闭选中磁盘详情"
        title="关闭"
        @pointerdown.stop
        @click.stop="clearSelectedDisk"
      >
        ×
      </button>
    </section>

    <section v-if="false" class="global-progress glass-panel" aria-label="导出任务总进度">
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

    <section v-if="!httpError" :class="['dashboard-lower-grid', { compact: !hasCurrentExport }]">
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
        <div class="object-current-block">
          <h2>当前对象与异常处理</h2>
          <div v-if="selectedDisk" class="object-detail-grid">
            <dl>
              <dt>对象路径</dt><dd>{{ selectedDisk.current_object?.key ?? (viewSummary.scan.current_key || "暂无") }}</dd>
              <dt>对象状态</dt><dd class="tone-running">{{ selectedDisk.current_object?.object_status ?? "等待对象" }}</dd>
              <dt>剩余大小</dt><dd>{{ formatBytes(selectedDisk.current_object?.remaining_bytes ?? 0) }} / {{ formatBytes(selectedDisk.current_object?.size_bytes ?? 0) }}</dd>
            </dl>
            <dl>
              <dt>传输速度</dt><dd>{{ formatSpeed(selectedDisk.speed_bytes_per_sec ?? 0) }}</dd>
              <dt>文件系统</dt><dd>{{ selectedDiskShortName }}</dd>
              <dt>对象标识</dt><dd>{{ selectedDisk.current_object?.key ?? "未返回" }}</dd>
            </dl>
            <dl>
              <dt>加密状态</dt><dd>{{ selectedDisk.current_object ? "已加密" : "未返回" }}</dd>
              <dt>写入阶段</dt><dd>{{ selectedDisk.runtime_status }}</dd>
              <dt>校验状态</dt><dd>{{ diskLifecycleStatusLabel(selectedDisk) }}</dd>
            </dl>
          </div>
          <p v-else class="object-empty">
            未选中运输盘。未注册或异常盘只有在 Edge 后端检测并返回后才会显示。
          </p>
          <div class="object-progress-row">
            <div class="progress-track object-progress"><b :style="{ width: `${currentObjectProgressPercent}%` }"></b></div>
            <span>{{ currentObjectProgressPercent.toFixed(2) }}%</span>
          </div>
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
          <dt>盘内状态</dt><dd>{{ diskLifecycleStatusLabel(selectedDisk) }}</dd>
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
