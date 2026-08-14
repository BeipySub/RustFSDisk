# RustFSDisk

RustFS 离线运输盘同步项目。

本项目包含中控端、边缘端、双端前端、共享 Rust crate、SQL 迁移和 Linux 部署文件。仓库结构按当前开发骨架维护。

## 仓库结构

```text
rustfs-transfer/
├── .gitignore
├── README.md
├── Cargo.toml                    # Cargo Workspace 顶层配置
├── docs/                         # 项目技术文档
│   └── v1.0冻结/                 # v1.0 冻结基线，禁止直接修改
│       ├── 需求文档.md
│       ├── 技术架构与技术选型.md
│       ├── 数据库设计.md
│       └── 开发协议契约.md
│
├── crates/                       # Rust 后端核心代码（Workspace）
│   ├── common/                   # 共享基础库（协议定义、AES 加密、HMAC 签名、错误码）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── protocol/         # disk_info.json 与 export_manifest.json 结构体
│   │       ├── crypto/           # AES-256-GCM / HMAC-SHA256 实现
│   │       ├── error.rs          # 统一错误码枚举
│   │       └── lib.rs
│   │
│   ├── center-backend/           # 中控端后端服务
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── api/              # HTTP REST API：接收边缘请求、注册、密钥分配
│   │       ├── ws/               # WebSocket 推送服务
│   │       ├── db/               # PostgreSQL/sqlx 权威库 CRUD 与事务
│   │       ├── importer/         # 硬盘扫描、校验、解密与 RustFS 导入引擎
│   │       └── main.rs
│   │
│   └── edge-backend/             # 边缘端后端服务
│       ├── Cargo.toml
│       └── src/
│           ├── api/              # HTTP REST API：本地控制 API、手动操作
│           ├── ws/               # WebSocket 推送服务
│           ├── db/               # PostgreSQL/sqlx 本地运行账本
│           ├── scanner/          # 边缘 RustFS S3 流式扫描器
│           ├── exporter/         # 分块、AES 加密写入、fsync 与封盘引擎
│           ├── udev/             # udev 事件处理与防抖
│           └── main.rs
│
├── web/                          # 前端项目
│   ├── center-web/               # 中控端 Vue 3 前端
│   │   ├── package.json
│   │   └── src/
│   │       ├── views/            # DashboardView 主页面
│   │       ├── components/       # 磁盘列表、导入进度、任务摘要、错误面板
│   │       └── ws/               # WebSocket 自动重连与事件分发
│   │
│   └── edge-web/                 # 边缘端 Vue 3 前端
│       ├── package.json
│       └── src/
│           ├── views/            # DashboardView 主页面
│           ├── components/       # 扫描汇总、多盘导出进度、异常面板
│           └── ws/               # WebSocket 自动重连与事件分发
│
├── sql/                          # sqlx 数据库迁移脚本
│   ├── center/                   # 中控端 PostgreSQL 迁移
│   │   ├── 0001_init_center.sql
│   │   └── 0002_seed_required_enum.sql
│   └── edge/                     # 边缘端 PostgreSQL 迁移
│       ├── 0001_init_edge.sql
│       └── 0002_seed_required_enum.sql
│
├── deploy/                       # 部署与系统配置文件
│   ├── systemd/                  # 服务守护配置
│   │   ├── rustfs-transfer-center.service
│   │   ├── rustfs-transfer-edge.service
│   │   └── rustfs-transfer-disk-rescan@.service
│   ├── udev/                     # 硬盘热插拔规则
│   │   └── 99-rustfs-transfer-disk.rules
│   └── config/                   # 示例配置文件
│       ├── center.example.toml
│       ├── center.env.example
│       └── edge.env.example
│
└── scripts/                      # 本地开发、部署和运维脚本
```

## 关键约定

- 后端固定使用 Rust，数据库访问固定使用 `sqlx` 和显式 SQL。
- 数据库固定使用 PostgreSQL，中控端和边缘端迁移分别放在 `sql/center/` 与 `sql/edge/`。
- 不引入 ORM、复杂任务队列或额外消息中间件。
- `center-backend` 和 `edge-backend` 不直接互相依赖；共享类型和工具放入 `crates/common/`。
- 中控端和边缘端前端独立部署。
- systemd 文件放在 `deploy/systemd/`，udev 规则放在 `deploy/udev/`。
- udev 只触发 rescan 通知服务，不直接执行导出或导入业务逻辑。
- Center 结构化示例配置放在 `deploy/config/center.example.toml`；Edge 运行配置只使用 `deploy/config/edge.env.example`。
- 真实 KEY、数据库密码、RustFS secret 和部署密钥不得提交。
- `Cargo.lock` 需要提交，用于锁定 Rust 依赖版本。

## 部署路径约定

```text
/opt/rustfs-transfer/              # 程序目录
/etc/rustfs-transfer/              # 配置目录
/var/lib/rustfs-transfer/          # 数据和任务运行目录
/var/log/rustfs-transfer/          # 日志目录
```

## 参考文档

- [需求文档.md](docs/v1.0冻结/需求文档.md)
- [开发协议契约.md](docs/v1.0冻结/开发协议契约.md)
- [数据库设计.md](docs/v1.0冻结/数据库设计.md)
- [技术架构与技术选型.md](docs/v1.0冻结/技术架构与技术选型.md)
