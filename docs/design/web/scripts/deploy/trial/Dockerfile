# DevelopmentOnly / isolated-trial frontend image. This file intentionally
# differs from the legacy deploy Dockerfile: web-antd currently emits build
# output under the repository build directory, not playground/dist.
FROM node:22-slim AS builder

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
ENV NODE_OPTIONS="--max-old-space-size=8192"
ENV CI=true

WORKDIR /app

# The Docker build context is src/web. Lockfile installation keeps the image
# reproducible; building/running the image is an operator-authorized action.
COPY . /app
RUN corepack enable && pnpm install --frozen-lockfile

# Use an explicit in-image output directory. The normal local configuration
# writes to the repository-level build/web path, which is outside this image
# worktree when the Vite config is evaluated in a container.
RUN pnpm --filter=@fustfs/web exec vite build \
    --configLoader runner \
    --mode production \
    --outDir /app/trial-dist

FROM nginx:stable-alpine AS trial

# The official nginx entrypoint renders templates using only environment
# variables. No credential is copied into the image or template.
RUN rm -f /etc/nginx/conf.d/default.conf
COPY --from=builder /app/trial-dist /usr/share/nginx/html
COPY scripts/deploy/trial/default.conf.template /etc/nginx/templates/default.conf.template
COPY scripts/deploy/trial/10-validate-isolated-trial-env.sh /docker-entrypoint.d/10-validate-isolated-trial-env.sh
RUN chmod 755 /docker-entrypoint.d/10-validate-isolated-trial-env.sh

ENV FUSTFS_LOCAL_API_UPSTREAM=http://127.0.0.1:18481
ENV FUSTFS_WEB_LISTEN_PORT=8080

EXPOSE 8080
