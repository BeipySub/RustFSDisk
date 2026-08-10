import { baseRequestClient } from './request';

export type LocalRole = 'CONTROL' | 'EDGE';
export type ViewFreshness = 'FAILED_RETAINED' | 'FRESH' | 'STALE' | 'UPDATING';
export type ReadinessState =
  | 'ERROR'
  | 'PERMISSION_DENIED'
  | 'READY'
  | 'UNKNOWN'
  | 'WARNING';
export type RuntimeState =
  | 'COMPLETED'
  | 'IDLE'
  | 'LOADING'
  | 'NO_MEDIA'
  | 'PAUSED'
  | 'PERMISSION_DENIED'
  | 'RISK_LOCKED'
  | 'RUNNING';
export type SnapshotViewState =
  | 'COLLECTION_FAILED'
  | 'LATEST'
  | 'PARTIAL'
  | 'STALE'
  | 'UPDATING';

/** Local Agent payload versions accepted by the web client during the v1-to-i4 migration. */
export type LocalViewSchemaVersion = '1' | '1.0' | 'i4.1';

export interface ViewMeta {
  data_as_of: null | string;
  freshness: ViewFreshness;
  generated_at: string;
  retained_after_failure: boolean;
  schema_version: LocalViewSchemaVersion;
  status_message: string;
}

export interface LocalViewContext {
  display_name: string;
  meta: ViewMeta;
  permissions: string[];
  role: LocalRole;
  site_id: string;
}

export interface SourceFacts {
  in_transit_versions: number;
  local_failed_versions: number;
  new_object_versions: null | number;
  packed_waiting_transport_versions: number;
  waiting_for_media_versions: number;
}

export interface CentralFacts {
  conflict_locked_versions: number;
  ingesting_batches: number;
  issued_receipts: number;
  target_verified_versions: number;
}

export interface EdgeRuntimeView {
  current: null | {
    batch_id: null | string;
    confirmed_bytes: number;
    eta_confidence: null | string;
    eta_seconds: null | number;
    progress_percent: null | number;
    stage: string;
    total_bytes: null | number;
  };
  display_name: string;
  media: {
    completed: number;
    connected: number;
    failed: number;
    running: number;
    standby: number;
    warning: number;
  };
  meta: ViewMeta;
  next_action: {
    action_code: string;
    detail: string;
    media_slot: null | string;
    priority: 'DANGER' | 'INFO' | 'NONE' | 'WARNING';
    requires_role: LocalRole | null;
    serial_suffix: null | string;
    title: string;
  };
  /** Optional while older A Agents are being upgraded. */
  source_export?: null | {
    copied_bytes: number;
    copied_versions: number;
    not_copied_bytes: number;
    not_copied_versions: number;
  };
  site_id: string;
  state: RuntimeState;
  state_label: string;
  throughput_bytes_per_second: null | number;
}

export type EdgeTransportDiskState =
  | 'FAILED'
  | 'INITIALIZING'
  | 'LOCKED'
  | 'READY_TO_SWAP'
  | 'STANDBY'
  | 'UNINITIALIZED'
  | 'WRITING';

/**
 * The task independently assigned to one physical transport disk.
 *
 * `transport_record_id` is deliberately disk-scoped: the runtime overview may describe a
 * different disk's work while several workers are active at once.
 */
export interface EdgeTransportDiskTask {
  confirmed_bytes: number;
  eta_confidence: null | string;
  eta_seconds: null | number;
  progress_percent: null | number;
  stage: string;
  transport_record_id: string;
  throughput_bytes_per_second: null | number;
  total_bytes: null | number;
}

/**
 * An Agent-issued, short-lived permission to initialize one newly discovered
 * medium. The browser receives neither a device path nor a media identifier.
 *
 * The capability is absent for registered media and unsafe observations.
 */
export interface EdgeTransportDiskInitializationCapability {
  capability: 'INITIALIZE_UNREGISTERED_MEDIA';
  discovery_token: string;
  requires_confirmation: true;
}

export interface EdgeTransportDisk {
  /** Absent only while an older Agent projection is being rolled out. */
  active_task?: EdgeTransportDiskTask | null;
  capacity_bytes: null | number;
  exclusion_reason: null | string;
  exclusion_state: ReadinessState;
  filesystem_label: null | string;
  in_use: boolean | null;
  /**
   * Present only when the local Agent has safely discovered a candidate for
   * unregistered-media initialization.
   */
  initialization?: EdgeTransportDiskInitializationCapability | null;
  life_percent: null | number;
  media_id_suffix: null | string;
  media_label: string;
  progress_percent: null | number;
  read_only: boolean | null;
  serial_suffix: string;
  slot: string;
  smart_state: ReadinessState;
  state: EdgeTransportDiskState;
  state_label: string;
  temperature_celsius: null | number;
}

export interface EdgeTransportDisksView {
  disks: EdgeTransportDisk[];
  last_scan_at: null | string;
  meta: ViewMeta;
  site_id: string;
  summary: {
    connected: number;
    failed: number;
    healthy: number;
    warning: number;
  };
}

export type EdgeMediaCandidateClass =
  | 'CANDIDATE'
  | 'REJECTED'
  | 'TRUSTED_SLOT';

/**
 * A deliberately de-identified candidate projection.  In particular it
 * contains no device selector, mount path, full serial, or filesystem UUID.
 */
export interface EdgeMediaCandidate {
  candidate_id: string;
  /** Bound to this precise Worker observation; never derive or persist it. */
  candidate_session_id: string;
  capacity_bytes: null | number;
  class: EdgeMediaCandidateClass;
  filesystem_type: null | string;
  mounted_filesystems: null | number;
  read_only: boolean | null;
  /** Registration lookup result from A's local media registry. */
  registration_state?:
    | 'IDENTITY_MISMATCH'
    | 'REGISTERED'
    | 'UNREGISTERED'
    | 'UNAVAILABLE';
  registration_detail?: null | string;
  rejection: null | string;
  /** Full hardware serial encoded as hexadecimal; render as ASCII. */
  serial_hex: string;
  serial_suffix: string;
  trusted_slot: null | string;
}

export interface EdgeMediaCandidatesView extends EdgeTransportDisksView {
  candidates: EdgeMediaCandidate[];
  last_scan_at: null | string;
  meta: ViewMeta;
  site_id: string;
}

export type LocalEventTopic = 'media' | 'runtime' | 'task';

export interface LocalViewEvent {
  event: `${LocalEventTopic}.changed`;
  revision: number;
  topic: LocalEventTopic;
}

export interface EdgeServerStatusView {
  capabilities: Array<{
    code: string;
    detail: string;
    label: string;
    state: ReadinessState;
  }>;
  health_trend: Array<null | number>;
  last_scan_at: null | string;
  meta: ViewMeta;
  overall: ReadinessState;
  overall_label: string;
  pending_object_versions: null | number;
  site_id: string;
  storage: {
    available_bytes: null | number;
    healthy_disks: number;
    recognized_disks: number;
    total_bytes: null | number;
    unknown_disks: number;
    warning_disks: number;
  };
}

export type EdgeSyncRecordState =
  | 'CLOSED'
  | 'FAILED'
  | 'PACKED'
  | 'PARTIALLY_CLOSED'
  | 'WAITING_CENTRAL'
  | 'WAITING_RECEIPT';

export interface EdgeSyncRecord {
  batch_id: string;
  completed_at: null | string;
  destination_label: string;
  events: Array<{
    at: string;
    label: string;
    result: string;
    state: 'FAILED' | 'PASSED' | 'PENDING';
  }>;
  failure_reason: null | string;
  failure_stage: null | string;
  logical_bytes: number;
  media_serial_suffix: string;
  result_label: string;
  retry_result: null | string;
  stages: Array<{
    at: null | string;
    label: string;
    state: 'FAILED' | 'PASSED' | 'PENDING';
  }>;
  state: EdgeSyncRecordState;
}

export interface EdgeSyncRecordsView {
  meta: ViewMeta;
  records: EdgeSyncRecord[];
  site_id: string;
  summary: {
    closed: number;
    failed: number;
    packed: number;
    total: number;
    waiting_receipt: number;
  };
  transport_media_connected: boolean;
}

export interface EdgeManagedSettingsView {
  collection: {
    endpoint_state: ReadinessState;
    last_collection_at: null | string;
    last_snapshot_label: string;
    trusted_control_label: string;
    trusted_control_state: ReadinessState;
  };
  discovery: {
    auto_discovery: ReadinessState;
    health_scan_interval_label: string;
    scan_scope_label: string;
  };
  identity: {
    access_label: string;
    access_state: ReadinessState;
    policy_source_label: string;
    site_role_label: string;
  };
  meta: ViewMeta;
  policy_state: ReadinessState;
  site_id: string;
}

export interface EdgeRegistrationView {
  can_generate_identity: boolean;
  capabilities: Array<{
    detail: string;
    label: string;
    state: ReadinessState;
  }>;
  meta: ViewMeta;
  package: {
    control_label: string;
    expires_at: string;
    package_id: string;
    signature_valid: boolean;
    site_display_name: string;
    site_id: string;
    site_role: 'EDGE';
    state:
      | 'CONSUMED'
      | 'EXPIRED'
      | 'REUSED'
      | 'SITE_MISMATCH'
      | 'TAMPERED'
      | 'VALID';
  };
  phase: 'CERTIFICATE' | 'COMPLETE' | 'CONFIRM' | 'IMPORT';
  site_id: string;
  /**
   * Explicit server-side gate for the isolated V1 rehearsal.  A missing or
   * unknown value is intentionally treated as unavailable by the page.
   */
  trial_mode: 'ISOLATED_READ_ONLY' | 'UNAVAILABLE';
}

export interface ControlSiteSummary {
  active_alerts: number;
  can_trigger_collection: boolean;
  central: CentralFacts;
  collection_blocked_reason: null | string;
  data_as_of: null | string;
  disks: {
    connected: null | number;
    label: string;
    state: ReadinessState;
  };
  display_name: string;
  in_transit_bytes: number;
  site_id: string;
  snapshot_state: SnapshotViewState;
  source: SourceFacts;
  unsynced_object_versions: null | number;
}

export interface ControlSitesView {
  failed_sites: number;
  latest_sites: number;
  meta: ViewMeta;
  sites: ControlSiteSummary[];
  stale_sites: number;
  total_sites: number;
  updating_sites: number;
}

export interface ControlSiteDetailView {
  display_timezone: 'Asia/Shanghai';
  latest_complete_snapshot_id: null | string;
  latest_complete_snapshot_seq: null | number;
  meta: ViewMeta;
  period_end_inclusive: null | string;
  period_start_exclusive: null | string;
  recent_batches: Array<{
    batch_id: string;
    logical_bytes: number;
    media_serial_suffix: string;
    result_label: string;
    state: string;
  }>;
  site: ControlSiteSummary;
}

export interface ControlCollectionView {
  collection_job_id: string;
  completed_at: null | string;
  failure_reason: null | string;
  failure_stage: null | string;
  meta: ViewMeta;
  next_scheduled_at: null | string;
  queued_at: string;
  site_id: string;
  snapshot_id: null | string;
  snapshot_seq: null | number;
  source_delta: null | SourceFacts;
  stage: string;
  stages: Array<{
    at: null | string;
    label: string;
    stage: string;
    state: string;
  }>;
  trigger: 'ON_DEMAND' | 'SCHEDULED';
  validation: null | {
    completeness: string;
    digest_valid: boolean;
    mtls_valid: boolean;
    policy_version: string;
    projection_rebuilt: boolean;
    scope_label: string;
    site_match: boolean;
  };
}

/**
 * CONTROL-side import projections. These are read-only operational views:
 * the browser never receives a mount path, manifest payload, or key material.
 */
export type ControlIngestTaskState =
  | 'COMMITTED'
  | 'CONFLICT'
  | 'FAILED'
  | 'IMPORTING'
  | 'QUEUED'
  | 'VERIFYING';

export interface ControlIngestTask {
  /** Canonical task identifier for new CONTROL projections. */
  transport_record_id?: string;
  batch_id: string;
  completed_at: null | string;
  failure_reason: null | string;
  logical_bytes: number;
  /** Stable physical medium identity; never expose a mount path to the browser. */
  media_id?: string;
  media_label: string;
  media_serial_suffix: string;
  object_count: number;
  progress_percent: number;
  receipt_id: null | string;
  result_label: string;
  source_site_id: string;
  started_at: string;
  stage_label: string;
  state: ControlIngestTaskState;
  updated_at: string;
  verified_bytes: number;
}

export interface ControlIngestOverviewView {
  meta: ViewMeta;
  site_id: string;
  /**
   * B-side archive storage capacity, collected locally by CONTROL.
   *
   * This remains optional while older CONTROL deployments are rolling out the
   * collector. A missing or incomplete value must be rendered as unavailable,
   * never substituted with a visual-review fixture.
   */
  storage?: {
    available_bytes: null | number;
    reported_at: null | string;
    total_bytes: null | number;
  };
  summary: {
    connected_media: number;
    conflict_locked: number;
    failed: number;
    importing: number;
    queued: number;
    source_sites: number;
    verified: number;
  };
  tasks: ControlIngestTask[];
}

export interface ControlIngestRecordsView extends ControlIngestOverviewView {}

/**
 * Transitional adapter for the deployed V1 projection.  New Agents publish
 * `transport_record_id`; the legacy field is read only until that rollout is
 * complete.  UI code must not label the legacy value as a batch.
 */
export function getTransportRecordId(
  task: Pick<ControlIngestTask, 'batch_id' | 'transport_record_id'>,
) {
  return task.transport_record_id ?? task.batch_id;
}

const bodyOptions = { responseReturn: 'body' as const };

function isFiniteNonNegativeNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function isNullableString(value: unknown): value is null | string {
  return value === null || typeof value === 'string';
}

function isNullableFiniteNonNegativeNumber(
  value: unknown,
): value is null | number {
  return value === null || isFiniteNonNegativeNumber(value);
}

function isControlArchiveStorage(value: unknown): boolean {
  if (!isObject(value)) return false;
  if (
    !isNullableFiniteNonNegativeNumber(value.available_bytes) ||
    !isNullableString(value.reported_at) ||
    !isNullableFiniteNonNegativeNumber(value.total_bytes)
  ) {
    return false;
  }
  return (
    value.available_bytes === null ||
    value.total_bytes === null ||
    value.available_bytes <= value.total_bytes
  );
}

function isEvidenceState(
  value: unknown,
): value is 'FAILED' | 'PASSED' | 'PENDING' {
  return value === 'FAILED' || value === 'PASSED' || value === 'PENDING';
}

function isEdgeSyncRecordState(value: unknown): value is EdgeSyncRecordState {
  return (
    value === 'CLOSED' ||
    value === 'FAILED' ||
    value === 'PACKED' ||
    value === 'PARTIALLY_CLOSED' ||
    value === 'WAITING_CENTRAL' ||
    value === 'WAITING_RECEIPT'
  );
}

function isControlIngestTaskState(value: unknown): value is ControlIngestTaskState {
  return [
    'COMMITTED',
    'CONFLICT',
    'FAILED',
    'IMPORTING',
    'QUEUED',
    'VERIFYING',
  ].includes(value as ControlIngestTaskState);
}

/** Reject incomplete control import data rather than presenting it as an empty queue. */
export function isControlIngestOverviewViewProjection(
  value: unknown,
): value is ControlIngestOverviewView {
  if (!isObject(value) || !isObject(value.summary) || !Array.isArray(value.tasks)) {
    return false;
  }
  const summary = value.summary;
  const summaryFields = [
    summary.connected_media,
    summary.conflict_locked,
    summary.failed,
    summary.importing,
    summary.queued,
    summary.source_sites,
    summary.verified,
  ];
  const taskIds = new Set<string>();
  return (
    typeof value.site_id === 'string' &&
    (value.storage === undefined || isControlArchiveStorage(value.storage)) &&
    summaryFields.every(isFiniteNonNegativeNumber) &&
    value.tasks.every((task) => {
      if (!isObject(task) || !isNonEmptyString(task.batch_id)) {
        return false;
      }
      const transportRecordId =
        isNonEmptyString(task.transport_record_id)
          ? task.transport_record_id
          : task.batch_id;
      if (taskIds.has(transportRecordId)) return false;
      taskIds.add(transportRecordId);
      return (
        isNullableString(task.completed_at) &&
        isNullableString(task.failure_reason) &&
        isFiniteNonNegativeNumber(task.logical_bytes) &&
        (task.media_id === undefined || isNonEmptyString(task.media_id)) &&
        isNonEmptyString(task.media_label) &&
        isNonEmptyString(task.media_serial_suffix) &&
        isFiniteNonNegativeNumber(task.object_count) &&
        isFiniteNonNegativeNumber(task.progress_percent) &&
        task.progress_percent <= 100 &&
        isNullableString(task.receipt_id) &&
        isNonEmptyString(task.result_label) &&
        isNonEmptyString(task.source_site_id) &&
        isNonEmptyString(task.started_at) &&
        isNonEmptyString(task.stage_label) &&
        isControlIngestTaskState(task.state) &&
        isNonEmptyString(task.updated_at) &&
        isFiniteNonNegativeNumber(task.verified_bytes)
      );
    })
  );
}

/**
 * Runtime views drive the EDGE home scene. Reject partial payloads rather
 * than letting missing counters or labels appear as an operational state.
 */
export function isEdgeRuntimeViewProjection(
  value: unknown,
): value is EdgeRuntimeView {
  if (!isObject(value) || !isObject(value.media) || !isObject(value.next_action)) {
    return false;
  }

  const media = value.media;
  const nextAction = value.next_action;
  const current = value.current;
  const sourceExport = value.source_export;
  const mediaFields = [
    media.completed,
    media.connected,
    media.failed,
    media.running,
    media.standby,
    media.warning,
  ];
  const currentIsValid =
    current === null ||
      (isObject(current) &&
      typeof current.stage === 'string' &&
      isFiniteNonNegativeNumber(current.confirmed_bytes) &&
      (current.progress_percent === null ||
        isFiniteNonNegativeNumber(current.progress_percent)));
  const sourceExportIsValid =
    sourceExport === undefined ||
    sourceExport === null ||
    (isObject(sourceExport) &&
      isFiniteNonNegativeNumber(sourceExport.copied_bytes) &&
      isFiniteNonNegativeNumber(sourceExport.copied_versions) &&
      isFiniteNonNegativeNumber(sourceExport.not_copied_bytes) &&
      isFiniteNonNegativeNumber(sourceExport.not_copied_versions));

  return (
    typeof value.display_name === 'string' &&
    typeof value.site_id === 'string' &&
    typeof value.state === 'string' &&
    typeof value.state_label === 'string' &&
    mediaFields.every(isFiniteNonNegativeNumber) &&
    typeof nextAction.action_code === 'string' &&
    typeof nextAction.detail === 'string' &&
    typeof nextAction.priority === 'string' &&
    typeof nextAction.title === 'string' &&
    (value.throughput_bytes_per_second === null ||
      isFiniteNonNegativeNumber(value.throughput_bytes_per_second)) &&
    currentIsValid &&
    sourceExportIsValid
  );
}

/**
 * Sync-record screens can represent an empty history, but never an omitted
 * history or a malformed record. This keeps route changes fail-closed.
 */
export function isEdgeSyncRecordsViewProjection(
  value: unknown,
): value is EdgeSyncRecordsView {
  if (!isObject(value) || !isObject(value.summary) || !Array.isArray(value.records)) {
    return false;
  }

  const summary = value.summary;
  const summaryFields = [
    summary.closed,
    summary.failed,
    summary.packed,
    summary.total,
    summary.waiting_receipt,
  ];
  const batchIds = new Set<string>();
  const recordsAreValid = value.records.every((record) => {
    if (
      !isObject(record) ||
      !isNonEmptyString(record.batch_id) ||
      batchIds.has(record.batch_id) ||
      !isNonEmptyString(record.destination_label) ||
      !isNonEmptyString(record.media_serial_suffix) ||
      !isNonEmptyString(record.result_label) ||
      !isEdgeSyncRecordState(record.state) ||
      !isFiniteNonNegativeNumber(record.logical_bytes) ||
      !isNullableString(record.completed_at) ||
      !isNullableString(record.failure_reason) ||
      !isNullableString(record.failure_stage) ||
      !isNullableString(record.retry_result) ||
      !Array.isArray(record.events) ||
      !Array.isArray(record.stages)
    ) {
      return false;
    }

    batchIds.add(record.batch_id);
    return (
      record.events.every(
        (event) =>
          isObject(event) &&
          isNonEmptyString(event.at) &&
          isNonEmptyString(event.label) &&
          isNonEmptyString(event.result) &&
          isEvidenceState(event.state),
      ) &&
      record.stages.every(
        (stage) =>
          isObject(stage) &&
          isNullableString(stage.at) &&
          isNonEmptyString(stage.label) &&
          isEvidenceState(stage.state),
      )
    );
  });

  return (
    typeof value.site_id === 'string' &&
    typeof value.transport_media_connected === 'boolean' &&
    summaryFields.every(isFiniteNonNegativeNumber) &&
    recordsAreValid
  );
}

export function getLocalViewContext() {
  return baseRequestClient.get<LocalViewContext>(
    '/api/local/v1/context',
    bodyOptions,
  );
}

export function getEdgeRuntimeView() {
  return baseRequestClient.get<EdgeRuntimeView>(
    '/api/local/v1/edge/runtime-overview',
    bodyOptions,
  );
}

export function getEdgeTransportDisksView() {
  return baseRequestClient.get<EdgeTransportDisksView>(
    '/api/local/v1/edge/nas-disks',
    bodyOptions,
  );
}

/** Reads the v2 all-disk candidate projection. */
export function getEdgeMediaCandidatesView() {
  return baseRequestClient.get<EdgeMediaCandidatesView>(
    '/api/local/v2/edge/nas-disks',
    bodyOptions,
  );
}

/**
 * Initializes a single currently observed candidate.  The opaque candidate ID
 * is issued by the Agent; the browser never submits a device path or mount.
 */
export function initializeEdgeMediaCandidate(
  candidateId: string,
  candidateSessionId: string,
) {
  return baseRequestClient.post<void>(
    '/api/local/v2/edge/nas-disks/candidates/initialize',
    { candidateId, candidateSessionId },
    bodyOptions,
  );
}

/**
 * Requests a non-destructive handoff of a desktop-mounted current candidate.
 * The server accepts only the opaque candidate/session pair and revalidates it
 * at the Worker; this endpoint never initializes or clears the disk.
 */
export function takeOverEdgeMediaCandidate(
  candidateId: string,
  candidateSessionId: string,
) {
  return baseRequestClient.post<void>(
    '/api/local/v2/edge/nas-disks/candidates/takeover',
    { candidateId, candidateSessionId },
    bodyOptions,
  );
}

/**
 * Requests initialization for exactly one Agent-discovered transport medium.
 *
 * `discoveryToken` is opaque and short-lived. Deliberately do not accept a
 * mount path, a block-device selector, capacity, or a caller-provided
 * `media_id`; the Agent re-validates the token and current medium identity.
 */
export function initializeUnregisteredEdgeTransportDisk(
  discoveryToken: string,
) {
  return baseRequestClient.post<void>(
    '/api/local/v1/edge/nas-disks/initialize',
    { discovery_token: discoveryToken },
    bodyOptions,
  );
}

export function getEdgeServerStatusView() {
  return baseRequestClient.get<EdgeServerStatusView>(
    '/api/local/v1/edge/server-status',
    bodyOptions,
  );
}

export function getEdgeSyncRecordsView() {
  return baseRequestClient.get<EdgeSyncRecordsView>(
    '/api/local/v1/edge/sync-records',
    bodyOptions,
  );
}

export function getEdgeManagedSettingsView() {
  return baseRequestClient.get<EdgeManagedSettingsView>(
    '/api/local/v1/edge/managed-settings',
    bodyOptions,
  );
}

export function getEdgeRegistrationView() {
  return baseRequestClient.get<EdgeRegistrationView>(
    '/api/local/v1/edge/registration',
    bodyOptions,
  );
}

/**
 * Reads the explicitly mounted, development-only registration projection.
 *
 * This is deliberately separate from the standard registration endpoint:
 * callers must not infer isolated-trial availability from a normal local
 * view, and this read-only request never starts a registration action.
 */
export function getIsolatedTrialRegistrationView() {
  return baseRequestClient.get<EdgeRegistrationView>(
    '/api/local/v1/edge/isolated-trial/registration',
    bodyOptions,
  );
}

/**
 * Isolated-trial-only action. The Agent generates every registration artifact
 * locally; the browser deliberately sends no CSR, proof, file, or path.
 */
export function exportIsolatedTrialRegistrationRequest() {
  return baseRequestClient.post<EdgeRegistrationView>(
    '/api/local/v1/edge/isolated-trial/registration/request',
    undefined,
    bodyOptions,
  );
}

/**
 * Imports only the response already placed in the Agent's fixed exchange
 * root. There is intentionally no file path or file-content parameter.
 */
export function importIsolatedTrialRegistrationResponse() {
  return baseRequestClient.post<EdgeRegistrationView>(
    '/api/local/v1/edge/isolated-trial/registration/response',
    undefined,
    bodyOptions,
  );
}

export function getControlSitesView() {
  return baseRequestClient.get<ControlSitesView>(
    '/api/local/v1/control/sites',
    bodyOptions,
  );
}

export function getControlSiteView(siteId: string) {
  return baseRequestClient.get<ControlSiteDetailView>(
    `/api/local/v1/control/sites/${encodeURIComponent(siteId)}`,
    bodyOptions,
  );
}

export function getControlCollectionView(siteId: string) {
  return baseRequestClient.get<ControlCollectionView>(
    `/api/local/v1/control/sites/${encodeURIComponent(siteId)}/collection`,
    bodyOptions,
  );
}

export function getControlIngestOverviewView() {
  return baseRequestClient.get<ControlIngestOverviewView>(
    '/api/local/v1/control/ingest-overview',
    bodyOptions,
  );
}

export function getControlIngestRecordsView() {
  return baseRequestClient.get<ControlIngestRecordsView>(
    '/api/local/v1/control/ingest-records',
    bodyOptions,
  );
}
