本文按当前已经跑通的实际情况整理，并补齐 Center 端。目标是：Windows 本地继续写源码和跑前端，Edge / Center 后端在 Linux VM 中编译运行，前端分别通过本地端口访问 VM 后端：

- Edge：`127.0.0.1:8081`
- Center：`127.0.0.1:8080`

## 当前约定

| 项目 | 值 |
|---|---|
| Windows 源码目录 | `F:\work\RustFSDisk` |
| Edge VM SSH alias | `rustfs-edge` |
| Edge VM 地址 | `edge@192.168.128.130` |
| Center VM SSH alias | `rustfs-center` |
| Center VM 地址 | `control@192.168.128.131` |
| SSH 私钥 | `C:\Users\Beipy\.ssh\fustfs_vm_codex` |
| Edge VM 同步目录 | `~/RustFSDisk-dev` |
| Center VM 同步目录 | `~/RustFSDisk-dev` |
| Edge 后端端口 | `8081` |
| Center 后端端口 | `8080` |
| Windows 本地 Edge 映射端口 | `127.0.0.1:8081` |
| Windows 本地 Center 映射端口 | `127.0.0.1:8080` |
| Edge Mutagen 同步会话 | `rustfs-edge-dev` |
| Center Mutagen 同步会话 | `rustfs-center-dev` |

## 整体链路

```text
Windows 本地改代码
-> Mutagen 单向同步到 Edge VM 和 Center VM
-> 两台 Linux VM 上分别用 cargo-watch 自动重新编译并重启 Edge / Center 后端
-> Windows SSH 隧道把 127.0.0.1:8081 映射到 VM Edge 后端
-> Windows SSH 隧道把 127.0.0.1:8080 映射到 VM Center 后端
-> Windows 本地 edge-web 前端通过 127.0.0.1:8081 联调 Edge 页面
-> Windows 本地 center-web 前端通过 127.0.0.1:8080 联调 Center 页面
```

Windows 本地只需要安装 Mutagen、Git/OpenSSH、Node.js/npm。`cargo-watch` 不装在 Windows，它只装在 Linux VM，因为后端是在 Linux VM 里编译运行。

## 已完成的一次性配置

### 1. Mutagen 已安装

当前使用：

```text
C:\tools\mutagen\mutagen.exe
```

验证：

```powershell
C:\tools\mutagen\mutagen.exe version
```

如果已加入 PATH，也可以直接：

```powershell
mutagen version
```

### 2. Center VM cargo-watch 已安装

Center VM 已安装：

```text
/home/control/.cargo/bin/cargo-watch
```

验证：

```powershell
ssh rustfs-center "bash -lc 'cargo --version && cargo watch --version'"
```

当前验证结果：

```text
cargo 1.97.1
cargo-watch 8.5.3
```

Center VM 的 `~/.profile` 已包含 rustup 默认配置：

```bash
. "$HOME/.cargo/env"
```

因此正常登录后可直接使用 `cargo watch`。

### 3. SSH alias 已配置

`C:\Users\Beipy\.ssh\config` 已配置或应配置为：

```sshconfig
Host rustfs-edge
    HostName 192.168.128.130
    User edge
    IdentityFile C:/Users/Beipy/.ssh/fustfs_vm_codex
    IdentitiesOnly yes

Host rustfs-center
    HostName 192.168.128.131
    User control
    IdentityFile C:/Users/Beipy/.ssh/fustfs_vm_codex
    IdentitiesOnly yes
```

验证：

```powershell
ssh rustfs-edge "hostname && whoami"
ssh rustfs-center "hostname && whoami"
```

期望输出类似：

```text
edge-VMware-Virtual-Platform
edge
control-VMware-Virtual-Platform
control
```

### 4. Mutagen 同步

Edge 同步已成功创建：

```text
Created session sync_pSzsiZS10tJxES4f6BzShfwiTbOmOZs5vJREotVmrVv
```

对应命令是：

```powershell
C:\tools\mutagen\mutagen.exe sync create --name rustfs-edge-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk rustfs-edge:~/RustFSDisk-dev
```

Center 也需要单独创建同步会话。当前已确认 `rustfs-center` SSH alias 可用，但 `rustfs-center-dev` 还需要创建：

```powershell
C:\tools\mutagen\mutagen.exe sync create --name rustfs-center-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk rustfs-center:~/RustFSDisk-dev
```

这里使用 `one-way-safe`：Windows 本地是主源码，Edge VM 和 Center VM 只接收同步，避免 VM 上的构建产物或临时改动反向污染本地源码。

查看同步状态：

```powershell
C:\tools\mutagen\mutagen.exe sync list
```

确认两台 VM 上已经同步到源码：

```powershell
ssh rustfs-edge "ls -la ~/RustFSDisk-dev | head"
ssh rustfs-center "ls -la ~/RustFSDisk-dev | head"
```

两个同步会话都同步整个仓库。Edge 后端使用 `rustfs-edge-dev`，Center 后端使用 `rustfs-center-dev`。

## 每次联调启动流程

### 1. 确认 Mutagen 正常同步

Windows PowerShell：

```powershell
C:\tools\mutagen\mutagen.exe sync list
```

如果看到 `rustfs-edge-dev` 和 `rustfs-center-dev` 都是 Watching for changes，说明本地源码会继续同步到两台 VM。

### 2. 停掉 systemd 部署版后端

开发模式由 `cargo watch` 启动后端，占用 Edge 的 `8081` 和 Center 的 `8080` 端口，所以先停掉旧 systemd 服务：

```powershell
ssh rustfs-edge "sudo systemctl stop rustfs-transfer-edge.service"
ssh rustfs-center "sudo systemctl stop rustfs-transfer-center.service"
```

确认 `8081` 和 `8080` 没有被旧进程占用：

```powershell
ssh rustfs-edge "ss -ltnp | grep ':8081' || true"
ssh rustfs-center "ss -ltnp | grep ':8080' || true"
```

### 3. 在 Linux VM 启动 Edge 开发后端

进入 VM：

```powershell
ssh rustfs-edge
```

在 VM 里执行：

```bash
cd ~/RustFSDisk-dev

export RUSTFS_TRANSFER__CONFIG_PATH=/etc/rustfs-transfer/edge.toml
export RUSTFS_TRANSFER__PATHS__DATA_DIR=/var/lib/rustfs-transfer/edge
export RUSTFS_TRANSFER__PATHS__LOG_DIR=/var/log/rustfs-transfer/edge

cargo watch -w crates -w sql -x "run -p rustfs-transfer-edge --bin rustfs-transfer-edge"
```

这条 `cargo watch` 的意思是：

- 监听 `crates` 和 `sql` 目录。
- 发现文件变化后执行 `cargo run`。
- 运行 `rustfs-transfer-edge` package 下的 `rustfs-transfer-edge` binary。
- 代码保存后会自动重新编译并重启 Edge 后端。

### 4. 在 Center VM 启动 Center 开发后端

另开一个 SSH 窗口进入 Center VM：

```powershell
ssh rustfs-center
```

在 VM 里执行：

```bash
cd ~/RustFSDisk-dev

set -a
. /etc/rustfs-transfer/center.env
set +a

export RUSTFS_TRANSFER__CONFIG_PATH=/etc/rustfs-transfer/center.toml
export RUSTFS_TRANSFER__PATHS__DATA_DIR="$HOME/RustFSDisk-dev/.dev-center/data"
export RUSTFS_TRANSFER__PATHS__LOG_DIR="$HOME/RustFSDisk-dev/.dev-center/log"

cargo watch -w crates -w sql -x "run -p rustfs-transfer-center --bin rustfs-transfer-center"
```

Center 开发态需要先加载 `/etc/rustfs-transfer/center.env`，因为 `center.toml` 只记录密钥环境变量名，真实值在 env 文件里。开发态数据和日志默认写到 `~/RustFSDisk-dev/.dev-center/`，避免 `control` 用户写生产 `/var/lib/rustfs-transfer/center` 和 `/var/log/rustfs-transfer/center`。Center 默认监听 `0.0.0.0:8080`。如果 `/etc/rustfs-transfer/center.toml` 或环境变量里改过 `RUSTFS_TRANSFER__SERVER__BIND`，要保证它仍然监听 `8080`，或同步调整下面的 SSH 隧道和 `center-web` 代理目标。

### 5. 建立 Windows 到 Linux VM 的端口映射

另开 Windows PowerShell 窗口，建立 Edge 隧道：

```powershell
ssh -N -L 127.0.0.1:8081:127.0.0.1:8081 rustfs-edge
```

这个窗口需要保持打开。映射关系是：

```text
Windows 127.0.0.1:8081
-> Edge VM 127.0.0.1:8081
```

再另开一个 Windows PowerShell 窗口，建立 Center 隧道：

```powershell
ssh -N -L 127.0.0.1:8080:127.0.0.1:8080 rustfs-center
```

这个窗口也需要保持打开。映射关系是：

```text
Windows 127.0.0.1:8080
-> Center VM 127.0.0.1:8080
```

验证：

```powershell
curl.exe http://127.0.0.1:8081/api/edge/dashboard/summary
curl.exe http://127.0.0.1:8080/api/center/summary
```

能返回接口响应、`401` 或明确的后端错误，都说明隧道和后端已经连通。连接被拒绝通常表示 VM 后端没跑起来，或者 SSH 隧道窗口没开。

### 6. 启动 Windows 本地 Edge 前端

Windows PowerShell：

```powershell
cd F:\work\RustFSDisk\web\edge-web
npm run dev
```

当前 `web/edge-web/vite.config.ts` 应代理：

- `/api/edge` -> `http://127.0.0.1:8081`
- `/ws/edge` -> `http://127.0.0.1:8081`

正常情况下，浏览器访问 Vite 输出的本地地址即可联调 Edge 页面。

### 7. 启动 Windows 本地 Center 前端

另开一个 Windows PowerShell：

```powershell
cd F:\work\RustFSDisk\web\center-web
npm run dev
```

当前 `web/center-web/vite.config.ts` 应代理：

- `/api/center` -> `http://127.0.0.1:8080`
- `/api/disk` -> `http://127.0.0.1:8080`
- `/api/edge/auth` -> `http://127.0.0.1:8080`
- `/ws/center` -> `http://127.0.0.1:8080`

如需改 Center 后端目标地址，可在 `web/center-web/.env.local` 写：

```text
VITE_CENTER_BACKEND_ORIGIN=http://127.0.0.1:8080
```

正常情况下，浏览器访问 Vite 输出的本地地址即可联调 Center 页面。Edge 和 Center 前端都用 Vite 默认端口时，第二个启动的前端会自动换到下一个可用端口，这是正常现象。

Center 前端调用 Center 后端的 `GET/POST/PUT/DELETE /api/center/edge-sites` 不需要 `X-Center-Control-Token`。`RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN` 只保留给导入、清理、重新初始化等更高风险的 Center 控制接口。联调这些高风险接口时，可在本地 PowerShell 查询当前 VM 中的值，只用于人工操作输入，不要写入仓库：

```powershell
ssh rustfs-center "bash -lc 'set -a; . /etc/rustfs-transfer/center.env; set +a; printf \"%s\n\" \"$RUSTFS_TRANSFER__SERVER__CONTROL_API_TOKEN\"'"
```

## 日常开发循环

```text
1. Windows 本地修改 Rust / SQL / 前端代码
2. Mutagen 自动把源码同步到 Edge VM 和 Center VM
3. 两台 Linux VM 的 Edge / Center cargo-watch 检测到变化
4. cargo-watch 自动重新编译并重启对应后端
5. Windows 本地 edge-web 通过 127.0.0.1:8081 请求 Edge 后端
6. Windows 本地 center-web 通过 127.0.0.1:8080 请求 Center 后端
```

前端代码由本地 Vite 热更新；后端代码由 VM 上的 `cargo-watch` 热重启。Edge 和 Center 后端建议分别放在两个 SSH 窗口里跑，便于单独看日志和重启。

## 常用命令

查看 Mutagen 同步：

```powershell
C:\tools\mutagen\mutagen.exe sync list
```

实时观察 Mutagen 同步：

```powershell
C:\tools\mutagen\mutagen.exe sync monitor rustfs-edge-dev
C:\tools\mutagen\mutagen.exe sync monitor rustfs-center-dev
```

暂停同步：

```powershell
C:\tools\mutagen\mutagen.exe sync pause rustfs-edge-dev
C:\tools\mutagen\mutagen.exe sync pause rustfs-center-dev
```

恢复同步：

```powershell
C:\tools\mutagen\mutagen.exe sync resume rustfs-edge-dev
C:\tools\mutagen\mutagen.exe sync resume rustfs-center-dev
```

删除同步会话：

```powershell
C:\tools\mutagen\mutagen.exe sync terminate rustfs-edge-dev
C:\tools\mutagen\mutagen.exe sync terminate rustfs-center-dev
```

重新创建同步会话：

```powershell
C:\tools\mutagen\mutagen.exe sync create --name rustfs-edge-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk rustfs-edge:~/RustFSDisk-dev
C:\tools\mutagen\mutagen.exe sync create --name rustfs-center-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk rustfs-center:~/RustFSDisk-dev
```

检查 Edge 后端：

```powershell
curl.exe http://127.0.0.1:8081/healthz
curl.exe http://127.0.0.1:8081/api/edge/dashboard/summary
```

检查 Center 后端：

```powershell
curl.exe http://127.0.0.1:8080/healthz
curl.exe http://127.0.0.1:8080/api/center/summary
```

## 已遇到的问题和结论

### mutagen 命令找不到

现象：

```text
mutagen : 无法将“mutagen”项识别为 cmdlet、函数、脚本文件或可运行程序的名称
```

结论：Mutagen 没有安装，或者没有加入 PATH。当前可直接使用完整路径：

```powershell
C:\tools\mutagen\mutagen.exe
```

### Mutagen 直接连 IP 会要求密码

失败命令类似：

```powershell
C:\tools\mutagen\mutagen.exe sync create --name rustfs-edge-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk edge@192.168.128.130:~/RustFSDisk-dev
```

现象：

```text
Connecting to agent (POSIX)...
edge@192.168.128.130's password:
```

结论：普通 SSH 显式带 `-i` 可以免密，但 Mutagen 直接用 `edge@192.168.128.130` 时没有自动使用该私钥。当前不要再用这个写法。

### MUTAGEN_SSH_COMMAND 本次没有解决问题

尝试过：

```powershell
$env:MUTAGEN_SSH_COMMAND='ssh -i C:\Users\Beipy\.ssh\fustfs_vm_codex -o IdentitiesOnly=yes'
```

仍然失败，报错包含：

```text
Permission denied (publickey,password).
```

结论：本次环境下不要依赖 `MUTAGEN_SSH_COMMAND`。实际可用方案是配置 SSH alias，并让 Mutagen 使用 `rustfs-edge:~/RustFSDisk-dev`。

### Edge 最终可用方案

使用 `C:\Users\Beipy\.ssh\config` 中的：

```sshconfig
Host rustfs-edge
    HostName 192.168.128.130
    User edge
    IdentityFile C:/Users/Beipy/.ssh/fustfs_vm_codex
    IdentitiesOnly yes
```

然后 Mutagen 使用：

```powershell
C:\tools\mutagen\mutagen.exe sync create --name rustfs-edge-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk rustfs-edge:~/RustFSDisk-dev
```

该方式已实际创建成功，当前只代表 Edge VM 的 `rustfs-edge-dev` 同步会话可用。Center VM 不共用 Edge 同步会话，必须按下一节创建 `rustfs-center-dev`。

### Center 同步补齐方案

Center VM 使用 `control@192.168.128.131`，应在 `C:\Users\Beipy\.ssh\config` 中配置：

```sshconfig
Host rustfs-center
    HostName 192.168.128.131
    User control
    IdentityFile C:/Users/Beipy/.ssh/fustfs_vm_codex
    IdentitiesOnly yes
```

验证：

```powershell
ssh rustfs-center "hostname && whoami"
```

创建 Center 同步会话：

```powershell
C:\tools\mutagen\mutagen.exe sync create --name rustfs-center-dev --sync-mode one-way-safe --ignore target --ignore node_modules --ignore dist --ignore .local.env F:\work\RustFSDisk rustfs-center:~/RustFSDisk-dev
```

创建后用以下命令确认 `rustfs-center-dev` 进入 `Watching for changes`：

```powershell
C:\tools\mutagen\mutagen.exe sync list
```

## Center 端补齐要点

Center 端联调至少需要四件事同时成立：

1. VM 上存在 `/etc/rustfs-transfer/center.toml`，或通过环境变量提供 Center 后端需要的数据库、RustFS、密钥和路径配置。
2. VM 上按“在 Center VM 启动 Center 开发后端”一节先加载 `/etc/rustfs-transfer/center.env`，再用 `cargo watch -w crates -w sql -x "run -p rustfs-transfer-center --bin rustfs-transfer-center"` 启动 Center 后端。
3. Windows 上保持 `ssh -N -L 127.0.0.1:8080:127.0.0.1:8080 rustfs-center` 隧道窗口打开。
4. Windows 本地 `web/center-web/vite.config.ts` 代理 `/api/center`、`/api/disk`、`/api/edge/auth`、`/ws/center` 到 `http://127.0.0.1:8080`。

如果 Center 页面显示 HTTP summary 不可用，优先按顺序检查：Center 后端窗口是否正在运行、`8080` SSH 隧道是否打开、`curl.exe http://127.0.0.1:8080/healthz` 是否有响应、`center.toml` 的数据库和 RustFS 配置是否可用。
