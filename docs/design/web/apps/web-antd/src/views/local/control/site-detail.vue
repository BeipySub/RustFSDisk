<!--
  @file site-detail.vue
  @description B 工厂单个子工厂同步详情的独立数据加载容器。
  @usage 当前 /control/sites/:siteId 由 experience.vue 连续场景承载；本文件保留供独立复用与测试。
  @baseline B-02-factory-sync-detail
-->
<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';

import { getControlSiteView } from '#/api/local-views';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { useLocalView } from '../use-local-view';
import ControlServerTabs from './control-server-tabs.vue';
import SiteDetailPanel from './site-detail-panel.vue';

const route = useRoute();
const siteId = computed(() => String(route.params.siteId ?? ''));
const { data, error, loading, reload } = useLocalView(() =>
  getControlSiteView(siteId.value),
);
</script>

<template>
  <ProductShell
    close-label="关闭中控服务器并返回入库总览"
    close-to="/control"
    display-name="中心 B · 中控"
    hide-navigation
    immersive
    role="CONTROL"
    show-close
  >
    <ControlServerTabs active="sites" />
    <ViewState
      v-if="loading"
      kind="loading"
      message="正在分别读取源端最近完整快照与中心目标校验事实。"
    />
    <ViewState
      v-else-if="error || !data"
      kind="error"
      :message="error || '未返回子工厂详情视图'"
      @retry="reload"
    />
    <SiteDetailPanel v-else :view="data" />
  </ProductShell>
</template>
