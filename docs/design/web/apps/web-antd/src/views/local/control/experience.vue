<!--
  @file experience.vue
  @description B 工厂连续场景编排器，统一承载子工厂列表、同步详情和采集快照详情。
  @route /control/sites · /control/sites/:siteId · /control/sites/:siteId/collection
  @scope 保持中心机柜和场景转场连续，数据始终来自 CONTROL 本机视图接口。
-->
<script setup lang="ts">
import type {
  ControlCollectionView,
  ControlSiteDetailView,
  ControlSitesView,
} from '#/api/local-views';

import { computed, ref, shallowRef, watch } from 'vue';
import { useRoute } from 'vue-router';

import { message } from 'ant-design-vue';

import { createCollectionJob, fustfsV1Transport } from '#/api/fustfs-v1';
import {
  getControlCollectionView,
  getControlSitesView,
  getControlSiteView,
} from '#/api/local-views';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import CollectionPanel from './collection-panel.vue';
import ControlServerTabs from './control-server-tabs.vue';
import SiteDetailPanel from './site-detail-panel.vue';
import SitesPanel from './sites-panel.vue';

type ControlScene = 'collection' | 'site' | 'sites';

withDefaults(defineProps<{ embedded?: boolean }>(), { embedded: false });

const route = useRoute();
const requestedScene = computed<ControlScene>(() => {
  if (route.path.endsWith('/collection')) return 'collection';
  if (route.params.siteId) return 'site';
  return 'sites';
});
const siteId = computed(() => String(route.params.siteId ?? ''));

const displayedScene = ref<ControlScene>(requestedScene.value);
const transitionDirection = ref<'back' | 'forward'>('forward');
const loading = ref(true);
const error = ref('');
const sites = shallowRef<ControlSitesView>();
const site = shallowRef<ControlSiteDetailView>();
const collection = shallowRef<ControlCollectionView>();
let requestVersion = 0;

const closeLabel = computed(() => '关闭中控服务器并返回入库总览');

function isI4Payload(value: unknown) {
  if (typeof value !== 'object' || value === null) return false;
  const meta = Reflect.get(value, 'meta');
  const schemaVersion =
    typeof meta === 'object' && meta !== null
      ? Reflect.get(meta, 'schema_version')
      : undefined;
  return (
    typeof meta === 'object' &&
    meta !== null &&
    (schemaVersion === '1' ||
      schemaVersion === '1.0' ||
      schemaVersion === 'i4.1')
  );
}

function getScenePayload(scene: ControlScene, targetSiteId: string) {
  if (scene === 'sites') return getControlSitesView();
  if (scene === 'site') return getControlSiteView(targetSiteId);
  return getControlCollectionView(targetSiteId);
}

async function loadRequestedScene() {
  const nextScene = requestedScene.value;
  const nextSiteId = siteId.value;
  const version = ++requestVersion;
  loading.value = true;
  error.value = '';

  try {
    const payload = await getScenePayload(nextScene, nextSiteId);

    if (version !== requestVersion) return;
    if (!isI4Payload(payload)) {
      throw new Error('The local view response has an unsupported schema version.');
    }

    if (nextScene === 'sites') sites.value = payload as ControlSitesView;
    if (nextScene === 'site') site.value = payload as ControlSiteDetailView;
    if (nextScene === 'collection') {
      collection.value = payload as ControlCollectionView;
    }

    transitionDirection.value =
      nextScene === 'sites' ||
      (nextScene === 'site' && displayedScene.value === 'collection')
        ? 'back'
        : 'forward';
    displayedScene.value = nextScene;
  } catch {
    if (version !== requestVersion) return;
    error.value =
      '本机只读视图暂不可用。系统不会使用演示数据替代，请检查 Agent 与安装角色策略。';
    displayedScene.value = nextScene;
  } finally {
    if (version === requestVersion) loading.value = false;
  }
}

function nonce(prefix: string) {
  const random = crypto.randomUUID().replaceAll('-', '');
  return `${prefix}_${random}`;
}

async function triggerCollection(targetSiteId: string) {
  try {
    const job = await createCollectionJob(fustfsV1Transport, {
      body: {
        requested_at: new Date().toISOString(),
        requested_mode: 'FULL',
        site_id: targetSiteId,
        trigger: 'ON_DEMAND',
      },
      idempotencyKey: nonce('idem'),
      requestId: nonce('req'),
    });
    message.success(`采集任务 ${job.collection_job_id} 已排队`);
    await loadRequestedScene();
  } catch {
    message.error('采集任务未创建；旧快照值保持不变');
  }
}

watch([requestedScene, siteId], loadRequestedScene, { immediate: true });
</script>

<template>
  <component
    :is="embedded ? 'div' : ProductShell"
    :class="{ 'control-experience-embedded': embedded }"
    v-bind="
      embedded
        ? {}
        : {
            closeLabel,
            closeTo: '/control',
            displayName: '中心 B · 中控',
            hideNavigation: true,
            immersive: true,
            role: 'CONTROL',
            showClose: true,
          }
    "
  >
    <ControlServerTabs v-if="!embedded" active="sites" />
    <section
      class="control-experience"
      :class="[`scene-${displayedScene}`, `transition-${transitionDirection}`]"
    >
      <Transition mode="out-in" name="control-panel">
        <ViewState
          v-if="loading && !sites && !site && !collection"
          key="loading"
          class="control-scene-panel"
          kind="loading"
          message="正在读取中心本机只读视图。"
        />
        <ViewState
          v-else-if="error"
          key="error"
          class="control-scene-panel"
          kind="error"
          :message="error"
          @retry="loadRequestedScene"
        />
        <SitesPanel
          v-else-if="displayedScene === 'sites' && sites"
          key="sites"
          class="control-scene-panel"
          embedded
          :view="sites"
          @trigger="triggerCollection"
        />
        <SiteDetailPanel
          v-else-if="displayedScene === 'site' && site"
          key="site"
          class="control-scene-panel control-site-detail-panel"
          :view="site"
        />
        <CollectionPanel
          v-else-if="displayedScene === 'collection' && collection"
          key="collection"
          class="control-scene-panel"
          embedded
          :view="collection"
        />
      </Transition>

      <aside
        v-if="!embedded"
        class="control-shared-rack"
        aria-label="中控 RustFS 归档设备"
      >
        <img alt="" src="/assets/fustfs-baseline/source-rack-cutout-v3.webp" />
      </aside>
    </section>
  </component>
</template>

<style scoped>
.control-experience-embedded {
  position: absolute;
  inset: 0;
}

.control-experience {
  position: relative;
  width: 100%;
  height: calc(100% - 58px);
  margin-top: 58px;
  overflow: hidden;
  background:
    radial-gradient(ellipse at 89% 76%, rgb(27 120 203 / 18%), transparent 17%),
    linear-gradient(135deg, #020407, #060d15 68%, #03070b);
}

.control-experience.scene-sites {
  position: absolute;
  inset: 0;
  width: auto;
  height: auto;
}

.control-experience > :deep(section:not(.control-shared-rack)) {
  position: absolute;
  inset: 0;
}

.control-shared-rack {
  position: absolute;
  top: 265px;
  right: 8px;
  z-index: 3;
  width: 280px;
  height: 400px;
  pointer-events: none;
  transition:
    opacity 240ms ease,
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
}

.control-shared-rack::before {
  position: absolute;
  right: -70px;
  bottom: -45px;
  width: 390px;
  height: 170px;
  content: '';
  background: radial-gradient(ellipse, rgb(30 125 211 / 22%), transparent 70%);
  filter: blur(8px);
}

.control-shared-rack img {
  position: relative;
  width: 100%;
  height: 100%;
  object-fit: contain;
  filter: drop-shadow(0 30px 30px rgb(0 0 0 / 65%));
  transform: scaleX(1.03);
}

.scene-sites .control-shared-rack {
  top: 78px;
  right: auto;
  left: -320px;
  width: 340px;
  height: 560px;
}

.scene-sites .control-shared-rack::before {
  right: -75px;
  bottom: -95px;
  left: -70px;
  width: auto;
  height: 160px;
}

.scene-sites .control-shared-rack img {
  object-fit: fill;
  transform: scaleX(-1);
}

.control-panel-enter-active {
  z-index: 2;
  transition:
    opacity 320ms ease,
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1),
    filter 260ms ease;
}

.control-scene-panel {
  transition:
    opacity 320ms ease,
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1),
    filter 260ms ease;
}

.control-panel-leave-active {
  z-index: 1;
  transition:
    opacity 180ms ease,
    transform 240ms cubic-bezier(0.4, 0, 1, 1),
    filter 180ms ease;
}

.transition-forward .control-panel-enter-from {
  opacity: 0;
  filter: blur(1.5px);
  transform: translateX(30px) scale(0.997);
}

.transition-forward .control-panel-leave-to {
  opacity: 0;
  filter: blur(1px);
  transform: translateX(-18px) scale(0.998);
}

.transition-back .control-panel-enter-from {
  opacity: 0;
  filter: blur(1.5px);
  transform: translateX(-24px) scale(0.997);
}

.transition-back .control-panel-leave-to {
  opacity: 0;
  filter: blur(1px);
  transform: translateX(18px) scale(0.998);
}

.control-site-detail-panel {
  z-index: 2;
  width: 1258px;
  padding: 2px 0 0;
}

.control-experience-embedded .scene-collection :deep(.collection-workspace) {
  width: 1218px;
  transform: translate(40px, -8px);
}

.control-experience-embedded .control-site-detail-panel {
  margin-left: 32px;
}

@media (prefers-reduced-motion: reduce) {
  .control-panel-enter-active,
  .control-panel-leave-active,
  .control-shared-rack {
    transition-duration: 0.01ms !important;
  }
}
</style>
