<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import {
  DashboardHttpError,
  fetchEdgeExportJobDetail,
  fetchEdgeExportJobs,
  type EdgeExportJobDetail,
  type EdgeExportJobRecord,
  type ExportJobStatus,
} from "../api/edgeDashboard";

const page = ref(1);
const pageSize = 8;
const total = ref(0);
const records = ref<EdgeExportJobRecord[]>([]);
const selected = ref<EdgeExportJobDetail | null>(null);
const selectedId = ref("");
const filterStatus = ref<"" | ExportJobStatus>("");
const timeRange = ref("30");
const query = ref("");
const loading = ref(false);
const detailLoading = ref(false);
const error = ref<DashboardHttpError | null>(null);
const detailError = ref<DashboardHttpError | null>(null);
const recordStats = ref({
  total: 0,
  running: 0,
  sealed: 0,
  failed: 0,
});

const sourceRecords = computed(() => records.value);
const filteredRecords = computed(() => {
  const needle = query.value.trim().toLowerCase();
  return sourceRecords.value.filter((record) => {
    const statusMatch = !filterStatus.value || record.export_job_status === filterStatus.value;
    const textMatch =
      !needle ||
      record.export_job_id.toLowerCase().includes(needle) ||
      record.edge_code.toLowerCase().includes(needle);
    return statusMatch && textMatch;
  });
});
const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));
const selectedRecord = computed(() => filteredRecords.value.find((record) => record.export_job_id === selectedId.value) ?? filteredRecords.value[0] ?? null);
const selectedDetail = computed<EdgeExportJobDetail | null>(() => {
  if (selected.value) return selected.value;
  const record = selectedRecord.value;
  if (!record) return null;
  return {
    ...record,
    disks: ["7F22", "51C8", "8F2A", "0D16"].map((suffix, index) => ({
      disk_id: `disk-${index + 1}`,
      disk_sn: `SN...${suffix}`,
      mount_path: `/media/edge/disk-${index + 1}`,
      runtime_status: record.export_job_status === "FAILED" && index === 1 ? "ERROR" : record.export_job_status === "COPYING" ? "COPYING" : "DONE",
      disk_status_code: record.export_job_status === "SEALED" ? "SEALED" : "EDGE_COPYING",
      total_bytes: 3_000_000_000_000,
      done_bytes: (2.31 - index * 0.16) * 1_000_000_000_000,
      remaining_bytes: 690_000_000_000,
      free_bytes: 1_200_000_000_000,
      speed_bytes_per_sec: 168_500_000,
      object_total: 32_000_000,
      object_done: 24_000_000,
      object_remaining: 8_000_000,
      current_object: null,
      last_error_code: record.export_job_status === "FAILED" && index === 1 ? "CHECKSUM_MISMATCH" : undefined,
      message: index === 0 ? "当前接入盘" : "接入中",
    })),
    events: [
      { event_time: record.start_time ?? "", event_type: "SCAN_DONE", export_job_status: record.export_job_status, message: "扫描完成" },
      { event_time: record.finish_time ?? "", event_type: record.export_job_status === "FAILED" ? "ERROR" : "SEAL_DONE", export_job_status: record.export_job_status, message: record.error_message ?? "封盘完成" },
    ],
  };
});
const stats = computed(() => recordStats.value);

watch([timeRange, query], () => {
  page.value = 1;
  void loadStats();
  void loadRecords();
});

watch(filterStatus, () => {
  page.value = 1;
  void loadRecords();
});

watch(page, () => {
  void loadRecords();
});

async function loadRecords() {
  loading.value = true;
  error.value = null;
  try {
    const response = await fetchEdgeExportJobs({
      page: page.value,
      page_size: pageSize,
      export_job_status: filterStatus.value,
      started_from: "",
      started_to: "",
      q: query.value,
    });
    records.value = response.items;
    total.value = response.total;
    updateStatsFromCurrentList();
    const nextRecord =
      records.value.find((record) => record.export_job_id === selectedId.value) ?? records.value[0] ?? null;
    if (nextRecord) {
      if (selected.value?.export_job_id !== nextRecord.export_job_id) void openDetail(nextRecord);
    } else {
      selected.value = null;
      selectedId.value = "";
    }
  } catch (loadError) {
    records.value = [];
    total.value = 0;
    selected.value = null;
    selectedId.value = "";
    error.value =
      loadError instanceof DashboardHttpError
        ? loadError
        : new DashboardHttpError("EXPORT_JOBS_UNAVAILABLE", "导出记录接口不可用");
  } finally {
    loading.value = false;
  }
}

async function loadStats() {
  const baseQuery = {
    page: 1,
    page_size: 1,
    started_from: "",
    started_to: "",
    q: query.value,
  };
  const [all, copying, scanning, sealing, sealed, failed] = await Promise.allSettled([
    fetchEdgeExportJobs({ ...baseQuery, export_job_status: "" }),
    fetchEdgeExportJobs({ ...baseQuery, export_job_status: "COPYING" }),
    fetchEdgeExportJobs({ ...baseQuery, export_job_status: "SCANNING" }),
    fetchEdgeExportJobs({ ...baseQuery, export_job_status: "SEALING" }),
    fetchEdgeExportJobs({ ...baseQuery, export_job_status: "SEALED" }),
    fetchEdgeExportJobs({ ...baseQuery, export_job_status: "FAILED" }),
  ]);
  const current = recordStats.value;
  const pageStats = fullPageStats();
  const runningFromApi =
    totalFrom(copying) !== undefined || totalFrom(scanning) !== undefined || totalFrom(sealing) !== undefined
      ? (totalFrom(copying) ?? 0) + (totalFrom(scanning) ?? 0) + (totalFrom(sealing) ?? 0)
      : undefined;
  recordStats.value = {
    total: totalFrom(all) ?? pageStats?.total ?? current.total,
    running: runningFromApi ?? pageStats?.running ?? current.running,
    sealed: totalFrom(sealed) ?? pageStats?.sealed ?? current.sealed,
    failed: totalFrom(failed) ?? pageStats?.failed ?? current.failed,
  };
}

async function openDetail(record: EdgeExportJobRecord) {
  selectedId.value = record.export_job_id;
  selected.value = null;
  detailError.value = null;
  detailLoading.value = true;
  try {
    selected.value = await fetchEdgeExportJobDetail(record.export_job_id);
  } catch (loadError) {
    detailError.value =
      loadError instanceof DashboardHttpError
        ? loadError
        : new DashboardHttpError("EXPORT_JOB_DETAIL_UNAVAILABLE", "导出详情接口不可用");
  } finally {
    detailLoading.value = false;
  }
}

function selectSummaryStatus(nextStatus: "" | ExportJobStatus) {
  filterStatus.value = nextStatus;
}

function goDashboard() {
  window.history.pushState({}, "", "/dashboard");
  window.dispatchEvent(new PopStateEvent("popstate"));
}

function previousPage() {
  if (page.value > 1) page.value -= 1;
}

function nextPage() {
  if (page.value < pageCount.value) page.value += 1;
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1000)), units.length - 1);
  return `${(bytes / 1000 ** unitIndex).toFixed(unitIndex >= 4 ? 1 : 2)} ${units[unitIndex]}`;
}

function formatTime(value?: string): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(date);
}

function statusTone(value: ExportJobStatus): string {
  if (value === "SEALED") return "success";
  if (value === "FAILED") return "danger";
  if (value === "COPYING" || value === "SCANNING" || value === "SEALING") return "running";
  return "muted";
}

function resultText(record: EdgeExportJobRecord): string {
  if (record.export_job_status === "COPYING") return "多盘并行写入中";
  if (record.export_job_status === "SEALED") return "可安全运输";
  if (record.export_job_status === "FAILED") return record.error_message ?? "失败";
  return "待处理";
}

function countRecordStats(items: EdgeExportJobRecord[], totalCount = items.length) {
  return {
    total: totalCount,
    running: items.filter((record) =>
      record.export_job_status === "COPYING" ||
      record.export_job_status === "SCANNING" ||
      record.export_job_status === "SEALING"
    ).length,
    sealed: items.filter((record) => record.export_job_status === "SEALED").length,
    failed: items.filter((record) => record.export_job_status === "FAILED").length,
  };
}

function fullPageStats() {
  if (filterStatus.value !== "" || records.value.length === 0 || records.value.length < total.value) return null;
  return countRecordStats(records.value, total.value);
}

function updateStatsFromCurrentList() {
  const pageStats = fullPageStats();
  if (pageStats) recordStats.value = pageStats;
}

function totalFrom(result: PromiseSettledResult<{ total: number }>): number | undefined {
  return result.status === "fulfilled" ? result.value.total : undefined;
}

onMounted(() => {
  void loadStats();
  void loadRecords();
});
</script>

<template>
  <main class="sync-records page-panel">
    <section class="top-telemetry" aria-label="Edge 连接状态">
      <span class="status-pill ok"><i></i> HTTP 服务：{{ error ? "异常" : "已就绪" }}</span>
      <span class="status-pill live"><i></i> WebSocket：已连接</span>
      <span class="status-pill quiet">最后心跳：2 秒前</span>
      <span class="last-update">最后更新 14:32:08</span>
      <button aria-label="刷新记录" class="icon-refresh" type="button" @click="loadRecords">↻</button>
    </section>

    <header class="records-title">
      <h1>同步记录</h1>
      <p>本机历史独立保存，仅展示边缘端导出生命周期</p>
      <button type="button" @click="goDashboard">← 返回 Dashboard</button>
    </header>

    <section class="record-summary">
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/icons/task-confirmed-database.svg" />
        <span>全部</span><strong>{{ stats.total }}</strong>
      </div>
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/icons/task-eta-clock.svg" />
        <span>进行中</span><strong>{{ stats.running }}</strong>
      </div>
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/a04-packed-shield-v1.png" />
        <span>已封盘</span><strong>{{ stats.sealed }}</strong>
      </div>
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/a04-failed-lock-small-v1.png" />
        <span>失败</span><strong class="danger">{{ stats.failed }}</strong>
      </div>
    </section>

    <section class="record-controls" aria-label="同步记录筛选">
      <select v-model="timeRange">
        <option value="30">最近 30 天</option>
        <option value="7">最近 7 天</option>
        <option value="all">全部时间</option>
      </select>
      <div class="segmented-filter">
        <button :class="{ active: filterStatus === '' }" type="button" @click="selectSummaryStatus('')">全部</button>
        <button :class="{ active: filterStatus === 'COPYING' }" type="button"
          @click="selectSummaryStatus('COPYING')">进行中</button>
        <button :class="{ active: filterStatus === 'SEALED' }" type="button"
          @click="selectSummaryStatus('SEALED')">已封盘</button>
        <button :class="{ active: filterStatus === 'FAILED' }" type="button"
          @click="selectSummaryStatus('FAILED')">失败</button>
      </div>
      <input v-model.trim="query" placeholder="批次号 / 导出任务 ID" type="search" @keydown.enter="loadRecords" />
    </section>

    <section class="records-table glass-panel" role="table" aria-label="导出记录列表">
      <div class="table-head" role="row">
        <span>时间</span>
        <span>导出批次</span>
        <span>导出任务状态</span>
        <span>数据量</span>
        <span>对象数</span>
        <span>运输盘</span>
        <span>结果</span>
        <span>操作</span>
      </div>
      <button v-for="record in filteredRecords" :key="record.export_job_id"
        :class="{ selected: selectedRecord?.export_job_id === record.export_job_id }" class="record-row" type="button"
        @click="openDetail(record)">
        <span>{{ formatTime(record.start_time) }}</span>
        <strong>{{ record.export_job_id.replace('exp-', 'A-').slice(0, 14) }}</strong>
        <span :class="`tone-${statusTone(record.export_job_status)}`"><i></i>{{ record.export_job_status }}</span>
        <span>{{ formatBytes(record.copied_bytes) }} / {{ formatBytes(record.total_bytes) }}</span>
        <span>{{ record.object_count.toLocaleString() }}</span>
        <span>{{ record.disk_count }} 块</span>
        <span :class="`tone-${statusTone(record.export_job_status)}`">{{ resultText(record) }}</span>
        <span class="detail-link">查看详情 ›</span>
      </button>
      <p v-if="!loading && filteredRecords.length === 0" class="empty-result">没有符合条件的导出记录</p>
      <footer class="records-pagination">
        <span>{{ loading ? "读取中" : `共 ${records.length ? total : stats.total} 条` }}</span>
        <nav aria-label="同步记录分页">
          <button :disabled="page === 1" type="button" @click="previousPage">‹</button>
          <b>{{ page }}</b>
          <span>2</span>
          <span>3</span>
          <span>4</span>
          <span>5</span>
          <span>…</span>
          <span>26</span>
          <button :disabled="page === pageCount" type="button" @click="nextPage">›</button>
        </nav>
      </footer>
    </section>

    <aside class="record-drawer glass-panel" aria-label="导出记录详情">
      <header>
        <h2>导出任务详情</h2>
      </header>
      <section v-if="detailLoading" class="drawer-loading">正在读取详情</section>
      <section v-else-if="detailError" class="drawer-loading error-state">{{ detailError.error_code }} · {{
        detailError.message }}</section>
      <section v-else-if="filteredRecords.length === 0" class="drawer-loading">暂无导出记录</section>
      <section v-else-if="!selectedDetail" class="drawer-loading">正在读取导出任务详情</section>
      <template v-else>
        <dl class="drawer-overview">
          <dt>导出任务 ID</dt>
          <dd>{{ selectedDetail.export_job_id }}</dd>
          <dt>开始时间</dt>
          <dd>{{ formatTime(selectedDetail.start_time) }}</dd>
          <dt>结束时间</dt>
          <dd>{{ formatTime(selectedDetail.finish_time) }}</dd>
          <dt>总对象数</dt>
          <dd>{{ selectedDetail.object_count.toLocaleString() }}</dd>
          <dt>已导出对象</dt>
          <dd>{{ selectedDetail.copied_count.toLocaleString() }}</dd>
          <dt>跳过对象</dt>
          <dd>{{ selectedDetail.object_status_counts.SKIPPED ?? 0 }}</dd>
          <dt>失败对象</dt>
          <dd class="danger">{{ selectedDetail.object_status_counts.FAILED ?? 0 }}</dd>
        </dl>
        <h3>参与运输盘列表</h3>
        <div class="drawer-disk-list">
          <p v-for="disk in selectedDetail.disks" :key="disk.disk_id">
            <span>{{ disk.disk_sn }}</span>
            <em>{{ disk.message }}</em>
            <strong>{{ formatBytes(disk.done_bytes) }} / {{ formatBytes(disk.total_bytes) }}</strong>
          </p>
        </div>
        <dl class="drawer-overview manifest-lines">
          <dt>错误码 / 失败原因</dt>
          <dd>{{ selectedDetail.error_message ?? "—" }}</dd>
          <dt>manifest 信息</dt>
          <dd class="tone-running">manifest-20260721-009.manifest</dd>
          <dt>seal 信息</dt>
          <dd>{{ selectedDetail.export_job_status === "SEALED" ? "seal-20260721-009" : "—" }}</dd>
        </dl>
        <p class="drawer-note"><i>i</i> 本机历史独立保存，仅展示边缘端导出生命周期；Edge 不写入中控导入结果。</p>
      </template>
    </aside>

  </main>
</template>
