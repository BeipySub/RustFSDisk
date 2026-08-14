# deploy/udev

udev disk hotplug rules.

`99-rustfs-transfer-disk.rules` only starts `rustfs-transfer-disk-rescan@.service`.

The rule must not directly run export, import, cleanup, reinitialization, formatting, or mount business logic. The long-running daemon performs startup scans and decides whether a disk can enter a task pool after ext4 and protocol checks.

udev does not mount disks for this project. The deployment must provide desktop
automount or an approved system mount strategy, then configure Edge
`disk_mount_roots` to include the resulting mount path.
