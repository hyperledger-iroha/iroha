---
id: security-hardening
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Security hardening & pen-test checklist
sidebar_label: Security hardening
description: Harden the developer portal before exposing the Try it sandbox outside the lab.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## 概述

路线图项目 **DOCS-1b** 需要 OAuth 设备代码登录，内容丰富
安全策略以及预览门户之前的可重复渗透测试
可以在非实验室网络上运行。本附录解释了威胁模型、
存储库中实施的控制措施以及门禁审查的上线清单
必须执行。

- **范围：** Try it 代理、嵌入式 Swagger/RapiDoc 面板和自定义
  尝试使用 `docs/portal/src/components/TryItConsole.jsx` 渲染的控制台。
- **超出范围：** Torii 本身（由 Torii 准备情况审查涵盖）和 SoraFS
  出版（由 DOCS-3/7 涵盖）。

## 威胁模型

|资产|风险|缓解措施 |
| ---| ---| ---|
| Torii 不记名代币 |在文档沙箱之外被盗或重复使用 |设备代码登录 (`DOCS_OAUTH_*`) 会生成短期令牌，代理会编辑标头，控制台会自动使缓存的凭据过期。 |
|尝试一下代理 |滥用作为开放中继或绕过 Torii 速率限制 | `scripts/tryit-proxy*.mjs` 强制执行来源允许列表、速率限制、运行状况探测和显式 `X-TryIt-Auth` 转发；没有保留任何凭证。 |
|门户运行时 |跨站点脚本或恶意嵌入| `docusaurus.config.js` 注入 Content-Security-Policy、Trusted Types 和 Permissions-Policy 标头；内联脚本仅限于 Docusaurus 运行时。 |
|可观测数据|遥测丢失或篡改| `docs/portal/docs/devportal/observability.md` 记录探针/仪表板； `scripts/portal-probe.mjs` 在发布之前在 CI 中运行。 |

对手包括查看公共预览版的好奇用户、恶意行为者
测试被盗链接，以及尝试抓取存储的受感染浏览器
凭据。所有控件都必须在不受信任的商品浏览器上运行
网络。

## 所需的控制

1. **OAuth设备码登录**
   - 配置 `DOCS_OAUTH_DEVICE_CODE_URL`、`DOCS_OAUTH_TOKEN_URL`、
     `DOCS_OAUTH_CLIENT_ID`，以及构建环境中的相关旋钮。
   - Try it 卡呈现一个登录小部件 (`OAuthDeviceLogin.jsx`)
     获取设备代码、轮询令牌端点并自动清除令牌
     一旦过期。紧急情况下仍可使用手动承载超控
     后备。
   - 当 OAuth 配置丢失或
     后备 TTL 漂移到 DOCS-1b 规定的 300 秒至 900 秒窗口之外；
     仅针对一次性本地预览设置 `DOCS_OAUTH_ALLOW_INSECURE=1`。
2. **代理护栏**
   - `scripts/tryit-proxy.mjs` 强制执行允许的来源、速率限制、请求
     大小上限和上游超时，同时标记流量
     `X-TryIt-Client` 并从日志中编辑令牌。
   - `scripts/tryit-proxy-probe.mjs` 加上 `docs/portal/docs/devportal/observability.md`
     定义活性探针和仪表板规则；在每个之前运行它们
     推出。
3. **CSP、可信类型、权限策略**
   - `docusaurus.config.js` 现在导出确定性安全标头：
     `Content-Security-Policy`（默认-src self，严格connect/img/script
     列表、可信类型要求）、`Permissions-Policy` 和
     `Referrer-Policy: no-referrer`。
   - CSP 连接列表将 OAuth 设备代码和令牌端点列入白名单
     （仅限 HTTPS，除非 `DOCS_SECURITY_ALLOW_INSECURE=1`）因此设备登录有效
     而不放宽其他来源的沙箱。
   - 标头直接嵌入生成的 HTML 中，因此静态主机可以这样做
     不需要额外的配置。将内联脚本限制为
     Docusaurus 引导程序。
4. **运行手册、可观察性和回滚**
   - `docs/portal/docs/devportal/observability.md` 描述了探头和
     用于监视登录失败、代理响应代码和请求的仪表板
     预算。
   - `docs/portal/docs/devportal/incident-runbooks.md` 涵盖升级
     沙箱被滥用时的路径；将其与
     `scripts/tryit-proxy-rollback.mjs` 安全翻转端点。

## 渗透测试和发布清单

为每个预览促销完成此列表（将结果附加到版本中）
票）：

1. **验证 OAuth 接线**
   - 使用生产 `DOCS_OAUTH_*` 导出在本地运行 `npm run start`。
   - 从干净的浏览器配置文件中，打开“尝试”控制台并确认
     设备代码流铸造一个令牌，倒计时生命周期，并清除
     过期或注销后的字段。
2. **探测代理**
   - `npm run tryit-proxy` 针对暂存 Torii，然后执行
     `npm run probe:tryit-proxy` 以及已配置的示例路径。
   - 检查日志中的 `authSource=override` 条目并确认速率限制
     当超出窗口时增加计数器。
3. **确认 CSP/可信类型**
   - `npm run build` 并打开 `build/index.html`。确保`<元
     http-equiv="Content-Security-Policy">` 标记与预期指令匹配
     并且 DevTools 在加载预览时显示没有 CSP 违规。
   - 使用`npm run probe:portal`（或curl）获取部署的HTML；探头
     现在，当 `Content-Security-Policy`、`Permissions-Policy` 或
     `Referrer-Policy` 元标记丢失或与声明的值不同
     在 `docusaurus.config.js` 中，因此治理审核者可以依赖退出
     代码而不是盯着curl 输出。
4. **审查可观察性**
   - 验证 Try it 代理仪表板是否为绿色（速率限制、错误率、
     运行状况探测指标）。
   - 在 `docs/portal/docs/devportal/incident-runbooks.md` 中运行事件演习
     如果主机发生更改（新的 Netlify/SoraFS 部署）。
5. **记录结果**
   - 将屏幕截图/日志附加到发布票证中。
   - 在补救报告模板中记录每项发现
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     因此所有者、SLA 和重新测试证据很容易在以后进行审核。
   - 链接回此清单，以便 DOCS-1b 路线图项目保持可审核状态。

如果任何步骤失败，请停止升级，提交阻止问题，并记下
`status.md` 中的修复计划。