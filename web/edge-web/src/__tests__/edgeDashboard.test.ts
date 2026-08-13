// @ts-nocheck
import assert from "node:assert/strict";
import test from "node:test";
import {
  buildExportJobsUrl,
  edgeDiskPrimaryStatusLabel,
  diskStatusDisplay,
  edgeRejectedDiskStatusLabel,
  edgeScanPath,
  isActiveExportJobStatus,
  localEdgePath,
  normalizeEdgeDashboardSummary,
  normalizeExportJobDetail,
  normalizeExportJobsResponse,
  visibleDiskStatusCode,
} from "../api/edgeDashboard.ts";
import { applyCopyProgressEvent, parseCopyProgressEvent } from "../ws/edgeCopyProgress.ts";

function exportJob(export_job_id, export_job_status, overrides = {}) {
  return {
    export_job_id,
    edge_code: "edge-demo",
    export_job_status,
    total_bytes: 0,
    done_bytes: 0,
    remaining_bytes: 0,
    speed_bytes_per_sec: 0,
    object_total: 0,
    object_done: 0,
    object_remaining: 0,
    ...overrides,
  };
}

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

test("uses local Edge scan path for manual RustFS scan", () => {
  assert.equal(edgeScanPath(), "/api/edge/scan");
});

test("displays Edge visible disk lifecycle codes in Chinese", () => {
  assert.equal(diskStatusDisplay("UNREGISTERED"), "未注册");
  assert.equal(diskStatusDisplay("INITIALIZED"), "已初始化");
  assert.equal(diskStatusDisplay("SEALED"), "已封盘");
  assert.equal(diskStatusDisplay("CENTER_IMPORTING"), "中控导入中");
  assert.equal(diskStatusDisplay("IMPORTED"), "已导入");
  assert.equal(diskStatusDisplay(undefined), "未返回");
});

test("normalizes summary and keeps imported disk lifecycle visible in Edge UI", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-hz-01",
    disk_status_code: "IMPORTED",
    object_inventory: {
      total_bytes: 100,
      exported_bytes: 30,
      total_count: 10,
      exported_count: 3,
    },
    export_job: exportJob("job-1", "COPYING", {
      total_bytes: 100,
      done_bytes: 30,
      remaining_bytes: 70,
      object_total: 10,
      object_done: 3,
      object_remaining: 7,
    }),
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

  assert.equal(summary.disk_status_code, "IMPORTED");
  assert.equal(summary.disks[0]?.disk_status_code, "IMPORTED");
  assert.equal(diskStatusDisplay(summary.disks[0]?.disk_status_code), "已导入");
});

test("labels sealed rejected disk as sealed instead of uninitialized", () => {
  assert.equal(
    edgeRejectedDiskStatusLabel({
      disk_status_code: "SEALED",
      last_error_code: "MANIFEST_INVALID",
      error_message: "MANIFEST_INVALID: disk status_code SEALED is not eligible for offline edge export; expected INITIALIZED",
      message: "",
    }),
    "已封盘",
  );
});

test("uses imported lifecycle as primary label even when runtime is rejected", () => {
  assert.equal(
    edgeDiskPrimaryStatusLabel({
      disk_status_code: "IMPORTED",
      runtime_status: "REJECTED",
      last_error_code: "MANIFEST_INVALID",
      error_message: "MANIFEST_INVALID: disk status_code IMPORTED is not eligible for offline edge export; expected INITIALIZED",
      message: "",
    }),
    "已导入",
  );
});

test("uses sealed lifecycle as primary label even when runtime is rejected", () => {
  assert.equal(
    edgeDiskPrimaryStatusLabel({
      disk_status_code: "SEALED",
      runtime_status: "REJECTED",
      last_error_code: "MANIFEST_INVALID",
      error_message: "MANIFEST_INVALID: disk status_code SEALED is not eligible for offline edge export; expected INITIALIZED",
      message: "",
    }),
    "已封盘",
  );
});

test("does not use runtime status as disk lifecycle primary label", () => {
  assert.equal(
    edgeDiskPrimaryStatusLabel({
      disk_status_code: undefined,
      runtime_status: "REJECTED",
      last_error_code: "MANIFEST_INVALID",
      error_message: "MANIFEST_INVALID: expected INITIALIZED",
      message: "",
    }),
    "未返回",
  );
});

test("labels imported rejected disk as imported instead of uninitialized", () => {
  assert.equal(
    edgeRejectedDiskStatusLabel({
      disk_status_code: "IMPORTED",
      last_error_code: "MANIFEST_INVALID",
      error_message: "MANIFEST_INVALID: disk status_code IMPORTED is not eligible for offline edge export; expected INITIALIZED",
      message: "",
    }),
    "已导入",
  );
});

test("labels missing disk info as uninitialized", () => {
  assert.equal(
    edgeRejectedDiskStatusLabel({
      disk_status_code: undefined,
      last_error_code: "MISSING_DISK_INFO",
      error_message: "missing disk_info.json transport protocol file",
      message: "",
    }),
    "未初始化",
  );
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
  assert.equal(visibleDiskStatusCode("IMPORTED"), "IMPORTED");
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

test("marks sealed historical export disks as removable when runtime was cleared", () => {
  const detail = normalizeExportJobDetail({
    export_job_id: "job-sealed",
    edge_code: "edge-demo",
    export_job_status: "SEALED",
    object_count: 1,
    copied_count: 1,
    total_bytes: 1024,
    copied_bytes: 1024,
    disk_count: 1,
    disks: [
      {
        disk_id: "disk-a",
        disk_sn: "",
        mount_path: "",
        runtime_status: undefined,
        total_bytes: 1024,
        done_bytes: 1024,
        remaining_bytes: 0,
        free_bytes: 0,
        speed_bytes_per_sec: 0,
        object_total: 1,
        object_done: 1,
        object_remaining: 0,
        current_object: null,
        message: "",
      },
    ],
  });

  assert.equal(detail.disks[0]?.disk_status_code, "SEALED");
  assert.equal(detail.disks[0]?.runtime_status, "DONE");
  assert.equal(detail.disks[0]?.message, "已封盘，可拔盘");
});

test("accepts scan and copy start websocket events from Edge realtime stream", () => {
  const scan = parseCopyProgressEvent(
    JSON.stringify({
      event_type: "SCAN_PROGRESS",
      source: "edge",
      edge_code: "edge-demo",
      export_job: exportJob("", "SCANNING"),
      global_progress: {
        total_bytes: 0,
        done_bytes: 0,
        remaining_bytes: 0,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
      },
      disks: [],
    }),
  );
  const copyStarted = parseCopyProgressEvent(
    JSON.stringify({
      event_type: "COPY_STARTED",
      source: "edge",
      edge_code: "edge-demo",
      export_job: exportJob("job-copying", "COPYING", {
        total_bytes: 100,
        done_bytes: 0,
        remaining_bytes: 100,
        speed_bytes_per_sec: 0,
        object_total: 1,
        object_done: 0,
        object_remaining: 1,
      }),
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
    }),
  );

  assert.equal(scan?.event_type, "SCAN_PROGRESS");
  assert.equal(copyStarted?.event_type, "COPY_STARTED");
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
    object_inventory: {
      total_bytes: 3331017885,
      exported_bytes: 1428414961,
      total_count: 173,
      exported_count: 59,
    },
    global_progress: {
      total_bytes: 3331017885,
      done_bytes: 1428414961,
      remaining_bytes: 1902602924,
      speed_bytes_per_sec: 0,
      object_total: 173,
      object_done: 59,
      object_remaining: 114,
    },
    export_job: exportJob("job-sealed", "SEALED", {
      total_bytes: 3331017885,
      done_bytes: 1428414961,
      remaining_bytes: 1902602924,
      object_total: 173,
      object_done: 59,
      object_remaining: 114,
    }),
    disks: [],
    message: "edge controlled HTTP API summary",
  });

  assert.equal(summary.export_job?.export_job_status, "SEALED");
  assert.equal(summary.export_job?.export_job_id, "job-sealed");
  assert.equal(summary.global_progress.object_total, 173);
});

test("replaces dashboard disks with the latest websocket disk list", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    object_inventory: {
      total_bytes: 300,
      exported_bytes: 0,
      total_count: 2,
      exported_count: 0,
    },
    export_job: exportJob("job-copying", "COPYING", {
      total_bytes: 300,
      done_bytes: 0,
      remaining_bytes: 300,
      object_total: 2,
      object_done: 0,
      object_remaining: 2,
    }),
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
    export_job: exportJob("job-copying", "COPYING", {
      total_bytes: 300,
      done_bytes: 50,
      remaining_bytes: 250,
      speed_bytes_per_sec: 10,
      object_total: 2,
      object_done: 0,
      object_remaining: 2,
    }),
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

  assert.equal(merged.disks.length, 1);
  assert.equal(merged.disks.find((disk) => disk.disk_id === "disk-a")?.runtime_status, "COPYING");
  assert.equal(merged.disks.find((disk) => disk.disk_id === "disk-b"), undefined);
});

test("replaces terminal HTTP summary with websocket snapshot", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    object_inventory: {
      total_bytes: 100,
      exported_bytes: 100,
      total_count: 1,
      exported_count: 1,
    },
    export_job: exportJob("job-sealed", "SEALED", {
      total_bytes: 100,
      done_bytes: 100,
      remaining_bytes: 0,
      object_total: 1,
      object_done: 1,
      object_remaining: 0,
    }),
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
    export_job: exportJob("job-sealed", "COPYING", {
      total_bytes: 100,
      done_bytes: 0,
      remaining_bytes: 100,
      object_total: 1,
      object_done: 0,
      object_remaining: 1,
    }),
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

  assert.equal(merged.export_job?.export_job_status, "COPYING");
  assert.equal(merged.global_progress.done_bytes, 0);
  assert.equal(merged.message, "stale copying");
  assert.equal(merged.ws_connected, true);
});

test("uses websocket disks as the current snapshot", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    object_inventory: {
      total_bytes: 1428414961,
      exported_bytes: 1428414961,
      total_count: 59,
      exported_count: 59,
    },
    export_job: exportJob("job-sealed", "SEALED", {
      total_bytes: 1428414961,
      done_bytes: 1428414961,
      remaining_bytes: 0,
      object_total: 59,
      object_done: 59,
      object_remaining: 0,
    }),
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
    export_job: exportJob("stale-copying-job", "COPYING", {
      total_bytes: 1428414961,
      done_bytes: 0,
      remaining_bytes: 1428414961,
      object_total: 59,
      object_done: 0,
      object_remaining: 59,
    }),
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

  assert.equal(merged.disks.length, 1);
  assert.equal(merged.disks[0]?.disk_id, "disk-a");
  assert.equal(merged.export_job?.export_job_status, "COPYING");
  assert.equal(merged.export_job?.export_job_id, "stale-copying-job");
  assert.equal(merged.global_progress.done_bytes, 0);
});

test("does not resurrect a removed disk when HTTP summary has no current disks", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    object_inventory: {
      total_bytes: 1428414961,
      exported_bytes: 1428414961,
      total_count: 59,
      exported_count: 59,
    },
    export_job: exportJob("job-sealed", "SEALED", {
      total_bytes: 1428414961,
      done_bytes: 1428414961,
      remaining_bytes: 0,
      object_total: 59,
      object_done: 59,
      object_remaining: 0,
    }),
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
    last_http_refresh_at: "2026-08-12T10:31:58Z",
    message: "summary says no current disks",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "DISK_REMOVED",
    event_time: "2026-08-12T10:23:01Z",
    source: "edge",
    edge_code: "edge-demo",
    disk_status_code: "UNREGISTERED",
    export_job: exportJob("", "PENDING"),
    global_progress: {
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
    },
    disks: [],

    message: "DISK_REMOVED",
  });

  assert.equal(merged.disks.length, 0);
  assert.equal(merged.ws_connected, true);
  assert.equal(merged.message, "DISK_REMOVED");
});

test("keeps imported lifecycle when websocket updates rejected runtime", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job: exportJob("job-copying", "COPYING", {
      total_bytes: 300,
      done_bytes: 50,
      remaining_bytes: 250,
      speed_bytes_per_sec: 10,
      object_total: 2,
      object_done: 0,
      object_remaining: 2,
    }),
    global_progress: {
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
    },
    disks: [
      {
        disk_id: "disk-imported",
        disk_sn: "SN-1",
        mount_path: "/media/edge/imported",
        disk_status_code: "IMPORTED",
        runtime_status: "CHECKING",
        total_bytes: 100,
        done_bytes: 0,
        remaining_bytes: 0,
        free_bytes: 40,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
        current_object: null,
        filesystem: "ext4",
        message: "summary",
      },
    ],
    ws_connected: false,
    last_http_refresh_at: "2026-08-12T10:31:58Z",
    message: "summary",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "DISK_REJECTED",
    event_time: "2026-08-12T10:32:00Z",
    source: "edge",
    edge_code: "edge-demo",
    export_job: exportJob("job-copying", "COPYING"),
    global_progress: summary.global_progress,
    disks: [
      {
        disk_id: "disk-imported",
        disk_sn: "SN-1",
        mount_path: "/media/edge/imported",
        disk_status_code: "IMPORTED",
        runtime_status: "REJECTED",
        filesystem: "ext4",
        total_bytes: 100,
        done_bytes: 0,
        remaining_bytes: 0,
        free_bytes: 0,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
        current_object: null,
        last_error_code: "MANIFEST_INVALID",
        error_message: "MANIFEST_INVALID: disk status_code IMPORTED is not eligible for offline edge export; expected INITIALIZED",
        message: "rejected",
      },
    ],
    message: "rejected",
  });

  assert.equal(merged.disks.length, 1);
  assert.equal(merged.disks[0]?.disk_status_code, "IMPORTED");
  assert.equal(merged.disks[0]?.runtime_status, "REJECTED");
  assert.equal(merged.disks[0]?.filesystem, "ext4");
  assert.equal(merged.disks[0]?.total_bytes, 100);
  assert.equal(edgeDiskPrimaryStatusLabel(merged.disks[0]!), "已导入");
});

test("keeps sealed lifecycle when websocket updates rejected runtime", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job: exportJob("job-copying", "COPYING"),
    global_progress: {
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
    },
    disks: [
      {
        disk_id: "disk-sealed",
        disk_sn: "SN-2",
        mount_path: "/media/edge/sealed",
        disk_status_code: "SEALED",
        runtime_status: "DONE",
        total_bytes: 200,
        done_bytes: 200,
        remaining_bytes: 0,
        free_bytes: 20,
        speed_bytes_per_sec: 0,
        object_total: 1,
        object_done: 1,
        object_remaining: 0,
        current_object: null,
        message: "summary",
      },
    ],
    ws_connected: false,
    last_http_refresh_at: "2026-08-12T10:31:58Z",
    message: "summary",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "DISK_REJECTED",
    event_time: "2026-08-12T10:32:00Z",
    source: "edge",
    edge_code: "edge-demo",
    export_job: exportJob("job-copying", "COPYING"),
    global_progress: summary.global_progress,
    disks: [
      {
        disk_id: "disk-sealed",
        disk_sn: "SN-2",
        mount_path: "/media/edge/sealed",
        disk_status_code: "SEALED",
        runtime_status: "REJECTED",
        total_bytes: 200,
        done_bytes: 0,
        remaining_bytes: 0,
        free_bytes: 0,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
        current_object: null,
        last_error_code: "MANIFEST_INVALID",
        error_message: "MANIFEST_INVALID: disk status_code SEALED is not eligible for offline edge export; expected INITIALIZED",
        message: "rejected",
      },
    ],
    message: "rejected",
  });

  assert.equal(merged.disks[0]?.disk_status_code, "SEALED");
  assert.equal(merged.disks[0]?.runtime_status, "REJECTED");
  assert.equal(edgeDiskPrimaryStatusLabel(merged.disks[0]!), "已封盘");
});

test("clears dashboard disks when websocket sends an empty disk list", () => {
  const summary = normalizeEdgeDashboardSummary({
    source: "edge",
    edge_code: "edge-demo",
    edge_name: "edge-demo",
    export_job: exportJob("job-copying", "COPYING"),
    global_progress: {
      total_bytes: 0,
      done_bytes: 0,
      remaining_bytes: 0,
      speed_bytes_per_sec: 0,
      object_total: 0,
      object_done: 0,
      object_remaining: 0,
    },
    disks: [
      {
        disk_id: "disk-a",
        disk_sn: "SN-A",
        mount_path: "/media/edge/a",
        disk_status_code: "INITIALIZED",
        runtime_status: "READY",
        total_bytes: 100,
        done_bytes: 0,
        remaining_bytes: 0,
        free_bytes: 100,
        speed_bytes_per_sec: 0,
        object_total: 0,
        object_done: 0,
        object_remaining: 0,
        current_object: null,
        message: "ready",
      },
    ],
    ws_connected: false,
    last_http_refresh_at: "2026-08-12T10:31:58Z",
    message: "summary",
  });

  const merged = applyCopyProgressEvent(summary, {
    event_type: "DISK_REMOVED",
    event_time: "2026-08-12T10:32:00Z",
    source: "edge",
    edge_code: "edge-demo",
    export_job: exportJob("job-copying", "COPYING"),
    global_progress: summary.global_progress,
    disks: [],
    message: "removed",
  });

  assert.equal(merged.disks.length, 0);
});
