<!-- A-05-settings：服务器本机只读设置视图，策略由管控中心统一下发。 -->
<script setup lang="ts">
import type {
  EdgeManagedSettingsView,
  ReadinessState,
} from '#/api/local-views';

import { Badge, Card } from 'ant-design-vue';

import { formatTimestamp } from '../model';
import ServerTabs from './server-tabs.vue';

defineProps<{ view: EdgeManagedSettingsView }>();

function stateColor(state: ReadinessState) {
  if (state === 'READY') return '#2dd591';
  if (state === 'WARNING') return '#ffb14a';
  if (state === 'UNKNOWN') return '#7e8999';
  if (state === 'PERMISSION_DENIED') return '#a66bff';
  return '#fb4e45';
}

function stateText(state: ReadinessState) {
  if (state === 'READY') return '正常';
  if (state === 'WARNING') return '注意';
  if (state === 'UNKNOWN') return '未知';
  if (state === 'PERMISSION_DENIED') return '无权限';
  return '异常';
}

function enabledText(state: ReadinessState) {
  return state === 'READY' ? '已启用' : stateText(state);
}

function effectiveText(state: ReadinessState) {
  return state === 'READY' ? '已生效' : stateText(state);
}

function verifiedText(state: ReadinessState) {
  return state === 'READY' ? '已验证' : stateText(state);
}
</script>

<template>
  <section aria-labelledby="edge-settings-title" class="edge-settings">
    <h1 id="edge-settings-title" class="screen-reader-only">服务器设置</h1>
    <ServerTabs active="settings" />

    <div class="settings-content server-section-enter">
      <header class="settings-heading">
        <div class="settings-heading-copy">
          <h2>系统托管设置</h2>
          <p class="collection-note">
            采集计划由中控管理，本机仅提供只读状态快照。
          </p>
        </div>
        <Badge
          class="settings-policy-state"
          :color="stateColor(view.policy_state)"
          :text="`站点策略${effectiveText(view.policy_state)}`"
        />
      </header>

      <section
        aria-labelledby="settings-discovery-title"
        class="settings-group"
      >
        <h3 id="settings-discovery-title">发现与扫描</h3>
        <div class="settings-grid discovery-grid">
          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>自动发现</h4>
                <p>自动定位本机 RustFS</p>
              </div>
              <Badge
                class="settings-value-badge"
                :color="stateColor(view.discovery.auto_discovery)"
                :text="enabledText(view.discovery.auto_discovery)"
              />
            </div>
          </Card>

          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>扫描范围</h4>
                <p>{{ view.discovery.scan_scope_label }}</p>
              </div>
              <span class="setting-chevron" aria-hidden="true">›</span>
            </div>
          </Card>

          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>健康扫描</h4>
                <p>{{ view.discovery.health_scan_interval_label }}</p>
              </div>
              <span class="setting-chevron" aria-hidden="true">›</span>
            </div>
          </Card>
        </div>
      </section>

      <section aria-labelledby="settings-identity-title" class="settings-group">
        <h3 id="settings-identity-title">安全与身份</h3>
        <div class="settings-grid identity-grid">
          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>站点角色</h4>
              </div>
              <strong>{{ view.identity.site_role_label }}</strong>
            </div>
          </Card>

          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>访问权限</h4>
                <p>{{ view.identity.access_label }}</p>
              </div>
              <Badge
                class="settings-value-badge"
                :color="stateColor(view.identity.access_state)"
                :text="effectiveText(view.identity.access_state)"
              />
            </div>
          </Card>

          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>策略来源</h4>
                <p>{{ view.identity.policy_source_label }}</p>
              </div>
            </div>
          </Card>
        </div>
      </section>

      <section
        aria-labelledby="settings-collection-title"
        class="settings-group settings-collection"
      >
        <h3 id="settings-collection-title">中控状态采集</h3>
        <div class="settings-grid collection-grid">
          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>采集接口</h4>
                <p>只读状态快照</p>
              </div>
              <Badge
                class="settings-value-badge"
                :color="stateColor(view.collection.endpoint_state)"
                :text="enabledText(view.collection.endpoint_state)"
              />
            </div>
          </Card>

          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>可信中控</h4>
                <p>{{ view.collection.trusted_control_label }}</p>
              </div>
              <Badge
                class="settings-value-badge"
                :color="stateColor(view.collection.trusted_control_state)"
                :text="verifiedText(view.collection.trusted_control_state)"
              />
            </div>
          </Card>

          <Card class="setting-tile" :bordered="true">
            <div class="setting-tile-content">
              <div>
                <h4>最近采集</h4>
                <p>
                  {{
                    view.collection.last_collection_at
                      ? formatTimestamp(view.collection.last_collection_at)
                      : '尚未采集'
                  }}
                </p>
              </div>
              <strong>{{ view.collection.last_snapshot_label }}</strong>
            </div>
          </Card>
        </div>
      </section>
    </div>
  </section>
</template>

<style scoped>
.edge-settings {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #d8dee5;
  isolation: isolate;
}

.edge-settings::before {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  content: '';
  background: linear-gradient(
    180deg,
    rgb(2 8 13 / 0%) 0%,
    rgb(2 8 13 / 0%) 42%,
    rgb(2 8 13 / 24%) 50%,
    rgb(2 8 13 / 74%) 59%,
    #02070b 68%,
    #02070b 100%
  );
}

.edge-settings :deep(.server-tabs) {
  z-index: 2;
}

.settings-content {
  --a05-panel: rgb(2 9 15 / 78%);
  --a05-panel-border: rgb(94 113 129 / 25%);
  --a05-primary: #e4e0dc;
  --a05-secondary: #aaa5a0;
  --a05-muted: #85888c;

  position: absolute;
  inset: 82px var(--fd-server-content-right, 55px) 34px
    var(--fd-server-content-left, 535px);
  z-index: 2;
  color: var(--a05-primary);
}

.settings-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 56px;
}

.settings-heading h2 {
  margin: 0;
  font-size: var(--fd-server-section-title-size, 30px);
  font-weight: 400;
  line-height: var(--fd-server-section-title-line-height, 36px);
  color: var(--fd-server-section-title-color, #d8e0e7);
}

.settings-heading-copy {
  display: flex;
  gap: 26px;
  align-items: baseline;
  min-width: 0;
}

.settings-policy-state {
  display: inline-flex;
  flex-direction: row-reverse;
  gap: 10px;
  align-items: center;
  padding-right: 8px;
}

.settings-policy-state :deep(.ant-badge-status-dot),
.settings-value-badge :deep(.ant-badge-status-dot) {
  width: 8px;
  height: 8px;
  box-shadow: 0 0 10px currentcolor;
}

.settings-policy-state :deep(.ant-badge-status-text) {
  margin: 0;
  font-size: 15px;
  line-height: 24px;
  color: var(--a05-secondary);
}

.settings-group h3 {
  margin: 0 0 12px;
  font-size: 21px;
  font-weight: 400;
  line-height: 28px;
  color: var(--a05-primary);
}

.settings-group + .settings-group {
  margin-top: 24px;
}

.settings-grid {
  display: grid;
  gap: 10px 14px;
}

.discovery-grid {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.discovery-grid .setting-tile:last-child {
  grid-column: 1;
}

.identity-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.collection-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.setting-tile.ant-card {
  height: 88px;
  overflow: hidden;
  color: var(--a05-primary);
  background:
    linear-gradient(105deg, rgb(7 17 25 / 50%), rgb(1 7 12 / 72%)),
    var(--a05-panel);
  border-color: var(--a05-panel-border);
  border-radius: 6px;
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 1.5%);
}

.setting-tile :deep(.ant-card-body) {
  height: 100%;
  padding: 0 22px;
}

.setting-tile-content {
  display: flex;
  gap: 18px;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  min-width: 0;
  height: 100%;
}

.setting-tile-content > div {
  min-width: 0;
}

.setting-tile h4 {
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 16px;
  font-weight: 400;
  line-height: 24px;
  color: var(--a05-primary);
  white-space: nowrap;
}

.setting-tile p {
  margin: 4px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 14px;
  line-height: 20px;
  color: var(--a05-secondary);
  white-space: nowrap;
}

.setting-tile strong {
  flex: 0 0 auto;
  max-width: 48%;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 14px;
  font-weight: 400;
  line-height: 22px;
  color: var(--a05-secondary);
  white-space: nowrap;
}

.settings-value-badge {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
}

.settings-value-badge :deep(.ant-badge-status-text) {
  margin-left: 9px;
  font-size: 14px;
  line-height: 20px;
  color: var(--a05-secondary);
}

.setting-chevron {
  flex: 0 0 auto;
  font-size: 16px;
  color: #b7bdc3;
}

.settings-collection {
  margin-top: 24px;
}

.collection-note {
  margin: 0;
  font-size: var(--fd-server-heading-note-size, 15px);
  line-height: var(--fd-server-heading-note-line-height, 24px);
  color: var(--fd-server-heading-note-color, #8d96a0);
  white-space: nowrap;
}
</style>
