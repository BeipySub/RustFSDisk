<!--
  @file site-detail-panel.vue
  @description B-02 子工厂同步详情面板，分开展示 A 源端快照与 B 目标校验事实。
  @usage 由 experience.vue 或 site-detail.vue 注入单站点数据。
  @baseline B-02-factory-sync-detail · 1672×941
-->
<script setup lang="ts">
import type { ControlSiteDetailView } from '#/api/local-views';

import StatusChip from '../components/status-chip.vue';
import {
  formatBytes,
  formatCount,
  formatTimestamp,
  freshnessLabel,
  freshnessTone,
  snapshotLabel,
  snapshotTone,
} from '../model';

defineProps<{ view: ControlSiteDetailView }>();

const tabs = [
  { active: true, label: '同步概览' },
  { disabled: true, label: '运输与入库' },
  { label: '快照与采集', path: 'collection' },
  { disabled: true, label: '异常与审计' },
];
</script>

<template>
  <section aria-labelledby="site-detail-title">
    <div class="page-heading">
      <div>
        <p class="eyebrow">B-02 · SITE DETAIL</p>
        <h1 id="site-detail-title">
          {{ view.site.display_name }} · 子工厂同步详情
        </h1>
        <p>
          {{ view.site.site_id }} · 数据截至
          {{ formatTimestamp(view.site.data_as_of) }} · 快照 #{{
            view.latest_complete_snapshot_seq ?? '未知'
          }}
        </p>
      </div>
      <div class="heading-actions">
        <StatusChip
          :label="snapshotLabel(view.site.snapshot_state)"
          :tone="snapshotTone(view.site.snapshot_state)"
        />
        <StatusChip
          :label="freshnessLabel(view.meta.freshness)"
          :tone="freshnessTone(view.meta.freshness)"
        />
      </div>
    </div>

    <nav aria-label="子工厂详情页签" class="detail-tabs">
      <template v-for="tab in tabs" :key="tab.label">
        <RouterLink
          v-if="tab.path"
          :to="`/control/sites/${view.site.site_id}/${tab.path}`"
        >
          {{ tab.label }}
        </RouterLink>
        <button
          v-else
          :aria-disabled="tab.disabled || undefined"
          :class="{ active: tab.active }"
          :disabled="tab.disabled"
          type="button"
        >
          {{ tab.label }}
        </button>
      </template>
    </nav>

    <section class="headline-metrics fd-panel" aria-label="同步摘要">
      <div>
        <span>可同步对象</span>
        <strong>{{
          formatCount(
            view.site.source.new_object_versions === null
              ? null
              : view.site.source.new_object_versions +
                  view.site.central.target_verified_versions,
          )
        }}</strong>
      </div>
      <div>
        <span>已同步到中控</span>
        <strong class="success">{{
          formatCount(view.site.central.target_verified_versions)
        }}</strong>
      </div>
      <div>
        <span>未同步</span>
        <strong class="running">{{
          formatCount(view.site.unsynced_object_versions)
        }}</strong>
      </div>
      <div>
        <span>在途</span>
        <strong class="warning">{{
          formatBytes(view.site.in_transit_bytes)
        }}</strong>
      </div>
      <div>
        <span>当前告警</span>
        <strong class="danger">{{ view.site.active_alerts }}</strong>
      </div>
    </section>

    <div class="facts-grid">
      <section class="facts-card fd-panel" aria-labelledby="source-facts-title">
        <div class="card-heading">
          <div>
            <p class="eyebrow">SOURCE SNAPSHOT</p>
            <h2 id="source-facts-title">
              {{ view.site.site_id }} 源端最近完整快照
            </h2>
          </div>
          <span>{{ formatTimestamp(view.period_end_inclusive) }}</span>
        </div>
        <dl>
          <div>
            <dt>本周期新增</dt>
            <dd>{{ formatCount(view.site.source.new_object_versions) }}</dd>
          </div>
          <div>
            <dt>待装盘</dt>
            <dd>
              {{ formatCount(view.site.source.waiting_for_media_versions) }}
            </dd>
          </div>
          <div>
            <dt>已装盘待运输</dt>
            <dd>
              {{
                formatCount(view.site.source.packed_waiting_transport_versions)
              }}
            </dd>
          </div>
          <div>
            <dt>在途对象版本</dt>
            <dd>{{ formatCount(view.site.source.in_transit_versions) }}</dd>
          </div>
          <div>
            <dt>本地失败</dt>
            <dd class="danger">
              {{ formatCount(view.site.source.local_failed_versions) }}
            </dd>
          </div>
        </dl>
        <p class="period">
          统计区间：
          {{
            view.period_start_exclusive
              ? `${formatTimestamp(view.period_start_exclusive)}（不含）— ${formatTimestamp(view.period_end_inclusive)}（含）`
              : '初始基线'
          }}
          · {{ view.display_timezone }}
        </p>
      </section>

      <div class="reconcile-arrow" aria-label="源端事实与中心事实独立对账">
        <strong>{{ formatCount(view.site.unsynced_object_versions) }}</strong>
        <span>未同步</span>
        <i aria-hidden="true"></i>
      </div>

      <section
        class="facts-card fd-panel"
        aria-labelledby="central-facts-title"
      >
        <div class="card-heading">
          <div>
            <p class="eyebrow">CONTROL FACTS</p>
            <h2 id="central-facts-title">中心 B 本地目标校验事实</h2>
          </div>
          <span>不采用 A 自报完成</span>
        </div>
        <dl>
          <div>
            <dt>目标校验通过</dt>
            <dd class="success">
              {{ formatCount(view.site.central.target_verified_versions) }}
            </dd>
          </div>
          <div>
            <dt>已签发 receipt</dt>
            <dd>{{ formatCount(view.site.central.issued_receipts) }}</dd>
          </div>
          <div>
            <dt>冲突锁定</dt>
            <dd class="danger">
              {{ formatCount(view.site.central.conflict_locked_versions) }}
            </dd>
          </div>
          <div>
            <dt>入库中批次</dt>
            <dd class="warning">
              {{ formatCount(view.site.central.ingesting_batches) }}
            </dd>
          </div>
          <div>
            <dt>投影状态</dt>
            <dd class="success">可从事实重建</dd>
          </div>
        </dl>
        <p class="period">
          由中心目标校验记录生成；源端装盘与运输中不直接计为完成。
        </p>
      </section>
    </div>

    <section class="recent fd-panel" aria-labelledby="recent-title">
      <h2 id="recent-title">最近批次</h2>
      <table>
        <caption class="screen-reader-only">
          该子工厂最近运输批次
        </caption>
        <thead>
          <tr>
            <th>批次</th>
            <th>运输盘</th>
            <th>状态</th>
            <th>数据量</th>
            <th>进度 / 结果</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="batch in view.recent_batches" :key="batch.batch_id">
            <th scope="row">{{ batch.batch_id }}</th>
            <td>SN …{{ batch.media_serial_suffix }}</td>
            <td>{{ batch.state }}</td>
            <td>{{ formatBytes(batch.logical_bytes) }}</td>
            <td>{{ batch.result_label }}</td>
          </tr>
          <tr v-if="view.recent_batches.length === 0">
            <td colspan="5">当前没有中心入库批次记录。</td>
          </tr>
        </tbody>
      </table>
    </section>
  </section>
</template>

<style scoped>
.heading-actions {
  display: flex;
  gap: 8px;
}

.detail-tabs {
  display: flex;
  gap: 28px;
  margin: -6px 0 16px;
  border-bottom: 1px solid var(--fd-line);
}

.detail-tabs button,
.detail-tabs a {
  position: relative;
  min-height: 48px;
  padding: 0 10px;
  color: var(--fd-text-secondary);
  text-decoration: none;
  cursor: pointer;
  background: none;
  border: 0;
}

.detail-tabs .active,
.detail-tabs a {
  color: #29b8ff;
}

.detail-tabs .active::after {
  position: absolute;
  right: 0;
  bottom: -1px;
  left: 0;
  height: 3px;
  content: '';
  background: #159cff;
}

.detail-tabs button:disabled {
  color: #596472;
  cursor: not-allowed;
}

.headline-metrics {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  min-height: 110px;
}

.headline-metrics div {
  display: grid;
  gap: 8px;
  place-content: center;
  text-align: center;
}

.headline-metrics div + div {
  border-left: 1px solid var(--fd-line);
}

.headline-metrics span {
  color: var(--fd-text-muted);
}

.headline-metrics strong {
  font-size: 28px;
  font-weight: 520;
}

.success {
  color: var(--fd-success);
}

.running {
  color: var(--fd-running);
}

.warning {
  color: var(--fd-warning);
}

.danger {
  color: var(--fd-danger);
}

.facts-grid {
  display: grid;
  grid-template-columns: 1fr 120px 1fr;
  gap: 20px;
  align-items: center;
  margin-top: 18px;
}

.facts-card {
  min-height: 285px;
  padding: 24px 26px;
}

.card-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--fd-line);
}

.card-heading h2 {
  margin: 5px 0 0;
  font-size: 18px;
  font-weight: 500;
}

.card-heading > span {
  color: var(--fd-text-muted);
}

.facts-card dl {
  margin: 8px 0;
}

.facts-card dl div {
  display: flex;
  justify-content: space-between;
  min-height: 34px;
  padding: 7px 0;
  border-bottom: 1px solid rgb(91 123 155 / 18%);
}

dt {
  color: var(--fd-text-muted);
}

dd {
  margin: 0;
  color: var(--fd-text-primary);
}

.period {
  margin: 14px 0 0;
  font-size: 12px;
  color: var(--fd-text-muted);
}

.reconcile-arrow {
  display: grid;
  place-items: center;
  color: var(--fd-running);
}

.reconcile-arrow strong {
  font-size: 30px;
}

.reconcile-arrow span {
  color: var(--fd-text-muted);
}

.reconcile-arrow i {
  position: relative;
  width: 100%;
  height: 2px;
  margin-top: 14px;
  background: linear-gradient(90deg, transparent, #178dea);
  box-shadow: 0 0 12px #168fff;
}

.reconcile-arrow i::after {
  position: absolute;
  top: -5px;
  right: 0;
  width: 10px;
  height: 10px;
  content: '';
  border-top: 2px solid #3ac9ff;
  border-right: 2px solid #3ac9ff;
  transform: rotate(45deg);
}

.recent {
  padding: 18px 24px 0;
  margin-top: 18px;
}

.recent h2 {
  margin: 0 0 10px;
  font-size: 18px;
  font-weight: 500;
}

table {
  width: 100%;
  border-collapse: collapse;
}

th,
td {
  height: 48px;
  padding: 8px 12px;
  color: var(--fd-text-secondary);
  text-align: left;
  border-top: 1px solid var(--fd-line);
}

thead th {
  font-size: 12px;
  font-weight: 500;
  color: var(--fd-text-muted);
}

@media (max-width: 1279px) {
  .facts-grid {
    grid-template-columns: 1fr 70px 1fr;
  }
}
</style>
