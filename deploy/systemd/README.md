# deploy/systemd

systemd service files for Linux deployment.

Install the binaries under `/opt/rustfs-transfer/`:

- `/opt/rustfs-transfer/rustfs-transfer-center`
- `/opt/rustfs-transfer/rustfs-transfer-edge`
- `/opt/rustfs-transfer/bin/rustfs-transfer-rescan`

Install configuration under `/etc/rustfs-transfer/` and runtime data under `/var/lib/rustfs-transfer/`.

The rescan template is intentionally a notification shim. It must only tell the local daemon that block devices changed; export, import, cleanup, and reinitialization work stay inside the long-running center or edge service.
