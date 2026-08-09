<!--
  @file sites.vue
  @description B 工厂子工厂列表的独立数据加载容器，并负责触发按需完整快照采集。
  @usage 当前 /control/sites 由 experience.vue 连续场景承载；本文件保留供独立复用与测试。
  @baseline B-02-factory-list
-->
<script setup lang="ts">
import { message } from 'ant-design-vue';

import { createCollectionJob, fustfsV1Transport } from '#/api/fustfs-v1';
import { getControlSitesView as getSites } from '#/api/local-views';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { useLocalView } from '../use-local-view';
import ControlServerTabs from './control-server-tabs.vue';
import SitesPanel from './sites-panel.vue';

const { data, error, loading, reload } = useLocalView(getSites);

function nonce(prefix: string) {
  const random = crypto.randomUUID().replaceAll('-', '');
  return `${prefix}_${random}`;
}

async function triggerCollection(siteId: string) {
  try {
    const job = await createCollectionJob(fustfsV1Transport, {
      body: {
        requested_at: new Date().toISOString(),
        requested_mode: 'FULL',
        site_id: siteId,
        trigger: 'ON_DEMAND',
      },
      idempotencyKey: nonce('idem'),
      requestId: nonce('req'),
    });
    message.success(`采集任务 ${job.collection_job_id} 已排队`);
    await reload();
  } catch {
    message.error('采集任务未创建；旧快照值保持不变');
  }
}
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
      message="正在读取各子工厂最近完整快照与中心目标校验事实。"
    />
    <ViewState
      v-else-if="error || !data"
      kind="error"
      :message="error || '未返回子工厂视图'"
      @retry="reload"
    />
    <SitesPanel v-else :view="data" @trigger="triggerCollection" />
  </ProductShell>
</template>
