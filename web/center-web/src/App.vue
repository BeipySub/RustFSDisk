<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import ProductHeader from "./components/ProductHeader.vue";
import DashboardView from "./views/DashboardView.vue";
import EdgeSitesView from "./views/EdgeSitesView.vue";
import SyncRecordsView from "./views/SyncRecordsView.vue";

type EdgeRoute = "/dashboard" | "/sync-records" | "/edge-sites";
interface EdgeIdentityDetail {
  edge_name?: string;
  edge_code?: string;
}

const routes: EdgeRoute[] = ["/dashboard", "/sync-records", "/edge-sites"];
const currentPath = ref<EdgeRoute>(normalizePath(window.location.pathname));
const edgeIdentity = ref("Center 中控端");

const currentView = computed(() => {
  if (currentPath.value === "/sync-records") return SyncRecordsView;
  if (currentPath.value === "/edge-sites") return EdgeSitesView;
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

function handlePopState() {
  currentPath.value = normalizePath(window.location.pathname);
}

function handleEdgeIdentity(event: Event) {
  const detail = (event as CustomEvent<EdgeIdentityDetail>).detail;
  edgeIdentity.value = detail.edge_name || detail.edge_code || "Center 中控端";
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
      <ProductHeader
        :current-path="currentPath"
        :edge-identity="edgeIdentity"
        @dashboard="navigate('/dashboard')"
        @sync-records="navigate('/sync-records')"
        @edge-sites="navigate('/edge-sites')"
      />
      <component :is="currentView" class="page-host" />
    </div>
  </div>
</template>
