<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { DashboardHttpError } from "../api/centerDashboard";
import {
  fetchCenterEdgeOptions,
  fetchCenterSyncRecordDetail,
  fetchCenterSyncRecords,
  type CenterEdgeOption,
  type CenterSyncRecord,
} from "../api/centerSyncRecords";
import EdgeTelemetry from "../components/EdgeTelemetry.vue";

const page = ref(1);
const pageSize = 8;
const total = ref(0);
const records = ref<CenterSyncRecord[]>([]);
const selected = ref<CenterSyncRecord | null>(null);
const selectedRecordKey = ref("");
const edgeOptions = ref<CenterEdgeOption[]>([]);
const selectedEdgeCode = ref("");
const timeRange = ref("30");
const query = ref("");
const loading = ref(false);
const detailLoading = ref(false);
const edgeLoading = ref(false);
const error = ref<DashboardHttpError | null>(null);
const detailError = ref<DashboardHttpError | null>(null);
const edgeError = ref<DashboardHttpError | null>(null);

const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));
const selectedRecord = computed(() => records.value.find((record) => recordKey(record) === selectedRecordKey.value) ?? records.value[0] ?? null);
const edgeCount = computed(() => edgeOptions.value.length || new Set(records.value.map((record) => record.edge_code).filter(Boolean)).size);
const chunkedCount = computed(() => records.value.filter((record) => record.chunk_group_id).length);
const importedBytes = computed(() => records.value.reduce((sum, record) => sum + record.source_size_bytes, 0));
const startedFrom = computed(() => {
  if (timeRange.value === "all") return "";
  const days = Number(timeRange.value);
  if (!Number.isFinite(days) || days <= 0) return "";
  const date = new Date();
  date.setDate(date.getDate() - days);
  return date.toISOString();
});

watch(selectedEdgeCode, () => {
  page.value = 1;
  void loadRecords();
});

watch(timeRange, () => {
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
    const response = await fetchCenterSyncRecords({
      page: page.value,
      page_size: pageSize,
      edge_code: selectedEdgeCode.value,
      imported_from: startedFrom.value,
      imported_to: "",
      q: query.value,
    });
    records.value = response.items;
    total.value = response.total;
    mergeEdgeOptions(response.edges ?? []);
    const nextRecord =
      records.value.find((record) => recordKey(record) === selectedRecordKey.value) ?? records.value[0] ?? null;
    if (nextRecord) {
      if (!selected.value || recordKey(selected.value) !== recordKey(nextRecord)) void openDetail(nextRecord);
    } else {
      selected.value = null;
      selectedRecordKey.value = "";
    }
  } catch (loadError) {
    records.value = [];
    total.value = 0;
    selected.value = null;
    selectedRecordKey.value = "";
    error.value =
      loadError instanceof DashboardHttpError
        ? loadError
        : new DashboardHttpError("CENTER_SYNC_RECORDS_UNAVAILABLE", "对象账本记录接口不可用");
  } finally {
    loading.value = false;
  }
}

async function loadEdgeOptions() {
  edgeLoading.value = true;
  edgeError.value = null;
  try {
    mergeEdgeOptions(await fetchCenterEdgeOptions());
  } catch (loadError) {
    edgeError.value =
      loadError instanceof DashboardHttpError
        ? loadError
        : new DashboardHttpError("CENTER_EDGE_OPTIONS_UNAVAILABLE", "边缘站点分类接口不可用");
  } finally {
    edgeLoading.value = false;
  }
}

async function openDetail(record: CenterSyncRecord) {
  selectedRecordKey.value = recordKey(record);
  selected.value = record;
  detailError.value = null;
  detailLoading.value = true;
  try {
    selected.value = await fetchCenterSyncRecordDetail(record.ledger_id ? String(record.ledger_id) : record.import_job_id);
  } catch (loadError) {
    detailError.value =
      loadError instanceof DashboardHttpError
        ? loadError
        : new DashboardHttpError("CENTER_SYNC_RECORD_DETAIL_UNAVAILABLE", "对象账本详情接口不可用");
  } finally {
    detailLoading.value = false;
  }
}

function submitSearch() {
  page.value = 1;
  void loadRecords();
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

function mergeEdgeOptions(nextOptions: CenterEdgeOption[]) {
  const byCode = new Map(edgeOptions.value.map((edge) => [edge.edge_code, edge]));
  for (const edge of nextOptions) {
    if (edge.edge_code) byCode.set(edge.edge_code, { ...byCode.get(edge.edge_code), ...edge });
  }
  for (const record of records.value) {
    if (record.edge_code && !byCode.has(record.edge_code)) {
      byCode.set(record.edge_code, {
        edge_code: record.edge_code,
        edge_name: record.edge_name,
      });
    }
  }
  edgeOptions.value = Array.from(byCode.values()).sort((a, b) => a.edge_code.localeCompare(b.edge_code));
}

function recordKey(record: CenterSyncRecord): string {
  return record.ledger_id
    ? `ledger-${record.ledger_id}`
    : `${record.import_job_id}-${record.edge_code}-${record.source_bucket}-${record.source_key}`;
}

function edgeLabel(record: CenterSyncRecord): string {
  return record.edge_name ? `${record.edge_name} / ${record.edge_code}` : record.edge_code || "未返回";
}

function objectPath(record: CenterSyncRecord): string {
  return `${record.source_bucket || "--"}/${record.source_key || "--"}`;
}

function importPath(record: CenterSyncRecord): string {
  return `${record.import_bucket || "--"}/${record.import_key || "--"}`;
}

function shortHash(value?: string): string {
  if (!value) return "--";
  return value.length > 14 ? `${value.slice(0, 10)}...${value.slice(-4)}` : value;
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  const unitIndex = Math.min(Math.floor(Math.log(bytes) / Math.log(1000)), units.length - 1);
  return `${(bytes / 1000 ** unitIndex).toFixed(unitIndex >= 4 ? 1 : 2)} ${units[unitIndex]}`;
}

function formatTime(value?: string): string {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(date);
}

onMounted(() => {
  void loadEdgeOptions();
  void loadRecords();
});
</script>

<template>
  <main class="sync-records page-panel">
    <EdgeTelemetry
      :http-tone="error ? 'warning' : 'ok'"
      local-tone="ok"
      ws-tone="quiet"
      refresh-label="刷新记录"
      :refresh-disabled="loading"
      :show-status-pills="false"
      @refresh="loadRecords"
    />

    <header class="records-title">
      <h1>同步记录</h1>
      <p>展示 Center object_ledger 已入账对象，按 edge_site.edge_code 分类查询；当前不展示模拟数据。</p>
      <button type="button" @click="goDashboard">← 返回 Dashboard</button>
    </header>

    <section class="record-summary">
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/icons/task-confirmed-database.svg" />
        <span>账本对象</span><strong>{{ total }}</strong>
      </div>
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/icons/task-eta-clock.svg" />
        <span>边缘分类</span><strong>{{ edgeCount }}</strong>
      </div>
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/a04-packed-shield-v1.png" />
        <span>当前页数据量</span><strong>{{ formatBytes(importedBytes) }}</strong>
      </div>
      <div class="summary-card">
        <img alt="" src="/assets/fustfs-baseline/a04-failed-lock-small-v1.png" />
        <span>分块对象</span><strong>{{ chunkedCount }}</strong>
      </div>
    </section>

    <section class="record-controls" aria-label="同步记录筛选">
      <select v-model="timeRange">
        <option value="30">最近 30 天</option>
        <option value="7">最近 7 天</option>
        <option value="all">全部时间</option>
      </select>
      <select v-model="selectedEdgeCode" :disabled="edgeLoading && edgeOptions.length === 0">
        <option value="">全部边缘站点</option>
        <option v-for="edge in edgeOptions" :key="edge.edge_code" :value="edge.edge_code">
          {{ edge.edge_name ? `${edge.edge_name} / ${edge.edge_code}` : edge.edge_code }}
        </option>
      </select>
      <input v-model.trim="query" placeholder="源 bucket/key / 导入任务 ID / ETag" type="search" @keydown.enter="submitSearch" />
      <button type="button" @click="submitSearch">查询</button>
    </section>

    <section v-if="error || edgeError" class="records-inline-error glass-panel">
      <i>i</i>
      <span>
        {{ error ? `对象账本记录接口不可用：${error.error_code}` : `边缘站点分类接口不可用：${edgeError?.error_code}` }}。当前不展示模拟数据。
      </span>
    </section>

    <section class="records-table glass-panel" role="table" aria-label="对象账本同步记录列表">
      <div class="table-head" role="row">
        <span>入账时间</span>
        <span>边缘站点</span>
        <span>源对象</span>
        <span>中控归档</span>
        <span>大小</span>
        <span>导入任务</span>
        <span>分块组</span>
        <span>操作</span>
      </div>
      <button
        v-for="record in records"
        :key="recordKey(record)"
        :class="{ selected: selectedRecord ? recordKey(selectedRecord) === recordKey(record) : false }"
        class="record-row"
        type="button"
        @click="openDetail(record)"
      >
        <span>{{ formatTime(record.imported_at) }}</span>
        <strong>{{ edgeLabel(record) }}</strong>
        <span :title="objectPath(record)">{{ objectPath(record) }}</span>
        <span :title="importPath(record)">{{ importPath(record) }}</span>
        <span>{{ formatBytes(record.source_size_bytes) }}</span>
        <span>{{ record.import_job_id || "未返回" }}</span>
        <span>{{ record.chunk_group_id ? "跨盘分块" : "普通对象" }}</span>
        <span class="detail-link">查看详情 →</span>
      </button>
      <p v-if="loading" class="empty-result">正在读取对象账本记录</p>
      <p v-else-if="records.length === 0 && !error" class="empty-result">没有符合条件的 object_ledger 记录</p>
      <footer class="records-pagination">
        <span>共 {{ total }} 条</span>
        <nav aria-label="同步记录分页">
          <button :disabled="page === 1 || loading" type="button" @click="previousPage">←</button>
          <b>{{ page }}</b>
          <span>/ {{ pageCount }}</span>
          <button :disabled="page === pageCount || loading" type="button" @click="nextPage">→</button>
        </nav>
      </footer>
    </section>

    <aside class="record-drawer glass-panel" aria-label="对象账本记录详情">
      <header>
        <h2>对象账本详情</h2>
      </header>
      <section v-if="detailLoading" class="drawer-loading">正在读取详情</section>
      <section v-else-if="detailError" class="drawer-loading error-state">详情接口不可用：{{ detailError.error_code }}。当前不展示模拟详情。</section>
      <section v-else-if="records.length === 0" class="drawer-loading">暂无对象账本记录</section>
      <section v-else-if="!selected" class="drawer-loading">正在读取对象账本详情</section>
      <template v-else>
        <dl class="drawer-overview">
          <dt>边缘站点</dt><dd>{{ edgeLabel(selected) }}</dd>
          <dt>源对象</dt><dd>{{ objectPath(selected) }}</dd>
          <dt>中控归档</dt><dd>{{ importPath(selected) }}</dd>
          <dt>源 ETag</dt><dd>{{ selected.source_etag ?? "--" }}</dd>
          <dt>源对象大小</dt><dd>{{ formatBytes(selected.source_size_bytes) }}</dd>
          <dt>源修改时间</dt><dd>{{ formatTime(selected.source_last_modified) }}</dd>
          <dt>导入时间</dt><dd>{{ formatTime(selected.imported_at) }}</dd>
        </dl>
        <h3>账本关联</h3>
        <dl class="drawer-overview manifest-lines">
          <dt>import_job_id</dt><dd>{{ selected.import_job_id || "--" }}</dd>
          <dt>export_job_id</dt><dd>{{ selected.export_job_id ?? "--" }}</dd>
          <dt>chunk_group_id</dt><dd>{{ selected.chunk_group_id ?? "--" }}</dd>
          <dt>plaintext_sha256</dt><dd>{{ shortHash(selected.plaintext_sha256) }}</dd>
          <dt>ciphertext_sha256</dt><dd>{{ shortHash(selected.ciphertext_sha256) }}</dd>
        </dl>
        <p class="drawer-note"><i>i</i> object_ledger 是中控对象导入去重和来源追踪权威表；分类来源为 edge_site.edge_code。</p>
      </template>
    </aside>
  </main>
</template>
