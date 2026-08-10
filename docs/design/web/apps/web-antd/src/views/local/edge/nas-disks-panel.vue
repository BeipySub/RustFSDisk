<!--
  @file nas-disks-panel.vue
  @description A-06 运输 NAS 盘位与选中盘位详情；仅对 Agent 安全发现且未注册的新盘展示现场确认初始化入口。
  @baseline A-06-transport-disk-detail / 1672×941
-->
<script setup lang="ts">
import type { LocalEventStreamStatus } from '../use-local-event-stream';

import type {
  EdgeMediaCandidate,
  EdgeMediaCandidatesView,
  EdgeRuntimeView,
  EdgeTransportDisk,
  EdgeTransportDisksView,
} from '#/api/local-views';

import { computed, ref, watch } from 'vue';

import {
  Button,
  Card,
  Modal,
  PageHeader,
  Progress,
} from 'ant-design-vue';

import {
  initializeEdgeMediaCandidate,
  initializeUnregisteredEdgeTransportDisk,
  takeOverEdgeMediaCandidate,
} from '#/api/local-views';

import { formatBytes, formatEta } from '../model';

const props = defineProps<{
  candidates?: EdgeMediaCandidatesView;
  eventStreamStatus?: LocalEventStreamStatus;
  runtime: EdgeRuntimeView;
  view: EdgeTransportDisksView;
}>();

const transportDiskAsset =
  '/assets/fustfs-baseline/transport-disk-cutout-v1.png';
const selectedSlot = ref('04');
const initializationDialogOpen = ref(false);
const initializationPending = ref(false);
const initializationFeedback = ref('');
const candidateInitializationDialogOpen = ref(false);
const candidateInitializationPending = ref(false);
const candidateInitializationFeedback = ref('');
const selectedCandidate = ref<EdgeMediaCandidate | null>(null);
const selectedCandidateId = ref('');
const candidateTakeoverPending = ref(false);
const candidateTakeoverFeedback = ref('');

const discoveredCandidates = computed(() => props.candidates?.candidates ?? []);
const hasCandidateProjection = computed(() => props.candidates !== undefined);
// A v2 response is the authoritative full-machine projection.  The legacy
// transport-slot cards are retained only for an older Agent that cannot send
// candidate data yet; rendering both would mix unrelated observations.
const displayedTrustedDisks = computed(() =>
  hasCandidateProjection.value ? [] : props.view.disks,
);
const diskSummary = computed(() => {
  if (!hasCandidateProjection.value) return props.view.summary;
  const candidates = discoveredCandidates.value;
  const failed = candidates.filter((candidate) => isFailedCandidate(candidate)).length;
  const healthy = candidates.filter((candidate) => isHealthyCandidate(candidate)).length;
  return {
    connected: candidates.length,
    failed,
    healthy,
    warning: candidates.length - healthy - failed,
  };
});

const realtimeLabel = computed(() => {
  if (props.eventStreamStatus === 'CONNECTED') return '实时同步';
  if (props.eventStreamStatus === 'RECONNECTING') return '正在重连';
  return '降级刷新';
});

const selectedDisk = computed(
  () =>
    displayedTrustedDisks.value.find((disk) => disk.slot === selectedSlot.value) ??
    displayedTrustedDisks.value[0] ??
    null,
);
const selectedCandidateDetail = computed(
  () =>
    discoveredCandidates.value.find(
      (candidate) => candidate.candidate_id === selectedCandidateId.value,
    ) ??
    discoveredCandidates.value[0] ??
    null,
);

const selectedConfirmedBytes = computed(() => {
  const disk = selectedDisk.value;
  const task = disk?.active_task;
  if (!task) {
    return null;
  }
  return task.confirmed_bytes;
});

const selectedProgress = computed(
  () => selectedDisk.value?.active_task?.progress_percent ?? selectedDisk.value?.progress_percent ?? null,
);

const selectedTask = computed(() => selectedDisk.value?.active_task ?? null);

function initializationCapabilityFor(
  disk: EdgeTransportDisk | null | undefined,
) {
  const capability = disk?.initialization;
  if (
    !disk ||
    disk.state !== 'UNINITIALIZED' ||
    capability?.capability !== 'INITIALIZE_UNREGISTERED_MEDIA' ||
    capability.requires_confirmation !== true ||
    !capability.discovery_token.trim()
  ) {
    return null;
  }
  return capability;
}

const selectedInitializationCapability = computed(() => {
  return initializationCapabilityFor(selectedDisk.value);
});

const selectedInstruction = computed(() => {
  const disk = selectedDisk.value;
  if (!disk) return '';
  if (props.runtime.next_action.media_slot === disk.slot) {
    return props.runtime.next_action.title;
  }
  if (disk.state === 'READY_TO_SWAP') return '当前盘位已可安全更换';
  if (disk.state === 'WRITING') return '任务运行中，请勿拔出硬盘';
  return '当前盘位保持只读观察';
});

watch(
  displayedTrustedDisks,
  (disks) => {
    if (disks.some((disk) => disk.slot === selectedSlot.value)) return;
    selectedSlot.value =
      disks.find((disk) => disk.smart_state === 'WARNING')?.slot ??
      disks[0]?.slot ??
      '';
  },
  { immediate: true },
);

watch(
  discoveredCandidates,
  (candidates) => {
    if (candidates.some((candidate) => candidate.candidate_id === selectedCandidateId.value)) return;
    selectedCandidateId.value = candidates[0]?.candidate_id ?? '';
  },
  { immediate: true },
);

function selectDisk(disk: EdgeTransportDisk) {
  selectedSlot.value = disk.slot;
  initializationDialogOpen.value = false;
  initializationFeedback.value = '';
}

function selectCandidate(candidate: EdgeMediaCandidate) {
  selectedCandidateId.value = candidate.candidate_id;
  candidateInitializationFeedback.value = '';
}

function openInitializationConfirmation(disk?: EdgeTransportDisk) {
  if (disk) {
    selectedSlot.value = disk.slot;
  }
  if (!selectedInitializationCapability.value) return;
  initializationFeedback.value = '';
  initializationDialogOpen.value = true;
}

async function confirmInitialization() {
  const capability = selectedInitializationCapability.value;
  if (!capability || initializationPending.value) return;

  initializationPending.value = true;
  initializationFeedback.value = '';
  try {
    await initializeUnregisteredEdgeTransportDisk(capability.discovery_token);
    initializationDialogOpen.value = false;
    initializationFeedback.value = '初始化请求已提交，等待本机 Agent 重新扫描确认。';
  } catch {
    initializationFeedback.value = '初始化请求未提交。请保持硬盘连接后重试。';
  } finally {
    initializationPending.value = false;
  }
}

function candidateInitializationCapabilityFor(candidate: EdgeMediaCandidate) {
  if (
    // Initialization is reserved for an unregistered disk, or an explicitly
    // surfaced registry identity conflict that the operator chooses to clear.
    // A normally registered disk must never be offered re-initialization.
    !['IDENTITY_MISMATCH', 'UNREGISTERED'].includes(candidate.registration_state ?? '') ||
    // Only the current untrusted, writable and unmounted candidate/session
    // pair can start a card-confirmed action. All unknown values fail closed.
    candidate.class !== 'CANDIDATE' ||
    candidate.trusted_slot !== null ||
    candidate.read_only !== false ||
    candidate.mounted_filesystems !== 0 ||
    candidate.rejection !== null ||
    !candidate.candidate_id.trim() ||
    !candidate.candidate_session_id.trim()
  ) return null;
  return {
    candidateId: candidate.candidate_id,
    candidateSessionId: candidate.candidate_session_id,
  };
}

function openCandidateInitializationConfirmation(candidate: EdgeMediaCandidate) {
  if (!candidateInitializationCapabilityFor(candidate)) return;
  selectedCandidate.value = candidate;
  candidateInitializationFeedback.value = '';
  candidateInitializationDialogOpen.value = true;
}

function candidateTakeoverCapabilityFor(candidate: EdgeMediaCandidate) {
  return candidate.class !== 'REJECTED' && candidate.mounted_filesystems !== 0
    && candidate.registration_state !== 'REGISTERED'
    && candidate.registration_state !== 'UNAVAILABLE'
    && candidate.candidate_id.trim() && candidate.candidate_session_id.trim();
}

async function takeOverCandidate(candidate: EdgeMediaCandidate) {
  if (!candidateTakeoverCapabilityFor(candidate) || candidateTakeoverPending.value) return;
  candidateTakeoverPending.value = true;
  candidateTakeoverFeedback.value = '';
  try {
    await takeOverEdgeMediaCandidate(candidate.candidate_id, candidate.candidate_session_id);
    candidateTakeoverFeedback.value = '接管请求已提交，Worker 将安全卸载桌面挂载后重新扫描。';
  } catch (error) {
    const status = (error as {
      response?: { status?: unknown };
      status?: unknown;
    })?.status ?? (error as { response?: { status?: unknown } })?.response?.status;
    candidateTakeoverFeedback.value = status === 401
      ? '接管未执行：本机管理员登录已失效。请重新登录后再确认接管。'
      : '接管失败：Worker 未能安全释放桌面挂载，未强制卸载。';
  } finally { candidateTakeoverPending.value = false; }
}

function requestCandidateTakeover(candidate: EdgeMediaCandidate) {
  if (!candidateTakeoverCapabilityFor(candidate)) return;
  Modal.confirm({
    title: '确认交由 Worker 接管',
    content: '将安全卸载桌面自动挂载；不会初始化、清理或写入硬盘。设备忙或身份变化时不会强制卸载。',
    okText: '确认接管',
    cancelText: '取消',
    onOk: () => takeOverCandidate(candidate),
  });
}

async function confirmCandidateInitialization() {
  const candidate = selectedCandidate.value;
  const capability = candidate && candidateInitializationCapabilityFor(candidate);
  if (!capability || candidateInitializationPending.value) return;
  candidateInitializationPending.value = true;
  candidateInitializationFeedback.value = '';
  try {
    await initializeEdgeMediaCandidate(
      capability.candidateId,
      capability.candidateSessionId,
    );
    candidateInitializationDialogOpen.value = false;
    selectedCandidate.value = null;
    candidateInitializationFeedback.value = '初始化请求已提交，等待本机 Agent 重新扫描并进入任务流程。';
    initializationFeedback.value = candidateInitializationFeedback.value;
  } catch {
    candidateInitializationFeedback.value =
      '初始化请求未提交。请保持硬盘连接后重新扫描并重试。';
  } finally {
    candidateInitializationPending.value = false;
  }
}

function formatCapacity(value: null | number) {
  if (value === null) return '容量未知';
  if (value >= 1_000_000_000_000 && value % 1_000_000_000_000 === 0) {
    return `${value / 1_000_000_000_000} TB`;
  }
  return formatBytes(value);
}

function serialAscii(serialHex: string) {
  if (!/^(?:[\da-fA-F]{2})+$/.test(serialHex)) return serialHex;
  const bytes = serialHex.match(/[\da-fA-F]{2}/g) ?? [];
  const decoded = String.fromCodePoint(...bytes.map((value) => Number.parseInt(value, 16)));
  return /^[ -~]+$/.test(decoded) ? decoded : serialHex;
}

function candidateNumber(index: number) {
  return String(index + 1).padStart(2, '0');
}

/** A registered, writable, unmounted candidate is already Worker-managed. */
function isHealthyCandidate(candidate: EdgeMediaCandidate) {
  return candidate.class === 'TRUSTED_SLOT'
    || (candidate.class === 'CANDIDATE'
      && candidate.registration_state === 'REGISTERED'
      && candidate.rejection === null
      && candidate.read_only === false
      && candidate.mounted_filesystems === 0);
}

function isFailedCandidate(candidate: EdgeMediaCandidate) {
  return candidate.class === 'REJECTED'
    || candidate.registration_state === 'UNAVAILABLE';
}

function candidateTone(candidate: EdgeMediaCandidate) {
  if (isFailedCandidate(candidate)) return 'danger';
  if (isHealthyCandidate(candidate)) return 'success';
  return 'warning';
}

function registrationLabel(candidate: EdgeMediaCandidate) {
  if (candidate.registration_state === 'REGISTERED') return '已注册盘';
  if (candidate.registration_state === 'IDENTITY_MISMATCH') return '注册身份不一致';
  if (candidate.registration_state === 'UNAVAILABLE') return '注册库暂不可用';
  return '非受信新盘';
}

function candidateControlStatus(candidate: EdgeMediaCandidate) {
  if (candidate.rejection) return candidate.rejection;
  if (candidate.mounted_filesystems !== 0) return '已自动挂载，等待交由 Worker 接管';
  if (candidate.registration_state === 'REGISTERED') return '已注册，已进入 Worker 自动处理';
  if (candidate.registration_state === 'IDENTITY_MISMATCH') return '注册身份不一致，等待确认初始化';
  if (candidate.registration_state === 'UNREGISTERED') return '未注册，等待确认初始化';
  return '等待安全检查完成';
}

function isRegisteredCandidate(candidate: EdgeMediaCandidate) {
  return candidate.registration_state === 'REGISTERED' && candidate.rejection === null;
}

function candidateDetailTitle(candidate: EdgeMediaCandidate) {
  return isRegisteredCandidate(candidate) ? '已注册运输盘信息' : '候选硬盘信息';
}

function candidateDetailInstruction(candidate: EdgeMediaCandidate) {
  if (candidate.rejection) return candidate.rejection;
  if (isRegisteredCandidate(candidate)) {
    return candidate.mounted_filesystems === 0
      ? '已在本机注册库登记，Worker 可自动处理。'
      : '已在本机注册库登记，但当前被桌面自动挂载；请申请交由 Worker 接管。';
  }
  return candidate.registration_detail ?? (candidate.mounted_filesystems === 0
    ? '等待管理员确认初始化或注册。'
    : '桌面已自动挂载；等待 Worker 受控接管。');
}

function candidateManagedStatus(candidate: EdgeMediaCandidate) {
  if (candidate.rejection) return candidate.rejection;
  if (isRegisteredCandidate(candidate)) {
    return candidate.mounted_filesystems === 0
      ? 'Worker 已管理，等待任务'
      : 'Worker 自动接管中';
  }
  return candidate.mounted_filesystems === 0
    ? '可进行管理员确认'
    : '已挂载，尚未交由 Worker 控制';
}

function candidateCurrentOperation(candidate: EdgeMediaCandidate) {
  if (isRegisteredCandidate(candidate)) {
    return candidate.mounted_filesystems === 0 ? 'Worker 受控待命' : '等待安全释放桌面挂载';
  }
  return candidate.trusted_slot
    ? `盘位 ${candidate.trusted_slot} 身份核验`
    : '候选盘安全检查';
}

function candidateInitializationCondition(candidate: EdgeMediaCandidate) {
  if (isRegisteredCandidate(candidate)) return '不适用（该盘已注册）';
  return candidateInitializationCapabilityFor(candidate)
    ? '满足，等待管理员确认'
    : '当前不满足';
}

function candidateSafetyRestriction(candidate: EdgeMediaCandidate) {
  if (isRegisteredCandidate(candidate) && candidate.mounted_filesystems === 0) {
    return '未挂载（桌面未占用）';
  }
  return candidate.mounted_filesystems === 0 ? '未挂载' : '已挂载，禁止初始化';
}

function formatScanTime(value: null | string) {
  if (!value) return '未知';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.valueOf())) return value;
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    second: '2-digit',
    timeZone: 'Asia/Shanghai',
  }).format(parsed);
}

function formatBytePair(confirmed: number, total: null | number) {
  const confirmedText = formatBytes(confirmed);
  if (total === null) return `${confirmedText} / 总量未知`;
  const totalText = formatBytes(total);
  const confirmedParts = confirmedText.split(' ');
  const totalParts = totalText.split(' ');
  return confirmedParts[1] === totalParts[1]
    ? `${confirmedParts[0]} / ${totalText}`
    : `${confirmedText} / ${totalText}`;
}

function formatDiskBytePair(confirmed: number, total: null | number) {
  if (total !== null && total >= 1_000_000_000_000) {
    return `${(confirmed / 1_000_000_000_000).toFixed(2)} / ${(
      total / 1_000_000_000_000
    ).toFixed(0)} TB`;
  }
  return formatBytePair(confirmed, total);
}

function confidenceLabel(value: null | string) {
  return (
    {
      HIGH: '高',
      LOW: '低',
      MEDIUM: '中',
    }[value ?? ''] ?? '未知'
  );
}

function smartLabel(disk: EdgeTransportDisk) {
  if (disk.smart_state === 'WARNING') return '注意';
  if (disk.smart_state === 'ERROR') return '异常';
  if (disk.smart_state === 'READY') return '正常';
  return '未知';
}

function formatThroughput(value: null | number) {
  return value === null ? '未知' : `${formatBytes(value)}/s`;
}

function mediaTypeLabel(disk: EdgeTransportDisk) {
  return disk.filesystem_label ?? '未识别';
}

function diskTone(disk: EdgeTransportDisk) {
  if (disk.state === 'FAILED' || disk.state === 'LOCKED') return 'danger';
  if (disk.smart_state === 'WARNING' || disk.exclusion_state === 'WARNING') {
    return 'warning';
  }
  if (disk.state === 'WRITING') return 'running';
  if (disk.state === 'READY_TO_SWAP') return 'success';
  if (disk.state === 'UNINITIALIZED' || disk.state === 'INITIALIZING') {
    return 'muted';
  }
  return 'standby';
}

function healthLabel(disk: EdgeTransportDisk) {
  if (disk.smart_state === 'WARNING') return '温度注意';
  if (disk.smart_state === 'ERROR') return 'SMART 异常';
  if (disk.exclusion_state !== 'READY') {
    return disk.exclusion_reason ?? '安全检查异常';
  }
  return disk.life_percent === null
    ? '寿命 未知'
    : `寿命 ${disk.life_percent}%`;
}

function safetyFacts(disk: EdgeTransportDisk) {
  const filesystem = disk.filesystem_label ?? '文件系统未知';
  let access = '读写未知';
  if (disk.read_only !== null) access = disk.read_only ? '只读' : '可读写';
  let usage = '占用未知';
  if (disk.in_use !== null) usage = disk.in_use ? '在用' : '未占用';
  const mediaId = disk.media_id_suffix
    ? `介质 …${disk.media_id_suffix}`
    : '未生成介质标识';
  return `${filesystem} · ${access} · ${usage} · ${mediaId}`;
}
</script>

<template>
  <section aria-label="运输 NAS 硬盘盘位" class="nas-disks-baseline">
    <PageHeader
      :back-icon="false"
      class="nas-disks-header nas-section-enter"
      title="运输盘位"
    >
      <template #tags>
        <div class="disk-summary" aria-label="运输盘汇总">
          <span class="summary-stat">
            <span>已接入</span>
            <b>{{ diskSummary.connected }}</b>
          </span>
          <span class="summary-stat">
            <span>正常</span>
            <b>{{ diskSummary.healthy }}</b>
          </span>
          <span class="summary-stat warning">
            <span>注意</span>
            <b>{{ diskSummary.warning }}</b>
          </span>
          <span class="summary-stat danger">
            <span>异常</span>
            <b>{{ diskSummary.failed }}</b>
          </span>
        </div>
      </template>
      <template #extra>
        <span class="scan-time">
          最近扫描 {{ formatScanTime(view.last_scan_at) }}
        </span>
        <span class="realtime-status" data-testid="realtime-status">{{ realtimeLabel }}</span>
      </template>
    </PageHeader>

    <div
      class="nas-disk-scroll nas-section-enter"
      data-testid="nas-disk-scroll"
    >
      <div v-if="displayedTrustedDisks.length > 0 || discoveredCandidates.length > 0" class="nas-disk-grid">
        <Card
          v-for="disk in displayedTrustedDisks"
          :key="disk.slot"
          :aria-label="`盘位 ${disk.slot}，${safetyFacts(disk)}`"
          :bordered="true"
          class="nas-disk-card"
          :class="[
            `tone-${diskTone(disk)}`,
            { 'is-selected': selectedDisk?.slot === disk.slot },
          ]"
          :data-safety-facts="safetyFacts(disk)"
          :aria-pressed="selectedDisk?.slot === disk.slot"
          role="button"
          tabindex="0"
          @click="selectDisk(disk)"
          @keydown.enter="selectDisk(disk)"
          @keydown.space.prevent="selectDisk(disk)"
        >
          <header>
            <strong>{{ disk.slot }}</strong>
            <span>SN …{{ disk.serial_suffix }}</span>
          </header>
          <p>
            {{ disk.media_label }} · {{ formatCapacity(disk.capacity_bytes) }}
          </p>
          <div class="disk-state">
            <b>{{ disk.state_label }}</b>
            <em v-if="disk.progress_percent !== null">
              {{ disk.progress_percent }}%
            </em>
            <span
              v-if="initializationCapabilityFor(disk)"
              class="new-disk-badge"
            >
              新硬盘
            </span>
            <Button
              v-if="initializationCapabilityFor(disk)"
              :aria-label="`初始化盘位 ${disk.slot} 的新硬盘`"
              class="disk-initialization-entry"
              data-testid="disk-initialization-entry"
              size="small"
              @click.stop="openInitializationConfirmation(disk)"
              @keydown.enter.stop
              @keydown.space.stop
            >
              初始化
            </Button>
          </div>
          <Progress
            v-if="disk.progress_percent !== null"
            class="disk-progress"
            :percent="disk.progress_percent"
            :show-info="false"
            stroke-linecap="butt"
            :stroke-width="6"
            trail-color="#29323b"
          />
          <div class="disk-health">
            <i aria-hidden="true"></i>
            <span>{{ healthLabel(disk) }}</span>
            <span>·</span>
            <span>
              {{
                disk.temperature_celsius === null
                  ? '温度未知'
                  : `${disk.temperature_celsius}°C`
              }}
            </span>
          </div>
        </Card>
        <Card
          v-for="(candidate, index) in discoveredCandidates"
          :key="candidate.candidate_id"
          :aria-label="`候选硬盘 ${candidateNumber(index)} · SN ${serialAscii(candidate.serial_hex)}`"
          :bordered="true"
          class="nas-disk-card nas-candidate-card"
          :class="{
            'is-selected': selectedCandidateDetail?.candidate_id === candidate.candidate_id,
            'tone-danger': candidateTone(candidate) === 'danger',
            'tone-success': candidateTone(candidate) === 'success',
            'tone-warning': candidateTone(candidate) === 'warning',
          }"
          :aria-pressed="selectedCandidateDetail?.candidate_id === candidate.candidate_id"
          role="button"
          tabindex="0"
          @click="selectCandidate(candidate)"
          @keydown.enter="selectCandidate(candidate)"
          @keydown.space.prevent="selectCandidate(candidate)"
        >
          <header>
            <strong>{{ candidateNumber(index) }}</strong>
            <span>SN {{ serialAscii(candidate.serial_hex) }}</span>
          </header>
          <p>{{ formatCapacity(candidate.capacity_bytes) }} · {{ candidate.filesystem_type ?? '文件系统未知' }}</p>
          <div class="disk-state">
            <b>{{ registrationLabel(candidate) }}</b>
          </div>
          <p class="candidate-status">
            {{ candidateControlStatus(candidate) }}
          </p>
          <div class="disk-health">
            <i aria-hidden="true"></i>
            <span>{{ candidate.read_only === false ? '可读写' : '只读或状态未知' }}</span>
            <span>·</span>
            <span>{{ candidate.mounted_filesystems === 0 ? '未挂载' : '已挂载' }}</span>
          </div>
          <Button
            v-if="candidateInitializationCapabilityFor(candidate)"
            :data-testid="`candidate-initialize-${candidate.candidate_id}`"
            class="disk-initialization-entry"
            size="small"
            @click.stop="openCandidateInitializationConfirmation(candidate)"
          >
            初始化
          </Button>
        </Card>
      </div>
      <div v-else class="nas-disks-empty">
        <strong>当前未检测到运输盘</strong>
        <span>{{ view.meta.status_message }}</span>
      </div>
    </div>

    <section v-if="selectedDisk" class="nas-disks-footer nas-section-enter">
      <article class="selected-disk-info">
        <h2>运输盘 {{ selectedDisk.slot }} 信息</h2>
        <div class="selected-disk-layout">
          <img alt="" class="transport-disk-cutout" :src="transportDiskAsset" />
          <div>
            <dl class="selected-disk-facts">
              <dt>类型</dt>
              <dd>{{ selectedDisk.media_label }}</dd>
              <dt>介质类型</dt>
              <dd>{{ mediaTypeLabel(selectedDisk) }}</dd>
              <dt>序列号</dt>
              <dd>…{{ selectedDisk.serial_suffix }}</dd>
              <dt>容量</dt>
              <dd>{{ formatCapacity(selectedDisk.capacity_bytes) }}</dd>
              <dt>剩余寿命</dt>
              <dd>
                {{
                  selectedDisk.life_percent === null
                    ? '未知'
                    : `${selectedDisk.life_percent}%`
                }}
              </dd>
              <dt>温度</dt>
              <dd :class="{ warning: diskTone(selectedDisk) === 'warning' }">
                {{
                  selectedDisk.temperature_celsius === null
                    ? '未知'
                    : `${selectedDisk.temperature_celsius}°C`
                }}
              </dd>
              <dt>SMART</dt>
              <dd :class="{ warning: diskTone(selectedDisk) === 'warning' }">
                {{ smartLabel(selectedDisk) }}
              </dd>
              <dt>最近扫描</dt>
              <dd>{{ formatScanTime(view.last_scan_at) }}</dd>
            </dl>
            <p class="selected-instruction">{{ selectedInstruction }}</p>
            <div
              v-if="selectedInitializationCapability"
              class="transport-initialization"
              data-testid="transport-initialization"
            >
              <p>
                此硬盘由本机 Agent 安全发现，尚未注册且没有初始化标识。现场确认后将由本机执行初始化并进入自动编排。
              </p>
              <Button
                aria-label="确认初始化当前运输盘"
                class="transport-initialization-button"
                type="primary"
                @click="openInitializationConfirmation()"
              >
                初始化运输盘
              </Button>
            </div>
            <p
              v-if="initializationFeedback"
              aria-live="polite"
              class="transport-initialization-feedback"
            >
              {{ initializationFeedback }}
            </p>
          </div>
        </div>
      </article>

      <article class="selected-disk-transfer">
        <h2>数据传输</h2>
        <p class="transfer-state">
          <i aria-hidden="true"></i>
          {{
            selectedDisk.state === 'WRITING'
              ? '正在参与传输'
              : selectedDisk.state_label
          }}
        </p>
        <div class="transfer-facts">
          <dl>
            <dt>关联任务</dt>
            <dd>{{ selectedTask?.transport_record_id ?? '当前无关联任务' }}</dd>
            <dt>数据位置</dt>
            <dd>盘位 {{ selectedDisk.slot }} · 运输介质</dd>
            <dt>当前操作</dt>
            <dd>{{ selectedTask?.stage ?? selectedDisk.state_label }}</dd>
          </dl>
          <dl>
            <dt>实时速度</dt>
            <dd>{{ formatThroughput(selectedTask?.throughput_bytes_per_second ?? null) }}</dd>
            <dt>任务进度</dt>
            <dd>
              {{
                selectedConfirmedBytes === null
                  ? '未知'
                  : formatDiskBytePair(
                      selectedConfirmedBytes,
                      selectedTask?.total_bytes ?? null,
                    )
              }}
            </dd>
            <dt>运输盘容量</dt>
            <dd>{{ formatCapacity(selectedDisk.capacity_bytes) }}</dd>
            <dt>预计剩余</dt>
            <dd>{{ formatEta(selectedTask?.eta_seconds ?? null) }}</dd>
            <dt>ETA 置信度</dt>
            <dd>
              {{ confidenceLabel(selectedTask?.eta_confidence ?? null) }}
            </dd>
            <dt>任务耗时</dt>
            <dd>数据源未提供</dd>
          </dl>
        </div>
        <div v-if="selectedProgress !== null" class="transfer-progress-row">
          <Progress
            class="transfer-progress"
            :percent="selectedProgress"
            :show-info="false"
            :stroke-color="{ '0%': '#087ae0', '100%': '#38c9ff' }"
            stroke-linecap="butt"
            :stroke-width="7"
            trail-color="#29323b"
          />
          <b>{{ selectedProgress }}%</b>
        </div>
      </article>
    </section>

    <section v-if="selectedCandidateDetail" class="nas-disks-footer nas-section-enter">
      <article class="selected-disk-info">
        <h2>{{ candidateDetailTitle(selectedCandidateDetail) }}</h2>
        <div class="selected-disk-layout">
          <img alt="" class="transport-disk-cutout" :src="transportDiskAsset" />
          <div>
            <dl class="selected-disk-facts">
              <dt>识别类别</dt>
              <dd>{{ registrationLabel(selectedCandidateDetail) }}</dd>
              <dt>文件系统</dt>
              <dd>{{ selectedCandidateDetail.filesystem_type ?? '未知' }}</dd>
              <dt>序列号</dt>
              <dd>{{ serialAscii(selectedCandidateDetail.serial_hex) }}</dd>
              <dt>容量</dt>
              <dd>{{ formatCapacity(selectedCandidateDetail.capacity_bytes) }}</dd>
              <dt>访问状态</dt>
              <dd>{{ selectedCandidateDetail.read_only === false ? '可读写' : '只读或未知' }}</dd>
              <dt>挂载状态</dt>
              <dd>{{ selectedCandidateDetail.mounted_filesystems === 0 ? '未挂载' : '已自动挂载' }}</dd>
              <dt>最近扫描</dt>
              <dd>{{ formatScanTime(view.last_scan_at) }}</dd>
            </dl>
            <p class="selected-instruction">
              {{ candidateDetailInstruction(selectedCandidateDetail) }}
            </p>
          </div>
        </div>
      </article>

      <article class="selected-disk-transfer">
        <h2>受控状态</h2>
        <p class="transfer-state">
          <i aria-hidden="true"></i>
          {{ candidateManagedStatus(selectedCandidateDetail) }}
        </p>
        <div class="transfer-facts">
          <dl>
            <dt>当前任务</dt>
            <dd>当前无关联任务</dd>
            <dt>当前操作</dt>
            <dd>{{ candidateCurrentOperation(selectedCandidateDetail) }}</dd>
          </dl>
          <dl>
            <dt>初始化条件</dt>
            <dd>{{ candidateInitializationCondition(selectedCandidateDetail) }}</dd>
            <dt>安全限制</dt>
            <dd>{{ candidateSafetyRestriction(selectedCandidateDetail) }}</dd>
          </dl>
        </div>
        <Button
          v-if="candidateTakeoverCapabilityFor(selectedCandidateDetail)"
          :loading="candidateTakeoverPending"
          class="transport-initialization-button"
          @click="requestCandidateTakeover(selectedCandidateDetail)"
        >
          申请交由 Worker 接管
        </Button>
        <Button
          v-if="candidateInitializationCapabilityFor(selectedCandidateDetail)"
          class="transport-initialization-button"
          type="primary"
          @click="openCandidateInitializationConfirmation(selectedCandidateDetail)"
        >
          初始化并交由 Worker 管理
        </Button>
        <p v-if="candidateTakeoverFeedback" class="transport-initialization-feedback">{{ candidateTakeoverFeedback }}</p>
      </article>
    </section>

    <Modal
      v-if="selectedInitializationCapability"
      :confirm-loading="initializationPending"
      :get-container="false"
      :open="initializationDialogOpen"
      cancel-text="取消"
      class="transport-initialization-confirm"
      :mask-closable="false"
      :ok-button-props="{ danger: true }"
      ok-text="确认初始化"
      title="确认初始化运输盘"
      @cancel="initializationDialogOpen = false"
      @ok="confirmInitialization"
    >
      <p>
        此操作仅初始化盘位 {{ selectedDisk?.slot }}、SN …{{
          selectedDisk?.serial_suffix
        }}、容量 {{ formatCapacity(selectedDisk?.capacity_bytes ?? null) }}
        的当前新硬盘。
      </p>
      <p>无需管理员登录；本机 Agent 仍会再次核验硬盘身份、连接状态和未注册状态。</p>
    </Modal>
    <Modal
      :confirm-loading="candidateInitializationPending"
      :get-container="false"
      :open="candidateInitializationDialogOpen"
      cancel-text="取消"
      :mask-closable="false"
      :ok-button-props="{ danger: true }"
      ok-text="确认初始化并交由 Worker 管理"
      title="确认初始化候选硬盘"
      @cancel="candidateInitializationDialogOpen = false"
      @ok="confirmCandidateInitialization"
    >
      <p v-if="candidateInitializationFeedback" class="transport-initialization-feedback" role="status">
        {{ candidateInitializationFeedback }}
      </p>
      <p>候选硬盘 SN {{ selectedCandidate ? serialAscii(selectedCandidate.serial_hex) : '未知' }}，容量 {{ formatCapacity(selectedCandidate?.capacity_bytes ?? null) }}。</p>
      <p>该设备来自非受信连接；本次插入会话必须由管理员确认。</p>
      <p>当前设备未挂载、可写且未被拒绝。确认后将重新验证身份，并初始化后交由 Worker 管理；状态变化时不会写入。</p>
    </Modal>
  </section>
</template>

<style scoped>
.realtime-status {
  margin-left: 12px;
  color: #75d6ff;
  font-size: 12px;
}

.nas-disks-baseline {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #d8dee6;
}

.nas-disks-header {
  position: absolute;
  top: 72px;
  left: 75px;
  z-index: 2;
  width: 1050px;
  padding: 0;
  background: transparent;
}

.nas-section-enter {
  animation: nas-disks-section-enter 260ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.nas-disk-scroll.nas-section-enter {
  animation-delay: 24ms;
}

.nas-disks-footer.nas-section-enter {
  animation-delay: 48ms;
}

@keyframes nas-disks-section-enter {
  from {
    opacity: 0;
    filter: blur(2px);
    transform: translate3d(-16px, 0, 0);
  }

  to {
    opacity: 1;
    filter: blur(0);
    transform: translate3d(0, 0, 0);
  }
}

.nas-disks-header :deep(.ant-page-header-heading) {
  display: flex;
  align-items: center;
  min-height: 32px;
}

.nas-disks-header :deep(.ant-page-header-heading-left) {
  display: flex;
  flex-wrap: nowrap;
  gap: 18px;
  align-items: center;
  min-width: 0;
}

.nas-disks-header :deep(.ant-page-header-heading-title) {
  margin-right: 0;
  font-size: 24px;
  font-weight: 500;
  line-height: 32px;
  color: #e1e7ed;
}

.nas-disks-header :deep(.ant-page-header-heading-tags) {
  margin: 0;
}

.scan-time {
  display: inline-flex;
  align-items: center;
  font-size: 14px;
  line-height: 20px;
  color: #8f99a4;
  white-space: nowrap;
}

.nas-disks-header :deep(.ant-page-header-heading-extra) {
  align-self: center;
  margin: 0;
}

.disk-summary {
  display: flex;
  align-items: center;
  height: 42px;
  font-size: 15px;
  line-height: 22px;
  color: #aab3bd;
}

.summary-stat {
  position: relative;
  display: flex;
  gap: 8px;
  align-items: center;
  height: 100%;
  padding: 0 15px;
}

.summary-stat:first-child {
  padding-left: 0;
}

.summary-stat + .summary-stat::before {
  position: absolute;
  top: 10px;
  bottom: 10px;
  left: 0;
  width: 1px;
  content: '';
  background: rgb(112 130 149 / 28%);
}

.disk-summary b {
  font-size: 24px;
  font-weight: 350;
  font-variant-numeric: tabular-nums;
  line-height: 30px;
  color: #dce3ea;
}

.disk-summary .warning,
.disk-summary .warning b {
  color: #ffb22c;
}

.disk-summary .danger,
.disk-summary .danger b {
  color: #ff535d;
}

.nas-disk-scroll {
  position: absolute;
  top: 151px;
  bottom: calc(100% - var(--fd-edge-footer-start) + 20px);
  left: 75px;
  z-index: 2;
  width: 1068px;
  height: auto;
  padding-right: 18px;
  overflow: hidden auto;
  scrollbar-color: #21b8ff #1c2c39;
  scrollbar-width: thin;
}

.nas-disk-scroll::-webkit-scrollbar {
  width: 7px;
}

.nas-disk-scroll::-webkit-scrollbar-track {
  background: #1c2c39;
  border-radius: 4px;
}

.nas-disk-scroll::-webkit-scrollbar-track-piece {
  background: #1c2c39;
}

.nas-disk-scroll::-webkit-scrollbar-thumb {
  background: #21b8ff;
  border-radius: 4px;
  box-shadow: 0 0 10px rgb(33 184 255 / 42%);
}

.nas-disk-scroll::-webkit-scrollbar-button,
.nas-disk-scroll::-webkit-scrollbar-button:single-button,
.nas-disk-scroll::-webkit-scrollbar-button:vertical,
.nas-disk-scroll::-webkit-scrollbar-button:vertical:decrement,
.nas-disk-scroll::-webkit-scrollbar-button:vertical:increment,
.nas-disk-scroll::-webkit-scrollbar-button:vertical:start:decrement,
.nas-disk-scroll::-webkit-scrollbar-button:vertical:end:increment,
.nas-disk-scroll::-webkit-scrollbar-button:start:decrement,
.nas-disk-scroll::-webkit-scrollbar-button:end:increment {
  display: none !important;
  width: 0 !important;
  height: 0 !important;
  min-height: 0 !important;
  appearance: none;
  background-color: transparent !important;
  background-image: none !important;
  border: 0 !important;
}

.nas-disk-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
}

.nas-disk-card {
  height: 150px;
  overflow: hidden;
  cursor: pointer;
  background-color: rgb(4 12 20 / 18%);
  background-image: none;
  border-color: rgb(203 232 255 / 16%);
  border-radius: 8px;
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 13%),
    inset 1px 0 0 rgb(203 232 255 / 5%),
    inset 0 -1px 0 rgb(0 0 0 / 18%),
    0 14px 24px rgb(0 0 0 / 18%);
  backdrop-filter: blur(10px) saturate(124%);
  transition:
    transform 180ms cubic-bezier(0.22, 1, 0.36, 1),
    background-color 180ms ease,
    border-color 180ms ease,
    box-shadow 180ms ease;
}

.nas-candidate-card {
  cursor: pointer;
}

.nas-candidate-card .candidate-status {
  min-height: 20px;
  margin-top: 2px;
  overflow: hidden;
  color: #bdc5cd;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nas-candidate-card .disk-initialization-entry {
  position: absolute;
  right: 16px;
  bottom: 8px;
}

.nas-disk-card.is-selected {
  background-color: rgb(4 17 27 / 22%);
  border-color: #18c3ff;
  box-shadow:
    inset 0 0 0 1px rgb(24 195 255 / 42%),
    inset 0 1px 0 rgb(255 255 255 / 13%),
    0 14px 24px rgb(0 0 0 / 18%),
    0 0 16px rgb(14 163 224 / 12%);
}

@media (hover: hover) and (pointer: fine) {
  .nas-disk-card:hover {
    background-color: rgb(7 19 30 / 24%);
    border-color: rgb(203 232 255 / 28%);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 16%),
      inset 1px 0 0 rgb(203 232 255 / 7%),
      inset 0 -1px 0 rgb(0 0 0 / 16%),
      0 18px 28px rgb(0 0 0 / 24%);
  }

  .nas-disk-card.is-selected:hover {
    border-color: #39c8ff;
    box-shadow:
      inset 0 0 0 1px rgb(57 200 255 / 48%),
      inset 0 1px 0 rgb(255 255 255 / 16%),
      0 18px 28px rgb(0 0 0 / 24%),
      0 0 20px rgb(14 163 224 / 16%);
  }
}

.nas-disk-card:active {
  transform: scale(0.99);
  transition-duration: 80ms;
}

.nas-disk-card:focus-visible {
  outline: 2px solid #18c3ff;
  outline-offset: 2px;
}

.nas-disk-card :deep(.ant-card-body) {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 10px 16px 9px;
  background: transparent;
}

.nas-disk-card header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.nas-disk-card header strong {
  font-size: 32px;
  font-weight: 300;
  font-variant-numeric: tabular-nums;
  line-height: 34px;
  color: #e7edf3;
}

.nas-disk-card header span {
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 14px;
  line-height: 20px;
  color: #a4adb7;
  white-space: nowrap;
}

.nas-disk-card p {
  margin: 2px 0 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 14px;
  line-height: 20px;
  color: #bdc5cd;
  white-space: nowrap;
}

.disk-state {
  display: flex;
  gap: 6px;
  align-items: baseline;
  min-height: 22px;
  font-size: 16px;
  line-height: 22px;
}

.disk-state b,
.disk-state em {
  font-style: normal;
  font-weight: 400;
  color: #1abaff;
}

.disk-state em {
  margin-left: 0;
}

.new-disk-badge {
  flex: 0 0 auto;
  padding: 0 6px;
  font-size: 11px;
  font-weight: 600;
  line-height: 18px;
  color: #aebdcb;
  letter-spacing: 0.04em;
  background: rgb(114 131 153 / 12%);
  border: 1px solid rgb(143 160 178 / 42%);
  border-radius: 4px;
}

.disk-initialization-entry {
  flex: 0 0 auto;
  height: 24px;
  padding: 0 8px;
  margin-left: auto;
  font-size: 12px;
  color: #9fdfff;
  background: rgb(22 143 255 / 8%);
  border-color: rgb(88 220 255 / 58%);
  border-radius: 4px;
}

.disk-initialization-entry:hover,
.disk-initialization-entry:focus-visible {
  color: #d8f4ff !important;
  background: rgb(22 143 255 / 16%) !important;
  border-color: #58dcff !important;
}

.disk-initialization-entry:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 2px;
}

.tone-success .disk-state b,
.tone-standby .disk-state b {
  color: #31d7a0;
}

.tone-muted .disk-state b {
  color: #8fa0b2;
}

.tone-warning .disk-state b,
.tone-warning .disk-state em {
  color: #ffb22c;
}

.tone-danger .disk-state b,
.tone-danger .disk-state em {
  color: #ff535d;
}

.disk-progress {
  display: block;
  margin-top: -1px;
  line-height: 8px;
}

.disk-progress :deep(.ant-progress-outer) {
  display: block;
  padding: 0;
  margin: 0;
}

.disk-progress :deep(.ant-progress-inner) {
  vertical-align: top;
  background: #29323b;
  border-radius: 2px;
}

.disk-progress :deep(.ant-progress-bg) {
  background: #18baff;
  border-radius: 2px;
  box-shadow: 0 0 8px rgb(24 186 255 / 34%);
}

.tone-warning .disk-progress :deep(.ant-progress-bg) {
  background: #ffb22c;
}

.disk-health {
  display: flex;
  gap: 7px;
  align-items: center;
  min-width: 0;
  margin-top: auto;
  font-size: 13px;
  line-height: 18px;
  color: #aeb7c0;
}

.disk-health i {
  flex: 0 0 9px;
  width: 9px;
  height: 9px;
  background: #31d7a0;
  border-radius: 50%;
  box-shadow: 0 0 9px rgb(49 215 160 / 58%);
}

.tone-warning .disk-health {
  color: #ffb22c;
}

.tone-warning .disk-health i {
  background: #ffb22c;
  box-shadow: 0 0 9px rgb(255 178 44 / 58%);
}

.tone-danger .disk-health {
  color: #ff535d;
}

.tone-danger .disk-health i {
  background: #ff535d;
  box-shadow: 0 0 9px rgb(255 83 93 / 55%);
}

.tone-muted .disk-health i {
  background: #8494a4;
  box-shadow: 0 0 8px rgb(132 148 164 / 40%);
}

.nas-disks-empty {
  display: grid;
  place-content: center;
  height: 100%;
  color: #84919d;
  text-align: center;
}

.nas-disks-empty strong {
  font-size: 20px;
  font-weight: 400;
  color: #cbd3db;
}

.nas-disks-empty span {
  margin-top: 8px;
}

.nas-disks-footer {
  position: absolute;
  inset: var(--fd-edge-footer-start) 0 0;
  display: grid;
  grid-template-columns: 46% 54%;
  background: var(--fd-edge-footer-background);
  border-top: var(--fd-detail-footer-border);
}

.nas-disks-footer > article {
  position: relative;
  min-width: 0;
  padding: 22px 48px 16px;
}

.nas-disks-footer > article + article {
  border-left: var(--fd-detail-footer-divider);
}

.nas-disks-footer h2 {
  margin: 0 0 8px;
  font-size: 18px;
  font-weight: 400;
  line-height: 24px;
  color: #22b8ff;
}

.selected-disk-layout {
  display: grid;
  grid-template-columns: 178px minmax(0, 1fr);
  gap: 24px;
  align-items: start;
}

.transport-disk-cutout {
  display: block;
  width: 150px;
  height: 184px;
  margin-top: 0;
  object-fit: contain;
}

.selected-disk-facts {
  display: grid;
  grid-template-columns: 86px minmax(0, 1fr);
  margin: 0;
  font-size: 13px;
  line-height: 19px;
}

.selected-disk-facts dt {
  color: #99a3ad;
}

.selected-disk-facts dd {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #d0d7df;
  white-space: nowrap;
}

.selected-disk-facts dd.warning {
  color: #ffb22c;
}

.selected-instruction {
  margin: 5px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 13px;
  line-height: 20px;
  color: #ffb22c;
  white-space: nowrap;
}

.transport-initialization {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-top: 8px;
}

.transport-initialization p {
  flex: 1;
  margin: 0;
  font-size: 12px;
  line-height: 17px;
  color: #aeb8c2;
}

.transport-initialization-button {
  flex: 0 0 auto;
  border-color: #168fcf;
  background: #087ec0;
  box-shadow: 0 0 12px rgb(24 186 255 / 20%);
}

.transport-initialization-feedback {
  margin: 7px 0 0;
  font-size: 12px;
  line-height: 17px;
  color: #8fc7e6;
}

.selected-disk-transfer {
  padding-left: 48px !important;
}

.transfer-state {
  display: flex;
  gap: 8px;
  align-items: center;
  margin: 0 0 12px;
  font-size: 13px;
  line-height: 20px;
  color: #31d7a0;
}

.transfer-state i {
  width: 8px;
  height: 8px;
  background: #31d7a0;
  border-radius: 50%;
  box-shadow: 0 0 9px rgb(49 215 160 / 58%);
}

.transfer-facts {
  display: grid;
  grid-template-columns: 1.16fr 0.84fr;
  gap: 32px;
}

.transfer-facts dl {
  display: grid;
  grid-template-columns: 100px minmax(0, 1fr);
  margin: 0;
  font-size: 13px;
  line-height: 26px;
}

.transfer-facts dt {
  color: #99a3ad;
  white-space: nowrap;
}

.transfer-facts dd {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #d4dbe2;
  white-space: nowrap;
}

.transfer-progress-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 58px;
  gap: 16px;
  align-items: center;
  margin-top: 6px;
}

.transfer-progress {
  width: 100%;
  min-width: 0;
}

.transfer-progress :deep(.ant-progress-outer) {
  display: block;
  padding: 0;
  margin: 0;
}

.transfer-progress :deep(.ant-progress-inner) {
  vertical-align: top;
  background: #29323b;
  border-radius: 2px;
}

.transfer-progress :deep(.ant-progress-bg) {
  border-radius: 2px;
  box-shadow: 0 0 10px rgb(19 143 255 / 40%);
}

.transfer-progress-row > b {
  font-size: 22px;
  font-weight: 350;
  line-height: 1;
  color: #e1e7ed;
  text-align: right;
}

@media (width <= 1350px) {
  .nas-disks-footer > article {
    padding-right: 28px;
    padding-left: 42px;
  }

  .selected-disk-layout {
    grid-template-columns: 180px minmax(0, 1fr);
    gap: 22px;
  }

  .transport-disk-cutout {
    width: 150px;
    height: 190px;
  }

  .transfer-facts {
    gap: 20px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .nas-section-enter {
    animation: none;
  }

  .nas-disk-card,
  .nas-disk-card:hover,
  .nas-disk-card:active,
  .nas-disk-card.is-selected {
    transform: none;
    transition: none;
  }
}
</style>
