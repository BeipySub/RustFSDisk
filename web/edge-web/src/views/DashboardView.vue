<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  DashboardHttpError,
  fetchEdgeDashboardSummary,
  type EdgeDashboardSummary,
  type EdgeDiskProgress,
} from "../api/edgeDashboard";
import {
  applyCopyProgressEvent,
  connectEdgeProgressSocket,
  edgeDashboardWsPath,
  type EdgeProgressSocket,
} from "../ws/edgeCopyProgress";

const summary = ref<EdgeDashboardSummary | null>(null);
const isRefreshing = ref(false);
const refreshMessage = ref("");
const httpError = ref<DashboardHttpError | null>(null);
const wsMessage = ref("WebSocket 尚未连接");
let progressSocket: EdgeProgressSocket | null = null;

const byteFormatter = new Intl.NumberFormat("zh-CN", {
  maximumFractionDigits: 1,
});

const percentFormatter = new Intl.NumberFormat("zh-CN", {
  maximumFractionDigits: 1,
});

const runningDisks = computed(
  () => summary.value?.disks.filter((disk) => disk.runtime_status === "COPYING").length ?? 0,
);

const rejectedDisks = computed(
  () =>
    summary.value?.disks.filter(
      (disk) => disk.runtime_status === "REJECTED" || disk.runtime_status === "ERROR",
    ).length ?? 0,
);

const diskCount = computed(() => summary.value?.disks.length ?? 0);
const isEmpty = computed(
  () =>
    !isRefreshing.value &&
    !httpError.value &&
    summary.value !== null &&
    summary.value.disks.length === 0 &&
    summary.value.global_progress.total_bytes === 0,
);

const globalProgressPercent = computed(() => {
  const progress = summary.value?.global_progress;
  if (!progress || progress.total_bytes === 0) {
    return 0;
  }

  return Math.min(100, (progress.done_bytes / progress.total_bytes) * 100);
});

function formatBytes(bytes: number): string {
  if (bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${byteFormatter.format(bytes / 1024 ** unitIndex)} ${units[unitIndex]}`;
}

function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

function formatPercent(doneBytes: number, totalBytes: number): string {
  if (totalBytes === 0) {
    return "0%";
  }

  return `${percentFormatter.format(Math.min(100, (doneBytes / totalBytes) * 100))}%`;
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) {
    return "未知";
  }

  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

function diskTone(disk: EdgeDiskProgress): string {
  if (disk.runtime_status === "COPYING") {
    return "tone-active";
  }

  if (disk.runtime_status === "READY" || disk.runtime_status === "DONE") {
    return "tone-ready";
  }

  if (disk.runtime_status === "REMOVED" || disk.runtime_status === "ERROR") {
    return "tone-danger";
  }

  if (disk.runtime_status === "REJECTED") {
    return "tone-warning";
  }

  return "tone-muted";
}

async function refreshFromHttpSummary() {
  isRefreshing.value = true;
  refreshMessage.value = "";
  httpError.value = null;

  try {
    summary.value = await fetchEdgeDashboardSummary();
    refreshMessage.value = "HTTP 汇总已恢复";
  } catch (error) {
    httpError.value =
      error instanceof DashboardHttpError
        ? error
        : new DashboardHttpError(
            "SUMMARY_UNAVAILABLE",
            error instanceof Error ? error.message : "HTTP summary unavailable",
          );
  } finally {
    isRefreshing.value = false;
  }
}

onMounted(() => {
  void refreshFromHttpSummary();
  progressSocket = connectEdgeProgressSocket({
    onEvent(event) {
      if (!summary.value) {
        return;
      }
      summary.value = applyCopyProgressEvent(summary.value, event);
      wsMessage.value = event.message;
    },
    onConnectionChange(connected, message) {
      wsMessage.value = message;
      if (summary.value) {
        summary.value = { ...summary.value, ws_connected: connected };
      }
    },
  });
});

onBeforeUnmount(() => {
  progressSocket?.close();
});
</script>

<template>
  <main class="dashboard">
    <section class="topbar">
      <div>
        <p class="eyebrow">{{ summary?.edge_code ?? "RustFS Transfer Edge" }}</p>
        <h1>{{ summary?.edge_name ?? "边缘端 Dashboard" }}</h1>
      </div>
      <button class="refresh-button" :disabled="isRefreshing" type="button" @click="refreshFromHttpSummary">
        {{ isRefreshing ? "恢复中" : "恢复 HTTP 汇总" }}
      </button>
    </section>

    <section class="connection-strip" aria-label="数据连接状态">
      <div>
        <span>HTTP</span>
        <strong>{{ httpError ? httpError.error_code : isRefreshing ? "LOADING" : "READY" }}</strong>
        <small>
          {{
            httpError
              ? `${httpError.message}；等待后端同源 summary 联调`
              : refreshMessage || "刷新会重新读取本端 summary"
          }}
        </small>
      </div>
      <div>
        <span>WebSocket</span>
        <strong>{{ summary?.ws_connected ? "CONNECTED" : "RECONNECTING" }}</strong>
        <small>{{ wsMessage }} · {{ edgeDashboardWsPath() }}</small>
      </div>
    </section>

    <section v-if="isRefreshing && !summary" class="state-panel">
      <strong>正在加载本端 HTTP summary</strong>
      <span>页面不会携带 Edge-Control token；如后端仍要求控制 token，请提供只读 Dashboard summary 或同源代理。</span>
    </section>

    <section v-else-if="httpError && !summary" class="state-panel error-state">
      <strong>{{ httpError.error_code }}</strong>
      <span>{{ httpError.message }}</span>
    </section>

    <section v-else-if="isEmpty" class="state-panel">
      <strong>暂无运行中导出数据</strong>
      <span>HTTP summary 已返回，但还没有运输盘或导出任务；等待插盘、扫描或 COPY_PROGRESS 事件。</span>
    </section>

    <template v-if="summary">
      <section class="summary-band" aria-label="边缘端导出汇总">
      <article class="metric">
        <span>扫描</span>
        <strong>{{ summary.scan.scan_event_type }}</strong>
        <small>{{ summary.scan.scanned_object_count }} 对象 / {{ formatBytes(summary.scan.scanned_bytes) }}</small>
      </article>
      <article class="metric">
        <span>导出任务</span>
        <strong>{{ summary.export_job_status }}</strong>
        <small>{{ summary.export_job_id }}</small>
      </article>
      <article class="metric">
        <span>运输盘</span>
        <strong>{{ runningDisks }} / {{ diskCount }}</strong>
        <small>{{ rejectedDisks }} 块异常或拒绝</small>
      </article>
      <article class="metric">
        <span>WebSocket</span>
        <strong>{{ summary.ws_connected ? "CONNECTED" : "DISCONNECTED" }}</strong>
        <small>上次 HTTP {{ formatTime(summary.last_http_refresh_at) }}</small>
      </article>
      </section>

      <section class="progress-panel" aria-label="全局拷贝进度">
      <div class="panel-heading">
        <div>
          <p class="eyebrow">COPY_PROGRESS</p>
          <h2>多盘导出进度</h2>
        </div>
        <span class="progress-percent">{{ formatPercent(summary.global_progress.done_bytes, summary.global_progress.total_bytes) }}</span>
      </div>
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: `${globalProgressPercent}%` }"></div>
      </div>
      <div class="progress-stats">
        <span>{{ formatBytes(summary.global_progress.done_bytes) }} 已完成</span>
        <span>{{ formatBytes(summary.global_progress.remaining_bytes) }} 剩余</span>
        <span>{{ formatSpeed(summary.global_progress.speed_bytes_per_sec) }}</span>
        <span>{{ summary.global_progress.object_done }} / {{ summary.global_progress.object_total }} 对象</span>
      </div>
      </section>

      <section class="scan-strip" aria-label="扫描状态">
      <div>
        <span>当前 bucket</span>
        <strong>{{ summary.scan.current_bucket }}</strong>
      </div>
      <div>
        <span>当前对象</span>
        <strong>{{ summary.scan.current_key }}</strong>
      </div>
      <div>
        <span>稳定 / 跳过</span>
        <strong>{{ summary.scan.stable_object_count }} / {{ summary.scan.skipped_object_count }}</strong>
      </div>
      </section>

      <section class="disk-grid" aria-label="运输盘列表">
      <article v-for="disk in summary.disks" :key="disk.disk_id" class="disk-card" :class="diskTone(disk)">
        <header>
          <div>
            <p class="eyebrow">{{ disk.disk_sn }}</p>
            <h3>{{ disk.disk_id }}</h3>
          </div>
          <span class="runtime-pill">{{ disk.runtime_status }}</span>
        </header>

        <dl class="disk-meta">
          <div>
            <dt>生命周期</dt>
            <dd>{{ disk.disk_status_code ?? "待后端补充" }}</dd>
          </div>
          <div>
            <dt>挂载点</dt>
            <dd>{{ disk.mount_path }}</dd>
          </div>
          <div>
            <dt>设备</dt>
            <dd>{{ disk.device_path ?? "未知" }}</dd>
          </div>
          <div>
            <dt>文件系统</dt>
            <dd>{{ disk.filesystem ?? "未知" }}</dd>
          </div>
          <div>
            <dt>硬件 SN</dt>
            <dd>{{ disk.hardware_serial ?? disk.disk_sn }}</dd>
          </div>
          <div>
            <dt>ID_SERIAL</dt>
            <dd>{{ disk.id_serial ?? disk.stable_hardware_id ?? "未读取" }}</dd>
          </div>
          <div>
            <dt>FS UUID</dt>
            <dd>{{ disk.filesystem_uuid ?? "未读取" }}</dd>
          </div>
          <div>
            <dt>PARTUUID</dt>
            <dd>{{ disk.partition_uuid ?? "未读取" }}</dd>
          </div>
          <div>
            <dt>型号</dt>
            <dd>{{ [disk.vendor, disk.model].filter(Boolean).join(" ") || "未知" }}</dd>
          </div>
          <div>
            <dt>传输</dt>
            <dd>{{ disk.transport ?? "未知" }}</dd>
          </div>
          <div>
            <dt>剩余空间</dt>
            <dd>{{ formatBytes(disk.free_bytes) }}</dd>
          </div>
          <div>
            <dt>速度</dt>
            <dd>{{ formatSpeed(disk.speed_bytes_per_sec) }}</dd>
          </div>
        </dl>

        <div class="disk-progress">
          <div class="progress-track compact">
            <div class="progress-fill" :style="{ width: formatPercent(disk.done_bytes, disk.total_bytes) }"></div>
          </div>
          <div class="progress-stats compact">
            <span>{{ formatPercent(disk.done_bytes, disk.total_bytes) }}</span>
            <span>{{ formatBytes(disk.remaining_bytes) }} 待拷贝</span>
            <span>{{ disk.object_done }} / {{ disk.object_total }}</span>
          </div>
        </div>

        <div v-if="disk.current_object" class="current-object">
          <span>当前对象</span>
          <strong>{{ disk.current_object.display_name }}</strong>
          <small>{{ disk.current_object.bucket }}/{{ disk.current_object.key }}</small>
          <div class="object-row">
            <span>{{ disk.current_object.object_status }}</span>
            <span>{{ formatBytes(disk.current_object.remaining_bytes) }} 剩余</span>
            <span>{{ formatSpeed(disk.current_object.speed_bytes_per_sec) }}</span>
          </div>
        </div>

        <p v-else class="empty-object">{{ disk.message }}</p>

        <p v-if="disk.last_error_code" class="error-line">
          <strong>{{ disk.last_error_code }}</strong>
          {{ disk.error_message }}
        </p>
      </article>
      </section>

      <section class="recovery-panel" aria-label="断线恢复">
      <div>
        <p class="eyebrow">HTTP SUMMARY</p>
        <h2>刷新后恢复入口</h2>
      </div>
      <p>{{ refreshMessage || summary.message }}</p>
      </section>
    </template>
  </main>
</template>
