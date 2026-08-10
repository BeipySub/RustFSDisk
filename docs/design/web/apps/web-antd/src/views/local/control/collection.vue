<!--
  @file collection.vue
  @description B 工厂采集任务与快照详情的独立数据加载容器。
  @usage 当前采集详情路由由 experience.vue 连续场景承载；本文件保留供独立复用与测试。
  @baseline B-03-collection-snapshot
-->
<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';

import { getControlCollectionView } from '#/api/local-views';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { useLocalView } from '../use-local-view';
import CollectionPanel from './collection-panel.vue';
import ControlServerTabs from './control-server-tabs.vue';

const route = useRoute();
const siteId = computed(() => String(route.params.siteId ?? ''));
const { data, error, loading, reload } = useLocalView(() =>
  getControlCollectionView(siteId.value),
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
      message="正在读取持久化采集阶段、快照校验和中心投影结果。"
    />
    <ViewState
      v-else-if="error || !data"
      kind="error"
      :message="error || '未返回采集任务视图'"
      @retry="reload"
    />
    <CollectionPanel v-else :view="data" />
  </ProductShell>
</template>
