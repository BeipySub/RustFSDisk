import { DashboardHttpError } from "./centerDashboard";

export type EdgeStatus = "ACTIVE" | "DISABLED" | "ERROR";

export interface ManagedEdgeSite {
  edge_code: string;
  edge_name: string;
  auth_key_id: string;
  edge_status: EdgeStatus;
  object_count?: number;
  create_time?: string;
}

export interface CreateEdgeSiteInput {
  edge_code: string;
  edge_name: string;
  auth_key_id: string;
  edge_auth_secret: string;
  edge_status: EdgeStatus;
}

export interface UpdateEdgeSiteInput {
  edge_name: string;
  edge_status: EdgeStatus;
}

interface EdgeSitesWireResponse {
  items?: ManagedEdgeSite[];
  edges?: ManagedEdgeSite[];
}

export async function fetchManagedEdgeSites(): Promise<ManagedEdgeSite[]> {
  const payload = await getJson<EdgeSitesWireResponse | ManagedEdgeSite[]>(edgeSitesPath());
  const items = Array.isArray(payload) ? payload : payload.items ?? payload.edges ?? [];
  return items.map(normalizeManagedEdgeSite).filter((edge) => edge.edge_code);
}

export async function createManagedEdgeSite(input: CreateEdgeSiteInput): Promise<ManagedEdgeSite> {
  return normalizeManagedEdgeSite(
    await requestJson<ManagedEdgeSite>(edgeSitesPath(), {
      method: "POST",
      body: input,
    }),
  );
}

export async function updateManagedEdgeSite(
  edgeCode: string,
  input: UpdateEdgeSiteInput,
): Promise<ManagedEdgeSite> {
  return normalizeManagedEdgeSite(
    await requestJson<ManagedEdgeSite>(`${edgeSitesPath()}/${encodeURIComponent(edgeCode)}`, {
      method: "PUT",
      body: input,
    }),
  );
}

export async function deleteManagedEdgeSite(edgeCode: string): Promise<void> {
  await requestJson<unknown>(`${edgeSitesPath()}/${encodeURIComponent(edgeCode)}`, {
    method: "DELETE",
  });
}

function normalizeManagedEdgeSite(payload: Partial<ManagedEdgeSite>): ManagedEdgeSite {
  return {
    edge_code: payload.edge_code ?? "",
    edge_name: payload.edge_name ?? "",
    auth_key_id: payload.auth_key_id ?? "",
    edge_status: normalizeEdgeStatus(payload.edge_status),
    object_count: numberValue(payload.object_count),
    create_time: payload.create_time,
  };
}

function normalizeEdgeStatus(value: unknown): EdgeStatus {
  return value === "DISABLED" || value === "ERROR" ? value : "ACTIVE";
}

function edgeSitesPath(): string {
  return import.meta.env.VITE_CENTER_EDGE_SITES_PATH ?? "/api/center/edge-sites";
}

async function getJson<T>(path: string): Promise<T> {
  const response = await fetch(path, { headers: { Accept: "application/json" } });
  return readJson<T>(response, path);
}

async function requestJson<T>(
  path: string,
  options: { method: "POST" | "PUT" | "DELETE"; body?: unknown },
): Promise<T> {
  const headers: Record<string, string> = {
    Accept: "application/json",
  };
  let body: string | undefined;
  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
    body = JSON.stringify(options.body);
  }
  const response = await fetch(path, {
    method: options.method,
    headers,
    body,
  });
  return readJson<T>(response, path);
}

async function readJson<T>(response: Response, path: string): Promise<T> {
  if (!response.ok) {
    let message = `HTTP ${response.status} while loading ${path}`;
    try {
      const payload = (await response.json()) as { message?: string; error_code?: string };
      message = payload.message ?? message;
      throw new DashboardHttpError(payload.error_code ?? "CENTER_EDGE_SITE_HTTP_ERROR", message, response.status);
    } catch (error) {
      if (error instanceof DashboardHttpError) throw error;
      throw new DashboardHttpError("CENTER_EDGE_SITE_HTTP_ERROR", message, response.status);
    }
  }
  return (await response.json()) as T;
}

function numberValue(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}
