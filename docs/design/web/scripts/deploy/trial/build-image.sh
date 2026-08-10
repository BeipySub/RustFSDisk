#!/usr/bin/env bash
set -euo pipefail

# Build only. This script does not run containers, alter existing containers,
# connect to a VM, or publish an image. The caller must separately authorize
# Docker image pulls and the eventual isolated-VM deployment.
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
web_root="$(cd -- "${script_dir}/../../.." && pwd)"
image_tag="${1:-fustfsdisk-web-isolated-trial:local}"

docker build \
  --file "${script_dir}/Dockerfile" \
  --tag "${image_tag}" \
  "${web_root}"

printf 'Built candidate image: %s\n' "${image_tag}"
printf 'No container was started. Follow the isolated trial runbook for the separately authorized run command.\n'
