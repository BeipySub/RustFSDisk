# 隔离试运行前端候选包

本目录只提供 `web-antd` 的隔离试运行镜像候选物。它不包含 Agent、Control、PostgreSQL、RustFS、运输介质内容、账号、密码、令牌或密钥。

- `Dockerfile`：从 `src/web` 构建 `@fustfs/web`，并复制显式的容器内构建产物；不再引用已不适用的 `playground/dist`。
- `default.conf.template`：将同源 `/api/` 转发到显式配置的本机服务，后端不可用时把错误返回浏览器。
- `build-image.sh`：只构建镜像，不启动、停止、删除任何容器。

在 Linux 隔离试运行前，必须先获得负责人对 Docker 构建、启动以及任何现有服务操作的单独确认。具体步骤见仓库部署手册。
