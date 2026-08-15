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

const timeRangeOptions = [
  { value: "30", label: "最近 30 天" },
  { value: "7", label: "最近 7 天" },
  { value: "all", label: "全部时间" },
];

const recordColumns = [
  { title: "入账时间", key: "imported_at", width: 112, ellipsis: true },
  { title: "边缘站点", key: "edge", width: 138, ellipsis: true },
  { title: "源对象", key: "source", ellipsis: true },
  { title: "中控归档", key: "archive", ellipsis: true },
  { title: "大小", key: "size", width: 96, ellipsis: true },
  { title: "导入任务", key: "import_job", width: 92, ellipsis: true },
  { title: "存储模式", key: "storage_mode", width: 96, ellipsis: true },
  { title: "操作", key: "action", width: 92, fixed: "right" as const },
];

const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));
const selectedRecord = computed(() => records.value.find((record) => recordKey(record) === selectedRecordKey.value) ?? records.value[0] ?? null);
const edgeCount = computed(() => edgeOptions.value.length || new Set(records.value.map((record) => record.edge_code).filter(Boolean)).size);
const framesCount = computed(() => records.value.filter((record) => record.storage_mode === "FRAMES").length);
const importedBytes = computed(() => records.value.reduce((sum, record) => sum + record.source_size_bytes, 0));
const edgeSelectOptions = computed(() => [
  { value: "", label: "全部边缘站点" },
  ...edgeOptions.value.map((edge) => ({
    value: edge.edge_code,
    label: edge.edge_name ? `${edge.edge_name} / ${edge.edge_code}` : edge.edge_code,
  })),
]);
const tablePagination = computed(() => ({
  current: page.value,
  pageSize,
  total: total.value,
  size: "small" as const,
  showSizeChanger: false,
  showTotal: (count: number) => `共 ${count} 条`,
}));
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

function handleTableChange(pagination: { current?: number }) {
  page.value = pagination.current ?? 1;
}

function recordRowProps(record: CenterSyncRecord) {
  return {
    onClick: () => {
      void openDetail(record);
    },
  };
}

function recordRowClassName(record: CenterSyncRecord) {
  return selectedRecord.value && recordKey(selectedRecord.value) === recordKey(record) ? "selected" : "";
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

    <section class="records-title">
      <a-page-header class="records-page-header" title="同步记录" :ghost="false" :back-icon="false" />
    </section>

    <a-row class="record-summary" :gutter="16">
      <a-col :span="6">
        <a-card class="summary-card" size="small" :bordered="false">
          <img alt="" src="/assets/fustfs-baseline/icons/task-confirmed-database.svg" />
          <a-statistic title="账本对象" :value="total" />
        </a-card>
      </a-col>
      <a-col :span="6">
        <a-card class="summary-card" size="small" :bordered="false">
          <img alt="" src="/assets/fustfs-baseline/icons/task-eta-clock.svg" />
          <a-statistic title="边缘分类" :value="edgeCount" />
        </a-card>
      </a-col>
      <a-col :span="6">
        <a-card class="summary-card" size="small" :bordered="false">
          <img alt="" src="/assets/fustfs-baseline/a04-packed-shield-v1.png" />
          <a-statistic title="当前页数据量" :value="formatBytes(importedBytes)" />
        </a-card>
      </a-col>
      <a-col :span="6">
        <a-card class="summary-card" size="small" :bordered="false">
          <img alt="" src="/assets/fustfs-baseline/a04-failed-lock-small-v1.png" />
          <a-statistic title="FRAMES 对象" :value="framesCount" />
        </a-card>
      </a-col>
    </a-row>

    <a-form class="record-controls" layout="inline" size="middle" @finish="submitSearch">
      <a-form-item>
        <a-select v-model:value="timeRange" :options="timeRangeOptions" size="middle" />
      </a-form-item>
      <a-form-item>
        <a-select
          v-model:value="selectedEdgeCode"
          :disabled="edgeLoading && edgeOptions.length === 0"
          :options="edgeSelectOptions"
          size="middle"
        />
      </a-form-item>
      <a-form-item>
        <a-input-search
          v-model:value.trim="query"
          class="record-search"
          placeholder="源 bucket/key / 导入任务 ID / ETag"
          size="middle"
          @search="submitSearch"
        />
      </a-form-item>
    </a-form>

    <section v-if="error || edgeError" class="records-inline-error glass-panel">
      <i>i</i>
      <span>
        {{ error ? `对象账本记录接口不可用：${error.error_code}` : `边缘站点分类接口不可用：${edgeError?.error_code}` }}。当前不展示模拟数据。
      </span>
    </section>

    <section class="records-content">
      <a-table
        class="records-table glass-panel"
        size="middle"
        :columns="recordColumns"
        :custom-row="recordRowProps"
        :data-source="records"
        :loading="loading"
        :pagination="tablePagination"
        :row-class-name="recordRowClassName"
        :row-key="recordKey"
        :scroll="{ x: 980, y: 'calc(100vh - 470px)' }"
        @change="handleTableChange"
      >
        <template #emptyText>
          <span>{{ error ? "对象账本记录接口不可用" : "没有符合条件的 object_ledger 记录" }}</span>
        </template>
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'imported_at'">
            {{ formatTime(record.imported_at) }}
          </template>
          <template v-else-if="column.key === 'edge'">
            <strong>{{ edgeLabel(record) }}</strong>
          </template>
          <template v-else-if="column.key === 'source'">
            <span :title="objectPath(record)">{{ objectPath(record) }}</span>
          </template>
          <template v-else-if="column.key === 'archive'">
            <span :title="importPath(record)">{{ importPath(record) }}</span>
          </template>
          <template v-else-if="column.key === 'size'">
            {{ formatBytes(record.source_size_bytes) }}
          </template>
          <template v-else-if="column.key === 'import_job'">
            <span :title="record.import_job_id">{{ shortHash(record.import_job_id) }}</span>
          </template>
          <template v-else-if="column.key === 'storage_mode'">
            <a-tag :color="record.storage_mode === 'FRAMES' ? 'blue' : undefined">
              {{ record.storage_mode }}{{ record.frame_total > 0 ? ` / ${record.frame_total}` : "" }}
            </a-tag>
          </template>
          <template v-else-if="column.key === 'action'">
            <a-button type="link" size="small" @click.stop="openDetail(record)">查看详情</a-button>
          </template>
        </template>
      </a-table>

      <aside class="record-drawer glass-panel" aria-label="对象账本记录详情">
        <header>
          <h2>对象账本详情</h2>
        </header>
        <section v-if="detailLoading" class="drawer-loading">正在读取详情</section>
        <section v-else-if="detailError" class="drawer-loading error-state">详情接口不可用：{{ detailError.error_code }}。当前不展示模拟详情。</section>
        <section v-else-if="records.length === 0" class="drawer-loading">暂无对象账本记录</section>
        <section v-else-if="!selected" class="drawer-loading">正在读取对象账本详情</section>
        <template v-else>
          <a-descriptions class="drawer-descriptions" :column="1" size="small" :colon="false">
            <a-descriptions-item label="边缘站点">{{ edgeLabel(selected) }}</a-descriptions-item>
            <a-descriptions-item label="源对象">{{ objectPath(selected) }}</a-descriptions-item>
            <a-descriptions-item label="中控归档">{{ importPath(selected) }}</a-descriptions-item>
            <a-descriptions-item label="源 ETag">{{ selected.source_etag ?? "--" }}</a-descriptions-item>
            <a-descriptions-item label="源对象大小">{{ formatBytes(selected.source_size_bytes) }}</a-descriptions-item>
            <a-descriptions-item label="源修改时间">{{ formatTime(selected.source_last_modified) }}</a-descriptions-item>
            <a-descriptions-item label="导入时间">{{ formatTime(selected.imported_at) }}</a-descriptions-item>
          </a-descriptions>
          <h3>账本关联</h3>
          <a-descriptions class="drawer-descriptions manifest-lines" :column="1" size="small" :colon="false">
            <a-descriptions-item label="import_job_id">{{ selected.import_job_id || "--" }}</a-descriptions-item>
            <a-descriptions-item label="export_job_id">{{ selected.export_job_id ?? "--" }}</a-descriptions-item>
            <a-descriptions-item label="storage_mode">{{ selected.storage_mode }}</a-descriptions-item>
            <a-descriptions-item label="frame_total">{{ selected.frame_total }}</a-descriptions-item>
            <a-descriptions-item label="plaintext_sha256">{{ shortHash(selected.plaintext_sha256) }}</a-descriptions-item>
            <a-descriptions-item label="pack_ciphertext_sha256">{{ shortHash(selected.pack_ciphertext_sha256) }}</a-descriptions-item>
          </a-descriptions>
          <p class="drawer-note"><i>i</i> object_ledger 是中控对象导入去重和来源追踪权威表；分类来源为 edge_site.edge_code。</p>
        </template>
      </aside>
    </section>
  </main>
</template>
