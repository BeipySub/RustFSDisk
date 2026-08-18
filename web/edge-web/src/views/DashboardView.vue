<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  diskFilesystemDisplay,
  edgeDiskPrimaryStatusLabel,
  diskStatusDisplay,
  fetchEdgeDashboardSummary,
  fetchEdgeReadiness,
  isEdgeUninitializedDiskIssue,
  isEdgeUnregisteredDiskIssue,
  isEdgeUnsupportedDiskIssue,
  isActiveExportJobStatus,
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

type ParticlePathAnchors = {
  startX: number;
  startY: number;
  endX: number;
  endY: number;
};

function emptySummary(): EdgeDashboardSummary {
  const now = new Date().toISOString();
  return {
    source: "edge",
    edge_code: "",
    edge_name: "Edge",
    edge_status: undefined,
    object_inventory: {
      total_bytes: 0,
      exported_bytes: 0,
      total_count: 0,
      exported_count: 0,
    },
    export_job: null,
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
const selectedDisk = computed(() => disks.value.find((disk) => disk.disk_id === selectedDiskId.value) ?? null);
const currentExportJob = computed(() => viewSummary.value.export_job);
const currentExportStatus = computed(() => currentExportJob.value?.export_job_status);
const currentExportJobId = computed(() => currentExportJob.value?.export_job_id ?? "");
const hasCurrentExportDisk = computed(
  () =>
    Boolean(currentExportJobId.value) &&
    disks.value.some(
      (disk) =>
        disk.export_job_id === currentExportJobId.value &&
        disk.runtime_status !== "DONE" &&
        disk.runtime_status !== "REMOVED",
    ),
);
const showParticleStream = computed(
  () =>
    Boolean(currentExportJobId.value) &&
    disks.value.some(
      (disk) => disk.export_job_id === currentExportJobId.value && disk.runtime_status === "COPYING",
    ),
);
const globalProgressPercent = computed(() =>
  progressPercent(viewSummary.value.global_progress.done_bytes, viewSummary.value.global_progress.total_bytes),
);
const objectInventory = computed(() => {
  return (
    viewSummary.value.object_inventory ?? {
      total_bytes: 0,
      exported_bytes: 0,
      total_count: 0,
      exported_count: 0,
    }
  );
});
const rustFsObjectTotal = computed(() => objectInventory.value.total_count);
const rustFsTotalBytesLabel = computed(() => formatBytes(objectInventory.value.total_bytes));
const exportedInventoryObjectCount = computed(() => objectInventory.value.exported_count);
const exportedInventoryBytesLabel = computed(() => formatBytes(objectInventory.value.exported_bytes));
const selectedProgressPercent = computed(() =>
  progressPercent(
    selectedDisk.value?.progress?.done_bytes ?? selectedDisk.value?.done_bytes ?? 0,
    selectedDisk.value?.progress?.total_bytes ?? selectedDisk.value?.total_bytes ?? 0,
  ),
);
const hasSelectedCopyTask = computed(() => {
  const disk = selectedDisk.value;
  return Boolean(
    disk?.current_object ||
    disk?.runtime_status === "COPYING" ||
    (disk?.progress?.object_total ?? disk?.object_total ?? 0) > 0,
  );
});
const selectedDiskIndex = computed(() => (selectedDisk.value ? disks.value.findIndex((disk) => disk.disk_id === selectedDisk.value?.disk_id) : -1));
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
const estimatedDone = computed(() => {
  const speed = viewSummary.value.global_progress.speed_bytes_per_sec;
  if (speed <= 0) return "等待速度";
  return formatDuration(Math.ceil(viewSummary.value.global_progress.remaining_bytes / speed));
});
const isEmpty = computed(() => !isRefreshing.value && !httpError.value && disks.value.length === 0);
const hasCurrentExport = computed(() => {
  if (!isActiveExportJobStatus(currentExportStatus.value)) return false;
  if (currentExportStatus.value === "PENDING" || currentExportStatus.value === "SCANNING") return true;
  return hasCurrentExportDisk.value;
});
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

async function refreshFromHttpSummary(options: RefreshOptions = {}) {
  if (isRefreshing.value) return;
  if (!options.silent) isRefreshing.value = true;
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
    if (!options.silent) isRefreshing.value = false;
  }
}

function selectDisk(disk: EdgeDiskProgress) {
  selectedDiskId.value = disk.disk_id;
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
    // Both cabinet PNGs include transparent margins. These factors target the visible transfer ports.
    startX: clampParticleAnchor((sourceRect.left - stageRect.left + sourceRect.width * 0.76) / stageRect.width),
    startY: clampParticleAnchor((sourceRect.top - stageRect.top + sourceRect.height * 0.43) / stageRect.height),
    endX: clampParticleAnchor((nasRect.left - stageRect.left + nasRect.width * 0.105) / stageRect.width),
    endY: clampParticleAnchor((nasRect.top - stageRect.top + nasRect.height * 0.52) / stageRect.height),
  };
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
  return edgeDiskPrimaryStatusLabel(disk);
}

function diskCardStatusLabel(disk: EdgeDiskProgress): string {
  return disk.disk_status_code ? diskStatusDisplay(disk.disk_status_code) : "未返回";
}

function diskStatusTooltip(disk: EdgeDiskProgress): string {
  if (disk.disk_status_code === "SEALED") return "已封盘：可拔盘送往中控端导入，不可继续用于 Edge 导出。";
  if (disk.disk_status_code === "IMPORTED") return "已导入：请在中控端重新初始化后再用于 Edge 导出。";
  if (disk.disk_status_code === "CENTER_IMPORTING") return "中控导入中：当前不可用于 Edge 导出。";
  if (disk.runtime_status !== "REJECTED" && disk.runtime_status !== "ERROR") return "";
  return translateDiskIssue(diskIssueRawText(disk));
}

function diskIssueRawText(disk: EdgeDiskProgress): string {
  return disk.error_message || disk.last_error_code || disk.message || "";
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
  if (isEdgeUninitializedDiskIssue(value)) {
    return "未初始化：未检测到有效的盘内初始化信息，请先到中控端初始化后再用于 Edge 离线导出。";
  }
  if (isEdgeUnregisteredDiskIssue(value)) {
    return "未注册：需要先在中控端注册并初始化后再用于 Edge 离线导出。";
  }
  if (isEdgeUnsupportedDiskIssue(value)) {
    return "不可导出：当前磁盘不满足 Edge 离线导出要求，请按中控端初始化流程处理。";
  }
  return value;
}

function diskLifecycleStatusLabel(disk: EdgeDiskProgress | null): string {
  if (!disk) return "未返回";
  if (disk.disk_status_code) return diskStatusDisplay(disk.disk_status_code);
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
  const capacityBytes = disk.capacity_bytes ?? 0;
  if (capacityBytes > 0) {
    return Math.max(0, capacityBytes - Math.min(disk.free_bytes, capacityBytes));
  }
  const total = slotTotalBytes(disk);
  const doneBytes = disk.progress?.done_bytes ?? disk.done_bytes;
  if (doneBytes > 0) return Math.min(total, doneBytes);
  if (total > 0 && disk.free_bytes > 0) return Math.max(0, total - disk.free_bytes);
  return 0;
}

function slotTotalBytes(disk: EdgeDiskProgress): number {
  return (
    disk.capacity_bytes ||
    disk.object_budget_bytes ||
    disk.total_bytes ||
    (disk.progress?.done_bytes ?? disk.done_bytes) + (disk.progress?.remaining_bytes ?? disk.remaining_bytes) ||
    disk.free_bytes ||
    0
  );
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
    <EdgeTelemetry :http-tone="httpError ? 'warning' : 'ok'" :local-tone="readiness?.ok ? 'ok' : 'quiet'"
      :ws-tone="wsConnected ? 'ok' : 'quiet'" refresh-label="刷新 Dashboard" :refresh-disabled="isRefreshing"
      @refresh="refreshFromHttpSummary" />

    <section ref="runtimeStageRef" class="runtime-stage" aria-label="Edge 导出运行态">
      <button class="source-rack-link" type="button" aria-label="打开同步记录" title="同步记录" @click="openSyncRecords">
        <img ref="sourceRackRef" alt="" class="source-rack" src="/assets/fustfs-baseline/source-rack-cutout-v3.webp"
          @load="updateParticlePathAnchors" />
      </button>
      <ParticleAetherField v-if="showParticleStream" :end-x="particlePathAnchors.endX"
        :end-y="particlePathAnchors.endY" :start-x="particlePathAnchors.startX"
        :start-y="particlePathAnchors.startY" />
      <div class="transport-array">
        <div ref="nasShellRef" class="nas-shell">
          <img alt="" src="/assets/fustfs-baseline/transport-bay-inner-black-clean-alpha.png"
            @load="updateParticlePathAnchors" />
          <div class="disk-slot-matrix" aria-label="运输盘位列表">
            <template v-for="slot in transportSlots"
              :key="slot.disk ? `${slot.slotNumber}-${slot.disk.disk_id}` : `empty-${slot.slotNumber}`">
              <button v-if="slot.disk" :aria-pressed="selectedDisk?.disk_id === slot.disk.disk_id"
                :class="[`slot-${diskTone(slot.disk)}`, { selected: selectedDisk?.disk_id === slot.disk.disk_id }]"
                type="button" @click="selectDisk(slot.disk)">
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

    <section :class="['global-progress', 'glass-panel', { idle: !hasCurrentExport }]">
      <div>
        <span>{{ exportStatusTitle }}</span>
        <strong v-if="hasCurrentExport">{{ globalProgressPercent.toFixed(0) }}<small>%</small></strong>
        <strong v-else>--<small></small></strong>
        <em>{{ hasCurrentExport ? currentExportStatus : "IDLE" }}</em>
      </div>
      <div v-if="hasCurrentExport" class="progress-main">
        <p>
          <span>已完成 <b>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</b> / {{
            formatBytes(viewSummary.global_progress.total_bytes) }}</span>
          <span>剩余 <b>{{ formatBytes(viewSummary.global_progress.remaining_bytes) }}</b></span>
          <span>速度 <b>{{ formatSpeed(viewSummary.global_progress.speed_bytes_per_sec) }}</b></span>
        </p>
        <div class="progress-track"><b :style="{ width: `${globalProgressPercent}%` }"></b></div>
        <dl>
          <div>
            <dt>文件</dt>
            <dd>{{ rustFsObjectTotal.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>对象</dt>
            <dd>{{ exportedInventoryObjectCount.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>批次</dt>
            <dd>{{ currentExportJobId || "暂无" }}</dd>
          </div>
          <div>
            <dt>预计完成</dt>
            <dd>{{ estimatedDone }}</dd>
          </div>
        </dl>
      </div>
      <div v-else class="progress-main idle-copy">
        <p>{{ exportStatusNotice }}</p>
        <dl>
          <div>
            <dt>现场盘位</dt>
            <dd>{{ disks.length }} 盘位</dd>
          </div>
          <div>
            <dt>未注册盘</dt>
            <dd>会展示</dd>
          </div>
          <div>
            <dt>历史批次</dt>
            <dd>同步记录</dd>
          </div>
          <div>
            <dt>浏览器权限</dt>
            <dd>只读</dd>
          </div>
        </dl>
      </div>
    </section>


    <section v-if="selectedDisk && selectedDiskDetailVisible" class="selected-disk-strip glass-panel"
      aria-label="选中磁盘详情">
      <div class="selected-disk-content">
        <strong>{{ selectedDiskTitle }}</strong>
        <dl>
          <div>
            <dt>磁盘 ID</dt>
            <dd>{{ selectedDisk?.disk_id ?? "未返回" }}</dd>
          </div>
          <div>
            <dt>挂载路径</dt>
            <dd>{{ selectedDisk?.mount_path ?? "未返回" }}</dd>
          </div>
          <div>
            <dt>硬盘 SN</dt>
            <dd>{{ selectedDisk ? slotSnLabel(selectedDisk) : "未返回" }}</dd>
          </div>
          <div>
            <dt>系统格式</dt>
            <dd>{{ diskFilesystemDisplay(selectedDisk) }}</dd>
          </div>
          <div>
            <dt>运行状态</dt>
            <dd class="tone-running">{{ selectedDisk ? diskStatusLabel(selectedDisk) : "未返回" }}</dd>
          </div>
          <div>
            <dt>盘内状态</dt>
            <dd class="tone-running">{{ diskLifecycleStatusLabel(selectedDisk) }}</dd>
          </div>
          <div v-if="hasSelectedCopyTask">
            <dt>拷贝进度</dt>
            <dd>{{ selectedProgressPercent.toFixed(2) }}%</dd>
          </div>
          <div v-if="hasSelectedCopyTask">
            <dt>当前文件</dt>
            <dd :title="selectedCurrentObjectName">{{ selectedCurrentObjectName }}</dd>
          </div>
          <div>
            <dt>已用/总容量</dt>
            <dd>{{ selectedDisk ? formatTbPair(selectedDisk) : "0.00/0.00 TB" }}</dd>
          </div>
        </dl>
      </div>
      <button class="selected-disk-close" type="button" aria-label="关闭选中磁盘详情" title="关闭" @pointerdown.stop
        @click.stop="clearSelectedDisk">
        ×
      </button>
    </section>

    <section v-if="!httpError" :class="['dashboard-lower-grid', { compact: !hasCurrentExport }]">
      <article class="overview-panel glass-panel">
        <h2>扫描与导出概览</h2>
        <dl class="overview-metrics">
          <div>
            <dt>对象总数</dt>
            <dd>{{ rustFsObjectTotal.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>总数据量</dt>
            <dd>{{ rustFsTotalBytesLabel }}</dd>
          </div>
          <div>
            <dt>已导出对象</dt>
            <dd>{{ exportedInventoryObjectCount.toLocaleString() }}</dd>
          </div>
          <div>
            <dt>已导出数据量</dt>
            <dd>{{ exportedInventoryBytesLabel }}</dd>
          </div>
        </dl>
      </article>

      <article class="object-panel object-wide-panel glass-panel">
        <div class="object-current-block">
          <h2>当前对象与异常处理</h2>
          <div v-if="selectedDisk" class="object-detail-grid">
            <dl>
              <dt>对象路径</dt>
              <dd>{{ selectedDisk.current_object?.key ?? "暂无" }}</dd>
              <dt>对象状态</dt>
              <dd class="tone-running">{{ selectedDisk.current_object?.object_status ?? "等待对象" }}</dd>
              <dt>剩余大小</dt>
              <dd>{{ formatBytes(selectedDisk.current_object?.remaining_bytes ?? 0) }} / {{
                formatBytes(selectedDisk.current_object?.size_bytes ?? 0) }}</dd>
            </dl>
            <dl>
              <dt>传输速度</dt>
              <dd>{{ formatSpeed(selectedDisk.speed_bytes_per_sec ?? 0) }}</dd>
              <dt>对象标识</dt>
              <dd>{{ selectedDisk.current_object?.key ?? "未返回" }}</dd>
            </dl>
            <dl>
              <dt>加密状态</dt>
              <dd>{{ selectedDisk.current_object ? "已加密" : "未返回" }}</dd>
              <dt>写入阶段</dt>
              <dd>{{ selectedDisk.runtime_status }}</dd>
              <dt>校验状态</dt>
              <dd>{{ diskLifecycleStatusLabel(selectedDisk) }}</dd>
            </dl>
          </div>
          <p v-else class="object-empty">
            未选中运输盘。未注册或异常盘只有在 Edge 后端检测并返回后才会显示。
          </p>
          <div class="object-progress-row">
            <div class="progress-track object-progress"><b :style="{ width: `${currentObjectProgressPercent}%` }"></b>
            </div>
            <span>{{ currentObjectProgressPercent.toFixed(2) }}%</span>
          </div>
        </div>
      </article>
    </section>
  </main>
</template>
