<!--
  @file server.vue
  @description A 工厂服务器状态的独立数据加载容器，负责加载态、失败态和面板装配。
  @usage 当前 /edge/server 由 runtime.vue 连续场景承载；本文件保留供独立复用与测试。
  @baseline A-02-server-status
-->
<script setup lang="ts">
import {
  getEdgeRuntimeView,
  getEdgeServerStatusView,
  getEdgeSyncRecordsView,
} from '#/api/local-views';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { useLocalView } from '../use-local-view';
import ServerPanel from './server-panel.vue';

const {
  data: server,
  error,
  loading,
  reload,
} = useLocalView(getEdgeServerStatusView);
const {
  data: records,
  error: recordsError,
  loading: recordsLoading,
  reload: reloadRecords,
} = useLocalView(getEdgeSyncRecordsView);
const { data: runtime } = useLocalView(getEdgeRuntimeView);

function reloadAll() {
  void reload();
  void reloadRecords();
}
</script>

<template>
  <ProductShell
    close-label="关闭服务器页面并返回运行首页"
    close-to="/edge"
    :display-name="runtime?.display_name ?? '工厂 A'"
    hide-navigation
    immersive
    role="EDGE"
    show-close
  >
    <ViewState
      v-if="loading || recordsLoading"
      kind="loading"
      message="正在读取服务器容量、发现状态和安全能力。"
    />
    <ViewState
      v-else-if="error || !server || recordsError || !records"
      kind="error"
      :message="error || recordsError || '未返回服务器或同步记录视图'"
      @retry="reloadAll"
    />
    <ServerPanel v-else :records="records" :view="server" />
  </ProductShell>
</template>
