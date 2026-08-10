<!--
  @file role-gate.vue
  @description 本机视图角色入口，读取受信任安装角色并跳转至 EDGE 或 CONTROL 首页。
  @route /
  @scope 角色不可用时保持失败关闭，不展示任何示例业务数据。
-->
<script setup lang="ts">
import { watch } from 'vue';
import { useRouter } from 'vue-router';

import { getLocalViewContext } from '#/api/local-views';

import ProductShell from './components/product-shell.vue';
import ViewState from './components/view-state.vue';
import { useLocalView } from './use-local-view';

const router = useRouter();
const { data, error, loading, reload } = useLocalView(getLocalViewContext);

watch(data, async (context) => {
  if (!context) return;
  await router.replace(context.role === 'EDGE' ? '/edge' : '/control');
});
</script>

<template>
  <ProductShell display-name="正在识别本机角色" role="EDGE">
    <ViewState
      v-if="loading"
      kind="loading"
      message="正在从受信任的安装策略读取 EDGE 或 CONTROL 角色。"
    />
    <ViewState
      v-else-if="error"
      kind="error"
      :message="error"
      @retry="reload"
    />
  </ProductShell>
</template>
