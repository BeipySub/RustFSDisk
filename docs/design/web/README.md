# FustFsDisk Web

产品 Web 基线采用 Vue 3、Vite 和 Vben Admin 5.7.0 的 `web-antd` 工程。

- 上游版本：`v5.7.0`
- 上游提交：`63a38dce49ba109f61607994e21ba921d8e970e9`
- 上游仓库：<https://github.com/vbenjs/vue-vben-admin>
- 许可证：MIT，副本见 `vendor-licenses/Vben-Admin-MIT.txt`

本目录只保留 `web-antd`、其 workspace 包和构建工具。上游其他 UI 应用、mock 后端、文档站和 playground 未导入。I4 已按冻结基线实现 A-01/A-02、B-02/B-03；B-01、B-04～B-08 已形成冻结基线视觉结构，使用明确标记的固定视觉夹具且不调用虚构 API。B-08 管理员路由在可信 `CONTROL_ADMIN` 身份合同落地前保持失败关闭，不能据此宣称生产数据接入或管理员功能完成。

I4 浏览器验证：

```powershell
pnpm exec playwright test -c playwright.config.ts
```
