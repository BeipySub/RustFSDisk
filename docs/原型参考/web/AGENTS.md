# Web 工程协作规范

本目录继承 `src/AGENTS.md`。

- 技术基线：TypeScript、Vue 3、Vite、Vben Admin 5.7.0 `web-antd`。
- 上游 Vben 仅提供工程、布局和组件能力；页面、导航、字段、状态、权限和危险操作必须来自冻结原型与需求。
- 禁止恢复上游 mock API、演示账号、演示仪表盘或外部生产 API 地址。
- API 根路径固定从配置读取，正式接口必须由 OpenAPI 生成 client。
- Node.js 和 pnpm 仅用于构建，不进入产品运行时。
- 修改后至少执行 `corepack pnpm run check:type`、`corepack pnpm run lint`、`corepack pnpm run test:unit` 和 `corepack pnpm run build:antd`。
