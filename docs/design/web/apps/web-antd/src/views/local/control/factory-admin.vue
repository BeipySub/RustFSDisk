<!-- B-08-factory-admin · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import type { TableColumnsType } from 'ant-design-vue';

import { computed, ref } from 'vue';

import { Button, Input, Table, Tag } from 'ant-design-vue';

import ControlStageShell from './control-stage-shell.vue';

type FactoryState = 'CERT_WARNING' | 'NORMAL' | 'PAUSED' | 'PENDING';

interface FactoryRow {
  collection: string;
  id: string;
  latest: string;
  mtls: string;
  name: string;
  registration: string;
  state: FactoryState;
  stateLabel: string;
}

const filter = ref<'ALL' | FactoryState>('ALL');
const query = ref('');
const rows: FactoryRow[] = [
  {
    collection: '每日错峰',
    id: 'factory-a-001',
    latest: '今天 18:42',
    mtls: '有效',
    name: 'A-001',
    registration: '已注册',
    state: 'NORMAL',
    stateLabel: '正常',
  },
  {
    collection: '每日错峰',
    id: 'factory-a-002',
    latest: '今天 18:30',
    mtls: '有效',
    name: 'A-002',
    registration: '已注册',
    state: 'NORMAL',
    stateLabel: '正常',
  },
  {
    collection: '每日错峰',
    id: 'factory-a-003',
    latest: '今天 17:55',
    mtls: '有效',
    name: 'A-003',
    registration: '已注册',
    state: 'NORMAL',
    stateLabel: '正常',
  },
  {
    collection: '已暂停',
    id: 'factory-a-004',
    latest: '07-20 18:42',
    mtls: '有效',
    name: 'A-004',
    registration: '已暂停',
    state: 'PAUSED',
    stateLabel: '已暂停',
  },
  {
    collection: '尚未启用',
    id: 'factory-a-005',
    latest: '—',
    mtls: '待签发',
    name: 'A-005',
    registration: '待注册',
    state: 'PENDING',
    stateLabel: '待注册',
  },
  {
    collection: '每日错峰',
    id: 'factory-a-006',
    latest: '今天 16:10',
    mtls: '即将到期',
    name: 'A-006',
    registration: '已注册',
    state: 'CERT_WARNING',
    stateLabel: '证书异常',
  },
];
const columns: TableColumnsType<FactoryRow> = [
  { dataIndex: 'name', key: 'name', title: '工厂', width: 145 },
  { dataIndex: 'id', key: 'id', title: '站点 ID', width: 205 },
  {
    dataIndex: 'registration',
    key: 'registration',
    title: '注册状态',
    width: 160,
  },
  { dataIndex: 'mtls', key: 'mtls', title: 'mTLS', width: 150 },
  { dataIndex: 'collection', key: 'collection', title: '采集策略', width: 170 },
  { dataIndex: 'latest', key: 'latest', title: '最近采集', width: 165 },
  { dataIndex: 'stateLabel', key: 'state', title: '当前状态', width: 150 },
  { key: 'action', title: '', width: 40 },
];
const filters = [
  ['ALL', '全部'],
  ['NORMAL', '正常'],
  ['PENDING', '待注册'],
  ['PAUSED', '已暂停'],
  ['CERT_WARNING', '证书异常'],
] as const;

const displayedRows = computed(() => {
  const needle = query.value.trim().toLowerCase();
  return rows.filter(
    (row) =>
      (filter.value === 'ALL' || row.state === filter.value) &&
      (!needle ||
        row.name.toLowerCase().includes(needle) ||
        row.id.toLowerCase().includes(needle)),
  );
});

function tagColor(state: FactoryState) {
  if (state === 'NORMAL') return 'success';
  if (state === 'PENDING') return 'processing';
  if (state === 'PAUSED') return 'warning';
  return 'error';
}
</script>

<template>
  <ControlStageShell
    active-tab="factories"
    admin-enabled
    baseline-key="B-08-factory-admin"
    close-label="关闭子工厂管理并返回入库总览"
  >
    <article
      class="factory-admin-page"
      aria-labelledby="factory-admin-title"
      data-required-role="CONTROL_ADMIN"
    >
      <p class="breadcrumb">中控配置&nbsp;&nbsp;/&nbsp;&nbsp;子工厂管理</p>
      <header>
        <div>
          <h1 id="factory-admin-title">子工厂管理</h1>
          <p>登记、配对与采集策略</p>
        </div>
        <RouterLink to="/control/settings/factories/new">
          <Button ghost type="primary">新增子工厂</Button>
        </RouterLink>
      </header>

      <section class="admin-summary" aria-label="子工厂登记摘要">
        <div><span>已登记</span><strong>6</strong></div>
        <div><span>正常</span><strong class="success">4</strong></div>
        <div><span>待注册</span><strong>1</strong></div>
        <div><span>证书异常</span><strong class="danger">1</strong></div>
        <div><span>最近变更</span><b>今天 18:42</b></div>
      </section>

      <aside class="admin-note" role="note">
        <span aria-hidden="true">i</span>
        仅管理员可登记站点和更新信任；普通人员只查看运行状态。
      </aside>

      <div class="admin-tools">
        <div aria-label="按当前状态筛选" class="state-filters" role="group">
          <Button
            v-for="item in filters"
            :key="item[0]"
            :aria-pressed="filter === item[0]"
            :class="{ active: filter === item[0] }"
            @click="filter = item[0]"
          >
            {{ item[1] }}
          </Button>
        </div>
        <Input
          v-model:value="query"
          allow-clear
          aria-label="搜索工厂名称或站点 ID"
          placeholder="工厂名称 / 站点 ID"
        />
      </div>

      <Table
        class="factory-table"
        :columns="columns"
        :data-source="displayedRows"
        :pagination="false"
        row-key="id"
        size="middle"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'registration'">
            <Tag :color="tagColor(record.state)">{{ record.registration }}</Tag>
          </template>
          <template v-else-if="column.key === 'mtls'">
            <span :class="{ danger: record.state === 'CERT_WARNING' }">
              {{ record.mtls }}
            </span>
          </template>
          <template v-else-if="column.key === 'state'">
            <strong
              :class="`tone-${record.state.toLowerCase().replace('_', '-')}`"
            >
              {{ record.stateLabel }}
            </strong>
          </template>
          <template v-else-if="column.key === 'action'">
            <RouterLink
              v-if="record.state === 'PENDING'"
              :aria-label="`继续 ${record.name} 的注册与连接验证`"
              :to="`/control/settings/factories/${record.id}/registration`"
            >
              ›
            </RouterLink>
            <span v-else aria-hidden="true">›</span>
          </template>
        </template>
        <template #emptyText>没有符合当前筛选条件的子工厂。</template>
      </Table>
      <p class="admin-count">共 {{ displayedRows.length }} 个子工厂</p>
    </article>
  </ControlStageShell>
</template>

<style scoped>
.factory-admin-page {
  width: 1260px;
  padding: 24px 0 0 6px;
}

.breadcrumb {
  margin: 0 0 20px;
  color: #99a3ad;
}

.factory-admin-page > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}

.factory-admin-page > header > div {
  display: flex;
  gap: 25px;
  align-items: baseline;
}

.factory-admin-page h1 {
  margin: 0;
  font-size: 38px;
  font-weight: 500;
  color: #edf2f7;
}

.factory-admin-page header p {
  margin: 0;
  color: #9da7b1;
}

.factory-admin-page :deep(.ant-btn-primary) {
  min-width: 140px;
  height: 42px;
  color: #27baff;
  border-color: #098dd1;
}

.admin-summary {
  display: grid;
  grid-template-columns: repeat(4, 1fr) 1.3fr;
  height: 110px;
  border: 1px solid rgb(93 121 148 / 35%);
}

.admin-summary > div {
  position: relative;
  display: grid;
  gap: 6px;
  place-content: center;
  text-align: center;
}

.admin-summary > div + div::before {
  position: absolute;
  top: 27px;
  bottom: 27px;
  left: 0;
  width: 1px;
  content: '';
  background: rgb(93 121 148 / 34%);
}

.admin-summary span {
  color: #99a3ad;
}

.admin-summary strong {
  font-size: 31px;
  font-weight: 500;
  color: #1daeff;
}

.admin-summary b {
  font-size: 23px;
  font-weight: 400;
  color: #dfe5ea;
}

.success {
  color: var(--fd-success) !important;
}

.danger {
  color: var(--fd-danger) !important;
}

.admin-note {
  display: flex;
  gap: 13px;
  align-items: center;
  height: 49px;
  padding: 0 15px;
  margin-top: 12px;
  color: #aeb8c2;
  background: rgb(14 29 44 / 75%);
  border: 1px solid rgb(72 101 130 / 28%);
}

.admin-note span {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  color: #0a1119;
  background: #8995a0;
  border-radius: 50%;
}

.admin-tools {
  display: flex;
  justify-content: space-between;
  margin: 19px 0;
}

.state-filters {
  display: flex;
}

.admin-tools :deep(.ant-btn) {
  min-width: 92px;
  color: #aeb7c0;
  background: rgb(4 9 14 / 72%);
  border-color: #273849;
  border-radius: 0;
}

.admin-tools :deep(.ant-btn.active) {
  color: #20baff;
  border-color: #079ce6;
}

.admin-tools :deep(.ant-input-affix-wrapper) {
  width: 318px;
  color: #dbe3ea;
  background: rgb(4 9 14 / 82%);
  border-color: #324558;
}

.factory-table {
  width: 100%;
}

.factory-table :deep(.ant-table),
.factory-table :deep(.ant-table-cell) {
  color: #c8d0d8;
  background: rgb(2 7 12 / 68%);
}

.factory-table :deep(.ant-table-thead > tr > th) {
  height: 54px;
  color: #aeb7c0;
  background: rgb(10 24 37 / 88%);
  border-color: rgb(78 101 124 / 30%);
}

.factory-table :deep(.ant-table-tbody > tr > td) {
  height: 54px;
  border-color: rgb(78 101 124 / 30%);
}

.factory-table :deep(.ant-table-tbody > tr:hover > td) {
  background: rgb(10 28 43 / 88%);
}

.factory-table :deep(.ant-tag) {
  padding: 0;
  margin: 0;
  font-size: 15px;
  background: transparent;
  border: 0;
}

.factory-table a,
.factory-table td:last-child span {
  font-size: 28px;
  color: #d5dde4;
  text-decoration: none;
}

.tone-normal {
  color: var(--fd-success);
}

.tone-pending {
  color: #1bb9ff;
}

.tone-paused {
  color: var(--fd-warning);
}

.tone-cert-warning {
  color: var(--fd-danger);
}

.admin-count {
  margin: 17px 0 0;
  color: #9ca6b0;
}
</style>
