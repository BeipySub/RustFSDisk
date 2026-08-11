// @ts-nocheck
import assert from "node:assert/strict";
import test from "node:test";
import {
  buildExportJobsUrl,
  diskStatusDisplay,
  isActiveExportJobStatus,
  localEdgePath,
  normalizeEdgeDashboardSummary,
  normalizeExportJobsResponse,
  visibleDiskStatusCode,
} from "../api/edgeDashboard.ts";
import { applyCopyProgressEvent } from "../ws/edgeCopyProgress.ts";

test("builds final export job list URL without empty filters", () => {
  assert.equal(
    buildExportJobsUrl("/api/edge/dashboard/export-jobs", {
      page: 2,
      page_size: 8,
      export_job_status: "COPYING",
      q: "  disk-a  ",
    }),
    "/api/edge/dashboard/export-jobs?page=2&page_size=8&export_job_status=COPYING&q=disk-a",
  );
});

test("keeps browser API paths local to the Edge origin", () => {
  assert.equal(
    localEdgePath("https://center.example/not-local", "/api/edge/dashboard/summary"),
    "/api/edge/dashboard/summary",
  );
  assert.equal(
    buildExportJobsUrl("https://center.example/api/edge/dashboard/export-jobs", {
      page: 1,
      page_size: 8,
    }),
    "/api/edge/dashboard/export-jobs?page=1&page_size=8",
  );
});

test("normalizes summary and hides imported disk lifecycle from Edge UI", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-hz-01",
    disk_status_code: "IMPORTED",
    latest_export_job: {
      export_job_id: "job-1",
      edge_code: "edge-hz-01",
      export_job_status: "COPYING",
      total_bytes: 100,
      copied_bytes: 30,
      object_count: 10,
      copied_count: 3,
    },
    disks: [
      {
        disk_id: "disk-1",
        disk_sn: "SN-1",
        mount_path: "/media/edge/a",
        runtime_status: "READY",
        disk_status_code: "IMPORTED",
        free_bytes: 10,
      },
    ],
  });

  assert.equal(summary.disk_status_code, undefined);
  assert.equal(summary.disks[0]?.disk_status_code, undefined);
  assert.equal(diskStatusDisplay(summary.disks[0]?.disk_status_code).includes("IMPORTED"), false);
});

test("normalizes export job list and drops non Edge object statuses", () => {
  const response = normalizeExportJobsResponse(
    {
      page: 1,
      page_size: 8,
      total: 1,
      items: [
        {
          export_job_id: "job-1",
          edge_code: "edge-hz-01",
          export_job_status: "SEALED",
          object_count: 2,
          copied_count: 2,
          total_bytes: 20,
          copied_bytes: 20,
          disk_count: 1,
          object_status_counts: {
            EXPORTED: 2,
            IMPORTED: 2,
          } as never,
        },
      ],
    },
    { page: 1, page_size: 8 },
  );

  assert.equal(response.items[0]?.object_status_counts.EXPORTED, 2);
  assert.equal("IMPORTED" in (response.items[0]?.object_status_counts ?? {}), false);
  assert.equal(visibleDiskStatusCode("INITIALIZED"), "INITIALIZED");
  assert.equal(visibleDiskStatusCode("IMPORTED"), undefined);
});

test("normalizes deployed export job list wire shape", () => {
  const response = normalizeExportJobsResponse(
    {
      page: 1,
      page_size: 8,
      total_count: 1,
      records: [
        {
          export_job_id: "job-2",
          edge_code: "edge-demo",
          export_job_status: "SEALED",
          object_count: 114,
          copied_count: 57,
          total_bytes: 1902602924,
          copied_bytes: 951301462,
        },
      ],
    },
    { page: 1, page_size: 8 },
  );

  assert.equal(response.total, 1);
  assert.equal(response.items.length, 1);
  assert.equal(response.items[0]?.export_job_id, "job-2");
});

test("keeps production export statuses from the shared contract", () => {
  const response = normalizeExportJobsResponse(
    [
      {
        export_job_id: "job-sealing",
        edge_code: "edge-demo",
        export_job_status: "SEALING",
        object_count: 1,
        copied_count: 1,
        total_bytes: 1024,
        copied_bytes: 1024,
        disk_count: 1,
      },
    ],
    { page: 1, page_size: 8 },
  );

  assert.equal(response.items[0]?.export_job_status, "SEALING");
});

test("separates active export statuses from historical terminal jobs", () => {
  assert.equal(isActiveExportJobStatus("PENDING"), false);
  assert.equal(isActiveExportJobStatus("SCANNING"), true);
  assert.equal(isActiveExportJobStatus("COPYING"), true);
  assert.equal(isActiveExportJobStatus("SEALING"), true);
  assert.equal(isActiveExportJobStatus("SEALED"), false);
  assert.equal(isActiveExportJobStatus("FAILED"), false);
  assert.equal(isActiveExportJobStatus("CANCELLED"), false);
});

test("normalizes deployed dashboard summary wire shape", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job_status: "SEALED",
    scan: {
      event_type: "SCAN_DONE",
      event_time: "2026-08-11T05:32:02Z",
      scan_phase: "DONE",
      bucket_done: 3,
      object_seen: 59,
      total_bytes: 1428414961,
    },
    global_progress: {
      total_bytes: 3331017885,
      done_bytes: 1428414961,
      object_total: 173,
      object_done: 59,
    },
    latest_export_job: {
      export_job_id: "job-sealed",
      edge_code: "edge-demo",
      export_job_status: "SEALED",
      object_count: 173,
      copied_count: 59,
      total_bytes: 3331017885,
      copied_bytes: 1428414961,
    },
    disks: [],
    message: "edge controlled HTTP API summary",
  });

  assert.equal(summary.export_job_status, "SEALED");
  assert.equal(summary.scan.scan_event_type, "SCAN_DONE");
  assert.equal(summary.global_progress.object_total, 173);
});

test("merges websocket disks without dropping restored HTTP disk list", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job_id: "job-copying",
    export_job_status: "COPYING",
    scan: {
      scan_event_type: "SCAN_DONE",
      scanned_bucket_count: 1,
      scanned_object_count: 2,
      scanned_bytes: 300,
      stable_object_count: 2,
      skipped_object_count: 0,
      current_bucket: "",
      current_key: "",
      last_scan_at: "2026-08-11T05:32:02Z",
      message: "done",
    },
    global_progress: {
      total_bytes: 300,
      done_bytes: 0,
      remaining_bytes: 300,
      speed_bytes_per_sec: 0,
      object_total: 2,
      object_done: 0,
      object_remaining: 2,
    },
    disks: [
      {
        disk_id: "disk-a",
        disk_sn: "SN-A",
        mount_path: "/mnt/a",
        runtime_status: "READY",
        total_bytes: 100,
        done_bytes: 0,
        remaining_bytes: 100,
        free_bytes: 100,
        speed_bytes_per_sec: 0,
        object_total: 1,
        object_done: 0,
        object_remaining: 1,
        current_object: null,
        message: "ready",
      },
      {
        disk_id: "disk-b",
        disk_sn: "SN-B",
        mount_path: "/mnt/b",
        runtime_status: "READY",
        total_bytes: 200,
        done_bytes: 0,
        remaining_bytes: 200,
        free_bytes: 200,
        speed_bytes_per_sec: 0,
        object_total: 1,
        object_done: 0,
        object_remaining: 1,
        current_object: null,
        message: "ready",
      },
    ],
    ws_connected: false,
    last_http_refresh_at: "2026-08-11T05:32:02Z",
    message: "summary",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "COPY_PROGRESS",
    event_time: "2026-08-11T05:33:00Z",
    source: "edge",
    edge_code: "edge-demo",
    export_job_id: "job-copying",
    export_job_status: "COPYING",
    global_progress: {
      total_bytes: 300,
      done_bytes: 50,
      remaining_bytes: 250,
      speed_bytes_per_sec: 10,
      object_total: 2,
      object_done: 0,
      object_remaining: 2,
    },
    disks: [
      {
        disk_id: "disk-a",
        disk_sn: "SN-A",
        mount_path: "/mnt/a",
        runtime_status: "COPYING",
        total_bytes: 100,
        done_bytes: 50,
        remaining_bytes: 50,
        free_bytes: 100,
        speed_bytes_per_sec: 10,
        object_total: 1,
        object_done: 0,
        object_remaining: 1,
        current_object: null,
        message: "copying",
      },
    ],
    message: "copying",
  });

  assert.equal(merged.disks.length, 2);
  assert.equal(merged.disks.find((disk) => disk.disk_id === "disk-a")?.runtime_status, "COPYING");
  assert.equal(merged.disks.find((disk) => disk.disk_id === "disk-b")?.runtime_status, "READY");
});

test("does not regress a terminal HTTP export job from stale websocket progress", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job_id: "job-sealed",
    export_job_status: "SEALED",
    scan: {
      scan_event_type: "SCAN_DONE",
      scanned_bucket_count: 1,
      scanned_object_count: 1,
      scanned_bytes: 100,
      stable_object_count: 1,
      skipped_object_count: 0,
      current_bucket: "",
      current_key: "",
      last_scan_at: "2026-08-11T05:32:02Z",
      message: "done",
    },
    global_progress: {
      total_bytes: 100,
      done_bytes: 100,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 1,
      object_done: 1,
      object_remaining: 0,
    },
    disks: [],
    ws_connected: false,
    last_http_refresh_at: "2026-08-11T05:32:02Z",
    message: "sealed",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "COPY_PROGRESS",
    event_time: "2026-08-11T05:34:00Z",
    source: "edge",
    edge_code: "edge-demo",
    export_job_id: "job-sealed",
    export_job_status: "COPYING",
    global_progress: {
      total_bytes: 100,
      done_bytes: 0,
      remaining_bytes: 100,
      speed_bytes_per_sec: 0,
      object_total: 1,
      object_done: 0,
      object_remaining: 1,
    },
    disks: [],
    message: "stale copying",
  });

  assert.equal(merged.export_job_status, "SEALED");
  assert.equal(merged.global_progress.done_bytes, 100);
  assert.equal(merged.message, "sealed");
  assert.equal(merged.ws_connected, true);
});

test("does not resurrect disks from stale websocket when HTTP summary is terminal and empty", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job_id: "job-sealed",
    export_job_status: "SEALED",
    scan: {
      scan_event_type: "SCAN_DONE",
      scanned_bucket_count: 1,
      scanned_object_count: 59,
      scanned_bytes: 1428414961,
      stable_object_count: 59,
      skipped_object_count: 0,
      current_bucket: "",
      current_key: "",
      last_scan_at: "2026-08-11T08:27:40Z",
      message: "done",
    },
    global_progress: {
      total_bytes: 1428414961,
      done_bytes: 1428414961,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 59,
      object_done: 59,
      object_remaining: 0,
    },
    disks: [],
    ws_connected: false,
    last_http_refresh_at: "2026-08-11T08:27:40Z",
    message: "summary says no mounted disks",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "COPY_PROGRESS",
    event_time: "2026-08-11T08:27:41Z",
    source: "edge",
    edge_code: "edge-demo",
    export_job_id: "stale-copying-job",
    export_job_status: "COPYING",
    global_progress: {
      total_bytes: 1428414961,
      done_bytes: 0,
      remaining_bytes: 1428414961,
      speed_bytes_per_sec: 0,
      object_total: 59,
      object_done: 0,
      object_remaining: 59,
    },
    disks: [
      {
        disk_id: "disk-a",
        disk_sn: "SN-A",
        mount_path: "/mnt/a",
        runtime_status: "COPYING",
        total_bytes: 1428414961,
        done_bytes: 0,
        remaining_bytes: 1428414961,
        free_bytes: 0,
        speed_bytes_per_sec: 0,
        object_total: 59,
        object_done: 0,
        object_remaining: 59,
        current_object: null,
        message: "stale copying",
      },
    ],
    message: "old ws snapshot",
  });

  assert.equal(merged.disks.length, 0);
  assert.equal(merged.export_job_status, "SEALED");
  assert.equal(merged.export_job_id, "job-sealed");
  assert.equal(merged.global_progress.done_bytes, 1428414961);
});
