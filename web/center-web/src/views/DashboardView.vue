<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  centerDashboardSummaryPath,
  fetchCenterDashboardSummary,
  type CenterDashboardSummary,
  type CenterDiskProgress,
  type DiskLifecycleCode,
  type ImportJobStatus,
  type RuntimeStatus,
} from "../api/centerDashboard";
import {
  applyImportProgressEvent,
  centerDashboardWsPath,
  connectCenterProgressSocket,
  type CenterProgressSocket,
} from "../ws/centerImportProgress";

type ActionTone = "primary" | "warning" | "danger" | "quiet";

interface DiskAction {
  label: string;
  detail: string;
  tone: ActionTone;
  endpoint: string;
  enabled: boolean;
}

const summary = ref<CenterDashboardSummary | null>(null);
const isRefreshing = ref(false);
const refreshMessage = ref("");
const httpError = ref<DashboardHttpError | null>(null);
const wsMessage = ref("WebSocket 尚未连接");
const selectedDiskId = ref("");
const actionMessage = ref("选择运输盘后显示可执行动作");
const isDevMode = import.meta.env.DEV;
let progressSocket: CenterProgressSocket | null = null;

const disks = computed(() => summary.value?.disks ?? []);
const pendingImportDisks = computed(() =>
  disks.value.filter((disk) => disk.disk_status_code === "SEALED" && !disk.imported_before),
);
const activeImportDisks = computed(() =>
  disks.value.filter((disk) => disk.import_job_status === "IMPORTING"),
);
const reusableDisks = computed(() =>
  disks.value.filter((disk) => disk.disk_status_code === "INITIALIZED" && disk.reusable),
);
const importedButBlockedDisks = computed(() =>
  disks.value.filter((disk) => disk.last_error_code === "REINIT_FAILED"),
);
const abnormalDisks = computed(() =>
  disks.value.filter((disk) => disk.runtime_status === "ERROR" || disk.disk_status_code === "ERROR" || disk.last_error_code),
);
const selectedDisk = computed(() =>
  disks.value.find((disk) => disk.disk_id === selectedDiskId.value) ??
  activeImportDisks.value[0] ??
  pendingImportDisks.value[0] ??
  disks.value[0],
);
const centerName = computed(() => summary.value?.center_name || "Center 控制中心");
const overallProgress = computed(() =>
  percent(summary.value?.global_progress.done_bytes ?? 0, summary.value?.global_progress.total_bytes ?? 0),
);
const isEmpty = computed(
  () =>
    !isRefreshing.value &&
    !httpError.value &&
    summary.value !== null &&
    disks.value.length === 0 &&
    summary.value.global_progress.total_bytes === 0,
);
const diskSlots = computed(() => {
  const base = disks.value.slice(0, 16);
  return Array.from({ length: 16 }, (_, index) => {
    const disk = base[index];
    return {
      index: index + 1,
      disk,
      label: String(index + 1).padStart(2, "0"),
      className: disk ? slotTone(disk) : "slot-empty",
    };
  });
});
const currentObject = computed(() => selectedDisk.value?.current_object);
const selectedDiskAction = computed(() =>
  selectedDisk.value ? primaryActionForDisk(selectedDisk.value) : null,
);

watch(disks, (next) => {
  if (!next.length) {
    selectedDiskId.value = "";
    return;
  }
  if (!next.some((disk) => disk.disk_id === selectedDiskId.value)) {
    selectedDiskId.value = activeImportDisks.value[0]?.disk_id ?? pendingImportDisks.value[0]?.disk_id ?? next[0].disk_id;
  }
});

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

function formatTime(value?: string) {
  if (!value) {
    return "--";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function shortId(value?: string) {
  if (!value) {
    return "--";
  }
  if (value.length <= 12) {
    return value;
  }
  return `${value.slice(0, 8)}...${value.slice(-4)}`;
}

function lifecycleLabel(disk_status_code: DiskLifecycleCode) {
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
  return labels[disk_status_code];
}

function runtimeLabel(runtime_status: RuntimeStatus) {
  const labels: Record<RuntimeStatus, string> = {
    DETECTED: "已检测",
    CHECKING: "校验中",
    READY: "就绪",
    COPYING: "写盘中",
    CLEANING: "清理中",
    REINITIALIZING: "重初始化中",
    DONE: "完成",
    REJECTED: "已拒绝",
    REMOVED: "已拔出",
    ERROR: "运行异常",
  };
  return labels[runtime_status];
}

function importStatusLabel(import_job_status?: ImportJobStatus) {
  const labels: Record<ImportJobStatus, string> = {
    PENDING: "待导入",
    IMPORTING: "导入中",
    DONE: "已完成",
    FAILED: "失败",
    CANCELLED: "已取消",
  };
  return import_job_status ? labels[import_job_status] : "无任务";
}

function progressStyle(done: number, total: number) {
  return { width: `${percent(done, total)}%` };
}

function slotProgressStyle(disk?: CenterDiskProgress) {
  if (!disk) {
    return { width: "0%" };
  }
  return progressStyle(disk.done_bytes, disk.total_bytes);
}

function diskTone(disk: CenterDiskProgress) {
  if (disk.last_error_code || disk.runtime_status === "ERROR" || disk.disk_status_code === "ERROR") {
    return "tone-danger";
  }
  if (disk.import_job_status === "IMPORTING" || disk.disk_status_code === "CENTER_IMPORTING") {
    return "tone-active";
  }
  if (disk.disk_status_code === "SEALED" || disk.runtime_status === "CLEANING" || disk.runtime_status === "REINITIALIZING") {
    return "tone-warning";
  }
  if (disk.disk_status_code === "INITIALIZED") {
    return "tone-ready";
  }
  return "tone-muted";
}

function slotTone(disk: CenterDiskProgress) {
  if (disk.last_error_code || disk.runtime_status === "ERROR" || disk.disk_status_code === "ERROR") {
    return "slot-error";
  }
  if (disk.import_job_status === "IMPORTING" || disk.disk_status_code === "CENTER_IMPORTING") {
    return "slot-active";
  }
  if (disk.disk_status_code === "SEALED") {
    return "slot-sealed";
  }
  if (disk.disk_status_code === "INITIALIZED") {
    return "slot-ready";
  }
  if (!disk.registered || disk.disk_status_code === "UNREGISTERED") {
    return "slot-new";
  }
  return "slot-idle";
}

function primaryActionForDisk(disk: CenterDiskProgress): DiskAction {
  if (!disk.registered || disk.disk_status_code === "UNREGISTERED") {
    return {
      label: "注册并初始化",
      detail: "写入 disk_list 后生成 disk_info.json 与数据密钥",
      tone: "primary",
      endpoint: "POST /api/disk/register + /api/disk/initialize",
      enabled: disk.filesystem === "ext4" || !disk.filesystem,
    };
  }
  if (disk.can_initialize && disk.disk_status_code === "REGISTERED") {
    return {
      label: "初始化运输盘",
      detail: "为已登记盘写入可交付边缘端的协议结构",
      tone: "primary",
      endpoint: "POST /api/disk/initialize",
      enabled: disk.filesystem === "ext4" || !disk.filesystem,
    };
  }
  if (disk.disk_status_code === "SEALED" && !disk.imported_before) {
    return {
      label: "开始导入",
      detail: "校验 manifest、解密对象并写入 object_ledger",
      tone: "primary",
      endpoint: "POST /api/center/import-jobs/start",
      enabled: disk.runtime_status !== "ERROR",
    };
  }
  if (disk.disk_status_code === "IMPORTED" || disk.last_error_code === "REINIT_FAILED") {
    return {
      label: "清理并重初始化",
      detail: "只由中控端清理封存数据，成功后回到 INITIALIZED",
      tone: disk.last_error_code === "REINIT_FAILED" ? "warning" : "primary",
      endpoint: `POST /api/center/disks/${disk.disk_id}/reinitialize`,
      enabled: true,
    };
  }
  if (disk.disk_status_code === "INITIALIZED") {
    return {
      label: "可交付边缘端",
      detail: "当前盘可拔出并交给 Edge 写入",
      tone: "quiet",
      endpoint: "无写操作",
      enabled: false,
    };
  }
  return {
    label: "查看异常",
    detail: disk.last_error_code ?? "等待后端状态推进",
    tone: "danger",
    endpoint: "无自动修复",
    enabled: false,
  };
}

function selectDisk(disk?: CenterDiskProgress) {
  if (!disk) {
    return;
  }
  selectedDiskId.value = disk.disk_id;
  actionMessage.value = `${shortId(disk.disk_id)} 已选中`;
}

function previewAction(action: DiskAction) {
  actionMessage.value = `${action.label}：${action.endpoint}`;
}

async function refreshFromHttpSummary() {
  isRefreshing.value = true;
  refreshMessage.value = "";
  httpError.value = null;

  try {
    summary.value = await fetchCenterDashboardSummary();
    refreshMessage.value = "HTTP summary 已刷新";
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
  <main class="dashboard shell">
    <section class="topbar" aria-label="中控端连接状态">
      <div class="brand-lockup">
        <strong>RustFS离线同步中心</strong>
        <span></span>
        <p>{{ centerName }}</p>
      </div>

      <div class="status-cluster">
        <div class="status-chip" :class="{ danger: httpError }">
          <span class="dot"></span>
          <strong>HTTP 服务</strong>
          <small>{{ httpError ? httpError.error_code : isRefreshing ? "加载中" : "已就绪" }}</small>
        </div>
        <div class="status-chip" :class="{ active: summary?.ws_connected }">
          <span class="dot blue"></span>
          <strong>WebSocket</strong>
          <small>{{ summary?.ws_connected ? "已连接" : "重连中" }}</small>
        </div>
        <div class="last-update">
          <span>最后更新</span>
          <strong>{{ formatTime(summary?.event_time ?? summary?.last_http_refresh_at) }}</strong>
        </div>
        <button class="icon-button" :disabled="isRefreshing" type="button" title="刷新" @click="refreshFromHttpSummary">
          <span aria-hidden="true">↻</span>
        </button>
      </div>
    </section>

    <section class="hero-console" aria-label="中控端运输盘控制台">
      <div class="center-node">
        <p class="section-kicker">Center RustFS</p>
        <h1>中控导入控制台</h1>
        <div class="node-visual">
          <img src="/assets/fustfs-baseline/source-rack-cutout-v3.webp" alt="Center server rack" />
        </div>
        <button class="secondary-button" type="button" @click="actionMessage = centerDashboardSummaryPath()">
          查看中控摘要
        </button>
      </div>

      <div class="data-arc" aria-hidden="true">
        <span></span>
        <span></span>
        <span></span>
      </div>

      <div class="bay-node">
        <div class="bay-heading">
          <p class="section-kicker">运输盘位 · 16 盘位</p>
          <span class="firmware-dot">协议版本 1.0</span>
        </div>

        <div class="bay-frame">
          <img src="/assets/fustfs-baseline/transport-nas-cutout-v3.webp" alt="Transport disk bay" />
          <div class="slot-grid">
            <button
              v-for="slot in diskSlots"
              :key="slot.label"
              class="disk-slot"
              :class="[slot.className, { selected: slot.disk?.disk_id === selectedDisk?.disk_id }]"
              type="button"
              @click="selectDisk(slot.disk)"
            >
              <span>{{ slot.label }}</span>
              <strong>{{ slot.disk ? lifecycleLabel(slot.disk.disk_status_code) : "空位" }}</strong>
              <small>{{ slot.disk ? shortId(slot.disk.disk_sn) : "--" }}</small>
              <i :style="slotProgressStyle(slot.disk)"></i>
            </button>
          </div>
        </div>
      </div>
    </section>

    <section class="global-progress-card" aria-label="中控导入进度">
      <div class="progress-number">
        <span>全局导入进度</span>
        <strong>{{ overallProgress }}%</strong>
      </div>
      <div class="wide-progress">
        <div>
          <span :style="progressStyle(summary?.global_progress.done_bytes ?? 0, summary?.global_progress.total_bytes ?? 0)"></span>
        </div>
        <dl>
          <div>
            <dt>已完成</dt>
            <dd>{{ formatBytes(summary?.global_progress.done_bytes ?? 0) }} / {{ formatBytes(summary?.global_progress.total_bytes ?? 0) }}</dd>
          </div>
          <div>
            <dt>剩余</dt>
            <dd>{{ formatBytes(summary?.global_progress.remaining_bytes ?? 0) }}</dd>
          </div>
          <div>
            <dt>速度</dt>
            <dd>{{ formatSpeed(summary?.global_progress.speed_bytes_per_sec ?? 0) }}</dd>
          </div>
          <div>
            <dt>对象</dt>
            <dd>{{ summary?.global_progress.object_done ?? 0 }} / {{ summary?.global_progress.object_total ?? 0 }}</dd>
          </div>
        </dl>
      </div>
    </section>

    <section v-if="isRefreshing && !summary" class="state-panel">
      <strong>正在加载中控端 summary</strong>
      <span>{{ centerDashboardSummaryPath() }}</span>
    </section>

    <section v-else-if="httpError && !summary" class="state-panel error-state">
      <strong>{{ httpError.error_code }}</strong>
      <span>{{ httpError.message }}</span>
    </section>

    <section v-else-if="isEmpty" class="state-panel">
      <strong>暂无运输盘或导入任务</strong>
      <span>等待插入运输盘或中控 WebSocket 事件</span>
    </section>

    <template v-if="summary">
      <section class="metric-grid" aria-label="中控关键指标">
        <article class="metric-card">
          <span>待导入</span>
          <strong>{{ pendingImportDisks.length }}</strong>
          <small>SEALED 未入账</small>
        </article>
        <article class="metric-card">
          <span>导入中</span>
          <strong>{{ activeImportDisks.length }}</strong>
          <small>{{ formatSpeed(summary.global_progress.speed_bytes_per_sec) }}</small>
        </article>
        <article class="metric-card success">
          <span>可复用</span>
          <strong>{{ reusableDisks.length }}</strong>
          <small>INITIALIZED</small>
        </article>
        <article class="metric-card danger">
          <span>异常</span>
          <strong>{{ abnormalDisks.length }}</strong>
          <small>需人工审查</small>
        </article>
      </section>

      <section class="operations-grid">
        <article class="panel import-queue">
          <div class="panel-heading">
            <div>
              <p class="section-kicker">Import Queue</p>
              <h2>待导入运输盘</h2>
            </div>
            <span>{{ pendingImportDisks.length }} disks</span>
          </div>

          <div class="queue-table">
            <div class="table-head">
              <span>盘 ID</span>
              <span>来源</span>
              <span>seal</span>
              <span>状态</span>
              <span>操作</span>
            </div>
            <button
              v-for="disk in pendingImportDisks"
              :key="disk.disk_id"
              class="queue-row"
              type="button"
              @click="selectDisk(disk)"
            >
              <strong>{{ shortId(disk.disk_id) }}</strong>
              <span>{{ disk.edge_code || "--" }}</span>
              <span>{{ shortId(disk.seal_id) }}</span>
              <span class="state-text">{{ importStatusLabel(disk.import_job_status) }}</span>
              <span>查看</span>
            </button>
            <p v-if="!pendingImportDisks.length" class="empty">当前没有 SEALED 待导入盘</p>
          </div>
        </article>

        <article class="panel selected-panel" :class="selectedDisk ? diskTone(selectedDisk) : 'tone-muted'">
          <div class="panel-heading">
            <div>
              <p class="section-kicker">Selected Disk</p>
              <h2>{{ selectedDisk ? shortId(selectedDisk.disk_id) : "未选择运输盘" }}</h2>
            </div>
            <span v-if="selectedDisk" class="state-pill">{{ lifecycleLabel(selectedDisk.disk_status_code) }}</span>
          </div>

          <dl v-if="selectedDisk" class="detail-grid">
            <div>
              <dt>注册</dt>
              <dd>{{ selectedDisk.registered ? "已注册" : "未注册" }}</dd>
            </div>
            <div>
              <dt>启用</dt>
              <dd>{{ selectedDisk.disk_enabled ? "启用" : "停用" }}</dd>
            </div>
            <div>
              <dt>运行态</dt>
              <dd>{{ runtimeLabel(selectedDisk.runtime_status) }}</dd>
            </div>
            <div>
              <dt>文件系统</dt>
              <dd :class="{ dangerText: selectedDisk.filesystem && selectedDisk.filesystem !== 'ext4' }">
                {{ selectedDisk.filesystem ?? "未知" }}
              </dd>
            </div>
            <div>
              <dt>设备</dt>
              <dd>{{ selectedDisk.device_path ?? "未知" }}</dd>
            </div>
            <div>
              <dt>挂载点</dt>
              <dd>{{ selectedDisk.mount_path ?? "未知" }}</dd>
            </div>
            <div>
              <dt>硬件 SN</dt>
              <dd>{{ selectedDisk.hardware_serial ?? selectedDisk.disk_sn }}</dd>
            </div>
            <div>
              <dt>导入任务</dt>
              <dd>{{ importStatusLabel(selectedDisk.import_job_status) }}</dd>
            </div>
          </dl>

          <div v-if="selectedDiskAction" class="action-box">
            <button
              class="primary-action"
              :class="selectedDiskAction.tone"
              :disabled="!selectedDiskAction.enabled"
              type="button"
              @click="previewAction(selectedDiskAction)"
            >
              {{ selectedDiskAction.label }}
            </button>
            <p>{{ selectedDiskAction.detail }}</p>
            <small>{{ selectedDiskAction.endpoint }}</small>
          </div>

          <p v-if="selectedDisk?.last_error_code" class="error-note">
            {{ selectedDisk.last_error_code }}：{{ selectedDisk.error_message }}
          </p>
          <p class="message">{{ selectedDisk?.message ?? actionMessage }}</p>
        </article>

        <article class="panel object-panel">
          <div class="panel-heading">
            <div>
              <p class="section-kicker">Current Object</p>
              <h2>当前导入对象</h2>
            </div>
            <span>{{ currentObject ? percent(currentObject.done_bytes, currentObject.size_bytes) : 0 }}%</span>
          </div>

          <dl class="object-detail">
            <div>
              <dt>对象路径</dt>
              <dd>{{ currentObject ? `${currentObject.bucket}/${currentObject.key}` : "--" }}</dd>
            </div>
            <div>
              <dt>显示名</dt>
              <dd>{{ currentObject?.display_name ?? "--" }}</dd>
            </div>
            <div>
              <dt>剩余大小</dt>
              <dd>{{ formatBytes(currentObject?.remaining_bytes ?? 0) }}</dd>
            </div>
            <div>
              <dt>速度</dt>
              <dd>{{ formatSpeed(currentObject?.speed_bytes_per_sec ?? 0) }}</dd>
            </div>
          </dl>
          <div class="progress-track">
            <span :style="progressStyle(currentObject?.done_bytes ?? 0, currentObject?.size_bytes ?? 0)"></span>
          </div>
        </article>

        <article v-if="isDevMode" class="panel dev-panel">
          <div class="panel-heading">
            <div>
              <p class="section-kicker">Dev Only</p>
              <h2>开发环境工具</h2>
            </div>
            <span>DEV</span>
          </div>

          <div class="dev-actions">
            <button type="button" @click="actionMessage = 'DEV: 清理 import_job / object_ledger / chunk_import_*'">
              一键清理同步记录
            </button>
            <button type="button" @click="actionMessage = 'DEV: 清理 disk_list / data_key 测试台账'">
              一键清理运输盘台账
            </button>
            <button type="button" @click="actionMessage = 'DEV: 对当前盘执行受控重新初始化'">
              重新初始化当前运输盘
            </button>
            <button class="danger" type="button" @click="actionMessage = 'DEV: 丢弃 SEALED 测试导出并重置'">
              丢弃 SEALED 测试盘
            </button>
          </div>
          <p>{{ actionMessage }}</p>
        </article>
      </section>

      <section class="panel inventory-panel" aria-label="运输盘台账">
        <div class="panel-heading">
          <div>
            <p class="section-kicker">Disk Ledger</p>
            <h2>已识别运输盘</h2>
          </div>
          <span>{{ disks.length }} disks</span>
        </div>

        <div class="inventory-grid">
          <button
            v-for="disk in disks"
            :key="disk.disk_id"
            class="inventory-card"
            :class="[diskTone(disk), { selected: disk.disk_id === selectedDisk?.disk_id }]"
            type="button"
            @click="selectDisk(disk)"
          >
            <img src="/assets/fustfs-baseline/transport-disk-cutout-v1.png" alt="" />
            <div>
              <strong>{{ shortId(disk.disk_id) }}</strong>
              <span>{{ disk.disk_sn }} · {{ disk.edge_code || "center" }}</span>
              <small>{{ lifecycleLabel(disk.disk_status_code) }} / {{ runtimeLabel(disk.runtime_status) }}</small>
            </div>
            <i>{{ percent(disk.done_bytes, disk.total_bytes) }}%</i>
          </button>
          <p v-if="!disks.length" class="empty">暂无已识别运输盘</p>
        </div>
      </section>
    </template>

    <section class="footer-hint">
      <span></span>
      <p>{{ httpError ? httpError.message : refreshMessage || wsMessage }}</p>
    </section>
  </main>
</template>
