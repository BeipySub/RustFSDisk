# deploy/udev

udev disk hotplug rules.

`99-rustfs-transfer-disk.rules` only starts `rustfs-transfer-disk-rescan@.service`.
`remove` events use `systemctl --no-block start` because the block device unit can
disappear before `SYSTEMD_WANTS` starts the rescan service.

The rule must not directly run export, import, cleanup, reinitialization, formatting, or mount business logic. The long-running daemon performs startup scans and decides whether a disk can enter a task pool after ext4 and protocol checks.

udev does not mount disks for this project. The deployment must provide desktop
automount or an approved system mount strategy. Edge scans configured roots and
the Linux block-device table, so one-click deployments must keep
`/mnt/rustfs-transfer`, `/media`, and `/run/media` readable by the Edge service.
