<!-- A-04-records · A-04-packed-detail · A-04-failed-detail -->
<script setup lang="ts">
import type { StepProps, TableColumnsType } from 'ant-design-vue';

import type {
  EdgeSyncRecord,
  EdgeSyncRecordState,
  EdgeSyncRecordsView,
} from '#/api/local-views';

import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';

import { PageHeader, Steps, Table } from 'ant-design-vue';

import { formatBytes, formatTimestamp } from '../model';

const props = defineProps<{ embedded?: boolean; view: EdgeSyncRecordsView }>();
const route = useRoute();
const router = useRouter();
const query = ref('');
const filter = ref<'ALL' | EdgeSyncRecordState>('ALL');
const timeRange = ref<'7' | '30' | 'ALL'>('30');
const currentPage = ref(1);
const pageSize = 8;
const rackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';
const packedShieldAsset = '/assets/fustfs-baseline/a04-packed-shield-v1.png';
const failedLockAsset = '/assets/fustfs-baseline/a04-failed-lock-v1.png';
const failedLockSmallAsset =
  '/assets/fustfs-baseline/a04-failed-lock-small-v1.png';
const filterOptions: Array<{
  label: string;
  value: 'ALL' | EdgeSyncRecordState;
}> = [
  { label: '全部', value: 'ALL' },
  { label: '进行中', value: 'WAITING_CENTRAL' },
  { label: '装盘成功', value: 'PACKED' },
  { label: '失败', value: 'FAILED' },
];
const tableHeaders = ['时间', '批次', '状态', '数据量', '运输盘', '结果', ''];
type EventRow = EdgeSyncRecord['events'][number] & {
  key: string;
  time_label: string;
};
const eventColumns: TableColumnsType<EventRow> = [
  {
    dataIndex: 'time_label',
    key: 'time',
    title: '时间',
    width: '30%',
  },
  {
    dataIndex: 'label',
    key: 'event',
    title: '事件',
    width: '40%',
  },
  {
    dataIndex: 'result',
    key: 'result',
    title: '结果',
    width: '30%',
  },
];

const selected = computed(() => {
  const batchId =
    typeof route.params.batchId === 'string' ? route.params.batchId : '';
  return (
    props.view.records.find((record) => record.batch_id === batchId) ?? null
  );
});

const filteredRecords = computed(() => {
  const needle = query.value.trim().toLowerCase();
  const anchor = new Date(
    props.view.meta.data_as_of ?? props.view.meta.generated_at,
  );
  const rangeDays =
    timeRange.value === 'ALL' ? null : Number.parseInt(timeRange.value, 10);
  const cutoff =
    rangeDays && !Number.isNaN(anchor.valueOf())
      ? anchor.valueOf() - rangeDays * 24 * 60 * 60 * 1000
      : null;

  return props.view.records.filter((record) => {
    const stateMatch = filter.value === 'ALL' || record.state === filter.value;
    const completedAt = record.completed_at
      ? new Date(record.completed_at).valueOf()
      : Number.NaN;
    const timeMatch =
      cutoff === null || (!Number.isNaN(completedAt) && completedAt >= cutoff);
    const textMatch =
      !needle ||
      record.batch_id.toLowerCase().includes(needle) ||
      record.media_serial_suffix.toLowerCase().includes(needle);
    return stateMatch && timeMatch && textMatch;
  });
});

const pageCount = computed(() =>
  Math.max(1, Math.ceil(filteredRecords.value.length / pageSize)),
);

const pagedRecords = computed(() => {
  const start = (currentPage.value - 1) * pageSize;
  return filteredRecords.value.slice(start, start + pageSize);
});

function stageStepStatus(
  state: EdgeSyncRecord['stages'][number]['state'],
): StepProps['status'] {
  if (state === 'FAILED') return 'error';
  if (state === 'PASSED') return 'finish';
  return 'wait';
}

const stageItems = computed<StepProps[]>(() =>
  (selected.value?.stages ?? []).map((stage) => ({
    description: stage.at ? shortTime(stage.at) : '未执行',
    status: stageStepStatus(stage.state),
    title: stage.label,
  })),
);

const eventRows = computed<EventRow[]>(() =>
  (selected.value?.events ?? []).map((event, index) => ({
    ...event,
    key: `${event.at}-${event.label}-${index}`,
    time_label: formatTimestamp(event.at),
  })),
);

const selectedStatus = computed(() => {
  const record = selected.value;
  if (!record) return null;

  switch (record.state) {
    case 'CLOSED':
      return {
        detail: '已收到闭环凭据；以记录中的事件与阶段证据为准。',
        heading: '端到端闭环已确认',
        timeLabel: '闭环确认时间',
      };
    case 'PARTIALLY_CLOSED':
      return {
        detail: '回执未覆盖全部对象，不能视为完整闭环。',
        heading: '部分闭环，等待缺失回执',
        timeLabel: '最近状态时间',
      };
    case 'WAITING_CENTRAL':
      return {
        detail: '本机已保留运输记录，尚未确认中控已接收。',
        heading: '等待中控接收确认',
        timeLabel: '最近状态时间',
      };
    case 'WAITING_RECEIPT':
      return {
        detail: '中控回执尚未返回本机，不能视为端到端闭环。',
        heading: '等待中控回执',
        timeLabel: '最近状态时间',
      };
    case 'PACKED':
      return {
        detail: '本机已生成装盘记录；尚未确认中控接收或回执。',
        heading: '本机装盘完成，等待后续确认',
        timeLabel: '本机记录时间',
      };
    case 'FAILED':
      return {
        detail: '危险写入已停止，等待人工处置。',
        heading: '本地阶段异常锁定',
        timeLabel: '失败时间',
      };
  }
});

const hasStageEvidence = computed(
  () => (selected.value?.stages.length ?? 0) > 0,
);

function selectFilter(next: 'ALL' | EdgeSyncRecordState) {
  filter.value = next;
  currentPage.value = 1;
}

function previousPage() {
  if (currentPage.value > 1) currentPage.value -= 1;
}

function nextPage() {
  if (currentPage.value < pageCount.value) currentPage.value += 1;
}

function openRecord(record: EdgeSyncRecord) {
  void router.push(
    `/edge/server/records/${encodeURIComponent(record.batch_id)}`,
  );
}

function closeRecord() {
  void router.push('/edge/server');
}

function stateTone(state: EdgeSyncRecordState) {
  if (state === 'FAILED') return 'danger';
  if (state === 'CLOSED' || state === 'PARTIALLY_CLOSED') return 'success';
  if (state === 'PACKED') return 'running';
  return 'warning';
}

function eventTone(state: string) {
  if (state === 'FAILED') return 'danger';
  if (state === 'PASSED') return 'success';
  return 'muted';
}

function shortTime(value: null | string) {
  if (!value) return '未知';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) return '未知';
  return parsed.toLocaleString('zh-CN', {
    day: '2-digit',
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    month: '2-digit',
    timeZone: 'Asia/Shanghai',
  });
}

watch([query, timeRange], () => {
  currentPage.value = 1;
});

watch(pageCount, (nextPageCount) => {
  if (currentPage.value > nextPageCount) currentPage.value = nextPageCount;
});
</script>

<template>
  <section
    aria-labelledby="edge-records-title"
    class="edge-records"
    :class="{ embedded }"
  >
    <h1 id="edge-records-title" class="screen-reader-only">同步记录</h1>
    <img v-if="!embedded" :src="rackAsset" alt="" class="fallback-rack" />

    <section v-if="!selected" class="record-list server-section-enter">
      <header class="records-heading">
        <div class="records-heading-copy">
          <h2>同步记录</h2>
          <p>本机历史独立保存</p>
        </div>
        <p class="media-notice">
          <span aria-hidden="true">i</span>
          {{
            view.transport_media_connected
              ? '运输盘已连接 · 历史记录可正常查询'
              : '当前未检测到运输盘 · 历史记录可正常查询'
          }}
        </p>
      </header>
      <div class="record-summary">
        <button
          :class="{ active: filter === 'ALL' }"
          type="button"
          @click="selectFilter('ALL')"
        >
          <span>全部</span>
          <strong>{{ view.summary.total }}</strong>
        </button>
        <button
          :class="{ active: filter === 'PACKED' }"
          type="button"
          @click="selectFilter('PACKED')"
        >
          <span>装盘成功</span>
          <strong>{{ view.summary.packed }}</strong>
        </button>
        <button
          :class="{ active: filter === 'FAILED' }"
          type="button"
          @click="selectFilter('FAILED')"
        >
          <span>失败</span>
          <strong class="danger">{{ view.summary.failed }}</strong>
        </button>
      </div>
      <div class="record-controls">
        <div class="filter-buttons" aria-label="同步记录状态筛选">
          <button
            v-for="item in filterOptions"
            :key="item.value"
            :aria-pressed="filter === item.value"
            :class="{ active: filter === item.value }"
            type="button"
            @click="selectFilter(item.value)"
          >
            {{ item.label }}
          </button>
        </div>
        <select v-model="timeRange" aria-label="同步记录时间范围">
          <option value="30">最近 30 天</option>
          <option value="7">最近 7 天</option>
          <option value="ALL">全部时间</option>
        </select>
        <label>
          <span class="screen-reader-only">搜索批次号或运输盘序列号</span>
          <input v-model="query" placeholder="批次号 / 序列号" type="search" />
        </label>
      </div>
      <div class="records-table" role="table" aria-label="本机同步记录">
        <div class="table-head" role="row">
          <span v-for="header in tableHeaders" :key="header || 'actions'">
            {{ header }}
          </span>
        </div>
        <div
          class="record-scroll"
          aria-label="同步记录列表，可纵向滚动"
          tabindex="0"
        >
          <button
            v-for="record in pagedRecords"
            :key="record.batch_id"
            :aria-label="`查看批次 ${record.batch_id} 详情`"
            class="record-row"
            type="button"
            @click="openRecord(record)"
          >
            <span>{{ shortTime(record.completed_at) }}</span>
            <span>{{ record.batch_id }}</span>
            <strong :class="`tone-${stateTone(record.state)}`">
              <i></i>{{ record.result_label }}
            </strong>
            <span>{{ formatBytes(record.logical_bytes) }}</span>
            <span>SN …{{ record.media_serial_suffix }}</span>
            <span>{{ record.destination_label }}</span>
            <span aria-hidden="true">›</span>
          </button>
          <p v-if="filteredRecords.length === 0" class="empty-result">
            没有符合当前筛选条件的同步记录
          </p>
        </div>
      </div>
      <footer class="records-pagination">
        <span>共 {{ filteredRecords.length }} 条</span>
        <nav aria-label="同步记录分页">
          <button
            aria-label="上一页"
            :disabled="currentPage === 1"
            type="button"
            @click="previousPage"
          >
            ‹
          </button>
          <span class="pagination-count">
            <b>{{ currentPage }}</b>
            <span aria-hidden="true">/</span>
            <span>{{ pageCount }}</span>
          </span>
          <button
            aria-label="下一页"
            :disabled="currentPage === pageCount"
            type="button"
            @click="nextPage"
          >
            ›
          </button>
        </nav>
      </footer>
    </section>

    <section
      v-else
      class="record-detail server-section-enter"
      :class="`detail-${selected.state.toLowerCase()}`"
    >
      <PageHeader
        class="detail-page-header"
        :sub-title="selected.batch_id"
        title="批次详情"
        @back="closeRecord"
      >
        <template #tags>
          <strong :class="`tone-${stateTone(selected.state)}`">
            {{ selected.result_label }}
          </strong>
        </template>
      </PageHeader>

      <dl class="detail-overview">
        <div>
          <dt>{{ selectedStatus?.timeLabel ?? '最近状态时间' }}</dt>
          <dd>{{ shortTime(selected.completed_at) }}</dd>
        </div>
        <div>
          <dt>数据量</dt>
          <dd>{{ formatBytes(selected.logical_bytes) }}</dd>
        </div>
        <div>
          <dt>运输盘</dt>
          <dd>SN …{{ selected.media_serial_suffix }}</dd>
        </div>
        <div>
          <dt>目标站点</dt>
          <dd>{{ selected.destination_label }}</dd>
        </div>
      </dl>

      <article class="stage-card">
        <header class="stage-card-heading">
          <h3>{{ selectedStatus?.heading ?? '状态证据不可用' }}</h3>
          <strong v-if="hasStageEvidence" class="tone-muted">
            已返回阶段证据
          </strong>
        </header>
        <Steps
          class="record-stage-steps"
          :items="stageItems"
          label-placement="vertical"
          size="small"
        />
      </article>

      <template v-if="selected.state === 'FAILED'">
        <dl class="failure-summary">
          <div>
            <dt>失败阶段</dt>
            <dd>{{ selected.failure_stage ?? '未知' }}</dd>
          </div>
          <div>
            <dt>失败原因</dt>
            <dd>{{ selected.failure_reason ?? '未知' }}</dd>
          </div>
          <div>
            <dt>重试结果</dt>
            <dd>{{ selected.retry_result ?? '未执行' }}</dd>
          </div>
          <div>
            <dt>介质状态</dt>
            <dd>异常锁定</dd>
          </div>
        </dl>
        <p class="danger-lock">
          <img :src="failedLockAsset" alt="" class="detail-state-icon" />
          <span class="detail-state-copy">
            <strong>已停止危险写入 · 未生成可送出标记</strong>
            <span>保留对象身份、批次和检查点，等待换盘或管理员处理</span>
          </span>
          <img
            :src="failedLockSmallAsset"
            alt=""
            class="detail-state-icon detail-state-icon-small"
          />
        </p>
        <Table
          class="event-table"
          :columns="eventColumns"
          :data-source="eventRows"
          :pagination="false"
          row-key="key"
          size="small"
        >
          <template #bodyCell="{ column, record }">
            <strong
              v-if="column.key === 'result'"
              :class="`tone-${eventTone(record.state)}`"
            >
              <i></i>{{ record.result }}
            </strong>
          </template>
        </Table>
      </template>

      <template v-else>
        <p class="delivery-state">
          <img :src="packedShieldAsset" alt="" class="detail-state-icon" />
          <span class="detail-state-copy">
            <strong>{{ selectedStatus?.heading ?? '状态证据不可用' }}</strong>
            <span>{{ selectedStatus?.detail ?? '当前状态无法确认，请重新读取本机视图。' }}</span>
          </span>
        </p>
        <div class="object-preview">
          <h3>对象 / 审计明细</h3>
          <p class="object-unavailable">
            当前本机只读视图未返回对象级明细。请以批次阶段和已签名完成证据为准，
            不使用示例对象替代真实事实。
          </p>
        </div>
      </template>
    </section>
  </section>
</template>

<style scoped>
.edge-records {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #d8dee5;
  isolation: isolate;
}

.edge-records::before {
  position: absolute;
  inset: 50% 0 0;
  z-index: 0;
  pointer-events: none;
  content: '';
  background: linear-gradient(
    180deg,
    rgb(2 8 13 / 0%) 0%,
    rgb(2 8 13 / 76%) 27%,
    #02070b 54%,
    #02070b 100%
  );
}

.edge-records.embedded {
  z-index: 2;
}

.edge-records.embedded .record-list {
  inset: 235px var(--fd-server-content-right, 55px) 28px
    var(--fd-server-content-left, 535px);
}

.fallback-rack {
  display: none;
}

.record-list,
.record-detail {
  position: absolute;
  inset: 78px var(--fd-server-content-right, 55px) 34px
    var(--fd-server-content-left, 535px);
  z-index: 2;
}

.record-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.records-heading {
  display: grid;
  flex: 0 0 58px;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 30px;
  align-items: center;
}

.records-heading-copy {
  display: flex;
  gap: 26px;
  align-items: baseline;
}

.record-list h2,
.record-detail h2 {
  margin: 0;
  font-size: var(--fd-server-section-title-size, 30px);
  font-weight: 400;
  line-height: var(--fd-server-section-title-line-height, 36px);
  color: var(--fd-server-section-title-color, #d8e0e7);
}

.records-heading-copy p {
  margin: 0;
  font-size: var(--fd-server-heading-note-size, 15px);
  line-height: var(--fd-server-heading-note-line-height, 24px);
  color: var(--fd-server-heading-note-color, #8d96a0);
  white-space: nowrap;
}

.media-notice {
  display: flex;
  gap: 16px;
  align-items: center;
  min-width: 0;
  height: 38px;
  padding: 0 16px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #b3bbc3;
  white-space: nowrap;
  background: rgb(37 48 59 / 45%);
  border-radius: 6px;
}

.media-notice span {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  font-size: 12px;
  font-weight: 700;
  color: #0d151d;
  background: #9aa5af;
  border-radius: 50%;
}

.record-summary {
  display: grid;
  flex: 0 0 66px;
  grid-template-columns: repeat(3, 1fr);
  border-bottom: 1px solid rgb(112 130 149 / 28%);
}

.record-summary button {
  position: relative;
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: center;
  height: 100%;
  padding: 0;
  font-size: 16px;
  line-height: 24px;
  color: #aab2bb;
  background: transparent;
  border: 0;
}

.record-summary button > span {
  line-height: 24px;
}

.record-summary button + button::before {
  position: absolute;
  top: 16px;
  bottom: 17px;
  left: 0;
  width: 1px;
  content: '';
  background: rgb(112 130 149 / 28%);
}

.record-summary button.active {
  color: #1aa9ff;
}

.record-summary strong {
  font-size: 30px;
  font-weight: 350;
  line-height: 36px;
  color: #e7ebef;
}

.record-summary .warning {
  color: var(--fd-warning);
}

.record-summary .success {
  color: var(--fd-success);
}

.record-summary .danger {
  color: var(--fd-danger);
}

.record-controls {
  display: grid;
  flex: 0 0 70px;
  grid-template-columns: 1fr 180px 220px;
  gap: 12px;
  align-items: center;
}

.filter-buttons {
  display: flex;
  gap: 6px;
}

.filter-buttons button,
.record-controls select,
.record-controls input {
  height: 38px;
  padding: 0 14px;
  color: #aeb7c0;
  background: #111a22;
  border: 1px solid transparent;
  border-radius: 5px;
}

.filter-buttons button.active {
  color: #1baaff;
  border-color: #169ce5;
}

.record-controls select,
.record-controls input {
  width: 100%;
  background: rgb(6 13 19 / 84%);
  border-color: rgb(112 130 149 / 30%);
}

.record-controls input:focus-visible,
.record-controls select:focus-visible,
.filter-buttons button:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 2px;
}

.records-table {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  font-size: 15px;
  background: linear-gradient(180deg, rgb(6 17 27 / 34%), rgb(3 10 16 / 68%));
}

.table-head,
.record-row {
  display: grid;
  grid-template-columns: 145px 175px 145px 110px 125px 1fr 24px;
  align-items: center;
  min-height: 57px;
  padding: 0 14px;
}

.table-head {
  flex: 0 0 52px;
  min-height: 52px;
  color: #a7b0ba;
  background: rgb(21 32 42 / 74%);
}

.record-scroll {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden auto;
  scrollbar-gutter: stable;
  outline: none;
  scrollbar-color: rgb(60 108 144 / 72%) rgb(8 17 25 / 48%);
  scrollbar-width: thin;
}

.record-scroll::-webkit-scrollbar {
  width: 8px;
}

.record-scroll::-webkit-scrollbar-track {
  background: rgb(8 17 25 / 48%);
}

.record-scroll::-webkit-scrollbar-thumb {
  background: rgb(60 108 144 / 72%);
  border: 2px solid rgb(8 17 25 / 48%);
  border-radius: 999px;
}

.record-scroll:focus-visible {
  box-shadow: inset 0 0 0 2px rgb(88 220 255 / 70%);
}

.record-row {
  width: 100%;
  color: #aeb6bf;
  text-align: left;
  background: transparent;
  border: 0;
  border-bottom: 1px solid rgb(112 130 149 / 20%);
}

.record-row:hover,
.record-row:focus-visible {
  outline: 0;
  background: rgb(20 94 142 / 10%);
}

.record-row strong,
.event-table strong {
  font-weight: 400;
}

.record-row strong i,
.event-table strong i {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 8px;
  background: currentcolor;
  border-radius: 50%;
}

.tone-running {
  color: #169fff !important;
}

.tone-success {
  color: var(--fd-success) !important;
}

.tone-warning {
  color: var(--fd-warning) !important;
}

.tone-danger {
  color: var(--fd-danger) !important;
}

.tone-muted {
  color: #76818d !important;
}

.empty-result {
  padding: 34px;
  color: #77828e;
  text-align: center;
}

.record-list footer {
  display: flex;
  flex: 0 0 52px;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px 0 14px;
  color: #8c96a1;
  border-top: 1px solid rgb(112 130 149 / 20%);
}

.record-list footer b {
  color: #1aaaff;
}

.records-pagination nav {
  display: flex;
  gap: 12px;
  align-items: center;
}

.records-pagination button {
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
}

.records-pagination button:disabled {
  color: #46515d;
  cursor: not-allowed;
  border-color: rgb(112 130 149 / 14%);
}

.record-detail {
  --fd-running: #00a6ff;
  --fd-success: #2dd591;
  --fd-danger: #fb4e45;
  --a04-detail-panel: rgb(2 10 17 / 88%);
  --a04-detail-panel-strong: rgb(1 8 14 / 92%);
  --a04-detail-border: #18242d;
  --a04-detail-divider: #1d2931;
  --a04-detail-primary: #e4e0dc;
  --a04-detail-secondary: #aaa5a0;
  --a04-detail-muted: #85888c;
  --a04-detail-blue: #00a6ff;
  --a04-detail-header-title-size: 28px;
  --a04-detail-header-icon-optical-offset: 6px;

  top: 73px;
  color: var(--a04-detail-primary);
}

.detail-page-header {
  height: 58px;
  padding: 0;
  background: transparent;
}

.detail-page-header :deep(.ant-page-header-heading) {
  display: flex;
  align-items: center;
  height: 58px;
  margin: 0;
}

.detail-page-header :deep(.ant-page-header-heading-left) {
  display: flex;
  align-items: center;
  min-width: 0;
  height: 100%;
}

.detail-page-header :deep(.ant-page-header-back) {
  display: flex;
  align-items: center;
  align-self: center;
  height: 34px;
  margin-right: 14px;
}

.detail-page-header :deep(.ant-page-header-back-button) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  padding: 0;
  line-height: 1;
  color: var(--a04-detail-blue);
  background: rgb(0 166 255 / 6%);
  border: 1px solid rgb(0 166 255 / 20%);
  border-radius: 6px;
}

.detail-page-header :deep(.ant-page-header-back-button:hover) {
  color: #39b9ff;
  background: rgb(0 166 255 / 12%);
  border-color: rgb(0 166 255 / 42%);
}

.detail-page-header :deep(.ant-page-header-back-button .anticon) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--a04-detail-header-title-size);
  height: var(--a04-detail-header-title-size);
  font-size: var(--a04-detail-header-title-size);
  line-height: 0;
  vertical-align: 0;
  transform: translateY(var(--a04-detail-header-icon-optical-offset));
}

.detail-page-header :deep(.ant-page-header-back-button .anticon > svg) {
  display: block;
  width: 100%;
  height: 100%;
}

.detail-page-header :deep(.ant-page-header-heading-title) {
  display: inline-flex;
  align-items: center;
  height: 34px;
  padding-right: 0;
  margin-right: 16px;
  overflow: visible;
  font-size: var(--a04-detail-header-title-size);
  font-weight: 400;
  line-height: 34px;
  color: #ebe7e2;
}

.detail-page-header :deep(.ant-page-header-heading-sub-title) {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  max-width: 280px;
  height: 34px;
  margin-right: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 15px;
  line-height: 34px;
  color: var(--a04-detail-secondary);
  white-space: nowrap;
}

.detail-page-header :deep(.ant-page-header-heading-tags) {
  display: inline-flex;
  align-items: center;
  height: 34px;
  margin: 0;
}

.detail-page-header :deep(.ant-page-header-heading-tags > strong) {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 3px 10px;
  font-size: 14px;
  font-weight: 400;
  border: 1px solid currentcolor;
  border-radius: 5px;
}

.detail-page-header :deep(.ant-page-header-heading-tags > .tone-running) {
  background: rgb(0 166 255 / 10%);
}

.detail-page-header :deep(.ant-page-header-heading-tags > .tone-danger) {
  background: rgb(251 78 69 / 9%);
}

.detail-overview {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  height: 94px;
  margin: 0 0 10px;
  background: var(--a04-detail-panel);
  border: 1px solid var(--a04-detail-border);
  border-radius: 6px;
}

.detail-overview div {
  position: relative;
  display: grid;
  gap: 6px;
  align-content: center;
  padding-left: 32px;
}

.detail-overview div + div::before {
  position: absolute;
  top: 20px;
  bottom: 20px;
  left: 0;
  width: 1px;
  content: '';
  background: var(--a04-detail-divider);
}

.detail-overview dt,
.validation-summary dt,
.failure-summary dt {
  color: var(--a04-detail-secondary);
}

.detail-overview dd {
  font-size: 20px;
  color: var(--a04-detail-primary);
}

.stage-card {
  height: 164px;
  padding: 13px 20px 10px;
  background: var(--a04-detail-panel-strong);
  border: 1px solid var(--a04-detail-border);
  border-radius: 6px;
}

.stage-card-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.stage-card h3,
.object-preview h3 {
  margin: 0;
  font-size: 17px;
  font-weight: 500;
}

.stage-card-heading > strong {
  font-size: 14px;
  font-weight: 400;
}

.record-stage-steps {
  padding: 0 44px;
  margin-top: 25px;
}

.record-stage-steps :deep(.ant-steps-item-title) {
  font-size: 16px;
  line-height: 24px;
  color: var(--a04-detail-primary);
}

.record-stage-steps :deep(.ant-steps-item-description) {
  padding-top: 2px;
  font-size: 13px;
  line-height: 20px;
  color: var(--a04-detail-muted);
}

.detail-failed
  .record-stage-steps
  :deep(.ant-steps-item-finish .ant-steps-item-title) {
  color: var(--fd-success);
}

.detail-packed
  .record-stage-steps
  :deep(.ant-steps-item-finish .ant-steps-item-title) {
  color: var(--a04-detail-primary);
}

.record-stage-steps :deep(.ant-steps-item-error .ant-steps-item-title) {
  color: var(--fd-danger);
}

.record-stage-steps :deep(.ant-steps-item-wait .ant-steps-item-title) {
  color: var(--a04-detail-secondary);
}

.record-stage-steps :deep(.ant-steps-item-finish .ant-steps-item-icon) {
  color: var(--fd-success);
  background: transparent;
  border-color: var(--fd-success);
}

.record-stage-steps
  :deep(.ant-steps-item-finish .ant-steps-item-icon > .ant-steps-icon) {
  color: var(--fd-success);
}

.detail-packed
  .record-stage-steps
  :deep(.ant-steps-item-finish .ant-steps-item-icon) {
  background: linear-gradient(135deg, #31d99a, #1aa670);
  box-shadow: 0 0 10px rgb(45 213 145 / 24%);
}

.detail-packed
  .record-stage-steps
  :deep(.ant-steps-item-finish .ant-steps-item-icon > .ant-steps-icon) {
  color: #fff;
}

.record-stage-steps :deep(.ant-steps-item-error .ant-steps-item-icon) {
  background: transparent;
  border-color: var(--fd-danger);
}

.record-stage-steps
  :deep(.ant-steps-item-error .ant-steps-item-icon > .ant-steps-icon) {
  color: var(--fd-danger);
}

.record-stage-steps :deep(.ant-steps-item-tail::after) {
  height: 2px;
  background-color: #53595d;
}

.record-stage-steps :deep(.ant-steps-item-finish .ant-steps-item-tail::after) {
  background-color: var(--fd-success);
}

.record-stage-steps :deep(.ant-steps-item-error .ant-steps-item-tail::after) {
  background-color: var(--fd-danger);
}

.validation-summary,
.failure-summary {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  height: 86px;
  margin: 10px 0;
  background: var(--a04-detail-panel);
  border: 1px solid var(--a04-detail-border);
  border-radius: 6px;
}

.validation-summary div,
.failure-summary div {
  position: relative;
  display: grid;
  gap: 5px;
  align-content: center;
  padding-left: 24px;
}

.validation-summary dd {
  color: var(--fd-success);
}

.failure-summary dd {
  color: var(--fd-danger);
}

.delivery-state,
.danger-lock {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr);
  gap: 18px;
  align-content: center;
  align-items: center;
  height: 78px;
  padding: 8px 30px;
  margin: 0 0 10px;
  color: var(--a04-detail-secondary);
  background: var(--a04-detail-panel-strong);
  border: 1px solid var(--a04-detail-border);
  border-radius: 6px;
}

.delivery-state strong {
  font-size: 18px;
  font-weight: 400;
  color: var(--a04-detail-blue);
}

.danger-lock {
  grid-template-columns: 52px minmax(0, 1fr) 48px;
  background: rgb(42 8 10 / 38%);
  border-color: #6b292a;
}

.danger-lock strong {
  font-size: 18px;
  font-weight: 400;
  color: var(--fd-danger);
}

.detail-state-icon {
  display: block;
  width: 50px;
  height: auto;
}

.delivery-state .detail-state-icon {
  width: 58px;
}

.detail-state-icon-small {
  justify-self: end;
  width: 42px;
}

.detail-state-copy {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.detail-state-copy > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.object-preview {
  padding: 11px 18px;
  background: var(--a04-detail-panel);
  border: 1px solid var(--a04-detail-border);
  border-radius: 6px;
}

.object-preview p {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  padding: 9px 0;
  margin: 0;
  border-top: 1px solid var(--a04-detail-divider);
}

.object-preview .object-unavailable {
  display: block;
  line-height: 1.7;
  color: var(--a04-detail-muted);
}

.object-preview strong {
  color: var(--fd-success);
}

.event-table {
  padding: 10px 18px;
  background: var(--a04-detail-panel);
  border: 1px solid var(--a04-detail-border);
  border-radius: 6px;
}

.event-table :deep(.ant-table),
.event-table :deep(.ant-table-cell-fix-left),
.event-table :deep(.ant-table-cell-fix-right) {
  color: var(--a04-detail-primary);
  background: transparent;
}

.event-table :deep(.ant-table-container) {
  border: 0;
}

.event-table :deep(.ant-table-thead > tr > th) {
  padding: 10px 0;
  font-size: 14px;
  font-weight: 500;
  color: var(--a04-detail-secondary);
  background: transparent;
  border-color: var(--a04-detail-divider);
}

.event-table :deep(.ant-table-thead > tr > th::before) {
  display: none;
}

.event-table :deep(.ant-table-tbody > tr > td) {
  padding: 11px 0;
  color: var(--a04-detail-secondary);
  background: transparent;
  border-color: var(--a04-detail-divider);
}

.event-table :deep(.ant-table-tbody > tr:last-child > td) {
  border-bottom: 0;
}

.event-table :deep(.ant-table-tbody > tr.ant-table-row:hover > td) {
  background: rgb(0 166 255 / 4%);
}

.event-table :deep(.ant-empty-description) {
  color: var(--a04-detail-muted);
}

@media (prefers-reduced-motion: reduce) {
  * {
    scroll-behavior: auto !important;
  }
}
</style>
