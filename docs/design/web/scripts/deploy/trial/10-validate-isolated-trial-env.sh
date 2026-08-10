#!/bin/sh
set -eu

# The image only proxies to the co-located service through host networking.
# Refuse a public, cross-environment, credential-bearing, or malformed
# upstream before nginx renders its template.
is_tcp_port() {
  case "$1" in
    '' | *[!0-9]*) return 1 ;;
  esac
  [ "$1" -ge 1 ] && [ "$1" -le 65535 ]
}

upstream="${FUSTFS_LOCAL_API_UPSTREAM:-}"
case "$upstream" in
  http://127.0.0.1:*) upstream_port="${upstream#http://127.0.0.1:}" ;;
  *) upstream_port='' ;;
esac

if ! is_tcp_port "$upstream_port"; then
  echo >&2 'FUSTFS_LOCAL_API_UPSTREAM must be http://127.0.0.1:<approved-local-port>.'
  exit 1
fi

if ! is_tcp_port "${FUSTFS_WEB_LISTEN_PORT:-}"; then
  echo >&2 'FUSTFS_WEB_LISTEN_PORT must be a local TCP port from 1 through 65535.'
  exit 1
fi
