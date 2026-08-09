<!--
  @file collection-panel.vue
  @description B-03 采集任务与快照详情面板，展示七阶段进度、完整性、差异与失败原因。
  @usage 由 experience.vue 或 collection.vue 注入采集任务数据。
  @baseline B-03-collection-snapshot · 1672×941
-->
<script setup lang="ts">
import type { ControlCollectionView } from '#/api/local-views';

import { computed } from 'vue';

import { formatCount, freshnessLabel } from '../model';

const props = withDefaults(
  defineProps<{ embedded?: boolean; view: ControlCollectionView }>(),
  {
    embedded: false,
  },
);
const rackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';
const displaySiteId = computed(() =>
  props.view.site_id.replace(/^factory-/i, '').toUpperCase(),
);

const elapsedLabel = computed(() => {
  if (!props.view.completed_at) return '进行中';
  const start = Date.parse(props.view.queued_at);
  const end = Date.parse(props.view.completed_at);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) {
    return '耗时未知';
  }
  const total = Math.round((end - start) / 1000);
  return `${Math.floor(total / 60)}分${String(total % 60).padStart(2, '0')}秒`;
});

function stageLabel() {
  if (props.view.stage === 'FAILED') return '采集失败';
  if (props.view.stage === 'COMPLETED') return '已完成';
  return '进行中';
}

function formatCollectionTimestamp(value: null | string | undefined) {
  if (!value) return '未知';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) return value;
  return new Intl.DateTimeFormat('zh-CN', {
    day: '2-digit',
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    month: '2-digit',
    timeZone: 'Asia/Shanghai',
    year: 'numeric',
  })
    .format(parsed)
    .replaceAll('/', '-');
}

function formatStageTimestamp(value: null | string | undefined) {
  if (!value) return '未知';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) return value;
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    second: '2-digit',
    timeZone: 'Asia/Shanghai',
  }).format(parsed);
}
</script>

<template>
  <section
    aria-labelledby="collection-title"
    class="collection-baseline"
    data-baseline-key="B-03-collection-snapshot"
    data-view-source="frozen-baseline-fixture"
    :class="{
      embedded,
      'state-failed': view.stage === 'FAILED',
    }"
  >
    <div class="collection-workspace">
      <p class="breadcrumb">
        入库总览&nbsp;&nbsp;/&nbsp;&nbsp;子工厂&nbsp;&nbsp;/&nbsp;&nbsp;{{
          displaySiteId
        }}&nbsp;&nbsp;/&nbsp;&nbsp;采集任务
      </p>

      <header class="collection-heading">
        <div>
          <h1 id="collection-title">采集任务与快照详情</h1>
          <strong :class="{ danger: view.stage === 'FAILED' }">
            {{ stageLabel() }}
          </strong>
          <p>
            任务 {{ view.collection_job_id }} ·
            {{ view.trigger === 'SCHEDULED' ? '定时触发' : '按需触发' }} ·
            {{ formatCollectionTimestamp(view.queued_at) }}
          </p>
        </div>
        <RouterLink :to="`/control/sites/${view.site_id}`">
          返回 {{ displaySiteId }}
        </RouterLink>
      </header>

      <nav aria-label="子工厂详情页签" class="detail-tabs">
        <RouterLink :to="`/control/sites/${view.site_id}`">同步概览</RouterLink>
        <span aria-disabled="true">运输与入库</span>
        <strong>快照与采集</strong>
        <span aria-disabled="true">异常与审计</span>
      </nav>

      <section class="stage-panel" aria-labelledby="stages-title">
        <div class="section-title">
          <h2 id="stages-title">任务阶段</h2>
          <span>
            总耗时
            <b>{{ elapsedLabel }}</b>
          </span>
        </div>
        <ol>
          <li
            v-for="stage in view.stages"
            :key="stage.stage"
            :class="`state-${stage.state.toLowerCase()}`"
          >
            <strong>{{ stage.label }}</strong>
            <i aria-hidden="true"></i>
            <time v-if="stage.at">{{ formatStageTimestamp(stage.at) }}</time>
            <span v-else>尚未进入</span>
          </li>
        </ol>
      </section>

      <aside v-if="view.stage === 'FAILED'" class="failure-banner" role="alert">
        <strong>{{ view.failure_stage ?? '未知阶段' }}失败</strong>
        <span>{{ view.failure_reason ?? '未提供可行动原因' }}</span>
        <p>最近完整快照继续作为页面权威值，本次失败结果不会推进统计窗口。</p>
      </aside>

      <div class="snapshot-grid">
        <section class="snapshot-card" aria-labelledby="snapshot-scope-title">
          <div class="section-title">
            <h2 id="snapshot-scope-title">快照范围</h2>
            <b
              :class="{
                danger: view.stage === 'FAILED',
                success: view.validation?.completeness === 'COMPLETE',
              }"
            >
              {{
                view.validation
                  ? view.validation.completeness === 'COMPLETE'
                    ? '非部分快照'
                    : '部分快照'
                  : '未形成快照'
              }}
            </b>
          </div>
          <dl v-if="view.validation">
            <div>
              <dt>快照编号</dt>
              <dd>{{ view.snapshot_id ?? '未发布' }}</dd>
            </div>
            <div>
              <dt>序号</dt>
              <dd>#{{ view.snapshot_seq ?? '未知' }}</dd>
            </div>
            <div>
              <dt>完整性</dt>
              <dd class="success">
                {{
                  view.validation.completeness === 'COMPLETE'
                    ? '完整'
                    : view.validation.completeness
                }}
              </dd>
            </div>
            <div>
              <dt>扫描范围</dt>
              <dd>{{ view.validation.scope_label }}</dd>
            </div>
            <div>
              <dt>策略版本</dt>
              <dd>{{ view.validation.policy_version }}</dd>
            </div>
            <div>
              <dt>摘要校验</dt>
              <dd :class="{ success: view.validation.digest_valid }">
                {{ view.validation.digest_valid ? '通过' : '失败' }}
              </dd>
            </div>
            <div>
              <dt>站点匹配</dt>
              <dd :class="{ success: view.validation.site_match }">
                {{ view.validation.site_match ? displaySiteId : '不匹配' }}
              </dd>
            </div>
            <div>
              <dt>mTLS 校验</dt>
              <dd :class="{ success: view.validation.mtls_valid }">
                {{ view.validation.mtls_valid ? '通过' : '失败' }}
              </dd>
            </div>
          </dl>
          <p v-else class="empty">
            本次任务未形成可接受快照。最近完整序号
            <strong>#{{ view.snapshot_seq ?? '未知' }}</strong>
          </p>
        </section>

        <section class="snapshot-card" aria-labelledby="delta-title">
          <div class="section-title">
            <h2 id="delta-title">与上一次快照对比</h2>
            <span v-if="view.source_delta && view.snapshot_seq">
              上一快照 #{{ view.snapshot_seq - 1 }}
            </span>
          </div>
          <dl v-if="view.source_delta">
            <div>
              <dt>新增对象</dt>
              <dd class="success">
                +{{ formatCount(view.source_delta.new_object_versions) }}
              </dd>
            </div>
            <div>
              <dt>逻辑数据</dt>
              <dd>
                +{{ formatCount(view.source_delta.waiting_for_media_versions) }}
              </dd>
            </div>
            <div>
              <dt>待装盘</dt>
              <dd>
                +{{ formatCount(view.source_delta.waiting_for_media_versions) }}
              </dd>
            </div>
            <div>
              <dt>已装盘待运输</dt>
              <dd>
                +{{
                  formatCount(
                    view.source_delta.packed_waiting_transport_versions,
                  )
                }}
              </dd>
            </div>
            <div>
              <dt>本地失败</dt>
              <dd class="danger">
                +{{ formatCount(view.source_delta.local_failed_versions) }}
              </dd>
            </div>
            <div>
              <dt>投影重建</dt>
              <dd :class="{ success: view.validation?.projection_rebuilt }">
                {{ view.validation?.projection_rebuilt ? '完成' : '未完成' }}
              </dd>
            </div>
          </dl>
          <p v-else class="empty">初始基线或失败任务不显示虚构差异。</p>
          <p class="delta-note">差异仅用于展示，不直接计为中控完成。</p>
        </section>
      </div>

      <section class="reconcile-result" aria-labelledby="reconcile-title">
        <h2 id="reconcile-title">中心对账结果</h2>
        <div>
          <span>源端事实 <strong>已采集</strong></span>
          <span>目标校验记录 <strong>已读取</strong></span>
          <span>
            同步投影
            <strong>{{
              view.validation?.projection_rebuilt ? '已重建' : '未重建'
            }}</strong>
          </span>
        </div>
        <p>
          {{
            view.stage === 'FAILED'
              ? '失败时保留最近完整快照，并在此显示失败阶段、时间与原因。'
              : '未发现重复计数 · 未发现快照缺口'
          }}
        </p>
        <time v-if="view.next_scheduled_at">
          下次计划&nbsp;&nbsp;{{
            formatCollectionTimestamp(view.next_scheduled_at)
          }}
        </time>
        <small>{{ freshnessLabel(view.meta.freshness) }}</small>
      </section>
    </div>

    <aside class="collection-rack" aria-label="中控 RustFS 归档设备">
      <img :src="rackAsset" alt="" />
    </aside>
  </section>
</template>

<style scoped>
.collection-baseline {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background:
    radial-gradient(ellipse at 89% 76%, rgb(27 120 203 / 18%), transparent 17%),
    linear-gradient(135deg, #020407, #060d15 68%, #03070b);
}

.collection-baseline.embedded {
  background: transparent;
}

.collection-baseline.embedded .collection-rack {
  display: none;
}

.collection-workspace {
  width: 1258px;
  transform: translateY(-8px);
}

.stage-panel,
.failure-banner,
.snapshot-grid,
.reconcile-result {
  transform: translateX(-8px);
}

.breadcrumb {
  margin: 2px 0 18px 6px;
  color: #adb6c0;
}

.collection-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  height: 91px;
  padding: 0 7px;
}

.collection-heading > div {
  display: grid;
  grid-template-columns: auto auto;
  gap: 6px 24px;
  align-items: center;
}

.collection-heading h1 {
  margin: 0;
  font-size: 32px;
  font-weight: 450;
}

.collection-heading strong {
  font-size: 18px;
  font-weight: 400;
  color: var(--fd-success);
}

.collection-heading p {
  grid-column: 1 / -1;
  margin: 0;
  color: #9ca6b1;
}

.collection-heading > a {
  margin-top: 13px;
  font-size: 17px;
  color: #20b9ff;
  text-decoration: none;
}

.detail-tabs {
  display: flex;
  gap: 44px;
  align-items: center;
  height: 50px;
  margin-left: 7px;
  color: #8e98a3;
}

.detail-tabs a {
  color: #aab3bd;
  text-decoration: none;
}

.detail-tabs strong {
  display: grid;
  place-items: center;
  align-self: stretch;
  font-weight: 400;
  color: #22baff;
  border-bottom: 3px solid #18a9f4;
}

.stage-panel,
.snapshot-card,
.reconcile-result {
  background: rgb(3 9 15 / 54%);
  border: 1px solid rgb(101 122 143 / 45%);
  border-radius: 4px;
}

.stage-panel {
  height: 141px;
  padding: 14px 24px;
}

.section-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-title h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 430;
}

.section-title > span {
  color: #aab3bd;
}

.section-title > span b {
  font-weight: 400;
  color: var(--fd-success);
}

.stage-panel ol {
  display: flex;
  padding: 11px 32px 0;
  margin: 0;
  list-style: none;
}

.stage-panel li {
  position: relative;
  display: grid;
  flex: 1;
  gap: 5px;
  justify-items: center;
  color: #aab3bd;
}

.stage-panel li > strong {
  font-weight: 400;
}

.stage-panel li::before {
  position: absolute;
  top: 35px;
  right: 50%;
  left: -50%;
  height: 3px;
  content: '';
  background: #30404e;
}

.stage-panel li:first-child::before {
  display: none;
}

.stage-panel li > i {
  z-index: 1;
  width: 22px;
  height: 22px;
  background: #374451;
  border: 2px solid #6f7b87;
  border-radius: 50%;
}

.stage-panel li > i::after {
  display: block;
  color: white;
  text-align: center;
  content: '✓';
  transform: translateY(-2px);
}

.stage-panel .state-completed i {
  background: #17b86e;
  border-color: #4ce39d;
}

.stage-panel .state-completed::before {
  background: #129ee9;
}

.stage-panel .state-failed i {
  background: var(--fd-danger);
  border-color: #ff9baa;
}

.stage-panel time,
.stage-panel li > span {
  font-size: 12px;
}

.failure-banner {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 5px 20px;
  min-height: 62px;
  padding: 11px 20px;
  margin-top: 12px;
  color: #ffb5bf;
  background: rgb(255 77 97 / 8%);
  border: 1px solid rgb(255 77 97 / 35%);
}

.failure-banner p {
  grid-column: 1 / -1;
  margin: 0;
  color: #aab3bd;
}

.snapshot-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
  margin-top: 12px;
}

.snapshot-card {
  position: relative;
  height: 291px;
  padding: 14px 24px;
}

.snapshot-card .section-title {
  min-height: 29px;
  padding-bottom: 7px;
  border-bottom: 1px solid rgb(91 123 155 / 32%);
}

.snapshot-card .section-title b {
  font-weight: 400;
  color: var(--fd-success);
}

.snapshot-card dl {
  margin: 0;
}

.snapshot-card dl div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 29px;
  border-bottom: 1px solid rgb(91 123 155 / 25%);
}

dt {
  color: #929ca8;
}

dd {
  margin: 0;
  color: #e0e6ec;
}

.success {
  color: var(--fd-success) !important;
}

.danger {
  color: var(--fd-danger) !important;
}

.empty {
  display: grid;
  place-items: center;
  min-height: 170px;
  color: #85909c;
  border: 1px dashed rgb(91 123 155 / 30%);
}

.delta-note {
  position: absolute;
  right: 24px;
  bottom: 11px;
  left: 24px;
  padding-top: 8px;
  margin: 0;
  color: #909aa5;
  border-top: 1px solid rgb(91 123 155 / 24%);
}

.reconcile-result {
  position: relative;
  height: 187px;
  padding: 14px 24px;
  margin-top: 12px;
}

.reconcile-result h2 {
  margin: 0 0 10px;
  font-size: 20px;
  font-weight: 430;
}

.reconcile-result > div {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  height: 52px;
  border-top: 1px solid rgb(91 123 155 / 35%);
  border-bottom: 1px solid rgb(91 123 155 / 35%);
}

.reconcile-result > div span {
  display: grid;
  grid-template-columns: auto auto;
  gap: 12px;
  place-content: center;
  color: #bdc6d0;
}

.reconcile-result > div span + span {
  border-left: 1px solid rgb(91 123 155 / 35%);
}

.reconcile-result strong {
  font-weight: 400;
  color: var(--fd-success);
}

.reconcile-result p {
  margin: 12px 0 0;
  color: var(--fd-success);
}

.reconcile-result time {
  position: absolute;
  right: 82px;
  bottom: 16px;
  color: #20b9ff;
}

.reconcile-result small {
  position: absolute;
  top: 17px;
  right: 24px;
  color: #778390;
}

.collection-rack {
  position: absolute;
  top: 252px;
  right: 8px;
  width: 280px;
  height: 420px;
}

.collection-rack img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  filter: drop-shadow(0 30px 30px rgb(0 0 0 / 65%));
  transform: scaleX(0.9);
}

.collection-baseline.state-failed .snapshot-card {
  height: 220px;
}

.collection-baseline.state-failed .snapshot-card .empty {
  min-height: 105px;
}

.collection-baseline.state-failed .reconcile-result {
  height: 145px;
}
</style>
