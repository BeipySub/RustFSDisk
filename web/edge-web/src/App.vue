<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, provide, ref } from "vue";
import DashboardView from "./views/DashboardView.vue";
import SyncRecordsView from "./views/SyncRecordsView.vue";

type EdgeRoute = "/dashboard" | "/sync-records";
interface EdgeIdentityDetail {
  edge_name?: string;
  edge_code?: string;
}

const routes: EdgeRoute[] = ["/dashboard", "/sync-records"];
const currentPath = ref<EdgeRoute>(normalizePath(window.location.pathname));
const edgeIdentity = ref("Edge 本地节点");

const navItems: Array<{ label: string; path: EdgeRoute }> = [
  { label: "运行首页", path: "/dashboard" },
  { label: "同步记录", path: "/sync-records" },
];

const currentView = computed(() => {
  if (currentPath.value === "/sync-records") return SyncRecordsView;
  return DashboardView;
});

function normalizePath(pathname: string): EdgeRoute {
  return routes.includes(pathname as EdgeRoute) ? (pathname as EdgeRoute) : "/dashboard";
}

function navigate(path: EdgeRoute) {
  if (path === currentPath.value) return;
  window.history.pushState({}, "", path);
  currentPath.value = path;
}

provide("edgeNavigate", navigate);

function handlePopState() {
  currentPath.value = normalizePath(window.location.pathname);
}

function handleEdgeIdentity(event: Event) {
  const detail = (event as CustomEvent<EdgeIdentityDetail>).detail;
  edgeIdentity.value = detail.edge_name || detail.edge_code || "Edge 本地节点";
}

onMounted(() => {
  if (!routes.includes(window.location.pathname as EdgeRoute)) {
    window.history.replaceState({}, "", currentPath.value);
  }
  window.addEventListener("popstate", handlePopState);
  window.addEventListener("edge-dashboard:identity", handleEdgeIdentity);
});

onBeforeUnmount(() => {
  window.removeEventListener("popstate", handlePopState);
  window.removeEventListener("edge-dashboard:identity", handleEdgeIdentity);
});
</script>

<template>
  <div class="edge-app">
    <div class="edge-canvas">
      <header class="product-header">
        <button class="brand" type="button" @click="navigate('/dashboard')">RustFS离线同步中心</button>
        <span class="header-rule" aria-hidden="true"></span>
        <span class="site-identity">{{ edgeIdentity }}</span>
        <nav aria-label="Edge 页面导航">
          <button
            v-for="item in navItems"
            :key="item.path"
            :aria-current="currentPath === item.path ? 'page' : undefined"
            :class="{ active: currentPath === item.path }"
            type="button"
            @click="navigate(item.path)"
          >
            {{ item.label }}
          </button>
        </nav>
      </header>
      <component :is="currentView" class="page-host" />
    </div>
  </div>
</template>
