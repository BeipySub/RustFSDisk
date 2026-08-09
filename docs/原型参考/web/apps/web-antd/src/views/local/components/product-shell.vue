<!--
  @file product-shell.vue
  @description RustFS 离线同步本机视图产品外壳，提供顶栏、角色导航、关闭入口和逻辑画布。
  @usage 包裹 EDGE 与 CONTROL 页面，并按页面需要启用沉浸式或基线画布模式。
  @scope 只管理产品级布局与导航，不读取业务数据。
-->
<script setup lang="ts">
import type { LocalRole } from '#/api/local-views';

import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute } from 'vue-router';

import { $t } from '#/locales';

defineOptions({ name: 'FustfsProductShell' });

const props = defineProps<{
  baselineCanvas?: boolean;
  closeLabel?: string;
  closeTo?: string;
  displayName: string;
  hideNavigation?: boolean;
  immersive?: boolean;
  role: LocalRole;
  showClose?: boolean;
}>();

const route = useRoute();
interface ProductLink {
  activePrefixes?: string[];
  label: string;
  path: string;
}

const edgeLinks: ProductLink[] = [
  { label: '运行首页', path: '/edge' },
  { label: '服务器', path: '/edge/server' },
];
const controlLinks: ProductLink[] = [
  {
    label: '首页／入库总览',
    path: '/control',
  },
  {
    activePrefixes: ['/control/conflicts'],
    label: '运输盘',
    path: '/control/media',
  },
  {
    activePrefixes: ['/control/settings', '/control/sites'],
    label: '中控服务器',
    path: '/control/history',
  },
];
const links = computed(() =>
  props.role === 'EDGE' ? edgeLinks : controlLinks,
);
const closeTarget = computed(
  () => props.closeTo ?? (props.role === 'EDGE' ? '/edge' : '/control'),
);
const closeTargetIsCurrent = computed(() => route.path === closeTarget.value);

const immersiveScale = ref(1);
const usesBaselineCanvas = computed(
  () => props.immersive || props.baselineCanvas,
);
const immersiveStyle = computed<Record<string, string> | undefined>(() =>
  usesBaselineCanvas.value
    ? { '--fd-immersive-scale': String(immersiveScale.value) }
    : undefined,
);

function updateImmersiveScale() {
  if (!usesBaselineCanvas.value) return;
  immersiveScale.value = Math.min(
    window.innerWidth / 1672,
    window.innerHeight / 941,
  );
}

function isLinkActive(link: ProductLink) {
  return (
    route.path === link.path ||
    link.activePrefixes?.some((prefix) => route.path.startsWith(prefix))
  );
}

onMounted(() => {
  updateImmersiveScale();
  window.addEventListener('resize', updateImmersiveScale);
});

onBeforeUnmount(() =>
  window.removeEventListener('resize', updateImmersiveScale),
);
</script>

<template>
  <div
    class="product-shell"
    :class="{
      'product-shell-baseline-canvas': usesBaselineCanvas,
      'product-shell-immersive': immersive,
      'product-shell-navigation-hidden': hideNavigation,
    }"
    :data-role="role"
    :style="immersiveStyle"
  >
    <header class="product-header">
      <RouterLink class="brand" :to="role === 'EDGE' ? '/edge' : '/control'">
        {{ $t('page.localViews.brand') }}
      </RouterLink>
      <span class="header-rule" aria-hidden="true"></span>
      <span class="site-identity">
        <i aria-hidden="true"></i>
        {{ displayName }}
        <small>{{ role === 'EDGE' ? '边缘站点' : '中控站点' }}</small>
      </span>
      <slot v-if="!hideNavigation" name="header-end">
        <nav aria-label="主导航">
          <RouterLink
            v-for="link in links"
            :key="link.path"
            class="nav-link"
            :class="{ active: isLinkActive(link) }"
            :to="link.path"
          >
            {{ link.label }}
          </RouterLink>
        </nav>
      </slot>
      <RouterLink
        v-if="showClose"
        :aria-current="closeTargetIsCurrent ? 'page' : undefined"
        :aria-label="closeLabel ?? '关闭当前页面并返回首页'"
        class="product-close"
        :class="{ current: closeTargetIsCurrent }"
        :title="
          closeTargetIsCurrent
            ? '当前已在首页'
            : (closeLabel ?? '关闭当前页面并返回首页')
        "
        :to="closeTarget"
      >
        <span aria-hidden="true">×</span>
      </RouterLink>
    </header>
    <main class="product-main">
      <slot></slot>
    </main>
  </div>
</template>

<style src="../local-view.css"></style>
