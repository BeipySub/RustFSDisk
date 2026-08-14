# Debug 待修复列表

本文记录已经排查确认、但暂不立即实现的风险点。后续修复时应按冻结文档和当前部署边界处理，不直接修改 `docs/v1.0冻结/`。

## 1. Edge 插盘后依赖 Linux 已完成挂载

- 状态：已补部署保护 / 仍需部署层提供挂载策略。
- 现象：Linux 识别到 `/dev/sdb1` 之类的块设备，但如果没有挂载到 Edge 配置的扫描目录，例如 `/mnt/rustfs-transfer`、`/media/edge` 或开发环境 `.runtime/mnt`，Edge 扫描不到运输盘。
- 当前处理：Edge 只扫描已配置的挂载目录和其一级子目录；udev/systemd 只触发 rescan，不负责业务导出，也不负责自动 mount。
- 风险：部署环境没有桌面自动挂载或没有配置 systemd mount 时，插盘后页面看起来“没有反应”。
- 本次修复：`rustfs-transfer-disk-rescan@.service` 在通知 Edge 前增加短等待，降低 udev 事件早于 Linux 自动挂载完成导致的漏扫风险；`edge.env.example` 增加显式 `RUSTFS_TRANSFER__PATHS__DISK_MOUNT_ROOTS` 示例。
- 后续建议：
  - 不建议 Edge 主程序自行 mount 裸设备，避免误挂错盘和扩大系统权限。
  - 在部署层明确运输盘挂载策略，例如固定 `/mnt/rustfs-transfer` 或实际用户名下的 `/media/<user>`。
  - 部署检查或运维手册中增加 `lsblk`、`findmnt`、`disk_mount_roots` 对照检查。

## 2. Edge 缺少 READY 前写权限探测

- 状态：已修复。
- 现象：运输盘已经挂载并通过 ext4、`disk_info.json`、协议版本、签名、状态和 `.partial` 残留检查后，Edge 可能把盘标记为 `READY`；但如果运行 Edge 的用户没有写权限，真正导出写盘时才失败。
- 当前处理：导出阶段能识别权限错误，并在失败审计中归类为 `WRITE_BEFORE_PERMISSION_DENIED`。
- 风险：页面先显示可用，实际开始导出后才暴露权限问题，部署排查成本高。
- 本次修复：Edge 在 `INITIALIZED` 盘进入 `READY` 前，会在 `rustfs-transfer/` 协议目录下创建、写入、同步并删除 `.edge-write-probe-*` 临时文件；失败时记录 `DISK_WRITE_PERMISSION_DENIED` 并拒绝进入任务池。
- 后续可增强：
  - 前端可针对 `DISK_WRITE_PERMISSION_DENIED` 增加更明确的中文排查提示。
  - 运维手册可补充 systemd 运行用户与挂载目录权限的标准配置示例。
