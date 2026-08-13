<script setup lang="ts">
interface Props {
  currentPath: "/dashboard" | "/sync-records" | "/edge-sites";
  edgeIdentity: string;
}

defineProps<Props>();

const emit = defineEmits<{
  dashboard: [];
  syncRecords: [];
  edgeSites: [];
}>();
</script>

<template>
  <header class="product-header">
    <button class="brand" type="button" @click="emit('dashboard')">RustFS离线同步中心</button>
    <span class="header-rule" aria-hidden="true"></span>
    <span class="site-identity">{{ edgeIdentity }}</span>
    <nav aria-label="Center 页面导航">
      <button
        :aria-current="currentPath === '/dashboard' ? 'page' : undefined"
        :class="{ active: currentPath === '/dashboard' }"
        type="button"
        @click="emit('dashboard')"
      >
        运行首页
      </button>
      <button
        :aria-current="currentPath === '/sync-records' ? 'page' : undefined"
        :class="{ active: currentPath === '/sync-records' }"
        type="button"
        @click="emit('syncRecords')"
      >
        同步记录
      </button>
      <button
        :aria-current="currentPath === '/edge-sites' ? 'page' : undefined"
        :class="{ active: currentPath === '/edge-sites' }"
        type="button"
        @click="emit('edgeSites')"
      >
        Edge 管理
      </button>
    </nav>
  </header>
</template>
