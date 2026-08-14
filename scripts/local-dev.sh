#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOCAL_DIR="${ROOT_DIR}/.local/dev"
DISK_ROOT="${LOCAL_DIR}/mock-disks"
CONFIG_DIR="${LOCAL_DIR}/config"
LOG_DIR="${LOCAL_DIR}/logs"
DATA_DIR="${LOCAL_DIR}/data"

mkdir -p "${DISK_ROOT}/initialized/rustfs-transfer"/{data,meta,manifests,logs,quarantine/partial}
mkdir -p "${DISK_ROOT}/sealed/rustfs-transfer"/{data,meta,manifests,logs,quarantine/partial}
mkdir -p "${CONFIG_DIR}" "${LOG_DIR}/center" "${LOG_DIR}/edge" "${DATA_DIR}/center" "${DATA_DIR}/edge"

cp "${ROOT_DIR}/deploy/config/center.example.toml" "${CONFIG_DIR}/center.toml"
cp "${ROOT_DIR}/deploy/config/edge.example.toml" "${CONFIG_DIR}/edge.toml"

cat > "${DISK_ROOT}/initialized/rustfs-transfer/disk_info.json" <<'JSON'
{
  "protocol": { "version": "1.0" },
  "disk": {
    "disk_id": "mock-disk-initialized",
    "sn": "MOCK-SN-INITIALIZED",
    "capacity_bytes": 107374182400
  },
  "status": {
    "code": "INITIALIZED",
    "sealed": false,
    "imported": false,
    "reusable": true,
    "last_error": null
  },
  "security": {
    "center_key_id": "mock-center-key-id",
    "data_key_id": "mock-data-key-id",
    "encryption_alg": "AES-256-GCM",
    "signature_alg": "HMAC-SHA256"
  }
}
JSON

cat > "${DISK_ROOT}/sealed/rustfs-transfer/disk_info.json" <<'JSON'
{
  "protocol": { "version": "1.0" },
  "disk": {
    "disk_id": "mock-disk-sealed",
    "sn": "MOCK-SN-SEALED",
    "capacity_bytes": 107374182400
  },
  "status": {
    "code": "SEALED",
    "sealed": true,
    "imported": false,
    "reusable": false,
    "last_error": null
  },
  "security": {
    "center_key_id": "mock-center-key-id",
    "data_key_id": "mock-data-key-id",
    "encryption_alg": "AES-256-GCM",
    "signature_alg": "HMAC-SHA256"
  },
  "edge": {
    "edge_code": "edge-demo",
    "seal_id": "mock-seal-id"
  }
}
JSON

cat > "${DISK_ROOT}/sealed/rustfs-transfer/manifests/export_manifest.json" <<'JSON'
{
  "protocol_version": "1.0",
  "disk_id": "mock-disk-sealed",
  "seal_id": "mock-seal-id",
  "edge_code": "edge-demo",
  "objects": []
}
JSON

printf "sha256-not-calculated-in-day1-fixture  export_manifest.json\n" > "${DISK_ROOT}/sealed/rustfs-transfer/manifests/export_manifest.sha256"

cat > "${LOCAL_DIR}/center.env" <<'ENV'
RUSTFS_TRANSFER__CONFIG_PATH=.local/dev/config/center.toml
RUSTFS_TRANSFER__DATABASE__URL=postgres://rustfs_transfer_center:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_center_dev
RUSTFS_TRANSFER__PATHS__DATA_DIR=.local/dev/data/center
RUSTFS_TRANSFER__PATHS__LOG_DIR=.local/dev/logs/center
RUSTFS_TRANSFER__PATHS__TRANSPORT_MOUNT_ROOT=.local/dev/mock-disks
ENV

cat > "${LOCAL_DIR}/edge.env" <<'ENV'
RUSTFS_TRANSFER__CONFIG_PATH=.local/dev/config/edge.toml
RUSTFS_TRANSFER__DATABASE__URL=postgres://rustfs_transfer_edge:CHANGE_ME_PASSWORD@127.0.0.1:5432/rustfs_transfer_edge_dev
RUSTFS_TRANSFER__EDGE__EDGE_CODE=edge-demo
RUSTFS_TRANSFER__EDGE__EDGE_KEY=CHANGE_ME_EDGE_KEY_FROM_CENTER
RUSTFS_TRANSFER__PATHS__DATA_DIR=.local/dev/data/edge
RUSTFS_TRANSFER__PATHS__LOG_DIR=.local/dev/logs/edge
RUSTFS_TRANSFER__PATHS__TRANSPORT_MOUNT_ROOT=.local/dev/mock-disks
ENV

cat <<EOF
Prepared local RustFS Transfer layout:
  ${LOCAL_DIR}

Mock transport disks:
  ${DISK_ROOT}/initialized
  ${DISK_ROOT}/sealed

Next Day 2 steps:
  source .local/dev/center.env && cargo run -p rustfs-transfer-center
  source .local/dev/edge.env && cargo run -p rustfs-transfer-edge
EOF
