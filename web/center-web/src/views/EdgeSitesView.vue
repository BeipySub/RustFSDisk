<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import {
  createManagedEdgeSite,
  deleteManagedEdgeSite,
  fetchManagedEdgeSites,
  resetManagedEdgeKey,
  updateManagedEdgeSite,
  type EdgeStatus,
  type ManagedEdgeSite,
} from "../api/centerEdgeSites";

const edges = ref<ManagedEdgeSite[]>([]);
const page = ref(1);
const pageSize = 8;
const isLoading = ref(false);
const isSaving = ref(false);
const message = ref("正在加载边缘站点");
const errorMessage = ref("");
const editing = reactive<Record<string, { edge_name: string; edge_status: EdgeStatus }>>({});
const form = reactive({
  edge_code: "",
  edge_name: "",
  edge_status: "ACTIVE" as EdgeStatus,
});

const edgeColumns = [
  { title: "站点编码", key: "edge_code", dataIndex: "edge_code", width: 160 },
  { title: "站点名称", key: "edge_name", width: 220 },
  { title: "EDGE KEY", key: "edge_key", dataIndex: "edge_key", width: 320 },
  { title: "接入状态", key: "edge_status", width: 128 },
  { title: "对象数", key: "object_count", dataIndex: "object_count", width: 88, align: "right" as const },
  { title: "创建时间", key: "create_time", dataIndex: "create_time", width: 190 },
  { title: "操作", key: "action", width: 210, fixed: "right" as const },
];

const tablePagination = computed(() => ({
  current: page.value,
  pageSize,
  total: edges.value.length,
  size: "small" as const,
  showSizeChanger: false,
  showTotal: (count: number) => `共 ${count} 条`,
}));

onMounted(() => {
  void refreshEdges();
});

async function refreshEdges() {
  isLoading.value = true;
  errorMessage.value = "";
  try {
    edges.value = await fetchManagedEdgeSites();
    resetEditing();
    message.value = "边缘站点列表已刷新";
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : "边缘站点列表加载失败";
  } finally {
    isLoading.value = false;
  }
}

async function createEdge() {
  if (!validateCreateForm()) return;
  isSaving.value = true;
  errorMessage.value = "";
  try {
    await createManagedEdgeSite({ ...form });
    Object.assign(form, {
      edge_code: "",
      edge_name: "",
      edge_status: "ACTIVE" as EdgeStatus,
    });
    await refreshEdges();
    message.value = "边缘站点已添加，KEY 已由 Center 生成";
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : "添加边缘站点失败";
  } finally {
    isSaving.value = false;
  }
}

async function saveEdge(edge: ManagedEdgeSite) {
  const draft = editing[edge.edge_code];
  if (!draft) return;
  isSaving.value = true;
  errorMessage.value = "";
  try {
    await updateManagedEdgeSite(edge.edge_code, draft);
    await refreshEdges();
    message.value = `${edge.edge_code} 已更新`;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : "更新边缘站点失败";
  } finally {
    isSaving.value = false;
  }
}

async function removeEdge(edge: ManagedEdgeSite) {
  isSaving.value = true;
  errorMessage.value = "";
  try {
    await deleteManagedEdgeSite(edge.edge_code);
    await refreshEdges();
    message.value = `${edge.edge_code} 已删除`;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : "删除边缘站点失败";
  } finally {
    isSaving.value = false;
  }
}

async function resetEdgeKey(edge: ManagedEdgeSite) {
  isSaving.value = true;
  errorMessage.value = "";
  try {
    await resetManagedEdgeKey(edge.edge_code);
    await refreshEdges();
    message.value = `${edge.edge_code} 的 KEY 已重置`;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : "重置 KEY 失败";
  } finally {
    isSaving.value = false;
  }
}

function resetEditing() {
  for (const key of Object.keys(editing)) {
    delete editing[key];
  }
  for (const edge of edges.value) {
    editing[edge.edge_code] = {
      edge_name: edge.edge_name,
      edge_status: edge.edge_status,
    };
  }
}

function validateCreateForm() {
  const edgeCode = form.edge_code.trim();
  if (!edgeCode) {
    errorMessage.value = "需要填写边缘站点编码";
    return false;
  }
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(edgeCode)) {
    errorMessage.value = "站点编码只能使用小写字母、数字和单个连字符";
    return false;
  }
  if (!form.edge_name.trim()) {
    errorMessage.value = "需要填写边缘站点名称";
    return false;
  }
  return true;
}

function handleTableChange(pagination: { current?: number }) {
  page.value = pagination.current ?? 1;
}

async function copyEdgeKey(edge: ManagedEdgeSite) {
  if (!edge.edge_key) return;
  await navigator.clipboard.writeText(edge.edge_key);
  message.value = `${edge.edge_code} 的 KEY 已复制`;
}

function formatFullTime(value?: string): string {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}
</script>

<template>
  <main class="edge-sites-page page-host">
    <section class="edge-sites-layout">
      <form class="edge-site-form panel" @submit.prevent="createEdge">
        <div class="panel-heading">
          <div>
            <p class="section-kicker">新建</p>
            <h2>添加边缘站点</h2>
          </div>
        </div>

        <label>
          <span>站点编码</span>
          <input v-model="form.edge_code" placeholder="edge-a" autocomplete="off" />
        </label>
        <label>
          <span>站点名称</span>
          <input v-model="form.edge_name" placeholder="边缘站点 A" autocomplete="off" />
        </label>
        <label>
          <span>接入状态</span>
          <select v-model="form.edge_status">
            <option value="ACTIVE">启用</option>
            <option value="DISABLED">禁用</option>
            <option value="ERROR">异常</option>
          </select>
        </label>
        <button class="primary-action primary" type="submit" :disabled="isSaving">添加并生成 KEY</button>
      </form>

      <section class="edge-site-list panel">
        <div class="panel-heading">
          <div>
            <p class="section-kicker">站点权限</p>
            <h2>边缘站点列表</h2>
          </div>
          <p class="edge-site-message">{{ message }}</p>
        </div>

        <p v-if="errorMessage" class="edge-site-error">{{ errorMessage }}</p>

        <a-table class="edge-site-table" size="small" :columns="edgeColumns" :data-source="edges" :loading="isLoading"
          :pagination="tablePagination" row-key="edge_code" :scroll="{ x: 1240, y: 430 }" @change="handleTableChange">
          <template #emptyText>
            <span>暂无边缘站点</span>
          </template>
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'edge_code'">
              <strong>{{ record.edge_code }}</strong>
            </template>
            <template v-else-if="column.key === 'edge_name'">
              <a-input v-model:value="editing[record.edge_code].edge_name" size="small" />
            </template>
            <template v-else-if="column.key === 'edge_key'">
              <div class="edge-key-cell">
                <code>{{ record.edge_key }}</code>
                <a-button type="link" size="small" @click="copyEdgeKey(record)">复制</a-button>
              </div>
            </template>
            <template v-else-if="column.key === 'edge_status'">
              <a-select v-model:value="editing[record.edge_code].edge_status" size="small">
                <a-select-option value="ACTIVE">启用</a-select-option>
                <a-select-option value="DISABLED">禁用</a-select-option>
                <a-select-option value="ERROR">异常</a-select-option>
              </a-select>
            </template>
            <template v-else-if="column.key === 'object_count'">
              {{ record.object_count ?? 0 }}
            </template>
            <template v-else-if="column.key === 'create_time'">
              {{ formatFullTime(record.create_time) }}
            </template>
            <template v-else-if="column.key === 'action'">
              <a-space size="small">
                <a-button type="link" size="small" :disabled="isSaving" @click="saveEdge(record)">保存</a-button>
                <a-popconfirm :title="`重置 ${record.edge_code} 的 KEY？Edge 配置也要同步更新。`" ok-text="确认重置" cancel-text="取消" placement="left"
                  @confirm="resetEdgeKey(record)">
                  <a-button type="link" size="small" :disabled="isSaving">重置 KEY</a-button>
                </a-popconfirm>
                <a-popconfirm :title="`确认删除边缘站点 ${record.edge_code}？`" ok-text="确认删除" cancel-text="取消" placement="left"
                  @confirm="removeEdge(record)">
                  <a-button danger type="link" size="small" :disabled="isSaving">删除</a-button>
                </a-popconfirm>
              </a-space>
            </template>
          </template>
        </a-table>
      </section>
    </section>
  </main>
</template>

<style scoped>
.edge-key-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}

.edge-key-cell code {
  max-width: 230px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 900px) {
  .edge-sites-layout {
    grid-template-columns: 1fr;
  }
}
</style>
