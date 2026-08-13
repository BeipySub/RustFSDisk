<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  fetchCenterDashboardSummary,
  initializeCenterDisk,
  registerCenterDisk,
  reinitializeCenterDisk,
  type CenterDashboardSummary,
  type CenterDiskProgress,
  type DiskLifecycleCode,
  type ImportJobStatus,
  type RuntimeStatus,
} from "../api/centerDashboard";
import {
  applyImportProgressEvent,
  connectCenterProgressSocket,
  type CenterProgressSocket,
} from "../ws/centerImportProgress";
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
  loading: "验盘",
  running: "导入",
  paused: "暂停",
  complete: "可复用",
  error: "异常",
};
const particleSceneStates: ParticleSceneState[] = ["loading", "running", "paused", "complete", "error"];

function emptySummary(): CenterDashboardSummary {
  const now = new Date().toISOString();
  return {
    source: "center",
    center_id: "",
    center_name: "Center",
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
    message: "等待 Center Dashboard 只读接口",
  };
}

const summary = ref<CenterDashboardSummary | null>(null);
const selectedDiskId = ref("");
const selectedDiskDetailVisible = ref(false);
const diskActionBusyId = ref("");
const diskActionMessage = ref("");
const diskActionError = ref("");
const isRefreshing = ref(false);
const httpError = ref<DashboardHttpError | null>(null);
const wsConnected = ref(false);
const wsMessage = ref("WebSocket 尚未连接");
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
let progressSocket: CenterProgressSocket | null = null;
let particleAnchorObserver: ResizeObserver | null = null;
const TRANSPORT_SLOT_COUNT = 9;

interface RefreshOptions {
  silent?: boolean;
}

const viewSummary = computed(() => summary.value ?? emptySummary());
const disks = computed(() => viewSummary.value.disks);
const transportSlots = computed(() =>
  Array.from({ length: TRANSPORT_SLOT_COUNT }, (_, index) => ({
    slotNumber: index + 1,
    disk: disks.value[index] ?? null,
  })),
);
const selectedDisk = computed(() => disks.value.find((disk) => diskIdentity(disk) === selectedDiskId.value) ?? null);
const activeImportDisks = computed(() => disks.value.filter((disk) => isActiveImportDisk(disk)));
const reusableDisks = computed(() =>
  disks.value.filter((disk) => disk.disk_status_code === "INITIALIZED" && disk.reusable),
);
const pendingImportDisks = computed(() =>
  disks.value.filter((disk) => disk.disk_status_code === "SEALED" && !disk.imported_before),
);
const abnormalDisks = computed(() =>
  disks.value.filter((disk) => disk.runtime_status === "ERROR" || disk.disk_status_code === "ERROR" || Boolean(disk.last_error_code)),
);
const currentImportDisk = computed(() => activeImportDisks.value[0] ?? selectedDisk.value);
const currentImportStatus = computed(() => currentImportDisk.value?.import_job_status);
const currentImportJobId = computed(() => currentImportDisk.value?.import_job_id ?? selectedDisk.value?.import_job_id ?? "");
const showParticleStream = computed(
  () =>
    hasCurrentImport.value &&
    viewSummary.value.global_progress.total_bytes > 0 &&
    viewSummary.value.global_progress.done_bytes < viewSummary.value.global_progress.total_bytes,
);
const particleStreamActive = computed(() => showParticleStream.value || (showParticleDevPanel && particleSamplePlaying.value));
const particleCanvasActive = computed(
  () => showParticleStream.value || (showParticleDevPanel && particleSamplePlaying.value && particleSceneState.value === "running"),
);
const globalProgressPercent = computed(() =>
  progressPercent(viewSummary.value.global_progress.done_bytes, viewSummary.value.global_progress.total_bytes),
);
const importedObjectCount = computed(() => viewSummary.value.global_progress.object_done);
const importedBytesLabel = computed(() => formatBytes(viewSummary.value.global_progress.done_bytes));
const totalObjectCount = computed(() => viewSummary.value.global_progress.object_total);
const totalBytesLabel = computed(() => formatBytes(viewSummary.value.global_progress.total_bytes));
const selectedProgressPercent = computed(() =>
  progressPercent(selectedDisk.value?.done_bytes ?? 0, selectedDisk.value?.total_bytes ?? 0),
);
const hasSelectedImportTask = computed(() => {
  const disk = selectedDisk.value;
  return Boolean(disk?.current_object || isActiveImportDisk(disk) || (disk?.object_total ?? 0) > 0);
});
const selectedDiskIndex = computed(() =>
  selectedDisk.value ? disks.value.findIndex((disk) => diskIdentity(disk) === selectedDiskId.value) : -1,
);
const selectedSlotLabel = computed(() => (selectedDiskIndex.value >= 0 ? String(selectedDiskIndex.value + 1).padStart(2, "0") : "--"));
const selectedDiskTitle = computed(() =>
  selectedDisk.value ? `选中磁盘详情（盘位 ${selectedSlotLabel.value}）` : "选中磁盘详情（未选中）",
);
const currentObjectProgressPercent = computed(() =>
  progressPercent(selectedDisk.value?.current_object?.done_bytes ?? 0, selectedDisk.value?.current_object?.size_bytes ?? 0),
);
const selectedCurrentObjectName = computed(() => {
  const currentObject = selectedDisk.value?.current_object;
  return currentObject?.display_name || currentObject?.key || "暂无";
});
const selectedDiskShortName = computed(() => {
  const disk = selectedDisk.value;
  if (!disk) return "未选中";
  const serial = disk.disk_sn ? `SN...${disk.disk_sn.slice(-4)}` : "SN 未返回";
  return `盘位 ${selectedSlotLabel.value}（${serial}）`;
});
const importedDiskReadyForReinitialize = computed(
  () =>
    !httpError.value &&
    !hasCurrentImport.value &&
    viewSummary.value.disks.some((disk) => disk.disk_status_code === "IMPORTED" || disk.imported_before),
);
const estimatedDone = computed(() => {
  const speed = viewSummary.value.global_progress.speed_bytes_per_sec;
  if (speed <= 0) return "等待速度";
  return formatDuration(Math.ceil(viewSummary.value.global_progress.remaining_bytes / speed));
});
const isEmpty = computed(() => !isRefreshing.value && !httpError.value && disks.value.length === 0);
const hasCurrentImport = computed(
  () =>
    activeImportDisks.value.length > 0 ||
    (viewSummary.value.global_progress.total_bytes > 0 &&
      viewSummary.value.global_progress.done_bytes < viewSummary.value.global_progress.total_bytes),
);
const importStatusTitle = computed(() => {
  if (httpError.value) return "只读接口不可用";
  if (hasCurrentImport.value) return "当前导入进度";
  return "当前无导入任务";
});
const importStatusNotice = computed(() => {
  if (httpError.value) return `Center Dashboard 只读接口不可用：${httpError.value.error_code}。当前不展示模拟数据。`;
  if (hasCurrentImport.value) return `WS：${wsMessage.value}`;
  if (isEmpty.value) return "未检测到运输盘；插入已封盘或可初始化的运输盘后会显示在盘位区。";
  return "运输盘已被探测，但当前没有运行中的导入任务。";
});

watch(
  () => disks.value.map((disk) => diskIdentity(disk)).join("|"),
  () => {
    if (!disks.value.some((disk) => diskIdentity(disk) === selectedDiskId.value)) {
      selectedDiskId.value = "";
      selectedDiskDetailVisible.value = false;
    }
  },
  { immediate: true },
);

async function refreshFromHttpSummary(options: RefreshOptions = {}) {
  if (isRefreshing.value) return;
  if (!options.silent) isRefreshing.value = true;
  httpError.value = null;
  try {
    summary.value = await fetchCenterDashboardSummary();
    wsConnected.value = summary.value.ws_connected;
  } catch (error) {
    httpError.value =
      error instanceof DashboardHttpError
        ? error
        : new DashboardHttpError("DASHBOARD_UNAVAILABLE", error instanceof Error ? error.message : "HTTP summary unavailable");
  } finally {
    if (!options.silent) isRefreshing.value = false;
  }
}

function selectDisk(disk: CenterDiskProgress) {
  selectedDiskId.value = diskIdentity(disk);
  selectedDiskDetailVisible.value = true;
}

function clearSelectedDisk() {
  selectedDiskDetailVisible.value = false;
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
    startX: clampParticleAnchor((sourceRect.left - stageRect.left + sourceRect.width * 0.76) / stageRect.width),
    startY: clampParticleAnchor((sourceRect.top - stageRect.top + sourceRect.height * 0.43) / stageRect.height),
    endX: clampParticleAnchor((nasRect.left - stageRect.left + nasRect.width * 0.105) / stageRect.width),
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

function isActiveImportDisk(disk: CenterDiskProgress | null | undefined): boolean {
  return Boolean(
    disk &&
      (disk.import_job_status === "IMPORTING" ||
        disk.disk_status_code === "CENTER_IMPORTING" ||
        disk.runtime_status === "CLEANING" ||
        disk.runtime_status === "REINITIALIZING"),
  );
}

function diskIdentity(disk: CenterDiskProgress): string {
  return disk.presence_id || disk.disk_id || disk.mount_path || disk.device_path || disk.disk_sn;
}

function diskCapacityBytes(disk: CenterDiskProgress): number {
  return disk.capacity_bytes || disk.total_bytes || disk.done_bytes + (disk.current_object?.remaining_bytes ?? 0) || 0;
}

function canRunDiskAction(disk: CenterDiskProgress): boolean {
  if (diskActionBusyId.value || isActiveImportDisk(disk) || !disk.mount_path) return false;
  if (disk.can_register || !disk.registered || disk.disk_status_code === "UNREGISTERED") {
    return Boolean(disk.disk_sn && diskCapacityBytes(disk) > 0 && disk.disk_status_code === "UNREGISTERED");
  }
  if (disk.can_initialize) {
    return Boolean(disk.disk_id && diskCapacityBytes(disk) > 0);
  }
  if (disk.can_reinitialize) {
    return Boolean(disk.disk_id && disk.seal_id && (disk.disk_status_code === "SEALED" || disk.disk_status_code === "IMPORTED"));
  }
  return false;
}

function diskActionLabel(disk: CenterDiskProgress): string {
  if (diskActionBusyId.value === diskIdentity(disk)) return "处理中";
  if (disk.can_register || !disk.registered || disk.disk_status_code === "UNREGISTERED") return "添加";
  if (disk.can_initialize) return "初始化";
  if (disk.can_reinitialize) return disk.disk_status_code === "SEALED" ? "重初始" : "重初始";
  return disk.disk_status_code === "INITIALIZED" ? "可交付" : "查看";
}

function diskActionTitle(disk: CenterDiskProgress): string {
  if (disk.can_register || !disk.registered || disk.disk_status_code === "UNREGISTERED") return "注册并初始化运输盘";
  if (disk.can_initialize) return "初始化运输盘";
  if (disk.can_reinitialize && disk.disk_status_code === "SEALED") return "丢弃已封盘数据并重新初始化";
  if (disk.can_reinitialize) return "清理导入完成数据并重新初始化";
  if (disk.disk_status_code === "INITIALIZED") return "已初始化，可交付边缘端";
  return diskStatusTooltip(disk);
}

async function runDiskAction(disk: CenterDiskProgress) {
  if (!canRunDiskAction(disk)) {
    selectDisk(disk);
    return;
  }
  const actionId = diskIdentity(disk);
  diskActionBusyId.value = actionId;
  diskActionError.value = "";
  diskActionMessage.value = "";
  let targetDiskId = disk.disk_id;
  try {
    if (disk.can_register || !disk.registered || disk.disk_status_code === "UNREGISTERED") {
      const registered = await registerCenterDisk({
        sn: disk.disk_sn,
        capacity_bytes: diskCapacityBytes(disk),
        remark: `Center Dashboard ${disk.mount_path}`,
      });
      await initializeCenterDisk({
        disk_id: registered.disk_id,
        sn: disk.disk_sn,
        capacity_bytes: diskCapacityBytes(disk),
        mount_path: disk.mount_path,
      });
      diskActionMessage.value = "运输盘已注册并初始化";
      targetDiskId = registered.disk_id;
    } else if (disk.can_initialize) {
      await initializeCenterDisk({
        disk_id: disk.disk_id,
        sn: disk.disk_sn,
        capacity_bytes: diskCapacityBytes(disk),
        mount_path: disk.mount_path,
      });
      diskActionMessage.value = "运输盘已初始化";
    } else if (disk.can_reinitialize && disk.seal_id) {
      await reinitializeCenterDisk(disk.disk_id, {
        mount_path: disk.mount_path,
        seal_id: disk.seal_id,
        expected_status_code: disk.disk_status_code === "SEALED" ? "SEALED" : "IMPORTED",
        operator_reason: "Center Dashboard one-click reinitialize",
        confirm_reinitialize: true,
        confirm_discard_sealed_export: disk.disk_status_code === "SEALED",
      });
      diskActionMessage.value = "运输盘已重新初始化";
    }
    await refreshFromHttpSummary({ silent: true });
    const targetDisk = disks.value.find((item) => item.disk_id === targetDiskId);
    if (targetDisk) selectedDiskId.value = diskIdentity(targetDisk);
    selectedDiskDetailVisible.value = true;
  } catch (error) {
    diskActionError.value = error instanceof Error ? error.message : "磁盘操作失败";
  } finally {
    diskActionBusyId.value = "";
  }
}

function diskTone(disk: CenterDiskProgress): string {
  if (disk.runtime_status === "ERROR" || disk.disk_status_code === "ERROR" || disk.last_error_code) return "danger";
  if (isActiveImportDisk(disk)) return "running";
  if (disk.disk_status_code === "INITIALIZED" || disk.runtime_status === "DONE") return "success";
  if (disk.disk_status_code === "SEALED" || disk.runtime_status === "CHECKING" || disk.disk_status_code === "REGISTERED") return "warning";
  if (disk.runtime_status === "REMOVED") return "removed";
  return "muted";
}

function diskStatusLabel(disk: CenterDiskProgress): string {
  if (disk.last_error_code) return disk.error_message || disk.last_error_code;
  if (disk.import_job_status) return importStatusLabel(disk.import_job_status);
  return runtimeLabel(disk.runtime_status);
}

function diskCardStatusLabel(disk: CenterDiskProgress): string {
  return lifecycleLabel(disk.disk_status_code);
}

function diskStatusTooltip(disk: CenterDiskProgress): string {
  if (disk.error_message || disk.last_error_code) return disk.error_message || disk.last_error_code || "";
  if (disk.disk_status_code === "SEALED") return "已封盘：可在中控端导入。";
  if (disk.disk_status_code === "IMPORTED") return "已导入：可由中控端清理并重新初始化。";
  if (disk.disk_status_code === "INITIALIZED") return "已初始化：可交付边缘端写盘。";
  return runtimeLabel(disk.runtime_status);
}

function diskLifecycleStatusLabel(disk: CenterDiskProgress | null): string {
  if (!disk) return "未返回";
  return lifecycleLabel(disk.disk_status_code);
}

function lifecycleLabel(diskStatusCode: DiskLifecycleCode): string {
  const labels: Record<DiskLifecycleCode, string> = {
    UNREGISTERED: "未注册",
    REGISTERED: "已注册",
    INITIALIZED: "已初始化",
    EDGE_COPYING: "边缘写盘中",
    SEALED: "待导入",
    CENTER_IMPORTING: "中控导入中",
    IMPORTED: "已导入",
    ERROR: "生命周期异常",
  };
  return labels[diskStatusCode];
}

function runtimeLabel(runtimeStatus: RuntimeStatus): string {
  const labels: Record<RuntimeStatus, string> = {
    DETECTED: "已检测",
    CHECKING: "校验中",
    READY: "就绪",
    COPYING: "写盘中",
    CLEANING: "清理中",
    REINITIALIZING: "重新初始化中",
    DONE: "完成",
    REJECTED: "已拒绝",
    REMOVED: "已拔出",
    ERROR: "运行异常",
  };
  return labels[runtimeStatus];
}

function importStatusLabel(importJobStatus?: ImportJobStatus): string {
  const labels: Record<ImportJobStatus, string> = {
    PENDING: "待导入",
    IMPORTING: "导入中",
    DONE: "已完成",
    FAILED: "失败",
    CANCELLED: "已取消",
  };
  return importJobStatus ? labels[importJobStatus] : "无任务";
}

function slotSnLabel(disk: CenterDiskProgress): string {
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

function slotUsedBytes(disk: CenterDiskProgress): number {
  const total = slotTotalBytes(disk);
  const doneBytes = disk.done_bytes;
  if (doneBytes > 0) return Math.min(total, doneBytes);
  return 0;
}

function slotTotalBytes(disk: CenterDiskProgress): number {
  return disk.total_bytes || disk.capacity_bytes || disk.done_bytes + (disk.current_object?.remaining_bytes ?? 0) || 0;
}

function slotProgressPercent(disk: CenterDiskProgress): number {
  return progressPercent(slotUsedBytes(disk), slotTotalBytes(disk));
}

function formatTbValue(bytes: number): string {
  if (bytes <= 0) return "0.00";
  return (bytes / 1000 ** 4).toFixed(2);
}

function formatTbPair(disk: CenterDiskProgress): string {
  return `${formatTbValue(slotUsedBytes(disk))}/${formatTbValue(slotTotalBytes(disk))} TB`;
}

function formatTbNumberPair(disk: CenterDiskProgress): string {
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
  progressSocket = connectCenterProgressSocket({
    onEvent(event) {
      summary.value = applyImportProgressEvent(summary.value ?? emptySummary(), event);
      wsMessage.value = event.message || "";
      wsConnected.value = true;
    },
    onConnectionChange(connected, message) {
      wsConnected.value = connected;
      wsMessage.value = message;
      if (summary.value) summary.value = { ...summary.value, ws_connected: connected };
      if (!connected) void refreshFromHttpSummary({ silent: true });
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
      :local-tone="httpError ? 'quiet' : 'ok'"
      :ws-tone="wsConnected ? 'ok' : 'quiet'"
      refresh-label="刷新 Dashboard"
      :refresh-disabled="isRefreshing"
      @refresh="refreshFromHttpSummary"
    />

    <section ref="runtimeStageRef" class="runtime-stage" aria-label="Center 导入运行态">
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
                <strong>Center 首页业务样板</strong>
                <small>验盘 -> 导入 -> 去重 -> 清理 -> 可复用</small>
              </span>
              <button type="button" :class="{ 'is-playing': particleSamplePlaying }" :aria-pressed="particleSamplePlaying" @click="toggleParticleSample">
                {{ particleSamplePlaying ? "暂停" : "播放" }}
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
            <label for="center-particle-glow"><span>蓝色外发光</span><output>{{ Math.round(particleGlow * 100) }}%</output></label>
            <input id="center-particle-glow" v-model.number="particleGlow" type="range" min="0.25" max="2" step="0.05" />
          </section>
          <section class="particle-control-section particle-range-control">
            <label for="center-particle-speed"><span>导入速度</span><output>{{ particleSpeed.toFixed(2) }}x</output></label>
            <input id="center-particle-speed" v-model.number="particleSpeed" type="range" min="0.25" max="2.5" step="0.05" />
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
            <template v-for="slot in transportSlots" :key="slot.disk ? `${slot.slotNumber}-${diskIdentity(slot.disk)}` : `empty-${slot.slotNumber}`">
              <div
                v-if="slot.disk"
                :aria-pressed="selectedDisk ? diskIdentity(selectedDisk) === diskIdentity(slot.disk) : false"
                :class="[`slot-${diskTone(slot.disk)}`, { selected: selectedDisk ? diskIdentity(selectedDisk) === diskIdentity(slot.disk) : false }]"
                class="disk-slot-card"
                role="button"
                tabindex="0"
                @click="selectDisk(slot.disk)"
                @keydown.enter.prevent="selectDisk(slot.disk)"
                @keydown.space.prevent="selectDisk(slot.disk)"
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
              </div>
              <div v-else class="disk-slot-cell empty" :aria-label="`empty transport slot ${slot.slotNumber}`"></div>
            </template>
          </div>
        </div>
      </div>
    </section>

    <section v-if="!httpError" :class="['global-progress', 'glass-panel', { idle: !hasCurrentImport }]">
      <div>
        <span>{{ importStatusTitle }}</span>
        <strong v-if="hasCurrentImport">{{ globalProgressPercent.toFixed(0) }}<small>%</small></strong>
        <strong v-else>--<small></small></strong>
        <em>{{ hasCurrentImport ? currentImportStatus ?? "IMPORTING" : "IDLE" }}</em>
      </div>
      <div v-if="hasCurrentImport" class="progress-main">
        <p>
          <span>已完成 <b>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</b> / {{ formatBytes(viewSummary.global_progress.total_bytes) }}</span>
          <span>剩余 <b>{{ formatBytes(viewSummary.global_progress.remaining_bytes) }}</b></span>
          <span>速度 <b>{{ formatSpeed(viewSummary.global_progress.speed_bytes_per_sec) }}</b></span>
        </p>
        <div class="progress-track"><b :style="{ width: `${globalProgressPercent}%` }"></b></div>
        <dl>
          <div><dt>待导入盘</dt><dd>{{ pendingImportDisks.length }}</dd></div>
          <div><dt>对象</dt><dd>{{ importedObjectCount.toLocaleString() }}</dd></div>
          <div><dt>批次</dt><dd>{{ currentImportJobId || "暂无" }}</dd></div>
          <div><dt>预计完成</dt><dd>{{ estimatedDone }}</dd></div>
        </dl>
      </div>
      <div v-else class="progress-main idle-copy">
        <p>{{ importStatusNotice }}</p>
        <dl>
          <div><dt>现场盘位</dt><dd>{{ disks.length }} 盘位</dd></div>
          <div><dt>待导入盘</dt><dd>{{ pendingImportDisks.length }}</dd></div>
          <div><dt>可复用盘</dt><dd>{{ reusableDisks.length }}</dd></div>
          <div><dt>异常盘</dt><dd>{{ abnormalDisks.length }}</dd></div>
        </dl>
      </div>
    </section>

    <section v-if="httpError" class="selected-disk-strip glass-panel error-state" aria-label="Center Dashboard 错误态">
      <div class="selected-disk-content">
        <strong>Center Dashboard 只读接口不可用</strong>
        <dl>
          <div><dt>错误码</dt><dd class="tone-running">{{ httpError.error_code }}</dd></div>
          <div><dt>HTTP</dt><dd>{{ httpError.http_status ?? "未返回" }}</dd></div>
          <div><dt>展示策略</dt><dd>不展示模拟进度、模拟盘位或模拟对象</dd></div>
          <div><dt>WebSocket</dt><dd>{{ wsMessage }}</dd></div>
        </dl>
      </div>
    </section>

    <section v-else-if="importedDiskReadyForReinitialize && !selectedDiskDetailVisible" class="selected-disk-strip glass-panel" aria-label="导入完成可重新初始化">
      <div class="selected-disk-content">
        <strong>最近导入任务已完成，可清理并重新初始化</strong>
        <dl>
          <div><dt>导入任务</dt><dd>{{ currentImportJobId || "未返回" }}</dd></div>
          <div><dt>导入任务状态</dt><dd class="tone-running">{{ currentImportStatus ?? "DONE" }}</dd></div>
          <div><dt>盘内状态</dt><dd class="tone-running">已导入</dd></div>
          <div><dt>已导入</dt><dd>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</dd></div>
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
          <div v-if="hasSelectedImportTask"><dt>导入进度</dt><dd>{{ selectedProgressPercent.toFixed(2) }}%</dd></div>
          <div v-if="hasSelectedImportTask"><dt>当前文件</dt><dd :title="selectedCurrentObjectName">{{ selectedCurrentObjectName }}</dd></div>
          <div><dt>容量</dt><dd>{{ selectedDisk ? formatTbPair(selectedDisk) : "0.00/0.00 TB" }}</dd></div>
          <div v-if="diskActionMessage"><dt>操作结果</dt><dd class="tone-running">{{ diskActionMessage }}</dd></div>
          <div v-if="diskActionError"><dt>操作错误</dt><dd class="tone-running">{{ diskActionError }}</dd></div>
        </dl>
      </div>
      <button
        class="selected-disk-action"
        type="button"
        :disabled="!canRunDiskAction(selectedDisk)"
        :title="diskActionTitle(selectedDisk)"
        @pointerdown.stop
        @click.stop="runDiskAction(selectedDisk)"
      >
        {{ diskActionLabel(selectedDisk) }}
      </button>
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

    <section v-if="!httpError" :class="['dashboard-lower-grid', { compact: !hasCurrentImport }]">
      <article class="overview-panel glass-panel">
        <h2>导入与盘位概览</h2>
        <dl class="overview-metrics">
          <div><dt>待导入运输盘</dt><dd>{{ pendingImportDisks.length.toLocaleString() }}</dd></div>
          <div><dt>导入中运输盘</dt><dd>{{ activeImportDisks.length.toLocaleString() }}</dd></div>
          <div><dt>可复用运输盘</dt><dd>{{ reusableDisks.length.toLocaleString() }}</dd></div>
          <div><dt>对象总数</dt><dd>{{ totalObjectCount.toLocaleString() }}</dd></div>
          <div><dt>已导入对象</dt><dd>{{ importedObjectCount.toLocaleString() }}</dd></div>
          <div><dt>异常运输盘</dt><dd>{{ abnormalDisks.length.toLocaleString() }}</dd></div>
        </dl>
      </article>

      <article class="object-panel object-wide-panel glass-panel">
        <div class="object-current-block">
          <h2>当前对象与异常处理</h2>
          <div v-if="selectedDisk" class="object-detail-grid">
            <dl>
              <dt>对象路径</dt><dd>{{ selectedDisk.current_object?.key ?? "暂无" }}</dd>
              <dt>对象状态</dt><dd class="tone-running">{{ selectedDisk.import_job_status ? importStatusLabel(selectedDisk.import_job_status) : "等待对象" }}</dd>
              <dt>剩余大小</dt><dd>{{ formatBytes(selectedDisk.current_object?.remaining_bytes ?? 0) }} / {{ formatBytes(selectedDisk.current_object?.size_bytes ?? 0) }}</dd>
            </dl>
            <dl>
              <dt>传输速度</dt><dd>{{ formatSpeed(selectedDisk.speed_bytes_per_sec ?? 0) }}</dd>
              <dt>文件系统</dt><dd>{{ selectedDiskShortName }}</dd>
              <dt>对象标识</dt><dd>{{ selectedDisk.current_object?.key ?? "未返回" }}</dd>
            </dl>
            <dl>
              <dt>校验状态</dt><dd>{{ selectedDisk.current_object ? "已读取" : "未返回" }}</dd>
              <dt>导入阶段</dt><dd>{{ selectedDisk.runtime_status }}</dd>
              <dt>盘内状态</dt><dd>{{ diskLifecycleStatusLabel(selectedDisk) }}</dd>
            </dl>
          </div>
          <p v-else class="object-empty">
            未选中运输盘。待导入、异常或可复用盘会在 Center 后端检测并返回后显示在盘位区。
          </p>
          <div class="object-progress-row">
            <div class="progress-track object-progress"><b :style="{ width: `${currentObjectProgressPercent}%` }"></b></div>
            <span>{{ currentObjectProgressPercent.toFixed(2) }}%</span>
          </div>
          <p class="object-empty">总量 {{ totalBytesLabel }}，已导入 {{ importedBytesLabel }}。</p>
        </div>
      </article>
    </section>
  </main>
</template>
