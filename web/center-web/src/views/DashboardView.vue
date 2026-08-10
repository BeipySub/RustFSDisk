<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import {
  DashboardHttpError,
  centerDashboardSummaryPath,
  fetchCenterDashboardSummary,
  type CenterDashboardSummary,
  type CenterDiskProgress,
  type DiskLifecycleCode,
  type RuntimeStatus,
} from "../api/centerDashboard";
import {
  applyImportProgressEvent,
  centerDashboardWsPath,
  connectCenterProgressSocket,
  type CenterProgressSocket,
} from "../ws/centerImportProgress";

const summary = ref<CenterDashboardSummary | null>(null);
const isRefreshing = ref(false);
const refreshMessage = ref("");
const httpError = ref<DashboardHttpError | null>(null);
const wsMessage = ref("WebSocket 尚未连接");
let progressSocket: CenterProgressSocket | null = null;

const disks = computed(() => summary.value?.disks ?? []);
const pendingImportDisks = computed(() =>
  disks.value.filter((disk) => disk.disk_status_code === "SEALED" && !disk.imported_before),
);
const activeImportDisks = computed(() =>
  disks.value.filter((disk) => disk.import_job_status === "IMPORTING"),
);
const importedButBlockedDisks = computed(() =>
  disks.value.filter((disk) => disk.last_error_code === "REINIT_FAILED"),
);
const isEmpty = computed(
  () =>
    !isRefreshing.value &&
    !httpError.value &&
    summary.value !== null &&
    disks.value.length === 0 &&
    summary.value.global_progress.total_bytes === 0,
);
const overallProgress = computed(() =>
  percent(summary.value?.global_progress.done_bytes ?? 0, summary.value?.global_progress.total_bytes ?? 0),
);

function percent(done: number, total: number) {
  if (total <= 0) {
    return 0;
  }

  return Math.min(100, Math.round((done / total) * 100));
}

function formatBytes(bytes: number) {
  if (bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** exponent;
  return `${value.toFixed(value >= 10 || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

function formatSpeed(bytesPerSecond: number) {
  return `${formatBytes(bytesPerSecond)}/s`;
}

function lifecycleLabel(disk_status_code: DiskLifecycleCode) {
  const labels: Record<DiskLifecycleCode, string> = {
    UNREGISTERED: "未注册",
    REGISTERED: "已注册",
    INITIALIZED: "已初始化",
    EDGE_COPYING: "边缘拷贝中",
    SEALED: "已封盘待导入",
    CENTER_IMPORTING: "中控导入中",
    IMPORTED: "已导入",
    ERROR: "生命周期异常",
  };
  return labels[disk_status_code];
}

function runtimeLabel(runtime_status: RuntimeStatus) {
  const labels: Record<RuntimeStatus, string> = {
    DETECTED: "已检测",
    CHECKING: "校验中",
    READY: "可处理",
    COPYING: "拷贝中",
    CLEANING: "清理中",
    REINITIALIZING: "重新初始化中",
    DONE: "运行完成",
    REJECTED: "已拒绝",
    REMOVED: "已拔出",
    ERROR: "运行异常",
  };
  return labels[runtime_status];
}

function progressStyle(done: number, total: number) {
  return { width: `${percent(done, total)}%` };
}

function diskTone(disk: CenterDiskProgress) {
  if (disk.last_error_code === "REINIT_FAILED" || disk.runtime_status === "ERROR") {
    return "tone-danger";
  }
  if (disk.import_job_status === "IMPORTING" || disk.runtime_status === "CLEANING") {
    return "tone-active";
  }
  if (disk.disk_status_code === "SEALED") {
    return "tone-warning";
  }
  return "tone-muted";
}

async function refreshFromHttpSummary() {
  isRefreshing.value = true;
  refreshMessage.value = "";
  httpError.value = null;

  try {
    summary.value = await fetchCenterDashboardSummary();
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
  progressSocket = connectCenterProgressSocket({
    onEvent(event) {
      if (!summary.value) {
        return;
      }
      summary.value = applyImportProgressEvent(summary.value, event);
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
    <section class="toolbar">
      <div>
        <p class="eyebrow">RustFS Transfer Center</p>
        <h1>中控端 Dashboard</h1>
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
              : refreshMessage || centerDashboardSummaryPath()
          }}
        </small>
      </div>
      <div>
        <span>WebSocket</span>
        <strong>{{ summary?.ws_connected ? "CONNECTED" : "RECONNECTING" }}</strong>
        <small>{{ wsMessage }} · {{ centerDashboardWsPath() }}</small>
      </div>
    </section>

    <section v-if="isRefreshing && !summary" class="state-panel">
      <strong>正在加载本端 HTTP summary</strong>
      <span>中控端页面只读取 Dashboard summary，不直接调用注册、初始化、清理或重新初始化等高风险 API。</span>
    </section>

    <section v-else-if="httpError && !summary" class="state-panel error-state">
      <strong>{{ httpError.error_code }}</strong>
      <span>{{ httpError.message }}</span>
    </section>

    <section v-else-if="isEmpty" class="state-panel">
      <strong>暂无导入数据</strong>
      <span>HTTP summary 已返回，但当前没有运输盘或导入任务；等待插盘或 IMPORT_PROGRESS 事件。</span>
    </section>

    <template v-if="summary">
      <section class="summary-grid" aria-label="中控端导入汇总">
        <article class="summary-card">
          <span>待导入盘</span>
          <strong>{{ pendingImportDisks.length }}</strong>
          <small>SEALED 且未导入过</small>
        </article>
        <article class="summary-card">
          <span>导入中</span>
          <strong>{{ activeImportDisks.length }}</strong>
          <small>{{ formatSpeed(summary.global_progress.speed_bytes_per_sec) }}</small>
        </article>
        <article class="summary-card">
          <span>总进度</span>
          <strong>{{ overallProgress }}%</strong>
          <small>{{ summary.global_progress.object_done }} / {{ summary.global_progress.object_total }} 对象</small>
        </article>
        <article class="summary-card warning">
          <span>复用阻塞</span>
          <strong>{{ importedButBlockedDisks.length }}</strong>
          <small>REINIT_FAILED 不可复用</small>
        </article>
      </section>

      <section class="panel">
        <div class="panel-heading">
          <div>
            <p class="eyebrow">Import Queue</p>
            <h2>待导入运输盘</h2>
          </div>
          <span>{{ pendingImportDisks.length }} disks</span>
        </div>

        <div v-if="pendingImportDisks.length" class="queue-list">
          <article v-for="disk in pendingImportDisks" :key="disk.disk_id" class="queue-row">
            <div>
              <strong>{{ disk.disk_id }}</strong>
              <span>{{ disk.edge_code }} · {{ disk.seal_id }}</span>
            </div>
            <span class="state-pill">{{ lifecycleLabel(disk.disk_status_code) }}</span>
          </article>
        </div>
        <p v-else class="empty">当前没有等待导入的 SEALED 运输盘。</p>
      </section>

      <section class="disk-grid" aria-label="运输盘明细">
        <article v-for="disk in disks" :key="disk.disk_id" class="disk-card" :class="diskTone(disk)">
          <div class="disk-card-head">
            <div>
              <h2>{{ disk.disk_id }}</h2>
              <p>{{ disk.disk_sn }} · {{ disk.edge_code }}</p>
            </div>
            <span class="state-pill" :class="{ danger: disk.last_error_code === 'REINIT_FAILED' }">
              {{ lifecycleLabel(disk.disk_status_code) }}
            </span>
          </div>

          <dl class="meta-grid">
            <div>
              <dt>注册</dt>
              <dd>{{ disk.registered ? "已注册" : "未注册" }}</dd>
            </div>
            <div>
              <dt>启用</dt>
              <dd>{{ disk.disk_enabled ? "启用" : "未启用" }}</dd>
            </div>
            <div>
              <dt>可初始化</dt>
              <dd>{{ disk.can_initialize ? "是" : "否" }}</dd>
            </div>
            <div>
              <dt>运行态</dt>
              <dd>{{ runtimeLabel(disk.runtime_status) }}</dd>
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
              <dt>导入任务</dt>
              <dd>{{ disk.import_job_status ?? "无" }}</dd>
            </div>
            <div>
              <dt>可复用</dt>
              <dd>{{ disk.reusable ? "是" : "否" }}</dd>
            </div>
          </dl>

          <div class="progress-block">
            <div class="progress-top">
              <span>导入进度</span>
              <strong>{{ percent(disk.done_bytes, disk.total_bytes) }}%</strong>
            </div>
            <div class="progress-track" aria-hidden="true">
              <span :style="progressStyle(disk.done_bytes, disk.total_bytes)" />
            </div>
            <div class="progress-foot">
              <span>{{ formatBytes(disk.done_bytes) }} / {{ formatBytes(disk.total_bytes) }}</span>
              <span>{{ disk.object_done }} / {{ disk.object_total }} 对象</span>
            </div>
          </div>

          <div class="object-strip">
            <span>当前对象</span>
            <strong>{{ disk.current_object?.display_name ?? "无" }}</strong>
            <small v-if="disk.current_object">
              {{ disk.current_object.bucket }}/{{ disk.current_object.key }}
            </small>
          </div>

          <dl class="meta-grid compact">
            <div>
              <dt>速度</dt>
              <dd>{{ formatSpeed(disk.speed_bytes_per_sec) }}</dd>
            </div>
            <div>
              <dt>当前对象剩余</dt>
              <dd>{{ formatBytes(disk.current_object?.remaining_bytes ?? 0) }}</dd>
            </div>
          </dl>

          <p v-if="disk.last_error_code" class="error-note" :class="{ critical: disk.last_error_code === 'REINIT_FAILED' }">
            {{ disk.last_error_code }}：{{ disk.error_message }}
          </p>
          <p class="message">{{ disk.message }}</p>
        </article>
      </section>
    </template>
  </main>
</template>
