# Linux 环境开发运行手册

本文用于在一台 Linux 电脑上运行 Edge 开发环境。这个场景默认：

- Linux 电脑只跑 Edge 程序。
- PostgreSQL 使用外部已存在的数据库。
- RustFS 使用外部已存在的服务。
- Center 已经能创建 Edge 站点，并能拿到 `edge_code` 和 `edge_key`。
- Edge 运行配置统一使用 `edge.env`，不再使用 `edge.toml`。

开发机不安装 PostgreSQL，不代表可以没有 Edge 数据库。Edge 仍然需要连接一套已经建好库、跑过 `sql/edge/` 迁移的 PostgreSQL。

## 1. 准备系统依赖

Ubuntu / Debian 系统执行：

```bash
sudo apt update
sudo apt install -y git curl build-essential pkg-config libssl-dev
```

各依赖用途：

| 依赖 | 用途 |
|---|---|
| `git` | 拉取和同步项目代码 |
| `curl` | 下载 Rust 安装脚本，也可用于测试 HTTP 接口 |
| `build-essential` | 提供 `gcc`、`g++`、`make` 等基础编译工具 |
| `pkg-config` | 帮 Rust 依赖找到系统库路径 |
| `libssl-dev` | 提供 OpenSSL 开发库，HTTPS、TLS、S3、数据库 TLS 相关依赖经常需要 |

安装 Rust：

```bash
curl https://sh.rustup.rs -sSf | sh
source ~/.cargo/env
```

安装开发热重载工具：

```bash
cargo install cargo-watch
```

验证：

```bash
rustc --version
cargo --version
cargo watch --version
```

## 2. 拉取项目代码

第一次使用：

```bash
cd ~
git clone <你的 Git 仓库地址> RustFSDisk-dev
cd ~/RustFSDisk-dev
```

已经拉取过代码时：

```bash
cd ~/RustFSDisk-dev
git pull
```

## 3. 确认外部数据库

Edge 连接的是外部 PostgreSQL，但外部数据库里必须已经存在 Edge 表结构。

如果数据库管理员已经建好 Edge 库并执行过迁移，只需要拿到连接串：

```bash
postgres://用户名:密码@PostgreSQL地址:5432/edge库名
```

如果需要从这台 Linux 开发机执行迁移，可以安装 `sqlx-cli` 后对外部数据库执行：

```bash
cargo install sqlx-cli --no-default-features --features postgres

export DATABASE_URL='postgres://用户名:密码@PostgreSQL地址:5432/edge库名'
sqlx migrate run --source sql/edge
```

注意：

- 这一步只是连接外部 PostgreSQL 执行迁移，不是在 Linux 开发机本地安装 PostgreSQL。
- 如果外部 Edge 数据库没有跑迁移，Edge 程序可能能启动，但扫描、任务、磁盘记录等接口会因为缺表或缺字段报错。

## 4. 推荐的开发目录方式

开发环境推荐把运行文件放在项目目录下的 `.runtime/`，这样不用把配置和临时数据散到系统目录里，也方便清理。

```bash
cd ~/RustFSDisk
mkdir -p .runtime/edge-data
mkdir -p .runtime/edge-log
mkdir -p .runtime/mnt
cp deploy/config/edge.env.example .runtime/edge.env
nano .runtime/edge.env
```

`.runtime/edge.env` 至少要配置以下内容：

```bash
# Edge 服务监听地址
EDGE_BIND=0.0.0.0:8081

# Edge 本地 PostgreSQL，保存扫描结果、导出任务和运输盘运行态
DATABASE_URL=postgres://用户名:密码@PostgreSQL地址:5432/edge库名

# Edge 站点编码和 Center 生成的 Edge KEY
EDGE_CODE=Center里创建的edge_code
EDGE_KEY=Center里创建站点后生成的KEY

# RustFS 扫描结果复用窗口，单位分钟
SCAN_REUSE_MINUTES=5

# 插拔盘识别轮询开关和间隔
DISK_POLLING_ENABLED=true
DISK_POLLING_INTERVAL_SECONDS=1

# RustFS / S3 兼容服务配置
RUSTFS_ENDPOINT=http://RustFS地址:9000
RUSTFS_REGION=us-east-1
RUSTFS_ACCESS_KEY=RustFS访问KEY
RUSTFS_SECRET_KEY=RustFS访问SECRET

# Edge 本地目录和运输盘扫描目录
DATA_DIR=./.runtime/edge-data
LOG_DIR=./.runtime/edge-log
TRANSPORT_MOUNT_ROOT=./.runtime/mnt
DISK_MOUNT_ROOTS=./.runtime/mnt,/media/$USER

# 是否允许插盘后自动扫描 RustFS、创建导出任务并开始写盘
AUTO_EXPORT_ENABLED=false

# 服务启动或轮询发现 READY 盘后，是否自动启动导出
AUTO_EXPORT_START_ON_READY=false

# 至少有几块 READY 运输盘才允许自动导出
AUTO_EXPORT_MIN_READY_DISK_COUNT=1

# 自动导出冷却时间，单位秒，避免重复触发
AUTO_EXPORT_COOLDOWN_SECONDS=60
```

注意：

- 不要再配置 `RUSTFS_TRANSFER__CONFIG_PATH`。
- 不要再创建或维护 `edge.toml`。
- `EDGE_CODE` 和 `EDGE_KEY` 必须来自 Center 的 Edge 站点管理。
- `DISK_POLLING_ENABLED=true` 后，Edge 后端会按间隔自动识别插拔盘。
- 轮询只推最终状态，不再把 `DETECTED/CHECKING` 临时状态推给前端，避免页面在“未注册/已初始化”之间反复跳。
- 如果 Linux 桌面自动把 U 盘挂载到 `/media/用户名/...`，必须保留 `/media/$USER` 到 `DISK_MOUNT_ROOTS`。
- `.runtime/edge.env` 是通过 shell 的 `. .runtime/edge.env` 加载的，所以 `/media/$USER` 会展开成当前用户名。

## 5. 启动 Edge 后端

```bash
cd ~/RustFSDisk

set -a
. .runtime/edge.env
set +a

cargo watch -w crates -w sql -x "run -p rustfs-transfer-edge --bin rustfs-transfer-edge"
```

启动成功后，Edge 后端监听：

```text
http://Linux电脑IP:8081
```

本机测试：

```bash
curl -s http://127.0.0.1:8081/api/edge/dashboard/summary
```

其他电脑访问时，把 `127.0.0.1` 换成这台 Linux 电脑的局域网 IP。

## 6. 前端联调

如果前端跑在另一台电脑，需要把前端请求地址改成这台 Linux 电脑的 Edge 地址，例如：

```text
http://192.168.1.50:8081
```

如果访问失败，先在 Linux 电脑确认服务监听：

```bash
ss -lntp | grep 8081
```

再确认防火墙是否放行：

```bash
sudo ufw status
```

如需要临时放行：

```bash
sudo ufw allow 8081/tcp
```

## 7. 插入运输盘测试

Edge 端只识别和使用运输盘，不负责格式化、初始化或重新初始化运输盘。用于 Edge 导出的盘必须先在 Center 端完成初始化，并且文件系统应为 ext4。

插入硬盘后，先确认 Linux 系统能看到并挂载：

```bash
lsblk -o NAME,PATH,FSTYPE,LABEL,UUID,MOUNTPOINTS,SIZE,MODEL,SERIAL
findmnt
```

如果 Linux 没有桌面自动挂载，可能需要手动挂载到 `.runtime/mnt` 或 `/mnt/rustfs-transfer`。示例：

```bash
sudo mount /dev/sdX1 .runtime/mnt
```

把 `/dev/sdX1` 换成 `lsblk` 看到的真实分区路径。不要对不确定的磁盘执行格式化、清盘或重新初始化命令。

如果硬盘挂载在 `/media/$USER/...`，Edge 会通过：

```bash
DISK_MOUNT_ROOTS=./.runtime/mnt,/media/$USER
```

一起扫描。

确认当前运行 Edge 的用户能读写挂载目录：

```bash
touch .runtime/mnt/.edge-write-test
rm .runtime/mnt/.edge-write-test
```

如果这里没有权限，Edge 后续写盘也会失败。开发环境可以先修正挂载目录权限；正式环境应通过 systemd 运行用户和挂载策略统一处理。

查看 Edge 是否识别到盘：

```bash
curl -s http://127.0.0.1:8081/api/edge/dashboard/summary
```

Edge 后端默认通过内置轮询识别插拔盘。`DISK_POLLING_INTERVAL_SECONDS=1` 时，正常情况下插盘或拔盘后最多 1 秒进入下一轮扫描。

## 8. 正式服务目录方式

如果后面要按 Linux 服务方式长期运行，可以改用系统目录：

| 路径 | 用途 |
|---|---|
| `/etc/rustfs-transfer/edge.env` | Edge 运行配置 |
| `/var/lib/rustfs-transfer/edge` | Edge 本地运行数据 |
| `/var/log/rustfs-transfer/edge` | Edge 日志 |
| `/mnt/rustfs-transfer` | 手动挂载运输盘的默认目录 |

准备目录：

```bash
sudo mkdir -p /etc/rustfs-transfer
sudo mkdir -p /var/lib/rustfs-transfer/edge
sudo mkdir -p /var/log/rustfs-transfer/edge
sudo mkdir -p /mnt/rustfs-transfer

sudo cp deploy/config/edge.env.example /etc/rustfs-transfer/edge.env
sudo nano /etc/rustfs-transfer/edge.env
```

正式服务模式下，如果也要扫描桌面自动挂载目录，不要写 `/media/$USER`，要写实际用户名路径，例如：

```bash
DISK_MOUNT_ROOTS=/mnt/rustfs-transfer,/media/edge
```

开发运行不强制使用这些目录。只有要接近正式部署、systemd 托管、统一日志和系统权限管理时，才推荐使用。

## 9. 常见问题

### 9.1 cargo build 找不到 openssl

确认已安装：

```bash
sudo apt install -y pkg-config libssl-dev
```

### 9.2 插盘后 Edge 页面没反应

按顺序检查：

```bash
lsblk -o NAME,PATH,FSTYPE,LABEL,UUID,MOUNTPOINTS,SIZE,MODEL,SERIAL
findmnt
curl -s http://127.0.0.1:8081/api/edge/dashboard/summary
```

如果 `lsblk` 都看不到硬盘，说明问题在 Linux/USB/虚拟化透传层，Edge 程序还没有机会识别。

如果 `lsblk` 能看到，确认挂载点是否包含在：

```bash
echo "$DISK_MOUNT_ROOTS"
```

修改 `.runtime/edge.env` 后，必须重启 `cargo watch`，因为环境变量只在进程启动时读取一次。

### 9.3 8081 端口被占用

查看占用：

```bash
ss -lntp | grep 8081
```

如果已经有 systemd 版 Edge 在跑，开发模式启动前先停掉：

```bash
sudo systemctl stop rustfs-transfer-edge.service
```

### 9.4 数据库连接失败

确认 `DATABASE_URL` 指向外部 Edge 数据库，并且 Linux 电脑能访问数据库端口：

```bash
nc -vz PostgreSQL地址 5432
```

如果没有 `nc`：

```bash
sudo apt install -y netcat-openbsd
```

如果能连上端口，但 Edge 日志里出现表不存在、字段不存在，说明外部 Edge 数据库没有执行或没有执行完整 `sql/edge/` 迁移。

### 9.5 RustFS 连接失败

确认 endpoint、access key、secret key 正确，并且 Linux 电脑能访问 RustFS：

```bash
curl -I http://RustFS地址:9000
```

## 10. 每次开发的常用命令

```bash
cd ~/RustFSDisk

set -a
. .runtime/edge.env
set +a

cargo watch -w crates -w sql -x "run -p rustfs-transfer-edge --bin rustfs-transfer-edge"
```

这就是 Linux 开发机运行 Edge 的主流程。
