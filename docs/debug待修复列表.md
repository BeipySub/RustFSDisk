# Debug 待修复列表

本文只记录已经排查确认的问题、当前状态和现象。

- [ ] **Edge 插盘后依赖 Linux 已完成挂载** 

  - **描述：** Linux 识别到 `/dev/sdb1` 之类的块设备，但如果没有挂载到 Edge 配置的扫描目录，例如 `/mnt/rustfs-transfer`、`/media/edge` 或开发环境 `.runtime/mnt`，Edge 扫描不到运输盘。

  - **记录时间：** 2026-08-14

- [x] **Edge 缺少 READY 前写权限探测** 

  - **描述：** 运输盘已经挂载并通过 ext4、`disk_info.json`、协议版本、签名、状态和 `.partial` 残留检查后，Edge 可能把盘标记为 `READY`；但如果运行 Edge 的用户没有写权限，真正导出写盘时才失败。

  - **记录时间：** 2026-08-14
