<script setup lang="ts">
interface Props {
  edgeIdentity: string;
}

defineProps<Props>();

const emit = defineEmits<{
  dashboard: [];
}>();
</script>

<template>
  <header class="product-header">
    <button class="brand" type="button" @click="emit('dashboard')">RustFS离线同步中心</button>
    <span class="header-rule" aria-hidden="true"></span>
    <span class="site-identity">{{ edgeIdentity }}</span>
  </header>
</template>
