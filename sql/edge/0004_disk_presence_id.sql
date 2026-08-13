-- 作用：为一次运输盘物理在位周期分配稳定标识，供 Edge Dashboard HTTP/WS 关联同一张盘卡。
-- 不替代 disk_id；disk_id 仍是盘内协议与导出任务的业务身份。

ALTER TABLE disk_runtime
  ADD COLUMN IF NOT EXISTS disk_presence_id UUID;

CREATE INDEX IF NOT EXISTS idx_disk_runtime_presence_id
  ON disk_runtime(disk_presence_id);

COMMENT ON COLUMN disk_runtime.disk_presence_id IS
  '一次物理插入在位周期的 UUID；从 DETECTED 到 REMOVED 保持不变，仅用于运行态事件和前端卡片关联，不替代 disk_id。';
