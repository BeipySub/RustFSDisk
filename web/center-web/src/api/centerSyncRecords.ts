import { DashboardHttpError } from "./centerDashboard";

export interface CenterEdgeOption {
  edge_code: string;
  edge_name?: string;
  edge_status?: string;
  object_count?: number;
}

export interface CenterSyncRecord {
  ledger_id?: number;
  edge_code: string;
  edge_name?: string;
  source_bucket: string;
  source_key: string;
  source_etag?: string;
  source_size_bytes: number;
  source_last_modified?: string;
  plaintext_sha256?: string;
  ciphertext_sha256?: string;
  chunk_group_id?: string;
  import_bucket: string;
  import_key: string;
  export_job_id?: string;
  import_job_id: string;
  imported_at?: string;
}

export interface CenterSyncRecordsQuery {
  page: number;
  page_size: number;
  edge_code?: string;
  imported_from?: string;
  imported_to?: string;
  q?: string;
}

export interface CenterSyncRecordsResponse {
  page: number;
  page_size: number;
  total: number;
  items: CenterSyncRecord[];
  edges?: CenterEdgeOption[];
}

interface CenterSyncRecordsWireResponse {
  page?: number;
  page_size?: number;
  total?: number;
  total_count?: number;
  items?: Partial<CenterSyncRecord>[];
  records?: Partial<CenterSyncRecord>[];
  edges?: CenterEdgeOption[];
}

interface CenterEdgeOptionsWireResponse {
  items?: CenterEdgeOption[];
  records?: CenterEdgeOption[];
  edges?: CenterEdgeOption[];
}

export async function fetchCenterSyncRecords(
  query: CenterSyncRecordsQuery,
): Promise<CenterSyncRecordsResponse> {
  const payload = await getJson<CenterSyncRecordsWireResponse | Partial<CenterSyncRecord>[]>(
    buildSyncRecordsUrl(query),
  );
  return normalizeSyncRecordsResponse(payload, query);
}

export async function fetchCenterSyncRecordDetail(importJobId: string): Promise<CenterSyncRecord> {
  const basePath = centerSyncRecordsPath();
  const payload = await getJson<Partial<CenterSyncRecord>>(
    `${localPath(basePath, "/api/center/sync-records")}/${encodeURIComponent(importJobId)}`,
  );
  return normalizeSyncRecord(payload);
}

export async function fetchCenterEdgeOptions(): Promise<CenterEdgeOption[]> {
  const payload = await getJson<CenterEdgeOption[] | CenterEdgeOptionsWireResponse>(
    localPath(centerEdgeOptionsPath(), "/api/center/edge-sites"),
  );
  if (Array.isArray(payload)) return payload.map(normalizeEdgeOption).filter((edge) => edge.edge_code);
  return (payload.edges ?? payload.items ?? payload.records ?? [])
    .map(normalizeEdgeOption)
    .filter((edge) => edge.edge_code);
}

export function normalizeSyncRecordsResponse(
  payload: CenterSyncRecordsWireResponse | Partial<CenterSyncRecord>[],
  query: Pick<CenterSyncRecordsQuery, "page" | "page_size">,
): CenterSyncRecordsResponse {
  if (Array.isArray(payload)) {
    return {
      page: query.page,
      page_size: query.page_size,
      total: payload.length,
      items: payload.map(normalizeSyncRecord),
    };
  }

  return {
    page: numberValue(payload.page, query.page),
    page_size: numberValue(payload.page_size, query.page_size),
    total: numberValue(payload.total ?? payload.total_count),
    items: (payload.items ?? payload.records ?? []).map(normalizeSyncRecord),
    edges: payload.edges?.map(normalizeEdgeOption).filter((edge) => edge.edge_code),
  };
}

function buildSyncRecordsUrl(query: CenterSyncRecordsQuery): string {
  const url = new URL(localPath(centerSyncRecordsPath(), "/api/center/sync-records"), browserOrigin());
  url.searchParams.set("page", String(query.page));
  url.searchParams.set("page_size", String(query.page_size));
  if (query.edge_code) url.searchParams.set("edge_code", query.edge_code);
  if (query.imported_from) url.searchParams.set("imported_from", query.imported_from);
  if (query.imported_to) url.searchParams.set("imported_to", query.imported_to);
  if (query.q) url.searchParams.set("q", query.q);
  return url.pathname + url.search;
}

function normalizeSyncRecord(payload: Partial<CenterSyncRecord>): CenterSyncRecord {
  return {
    ledger_id: payload.ledger_id,
    edge_code: payload.edge_code ?? "",
    edge_name: payload.edge_name,
    source_bucket: payload.source_bucket ?? "",
    source_key: payload.source_key ?? "",
    source_etag: payload.source_etag,
    source_size_bytes: numberValue(payload.source_size_bytes),
    source_last_modified: payload.source_last_modified,
    plaintext_sha256: payload.plaintext_sha256,
    ciphertext_sha256: payload.ciphertext_sha256,
    chunk_group_id: payload.chunk_group_id,
    import_bucket: payload.import_bucket ?? "",
    import_key: payload.import_key ?? "",
    export_job_id: payload.export_job_id,
    import_job_id: payload.import_job_id ?? "",
    imported_at: payload.imported_at,
  };
}

function normalizeEdgeOption(payload: CenterEdgeOption): CenterEdgeOption {
  return {
    edge_code: payload.edge_code ?? "",
    edge_name: payload.edge_name,
    edge_status: payload.edge_status,
    object_count: numberValue(payload.object_count),
  };
}

function centerSyncRecordsPath(): string {
  return import.meta.env.VITE_CENTER_SYNC_RECORDS_PATH ?? "/api/center/sync-records";
}

function centerEdgeOptionsPath(): string {
  return import.meta.env.VITE_CENTER_EDGE_OPTIONS_PATH ?? "/api/center/edge-sites";
}

function localPath(value: string | undefined, defaultPath: string): string {
  const trimmed = value?.trim();
  if (!trimmed || trimmed.startsWith("//")) return defaultPath;
  try {
    const url = new URL(trimmed, browserOrigin());
    return url.origin === browserOrigin() ? url.pathname + url.search : defaultPath;
  } catch {
    return defaultPath;
  }
}

async function getJson<T>(path: string): Promise<T> {
  const response = await fetch(path, { headers: { Accept: "application/json" } });
  if (!response.ok) {
    throw new DashboardHttpError(
      response.status === 404 ? "CENTER_SYNC_RECORDS_NOT_READY" : "CENTER_SYNC_RECORDS_HTTP_ERROR",
      `HTTP ${response.status} while loading ${path}`,
      response.status,
    );
  }
  return (await response.json()) as T;
}

function browserOrigin(): string {
  return globalThis.location?.origin ?? "http://localhost";
}

function numberValue(value: unknown, defaultValue = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : defaultValue;
}
