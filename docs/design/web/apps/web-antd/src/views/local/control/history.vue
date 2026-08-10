<!-- B-06-records · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import type { TableColumnsType } from 'ant-design-vue';
import type { ControlIngestRecordsView } from '#/api/local-views';

import type { ControlHistoryRow, HistoryState } from './ops-fixtures';

import { computed, ref } from 'vue';

import { Alert, Button, Input, Select, Table } from 'ant-design-vue';

import ProductShell from '../components/product-shell.vue';
import ControlServerTabs from './control-server-tabs.vue';
import {
  controlAssets,
  historyRangeOptions,
  historyRows,
  historySourceOptions,
  historySummary,
} from './ops-fixtures';

type HistoryFilter = 'ALL' | HistoryState;

const props = withDefaults(
  defineProps<{ embedded?: boolean; view?: ControlIngestRecordsView }>(),
  { embedded: false, view: undefined },
);

const filter = ref<HistoryFilter>('ALL');
const query = ref('');
const range = ref('30');
const source = ref('ALL');
const rowNotice = ref('');

const sourceOptions = computed(() =>
  props.view
    ? [
        { label: '全部来源', value: 'ALL' },
        ...[...new Set(props.view.tasks.map((task) => task.source_site_id))].map(
          (siteId) => ({ label: siteId, value: siteId }),
        ),
      ]
    : historySourceOptions.map((item) => ({ ...item })),
);
const rangeOptions = historyRangeOptions.map((item) => ({ ...item }));
const summaryFilters = computed<Array<{
  count: number;
  label: string;
  tone?: 'danger' | 'running' | 'success' | 'warning';
  value: HistoryFilter;
}>>(() => {
  if (!props.view) return [
  { count: historySummary.total, label: '全部', value: 'ALL' },
  {
    count: historySummary.ingesting,
    label: '入库中',
    value: 'INGESTING',
  },
  {
    count: historySummary.verifying,
    label: '目标校验',
    tone: 'warning',
    value: 'VERIFYING',
  },
  {
    count: historySummary.signed,
    label: '完成并签发',
    tone: 'success',
    value: 'SIGNED',
  },
  {
    count: historySummary.conflict,
    label: '冲突锁定',
    tone: 'danger',
    value: 'CONFLICT',
  },
  {
    count: historySummary.failed,
    label: '失败',
    tone: 'danger',
    value: 'FAILED',
  },
  ];
  const tasks = props.view.tasks;
  return [
    { count: tasks.length, label: '全部', value: 'ALL' },
    { count: tasks.filter((task) => task.state === 'IMPORTING').length, label: '入库中', value: 'INGESTING' },
    { count: tasks.filter((task) => task.state === 'VERIFYING').length, label: '目标校验', tone: 'warning', value: 'VERIFYING' },
    { count: tasks.filter((task) => task.state === 'COMMITTED').length, label: '完成并签发', tone: 'success', value: 'SIGNED' },
    { count: tasks.filter((task) => task.state === 'CONFLICT').length, label: '冲突锁定', tone: 'danger', value: 'CONFLICT' },
    { count: tasks.filter((task) => task.state === 'FAILED').length, label: '失败', tone: 'danger', value: 'FAILED' },
  ];
});

const columns: TableColumnsType<ControlHistoryRow> = [
  { dataIndex: 'time', key: 'time', title: '时间', width: 155 },
  { dataIndex: 'site', key: 'site', title: '来源工厂', width: 142 },
  { dataIndex: 'batchId', key: 'batchId', title: '批次', width: 195 },
  { dataIndex: 'stateLabel', key: 'state', title: '状态', width: 176 },
  { dataIndex: 'bytes', key: 'bytes', title: '数据量', width: 135 },
  { dataIndex: 'media', key: 'media', title: '运输盘', width: 165 },
  { dataIndex: 'result', key: 'result', title: '结果' },
  { key: 'action', title: '', width: 42 },
];

const displayRows = computed<ControlHistoryRow[]>(() =>
  props.view
    ? props.view.tasks.map((task) => ({
        batchId: task.batch_id,
        bytes: formatBytes(task.logical_bytes),
        key: task.batch_id,
        media: `${task.media_label} · ${task.media_serial_suffix}`,
        result: task.failure_reason ?? task.result_label,
        site: task.source_site_id,
        state:
          task.state === 'IMPORTING'
            ? 'INGESTING'
            : task.state === 'VERIFYING'
              ? 'VERIFYING'
              : task.state === 'COMMITTED'
                ? 'SIGNED'
                : task.state === 'CONFLICT'
                  ? 'CONFLICT'
                  : 'FAILED',
        stateLabel: task.stage_label,
        time: task.updated_at,
      }))
    : historyRows,
);

function formatBytes(bytes: number) {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index < 3 ? 0 : 1)} ${units[index]}`;
}

const filteredRows = computed(() => {
  const needle = query.value.trim().toLowerCase();
  return displayRows.value.filter(
    (row) =>
      (filter.value === 'ALL' || row.state === filter.value) &&
      (source.value === 'ALL' || row.site === source.value) &&
      (!needle ||
        row.batchId.toLowerCase().includes(needle) ||
        row.media.toLowerCase().includes(needle)),
  );
});

function selectFilter(next: HistoryFilter) {
  filter.value = next;
  rowNotice.value = '';
}

function toneFor(state: unknown) {
  if (state === 'SIGNED') return 'success';
  if (state === 'VERIFYING') return 'warning';
  if (state === 'CONFLICT' || state === 'FAILED') return 'danger';
  return 'running';
}

function explainRow(row: Record<string, unknown>) {
  rowNotice.value = props.view
    ? `${String(row.batchId ?? '未知运输记录')} 已由 B 中控本机只读投影提供；详情页需使用介质与运输记录标识重新读取。`
    : `${String(row.batchId ?? '未知批次')} 仅提供冻结列表夹具，当前切片未提供记录详情。`;
}
</script>

<template>
  <component
    :is="embedded ? 'div' : ProductShell"
    :class="{ 'history-embedded': embedded }"
    v-bind="
      embedded
        ? {}
        : {
            closeLabel: '关闭同步记录并返回入库总览',
            closeTo: '/control',
            displayName: '中心 B · 中控',
            hideNavigation: true,
            immersive: true,
            role: 'CONTROL',
            showClose: true,
          }
    "
  >
    <ControlServerTabs v-if="!embedded" active="history" />

    <section
      class="history-page"
      aria-labelledby="history-title"
      data-baseline-key="B-06-records"
      :data-view-source="view ? 'local-control-api' : 'frozen-baseline-fixture'"
    >
      <p class="screen-reader-only" role="status">
        {{ view ? '本页展示 B 中控本机只读的导入、归档与回执记录；后端不可用时会失败关闭。' : '本页为 B-06 冻结基线视觉夹具，不代表生产实时数据。' }}
      </p>
      <img
        :src="controlAssets.environment"
        alt=""
        class="environment"
        draggable="false"
      />
      <h1 id="history-title" class="screen-reader-only">同步记录</h1>
      <section
        class="history-workspace"
        data-layout-reference="edge-server-records"
      >
        <header class="history-heading-row">
          <div class="history-heading-copy">
            <h2>同步记录</h2>
            <p>中心本机历史独立保存</p>
          </div>
          <Alert
            class="history-note"
            message="运输盘移除或清理后，中心入库、校验、冲突与 receipt 历史仍可查询"
            role="note"
            show-icon
            type="info"
          />
        </header>

        <section class="history-summary" aria-label="同步记录摘要">
          <Button
            v-for="item in summaryFilters"
            :key="item.value"
            :aria-pressed="filter === item.value"
            class="summary-filter"
            :class="{ active: filter === item.value }"
            type="text"
            @click="selectFilter(item.value)"
          >
            {{ item.label }}
            <strong :class="item.tone">{{ item.count }}</strong>
          </Button>
        </section>

        <div class="history-controls">
          <Select
            v-model:value="source"
            aria-label="按来源工厂筛选"
            class="source-select"
            :options="sourceOptions"
          />
          <Select
            v-model:value="range"
            aria-label="按时间范围筛选"
            class="range-select"
            :options="rangeOptions"
          />
          <Input
            v-model:value="query"
            allow-clear
            aria-label="按批次号或序列号搜索"
            class="history-search"
            placeholder="批次号 / 序列号"
          >
            <template #prefix><span aria-hidden="true">⌕</span></template>
          </Input>
        </div>

        <Table
          aria-label="中心入库与同步记录"
          class="history-table"
          :columns="columns"
          :data-source="filteredRows"
          :pagination="false"
          row-key="key"
          size="middle"
        >
          <template #bodyCell="{ column, record }">
            <strong
              v-if="column.key === 'state'"
              :class="`tone-${toneFor(record.state)}`"
            >
              <i aria-hidden="true"></i>{{ record.stateLabel }}
            </strong>
            <strong
              v-else-if="column.key === 'result'"
              :class="`tone-${toneFor(record.state)}`"
            >
              {{ record.result }}
            </strong>
            <Button
              v-else-if="column.key === 'action'"
              :aria-label="`查看批次 ${record.batchId}，当前仅有列表夹具`"
              class="row-action"
              type="text"
              @click="explainRow(record)"
            >
              ›
            </Button>
          </template>
          <template #emptyText>
            <span class="empty-result">
              没有符合筛选条件的记录；中心历史汇总值保持不变。
            </span>
          </template>
        </Table>

        <footer class="history-footer">
          <span>共 {{ displayRows.length }} 条</span>
          <nav aria-label="同步记录分页">
            <Button aria-label="上一页" disabled type="text">‹</Button>
            <span class="pagination-count">
              <b>1</b>
              <span aria-hidden="true">/</span>
              <span>50</span>
            </span>
            <Button
              aria-label="下一页，冻结夹具未提供后续页"
              disabled
              title="冻结基线夹具只包含当前可视页"
              type="text"
            >
              ›
            </Button>
          </nav>
        </footer>
      </section>

      <p class="screen-reader-only" aria-live="polite">{{ rowNotice }}</p>
    </section>
  </component>
</template>

<style scoped>
.history-page {
  position: absolute;
  inset: 0;
  overflow: hidden;
  color: #c8ced5;
  background: #02070b;
}

.history-embedded {
  position: absolute;
  inset: 0;
}

.environment {
  position: absolute;
  inset: 0;
  z-index: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  opacity: 0.38;
}

.history-page::after {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  content: '';
  background:
    linear-gradient(
      90deg,
      rgb(1 5 8 / 9%) 0,
      rgb(1 5 8 / 18%) 67%,
      transparent
    ),
    linear-gradient(180deg, #02070b 0, rgb(2 7 11 / 58%) 64%, transparent);
}

.history-workspace {
  position: absolute;
  inset: 94px 350px 34px 108px;
  z-index: 2;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.history-heading-row {
  display: grid;
  flex: 0 0 58px;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 30px;
  align-items: center;
}

.history-note,
.history-summary,
.history-controls,
.history-table,
.history-footer {
  position: relative;
  width: 100%;
}

.history-heading-copy {
  display: flex;
  gap: 26px;
  align-items: baseline;
  white-space: nowrap;
}

.history-heading-copy h2 {
  margin: 0;
  font-size: 30px;
  font-weight: 400;
  line-height: 36px;
  color: #d8e0e7;
}

.history-heading-copy p {
  margin: 0;
  font-size: 15px;
  line-height: 24px;
  color: #858f9a;
}

.history-note.ant-alert {
  min-width: 0;
  height: 38px;
  padding: 0 16px;
  margin: 0;
  overflow: hidden;
  background: rgb(37 48 59 / 45%);
  border: 0;
  border-radius: 6px;
}

.history-note :deep(.ant-alert-icon) {
  margin-right: 16px;
  font-size: 18px;
  color: #9aa5af;
}

.history-note :deep(.ant-alert-message) {
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 15px;
  color: #b3bbc3;
  white-space: nowrap;
}

.history-summary {
  display: grid;
  flex: 0 0 66px;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  border-bottom: 1px solid rgb(112 130 149 / 28%);
}

.history-summary .summary-filter.ant-btn {
  position: relative;
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: center;
  height: 66px;
  padding: 0;
  font-size: 16px;
  line-height: 24px;
  color: #aeb6bf;
  background: transparent;
  border: 0;
  border-radius: 0;
  box-shadow: none;
}

.history-summary :deep(.summary-filter.ant-btn > span) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.history-summary .summary-filter.ant-btn + .summary-filter.ant-btn {
  padding-left: 0;
}

.history-summary .summary-filter.ant-btn + .summary-filter.ant-btn::before {
  position: absolute;
  top: 16px;
  bottom: 17px;
  left: 0;
  width: 1px;
  content: '';
  background: rgb(112 130 149 / 34%);
}

.history-summary .summary-filter.ant-btn.active {
  color: #18afff;
}

.history-summary .summary-filter.ant-btn:hover,
.history-summary .summary-filter.ant-btn:focus {
  color: #18afff;
  background: rgb(22 143 255 / 6%);
}

.history-summary strong {
  font-size: 30px;
  font-weight: 350;
  line-height: 36px;
  color: #e7ebef;
}

.history-summary .running,
.tone-running {
  color: #168fff;
}

.history-summary .success,
.tone-success {
  color: #09d891;
}

.history-summary .warning,
.tone-warning {
  color: #ffad18;
}

.history-summary .danger,
.tone-danger {
  color: #ff4d61;
}

.history-controls {
  display: grid;
  flex: 0 0 70px;
  grid-template-columns: 160px 180px 220px minmax(0, 1fr);
  gap: 12px;
  align-items: center;
}

.source-select,
.range-select,
.history-search {
  width: 100%;
}

.range-select {
  grid-column: 2;
}

.history-search {
  grid-column: 3;
}

.source-select :deep(.ant-select-selector),
.range-select :deep(.ant-select-selector),
.history-search :deep(.ant-input-affix-wrapper),
.history-search {
  height: 38px !important;
  color: #c8ced5;
  background: rgb(6 13 19 / 84%) !important;
  border-color: rgb(112 130 149 / 30%) !important;
  border-radius: 5px !important;
  box-shadow: none !important;
}

.source-select :deep(.ant-select-selection-item),
.range-select :deep(.ant-select-selection-item) {
  display: flex;
  align-items: center;
  color: #c8ced5;
}

.source-select :deep(.ant-select-arrow),
.range-select :deep(.ant-select-arrow) {
  color: #9ca5af;
}

.history-search :deep(input) {
  height: 34px;
  color: #c8ced5;
  background: transparent;
}

.history-search :deep(input::placeholder) {
  color: #89939f;
}

.history-table {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
  background: linear-gradient(180deg, rgb(6 17 27 / 34%), rgb(3 10 16 / 68%));
  border-bottom: 1px solid rgb(91 123 155 / 30%);
}

.history-table :deep(.ant-table),
.history-table :deep(.ant-table-container),
.history-table :deep(.ant-table-content),
.history-table :deep(table) {
  background: transparent;
}

.history-table :deep(.ant-table-thead > tr > th) {
  height: 52px;
  padding: 0 14px;
  font-size: 15px;
  font-weight: 400;
  color: #aeb7c0;
  background: rgb(21 32 42 / 74%);
  border-color: rgb(91 123 155 / 31%);
}

.history-table :deep(.ant-table-thead > tr > th::before) {
  display: none;
}

.history-table :deep(.ant-table-tbody > tr > td) {
  height: 57px;
  padding: 0 14px;
  font-size: 15px;
  color: #afb8c1;
  background: transparent;
  border-color: rgb(91 123 155 / 25%);
}

.history-table :deep(.ant-table-tbody > tr:hover > td) {
  background: rgb(20 94 142 / 10%);
}

.history-table strong {
  font-weight: 400;
  white-space: nowrap;
}

.history-table strong i {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 8px;
  background: currentcolor;
  border-radius: 50%;
}

.row-action {
  width: 28px;
  height: 36px;
  padding: 0;
  font-size: 30px;
  line-height: 1;
  color: #d6e0e8;
  background: transparent;
  border: 0;
  box-shadow: none;
}

.row-action:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 2px;
}

.empty-result {
  color: #75818e;
}

.history-footer {
  display: flex;
  flex: 0 0 52px;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px 0 14px;
  color: #8c96a1;
  border-top: 1px solid rgb(112 130 149 / 20%);
}

.history-footer nav {
  display: flex;
  gap: 12px;
  align-items: center;
}

.history-footer :deep(.ant-btn) {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  font-size: 23px;
  line-height: 1;
  color: #aab5bf;
  background: rgb(8 17 25 / 72%);
  border: 1px solid rgb(112 130 149 / 28%);
  border-radius: 5px;
  box-shadow: none;
}

.history-footer :deep(.ant-btn:disabled) {
  color: #46515d;
  cursor: not-allowed;
  background: rgb(8 17 25 / 72%);
  border-color: rgb(112 130 149 / 14%);
}

.pagination-count {
  display: flex;
  gap: 8px;
  align-items: center;
}

.history-footer b {
  font-weight: 500;
  color: #18afff;
}

@media (prefers-reduced-motion: reduce) {
  .history-page * {
    transition: none !important;
  }
}
</style>
