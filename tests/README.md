# 测试目录说明

根目录 `tests/` 只作为测试说明入口，不承载主要测试代码。

## Rust 后端

- `crates/common/tests/`：公共协议、加密、错误类型等集成测试。
- `crates/center-backend/tests/`：中控端 API、导入、账本、密钥和恢复流程测试。
- `crates/edge-backend/tests/`：边缘端扫描、导出、对象分配、封盘和恢复流程测试。

模块内部纯函数、状态机和小范围逻辑优先使用源码文件内的 `#[cfg(test)] mod tests`。

## 前端

- `web/center-web/src/__tests__/`：中控端页面、组件、状态管理和 WebSocket 展示测试。
- `web/edge-web/src/__tests__/`：边缘端页面、组件、状态管理和 WebSocket 展示测试。

