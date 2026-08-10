<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  diskStatusDisplay,
  fetchEdgeDashboardSummary,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
  type RuntimeStatus,
} from "../api/edgeDashboard";
import {
  applyCopyProgressEvent,
  connectEdgeProgressSocket,
  type EdgeProgressSocket,
} from "../ws/edgeCopyProgress";

const previewDiskConfigs: Array<{
  doneTb: number;
  errorMessage?: string;
  lastErrorCode?: string;
  message?: string;
  status: RuntimeStatus;
}> = [
  { status: "COPYING", doneTb: 4.32 },
  { status: "COPYING", doneTb: 3.66 },
  { status: "COPYING", doneTb: 2.88 },
  { status: "COPYING", doneTb: 0.9 },
  { status: "READY", doneTb: 6, message: "就绪" },
  { status: "COPYING", doneTb: 5.34 },
  { status: "REJECTED", doneTb: 0, lastErrorCode: "FILESYSTEM_UNSUPPORTED", errorMessage: "写入被不支持", message: "拒绝" },
  { status: "READY", doneTb: 6, message: "就绪" },
  { status: "ERROR", doneTb: 0, lastErrorCode: "DISK_FULL", errorMessage: "写入错误", message: "错误" },
  { status: "COPYING", doneTb: 1.98 },
  { status: "REMOVED", doneTb: 0, message: "已移除" },
  { status: "READY", doneTb: 6, message: "就绪" },
  { status: "READY", doneTb: 6, message: "就绪" },
  { status: "COPYING", doneTb: 0.42 },
  { status: "READY", doneTb: 6, message: "就绪" },
  { status: "READY", doneTb: 6, message: "就绪" },
];

const previewDisks: EdgeDiskProgress[] = previewDiskConfigs.map((config, index) => {
  const id = index + 1;
  const status = config.status;
  const done = config.doneTb;
  const total = status === "REMOVED" ? 0 : 6;
  return {
    disk_id: `disk-${String(id).padStart(2, "0")}`,
    disk_sn: `SN...${["7F22", "51C8", "0D16", "8F2A", "5C19", "286E"][index % 6]}`,
    hardware_serial: `SN-${id}-20260721`,
    mount_path: `/media/edge/disk-${id}`,
    device_path: `/dev/sd${String.fromCharCode(98 + index)}`,
    filesystem: "ext4",
    filesystem_uuid: `0878ee5b-${String(id).padStart(4, "0")}-4ae0-8d0f-461c2732ee42`,
    disk_status_code: status === "COPYING" ? "EDGE_COPYING" : "INITIALIZED",
    runtime_status: status,
    total_bytes: total * 1_000_000_000_000,
    done_bytes: done * 1_000_000_000_000,
    remaining_bytes: Math.max(0, total - done) * 1_000_000_000_000,
    free_bytes: Math.max(0, total - done) * 1_000_000_000_000,
    speed_bytes_per_sec: status === "COPYING" ? 168_500_000 : 0,
    object_total: 8_041_000,
    object_done: status === "COPYING" ? 2_315_000 + index * 11_000 : status === "READY" ? 8_041_000 : 0,
    object_remaining: status === "READY" ? 0 : 5_726_000,
    current_object:
      status === "COPYING"
        ? {
            bucket: "media-bucket",
            key: "video/2026/07/sample_0001.mp4",
            display_name: "sample_0001.mp4",
            relative_data_path: "data/media-bucket/video/2026/07/sample_0001.mp4",
            size_bytes: 12_680_000_000,
            done_bytes: 2_340_000_000,
            remaining_bytes: 10_340_000_000,
            speed_bytes_per_sec: 168_500_000,
            object_status: "COPYING",
          }
        : null,
    last_error_code: config.lastErrorCode,
    error_message: config.errorMessage,
    message: config.message ?? (status === "REMOVED" ? "已移除" : status === "READY" ? "就绪" : status === "COPYING" ? "正在复制" : "需要处理"),
  };
});

const previewSummary: EdgeDashboardSummary = {
  source: "edge",
  edge_code: "edge-src-a-01",
  edge_name: "Edge 工厂 A",
  edge_status: "ACTIVE",
  export_job_id: "A-20260721-009",
  export_job_status: "COPYING",
  disk_status_code: "EDGE_COPYING",
  scan: {
    scan_event_type: "SCAN_DONE",
    scanned_bucket_count: 12,
    scanned_object_count: 128_532_118,
    scanned_bytes: 12_520_000_000_000,
    stable_object_count: 118_623_004,
    skipped_object_count: 12_348,
    current_bucket: "media-bucket",
    current_key: "video/2026/07/sample_0001.mp4",
    last_scan_at: "2026-07-21T06:32:08Z",
    message: "扫描完成",
  },
  global_progress: {
    total_bytes: 12_130_000_000_000,
    done_bytes: 8_240_000_000_000,
    remaining_bytes: 3_890_000_000_000,
    speed_bytes_per_sec: 1_460_000_000,
    object_total: 128_684_912,
    object_done: 98_532_118,
    object_remaining: 30_152_794,
  },
  disks: previewDisks,
  ws_connected: true,
  last_http_refresh_at: "2026-07-21T06:32:08Z",
  message: "多盘并行写入中",
};

const summary = ref<EdgeDashboardSummary | null>(null);
const selectedDiskId = ref("disk-02");
const isRefreshing = ref(false);
const httpError = ref<DashboardHttpError | null>(null);
const wsConnected = ref(false);
const wsMessage = ref("WebSocket 尚未连接");
const recoveryMessage = ref("");
let progressSocket: EdgeProgressSocket | null = null;

const viewSummary = computed(() => summary.value ?? previewSummary);
const disks = computed(() => viewSummary.value.disks);
const selectedDisk = computed(
  () => disks.value.find((disk) => disk.disk_id === selectedDiskId.value) ?? disks.value[0] ?? null,
);
const showParticleStream = computed(() => disks.value.some((disk) => disk.runtime_status === "COPYING"));
const globalProgressPercent = computed(() =>
  progressPercent(viewSummary.value.global_progress.done_bytes, viewSummary.value.global_progress.total_bytes),
);
const runningDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "COPYING").length);
const readyDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "READY" || disk.runtime_status === "DONE").length);
const removedDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "REMOVED").length);
const rejectedDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "REJECTED").length);
const errorDisks = computed(() => disks.value.filter((disk) => disk.runtime_status === "ERROR").length);
const recoveryDisks = computed(() => rejectedDisks.value + errorDisks.value);
const otherWarningDisks = computed(() => disks.value.filter((disk) => disk.last_error_code === "INSUFFICIENT_SPACE").length || (summary.value ? 0 : 1));
const selectedProgressPercent = computed(() =>
  progressPercent(selectedDisk.value?.done_bytes ?? 0, selectedDisk.value?.total_bytes ?? 0),
);
const lastHeartbeat = computed(() => (wsConnected.value ? "2 秒前" : "等待连接"));
const lastUpdate = computed(() => formatClock(viewSummary.value.last_http_refresh_at || new Date().toISOString()));
const estimatedDone = computed(() => {
  const speed = viewSummary.value.global_progress.speed_bytes_per_sec;
  if (speed <= 0) return "等待速度";
  return formatDuration(Math.ceil(viewSummary.value.global_progress.remaining_bytes / speed));
});

watch(
  () => disks.value.map((disk) => disk.disk_id).join("|"),
  () => {
    if (!disks.value.some((disk) => disk.disk_id === selectedDiskId.value)) {
      selectedDiskId.value = disks.value[0]?.disk_id ?? "";
    }
  },
  { immediate: true },
);

async function refreshFromHttpSummary() {
  isRefreshing.value = true;
  httpError.value = null;
  recoveryMessage.value = "";
  try {
    summary.value = await fetchEdgeDashboardSummary();
  } catch (error) {
    httpError.value =
      error instanceof DashboardHttpError
        ? error
        : new DashboardHttpError("DASHBOARD_UNAVAILABLE", error instanceof Error ? error.message : "HTTP summary unavailable");
  } finally {
    isRefreshing.value = false;
  }
}

async function runRecoveryCheck() {
  const exportJobId = viewSummary.value.export_job_id;
  if (!exportJobId) return;
  recoveryMessage.value = "正在请求恢复检查";
  try {
    const response = await fetch(`/api/edge/export-jobs/${encodeURIComponent(exportJobId)}/recover`, { method: "POST" });
    recoveryMessage.value = response.ok ? "恢复检查已触发" : `恢复检查接口返回 HTTP ${response.status}`;
  } catch (error) {
    recoveryMessage.value = error instanceof Error ? error.message : "恢复检查接口不可用";
  }
}

function selectDisk(disk: EdgeDiskProgress) {
  selectedDiskId.value = disk.disk_id;
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

function formatPercent(doneBytes: number, totalBytes: number): string {
  return `${progressPercent(doneBytes, totalBytes).toFixed(1)}%`;
}

function formatClock(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "14:32:08";
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
      summary.value = summary.value ? applyCopyProgressEvent(summary.value, event) : applyCopyProgressEvent(previewSummary, event);
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
      <span class="status-pill ok"><i></i> HTTP 服务：{{ httpError ? "异常" : "已就绪" }}</span>
      <span class="status-pill live"><i></i> WebSocket：{{ wsConnected ? "已连接" : "重连中" }}</span>
      <span class="status-pill quiet">最后心跳：{{ lastHeartbeat }}</span>
      <span class="last-update">最后更新 {{ lastUpdate }}</span>
      <button aria-label="刷新 Dashboard" class="icon-refresh" :disabled="isRefreshing" type="button" @click="refreshFromHttpSummary">↻</button>
    </section>

    <section class="runtime-stage" aria-label="Edge 导出运行态">
      <div class="source-meta">
        <strong>源服务器（RustFS） <i></i> 运行中</strong>
        <span>{{ viewSummary.edge_code }}</span>
        <button type="button">查看源端详情</button>
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
          <small><i></i> 固件版本 1.2.0</small>
        </header>
        <div class="nas-shell">
          <img alt="" src="/assets/fustfs-baseline/transport-nas-cutout-v3.webp" />
          <div class="disk-slot-matrix">
            <button
              v-for="(disk, index) in disks"
              :key="disk.disk_id"
              :aria-pressed="selectedDisk?.disk_id === disk.disk_id"
              :class="[`slot-${diskTone(disk)}`, { selected: selectedDisk?.disk_id === disk.disk_id }]"
              type="button"
              @click="selectDisk(disk)"
            >
              <b>{{ String(index + 1).padStart(2, "0") }}</b>
              <strong v-if="disk.runtime_status === 'COPYING'">{{ formatPercent(disk.done_bytes, disk.total_bytes) }}</strong>
              <strong v-else>{{ disk.message }}</strong>
              <span>{{ disk.runtime_status === "REMOVED" ? "已移除" : disk.last_error_code ?? formatBytes(disk.done_bytes) }}</span>
              <small>{{ disk.runtime_status === "COPYING" ? formatBytes(disk.total_bytes) : disk.error_message ?? formatBytes(disk.free_bytes) }}</small>
            </button>
          </div>
        </div>
      </div>
    </section>

    <section class="global-progress glass-panel">
      <div>
        <span>全局导出进度</span>
        <strong>{{ globalProgressPercent.toFixed(0) }}<small>%</small></strong>
        <em>总进度</em>
      </div>
      <div class="progress-main">
        <p>
          <span>已完成 <b>{{ formatBytes(viewSummary.global_progress.done_bytes) }}</b> / {{ formatBytes(viewSummary.global_progress.total_bytes) }}</span>
          <span>剩余 <b>{{ formatBytes(viewSummary.global_progress.remaining_bytes) }}</b></span>
          <span>速度 <b>{{ formatSpeed(viewSummary.global_progress.speed_bytes_per_sec) }}</b></span>
        </p>
        <div class="progress-track"><b :style="{ width: `${globalProgressPercent}%` }"></b></div>
        <dl>
          <div><dt>文件</dt><dd>{{ viewSummary.global_progress.object_total.toLocaleString() }}</dd></div>
          <div><dt>对象</dt><dd>{{ viewSummary.global_progress.object_done.toLocaleString() }}</dd></div>
          <div><dt>批次</dt><dd>{{ viewSummary.export_job_id }}</dd></div>
          <div><dt>预计完成</dt><dd>{{ estimatedDone }}</dd></div>
        </dl>
      </div>
    </section>

    <section class="dashboard-grid">
      <article class="overview-panel glass-panel">
        <h2>扫描与导出概览</h2>
        <dl class="metric-strip">
          <div><dt>扫描完成</dt><dd>98.62%</dd><small>{{ formatBytes(viewSummary.scan.scanned_bytes) }}</small></div>
          <div><dt>已发现对象</dt><dd>{{ viewSummary.scan.scanned_object_count.toLocaleString() }}</dd><small>总计对象</small></div>
          <div><dt>已导出对象</dt><dd>{{ viewSummary.global_progress.object_done.toLocaleString() }}</dd><small>{{ globalProgressPercent.toFixed(2) }}%</small></div>
          <div><dt>预计完成</dt><dd>{{ estimatedDone }}</dd><small>剩余时间</small></div>
        </dl>
        <h2>导出前置检查</h2>
        <ul class="check-list">
          <li>源端扫描完成</li>
          <li>运输盘已注册</li>
          <li>加密写入可用</li>
          <li>校验链路正常</li>
          <li>断点续传可用</li>
          <li>封盘前检查通过</li>
        </ul>
      </article>

      <article class="warning-panel glass-panel">
        <h2>异常盘汇总</h2>
        <div class="warning-cards">
          <span><b>{{ recoveryDisks }}</b>需恢复</span>
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
        <dl>
          <dt>对象路径</dt><dd>{{ selectedDisk?.current_object?.key ?? viewSummary.scan.current_key }}</dd>
          <dt>对象状态</dt><dd class="tone-running">{{ selectedDisk?.current_object?.object_status ?? "等待对象" }}</dd>
          <dt>剩余大小</dt><dd>{{ formatBytes(selectedDisk?.current_object?.remaining_bytes ?? 0) }}</dd>
          <dt>传输速度</dt><dd>{{ formatSpeed(selectedDisk?.speed_bytes_per_sec ?? 0) }}</dd>
          <dt>文件系统</dt><dd>{{ selectedDisk?.filesystem ?? "未返回" }}</dd>
          <dt>对象标识</dt><dd>{{ selectedDisk?.current_object?.display_name ?? "未返回" }}</dd>
          <dt>加密状态</dt><dd>已加密</dd>
          <dt>写入阶段</dt><dd>{{ selectedDisk?.runtime_status ?? "未返回" }}</dd>
          <dt>校验状态</dt><dd>校验中</dd>
        </dl>
        <div class="progress-track object-progress"><b :style="{ width: `${selectedProgressPercent}%` }"></b></div>
        <span class="object-percent">{{ selectedProgressPercent.toFixed(2) }}%</span>
      </article>
    </section>

    <section class="dashboard-actions glass-panel">
      <p><i>i</i> 提示：支持恢复导出作业、断点续传与完整性验证。建议在运输前完成封盘（Seal Disk）操作。</p>
      <button class="primary-action" type="button">查看选中盘详情</button>
      <button type="button" @click="runRecoveryCheck">执行恢复检查</button>
      <small v-if="recoveryMessage">{{ recoveryMessage }}</small>
      <small v-else-if="httpError">{{ httpError.error_code }} · 当前展示视觉预览数据</small>
      <small v-else>{{ wsMessage }}</small>
    </section>
  </main>
</template>
