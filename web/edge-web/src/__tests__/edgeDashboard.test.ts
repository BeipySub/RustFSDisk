// @ts-nocheck
import assert from "node:assert/strict";
import test from "node:test";
import {
  buildExportJobsUrl,
  diskStatusDisplay,
  localEdgePath,
  normalizeEdgeDashboardSummary,
  normalizeExportJobsResponse,
  visibleDiskStatusCode,
} from "../api/edgeDashboard.ts";

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
