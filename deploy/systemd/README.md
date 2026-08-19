# deploy/systemd

systemd service files for Linux deployment.

Install the binaries under `/opt/rustfs-transfer/`:

- `/opt/rustfs-transfer/rustfs-transfer-center`
- `/opt/rustfs-transfer/rustfs-transfer-edge`

Install configuration under `/etc/rustfs-transfer/` and runtime data under `/var/lib/rustfs-transfer/`.

Edge detects transport disk changes through its built-in polling loop. No udev
rule, rescan helper, or rescan oneshot service is installed. Deployment must
still provide a real mount strategy. Edge scans configured roots and the Linux
block-device table; the default roots cover `/mnt/rustfs-transfer`,
`/media/<user>/<label>`, and `/run/media/<user>/<label>`.

Edge runtime configuration is loaded from `/etc/rustfs-transfer/edge.env`.
Keep both
`RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED=false` and
`RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY=false` unless an approved Edge
rollout explicitly enables automatic scan/export. Rollback is the reverse:
set both values to `false` and restart `rustfs-transfer-edge`.
