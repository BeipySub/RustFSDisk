# deploy/systemd

systemd service files for Linux deployment.

Install the binaries under `/opt/rustfs-transfer/`:

- `/opt/rustfs-transfer/rustfs-transfer-center`
- `/opt/rustfs-transfer/rustfs-transfer-edge`
- `/opt/rustfs-transfer/bin/rustfs-transfer-rescan`

Install configuration under `/etc/rustfs-transfer/` and runtime data under `/var/lib/rustfs-transfer/`.

The rescan template is intentionally a notification shim. It must only tell the local daemon that block devices changed; export, import, cleanup, and reinitialization work stay inside the long-running center or edge service.

`rustfs-transfer-disk-rescan@.service` waits briefly before notifying Edge so
Linux desktop automount or an approved system mount unit can finish exposing the
partition. The wait is not a mount operation. Deployment must still provide a
real mount strategy and include that mount path in
`RUSTFS_TRANSFER__PATHS__DISK_MOUNT_ROOTS`.

Edge runtime configuration is loaded from `/etc/rustfs-transfer/edge.env`.
Keep both
`RUSTFS_TRANSFER__AUTO_EXPORT__ENABLED=false` and
`RUSTFS_TRANSFER__AUTO_EXPORT__START_ON_READY=false` unless an approved Edge
rollout explicitly enables automatic scan/export. Rollback is the reverse:
set both values to `false` and restart `rustfs-transfer-edge`.
