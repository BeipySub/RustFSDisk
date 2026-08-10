<!--
  @file sites-panel.vue
  @description B-02 子工厂状态列表；沿用 /control/history 的页面母版、Ant Design 组件和表格细节。
  @usage 由 experience.vue 或 sites.vue 注入 CONTROL 视图数据。
  @baseline B-02-factory-list · 1672×941（布局参考：/control/history）
-->
<script setup lang="ts">
import type { TableColumnsType } from 'ant-design-vue';

import type { ControlSitesView, SnapshotViewState } from '#/api/local-views';

import { computed, ref } from 'vue';

import { Alert, Button, Input, Table } from 'ant-design-vue';

import StatusChip from '../components/status-chip.vue';
import {
  formatBytes,
  formatCount,
  formatTimestamp,
  snapshotLabel,
  snapshotTone,
} from '../model';

const props = withDefaults(
  defineProps<{ embedded?: boolean; view: ControlSitesView }>(),
  {
    embedded: false,
  },
);
const emit = defineEmits<{ trigger: [siteId: string] }>();

type FilterValue = 'ALL' | SnapshotViewState;
type SiteRow = ControlSitesView['sites'][number];

const query = ref('');
const filter = ref<FilterValue>('ALL');
const rackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';

const summaryFilters = computed<
  Array<{
    count: number;
    label: string;
    tone?: 'danger' | 'running' | 'success' | 'warning';
    value: FilterValue;
  }>
>(() => [
  { count: props.view.total_sites, label: '子工厂', value: 'ALL' },
  {
    count: props.view.latest_sites,
    label: '最新',
    tone: 'success',
    value: 'LATEST',
  },
  {
    count: props.view.updating_sites,
    label: '正在更新',
    tone: 'running',
    value: 'UPDATING',
  },
  {
    count: props.view.stale_sites,
    label: '过期',
    tone: 'warning',
    value: 'STALE',
  },
  {
    count: props.view.failed_sites,
    label: '采集失败',
    tone: 'danger',
    value: 'COLLECTION_FAILED',
  },
]);

const columns: TableColumnsType<SiteRow> = [
  { dataIndex: 'display_name', key: 'site', title: '子工厂', width: 140 },
  { dataIndex: 'snapshot_state', key: 'state', title: '快照状态', width: 132 },
  { dataIndex: 'data_as_of', key: 'asOf', title: '数据截至', width: 150 },
  { key: 'new', title: '新增', width: 92 },
  { key: 'verified', title: '已同步到中控', width: 150 },
  {
    dataIndex: 'unsynced_object_versions',
    key: 'unsynced',
    title: '未同步',
    width: 98,
  },
  {
    dataIndex: 'in_transit_bytes',
    key: 'transit',
    title: '在途',
    width: 112,
  },
  { key: 'disks', title: '硬盘', width: 130 },
  { dataIndex: 'active_alerts', key: 'alerts', title: '当前告警', width: 100 },
  { key: 'action', title: '', width: 42 },
];

const rows = computed(() => {
  const needle = query.value.trim().toLowerCase();
  return props.view.sites.filter(
    (site) =>
      (filter.value === 'ALL' || site.snapshot_state === filter.value) &&
      (!needle ||
        site.site_id.toLowerCase().includes(needle) ||
        site.display_name.toLowerCase().includes(needle)),
  );
});

const collectionTarget = computed(
  () =>
    rows.value.find((site) => site.can_trigger_collection) ??
    rows.value[0] ??
    props.view.sites[0],
);

const noticeMessage = computed(() =>
  props.view.meta.retained_after_failure
    ? '采集失败保留最近完整快照；已同步到中控仅采用 B 端目标校验记录'
    : props.view.meta.status_message || '最近完整快照 · 中心目标校验口径',
);

function selectFilter(next: FilterValue) {
  filter.value = next;
}

function formatCollectionTime(value: null | string) {
  const formatted = formatTimestamp(value);
  return formatted.split(' ').at(-1) ?? formatted;
}
</script>

<template>
  <section
    aria-labelledby="control-sites-title"
    class="sites-baseline"
    :class="{ embedded }"
  >
    <div
      class="sites-workspace"
      data-baseline-key="B-02-factory-list"
      data-layout-reference="control-history"
    >
      <header class="sites-heading-row">
        <div class="sites-heading-copy">
          <h1 id="control-sites-title" aria-label="子工厂状态">子工厂状态</h1>
          <p>最近完整快照 · 中心目标校验口径</p>
        </div>

        <Alert
          class="sites-note"
          :message="noticeMessage"
          role="note"
          show-icon
          type="info"
        />

        <Button
          :aria-label="
            collectionTarget
              ? `立即获取 ${collectionTarget.display_name} 的最新快照`
              : '立即获取最新快照'
          "
          class="collect-button"
          :disabled="!collectionTarget?.can_trigger_collection"
          :title="collectionTarget?.collection_blocked_reason ?? undefined"
          type="primary"
          @click="collectionTarget && emit('trigger', collectionTarget.site_id)"
        >
          立即获取
        </Button>
      </header>

      <section class="summary-strip" aria-label="子工厂采集摘要">
        <Button
          v-for="item in summaryFilters"
          :key="item.value"
          :aria-pressed="filter === item.value"
          class="site-summary-filter"
          :class="{ active: filter === item.value }"
          type="text"
          @click="selectFilter(item.value)"
        >
          {{ item.label }}
          <strong :class="item.tone">{{ item.count }}</strong>
        </Button>
        <div class="latest-collection">
          <span>最近成功采集</span>
          <strong>{{ formatCollectionTime(view.meta.data_as_of) }}</strong>
        </div>
      </section>

      <div class="table-tools">
        <Input
          v-model:value="query"
          allow-clear
          aria-label="搜索子工厂"
          class="sites-search"
          placeholder="工厂编号"
        >
          <template #prefix><span aria-hidden="true">⌕</span></template>
        </Input>
      </div>

      <Table
        aria-label="中控子工厂状态列表"
        class="sites-table"
        :columns="columns"
        :data-source="rows"
        :pagination="false"
        row-key="site_id"
        size="middle"
      >
        <template #bodyCell="{ column, record }">
          <RouterLink
            v-if="column.key === 'site'"
            class="site-link"
            :to="`/control/sites/${record.site_id}`"
          >
            <strong>{{ record.display_name }}</strong>
            <small>{{ record.site_id }}</small>
          </RouterLink>
          <StatusChip
            v-else-if="column.key === 'state'"
            :label="snapshotLabel(record.snapshot_state)"
            :tone="snapshotTone(record.snapshot_state)"
          />
          <template v-else-if="column.key === 'asOf'">
            {{ formatTimestamp(record.data_as_of) }}
          </template>
          <template v-else-if="column.key === 'new'">
            {{ formatCount(record.source.new_object_versions) }}
          </template>
          <template v-else-if="column.key === 'verified'">
            {{ formatCount(record.central.target_verified_versions) }}
          </template>
          <template v-else-if="column.key === 'unsynced'">
            {{ formatCount(record.unsynced_object_versions) }}
          </template>
          <template v-else-if="column.key === 'transit'">
            {{ formatBytes(record.in_transit_bytes) }}
          </template>
          <strong
            v-else-if="column.key === 'disks'"
            :class="
              record.disks.state === 'READY' ? 'tone-success' : 'tone-warning'
            "
          >
            {{ formatCount(record.disks.connected) }} · {{ record.disks.label }}
          </strong>
          <strong
            v-else-if="column.key === 'alerts'"
            :class="{ 'tone-danger': record.active_alerts > 0 }"
          >
            {{ record.active_alerts }}
          </strong>
          <RouterLink
            v-else-if="column.key === 'action'"
            :aria-label="`查看 ${record.display_name} 同步详情`"
            class="detail-link"
            :to="`/control/sites/${record.site_id}`"
          >
            ›
          </RouterLink>
        </template>
        <template #emptyText>
          <span class="empty-result">
            没有符合当前筛选条件的子工厂；权威汇总值保持不变。
          </span>
        </template>
      </Table>

      <footer class="table-foot">
        <span>共 {{ view.total_sites }} 个子工厂</span>
        <span>数据截至时间随行展示</span>
      </footer>
    </div>

    <aside class="control-rack" aria-label="中控 RustFS 归档设备">
      <img :src="rackAsset" alt="" />
    </aside>
  </section>
</template>

<style scoped>
.sites-baseline {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #c8ced5;
  background:
    linear-gradient(180deg, rgb(2 7 11 / 52%), rgb(2 7 11 / 74%)),
    url('/assets/fustfs-baseline/factory-environment-v4.webp') center / cover
      no-repeat,
    #02070b;
}

.sites-baseline.embedded .control-rack {
  display: none;
}

.sites-workspace {
  position: absolute;
  inset: 36px 350px 34px 108px;
  z-index: 2;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.sites-heading-row {
  display: grid;
  flex: 0 0 58px;
  grid-template-columns: max-content minmax(0, 1fr) max-content;
  gap: 30px;
  align-items: center;
}

.sites-heading-copy {
  display: flex;
  gap: 26px;
  align-items: baseline;
  white-space: nowrap;
}

.sites-heading-copy h1 {
  margin: 0;
  font-size: 30px;
  font-weight: 400;
  line-height: 36px;
  color: #d8e0e7;
}

.sites-heading-copy p {
  margin: 0;
  font-size: 15px;
  line-height: 24px;
  color: #858f9a;
}

.sites-note,
.summary-strip,
.table-tools,
.sites-table,
.table-foot {
  position: relative;
  width: 100%;
}

.sites-note.ant-alert {
  min-width: 0;
  height: 38px;
  padding: 0 16px;
  margin: 0;
  overflow: hidden;
  background: rgb(37 48 59 / 45%);
  border: 0;
  border-radius: 6px;
}

.sites-note :deep(.ant-alert-icon) {
  margin-right: 16px;
  font-size: 18px;
  color: #9aa5af;
}

.sites-note :deep(.ant-alert-message) {
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 15px;
  color: #b3bbc3;
  white-space: nowrap;
}

.collect-button.ant-btn {
  min-width: 116px;
  height: 38px;
  color: #d8efff;
  background: rgb(4 111 181 / 50%);
  border-color: #078fd8;
  border-radius: 5px;
  box-shadow: none;
}

.collect-button.ant-btn:disabled {
  color: #697582;
  background: rgb(9 17 24 / 72%);
  border-color: #334454;
}

.summary-strip {
  display: grid;
  flex: 0 0 66px;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  border-bottom: 1px solid rgb(112 130 149 / 28%);
}

.site-summary-filter.ant-btn,
.latest-collection {
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
  white-space: nowrap;
  background: transparent;
  border: 0;
  border-radius: 0;
  box-shadow: none;
}

.summary-strip :deep(.site-summary-filter.ant-btn > span) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.site-summary-filter + .site-summary-filter::before,
.latest-collection::before {
  position: absolute;
  top: 16px;
  bottom: 17px;
  left: 0;
  width: 1px;
  content: '';
  background: rgb(112 130 149 / 34%);
}

.site-summary-filter.ant-btn.active,
.site-summary-filter.ant-btn:hover,
.site-summary-filter.ant-btn:focus {
  color: #18afff;
  background: rgb(22 143 255 / 6%);
}

.summary-strip strong {
  margin-left: 12px;
  font-size: 30px;
  font-weight: 350;
  line-height: 36px;
  color: #168fff;
}

.summary-strip .running,
.tone-running {
  color: #168fff;
}

.summary-strip .success,
.tone-success {
  color: #09d891;
}

.summary-strip .warning,
.tone-warning {
  color: #ffad18;
}

.summary-strip .danger,
.tone-danger {
  color: #ff4d61;
}

.latest-collection strong {
  margin-left: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 20px;
  line-height: 30px;
  color: #d8e0e7;
}

.table-tools {
  display: grid;
  flex: 0 0 70px;
  grid-template-columns: 220px minmax(0, 1fr);
  gap: 12px;
  align-items: center;
}

.sites-search {
  width: 220px;
  height: 38px;
  color: #c8ced5;
  background: rgb(6 13 19 / 84%) !important;
  border-color: rgb(112 130 149 / 30%) !important;
  border-radius: 5px !important;
  box-shadow: none !important;
}

.sites-search :deep(input) {
  height: 34px;
  color: #c8ced5;
  background: transparent;
}

.sites-search :deep(input::placeholder) {
  color: #89939f;
}

.sites-table {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
  background: linear-gradient(180deg, rgb(6 17 27 / 34%), rgb(3 10 16 / 68%));
  border-bottom: 1px solid rgb(91 123 155 / 30%);
}

.sites-table :deep(.ant-table),
.sites-table :deep(.ant-table-container),
.sites-table :deep(.ant-table-content),
.sites-table :deep(table) {
  background: transparent;
}

.sites-table :deep(.ant-table-thead > tr > th) {
  height: 52px;
  padding: 0 12px;
  font-size: 15px;
  font-weight: 400;
  color: #aeb7c0;
  background: rgb(21 32 42 / 74%);
  border-color: rgb(91 123 155 / 31%);
}

.sites-table :deep(.ant-table-thead > tr > th::before) {
  display: none;
}

.sites-table :deep(.ant-table-tbody > tr > td) {
  height: 57px;
  padding: 0 12px;
  font-size: 15px;
  color: #afb8c1;
  background: transparent;
  border-color: rgb(91 123 155 / 25%);
}

.sites-table :deep(.ant-table-tbody > tr:hover > td) {
  background: rgb(20 94 142 / 10%);
}

.site-link {
  display: grid;
  color: #d6dde5;
  text-decoration: none;
}

.site-link strong {
  font-weight: 450;
}

.site-link small {
  display: none;
}

.sites-table strong {
  font-weight: 450;
  white-space: nowrap;
}

.detail-link {
  display: grid;
  place-items: center;
  width: 28px;
  height: 36px;
  padding: 0;
  font-size: 30px;
  line-height: 1;
  color: #d6e0e8;
  text-decoration: none;
  background: transparent;
  border: 0;
  box-shadow: none;
}

.detail-link:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 2px;
}

.empty-result {
  color: #75818e;
}

.table-foot {
  display: flex;
  flex: 0 0 52px;
  align-items: center;
  justify-content: space-between;
  padding: 0 14px;
  color: #9ca5af;
  border-top: 1px solid rgb(112 130 149 / 20%);
}

.control-rack {
  position: absolute;
  top: 78px;
  left: -320px;
  z-index: 1;
  width: 340px;
  height: 560px;
}

.control-rack::after {
  position: absolute;
  right: -75px;
  bottom: -95px;
  left: -70px;
  height: 160px;
  content: '';
  background: radial-gradient(ellipse, rgb(31 136 219 / 29%), transparent 68%);
  filter: blur(10px);
}

.control-rack img {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  object-fit: fill;
  filter: drop-shadow(0 28px 28px rgb(0 0 0 / 64%));
  transform: scaleX(-1);
}

@media (prefers-reduced-motion: reduce) {
  .sites-baseline * {
    transition: none !important;
  }
}
</style>
