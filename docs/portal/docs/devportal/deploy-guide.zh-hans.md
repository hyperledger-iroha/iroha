---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ee0a1d26e0c9f3c1ab8908f7eb0dc73049d452e451aff9d2d892c19d733557e
source_last_modified: "2026-01-22T16:26:46.492940+00:00"
translation_last_reviewed: 2026-02-07
id: deploy-guide
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
---

## 概述

此剧本转换路线图项目 **DOCS-7** （SoraFS 发布）和 **DOCS-8**
（CI/CD 引脚自动化）转化为开发人员门户的可操作程序。
它涵盖了构建/lint 阶段、SoraFS 打包、Sigstore 支持的清单
签名、别名升级、验证和回滚演练，以便每次预览和
发布工件是可重现和可审计的。

该流程假设您有 `sorafs_cli` 二进制文件（使用
`--features cli`)，使用 PIN 注册权限访问 Torii 端点，以及
OIDC Sigstore 的凭据。存储长期秘密（`IROHA_PRIVATE_KEY`，
`SIGSTORE_ID_TOKEN`、Torii 代币）在您的 CI 金库中；本地运行可以获取它们
来自壳出口。

## 先决条件

- 节点 18.18+ 具有 `npm` 或 `pnpm`。
- `sorafs_cli` 来自 `cargo run -p sorafs_car --features cli --bin sorafs_cli`。
- Torii 公开 `/v2/sorafs/*` 的 URL 以及授权帐户/私钥
  可以提交清单和别名。
- OIDC 发行者（GitHub Actions、GitLab、工作负载身份等）来铸造
  `SIGSTORE_ID_TOKEN`。
- 可选：`examples/sorafs_cli_quickstart.sh` 用于试运行和
  `docs/source/sorafs_ci_templates.md` 用于 GitHub/GitLab 工作流程脚手架。
- 配置 Try it OAuth 变量 (`DOCS_OAUTH_*`) 并运行
  升级构建之前的[安全强化清单](./security-hardening.md)
  实验室外。当这些变量丢失时，门户构建现在会失败
  或者当 TTL/轮询旋钮落在强制窗口之外时；出口
  `DOCS_OAUTH_ALLOW_INSECURE=1` 仅适用于一次性本地预览。附上
  释放票的渗透测试证据。

## 步骤 0 — 捕获 Try it 代理包

在将预览版推广到 Netlify 或网关之前，请标记 Try it 代理
源并签名 OpenAPI 清单摘要到确定性捆绑包中：

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` 复制代理/探针/回滚助手，
验证 OpenAPI 签名，并写入 `release.json` plus
`checksums.sha256`。将此捆绑包附加到 Netlify/SoraFS 网关促销
票证，以便审阅者可以重播确切的代理源和 Torii 目标提示
无需重建。该捆绑包还记录客户端提供的承载是否
启用 (`allow_client_auth`) 以保持部署计划和 CSP 规则同步。

## 步骤 1 — 构建并检查门户

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` 自动执行 `scripts/write-checksums.mjs`，产生：

- `build/checksums.sha256` — SHA256 清单适用于 `sha256sum -c`。
- `build/release.json` — 元数据（`tag`、`generated_at`、`source`）固定到
  每辆车/舱单。

将这两个文件与 CAR 摘要一起存档，以便审阅者可以比较预览
文物无需重建。

## 步骤 2 — 打包静态资源

针对 Docusaurus 输出目录运行 CAR 加壳程序。下面的例子
将所有工件写入 `artifacts/devportal/` 下。

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

摘要 JSON 捕获块计数、摘要和证明计划提示
`manifest build` 和 CI 仪表板稍后重用。

## 步骤 2b — 软件包 OpenAPI 和 SBOM 同伴

DOCS-7 需要发布门户网站、OpenAPI 快照和 SBOM 有效负载
作为不同的清单，因此网关可以装订 `Sora-Proof`/`Sora-Content-CID`
每个工件的标题。发布助手
(`scripts/sorafs-pin-release.sh`)已经打包了OpenAPI目录
(`static/openapi/`) 和通过 `syft` 发出的 SBOM 到单独的
`openapi.*`/`*-sbom.*` CAR 并将元数据记录在
`artifacts/sorafs/portal.additional_assets.json`。运行手动流程时，
对每个具有自己的前缀和元数据标签的有效负载重复步骤 2-4
（例如 `--car-out "$OUT"/openapi.car` 加
`--metadata alias_label=docs.sora.link/openapi`）。注册每个清单/别名
在切换 DNS 之前在 Torii（站点、OpenAPI、门户 SBOM、OpenAPI SBOM）中进行配对
该网关可以为所有已发布的文物提供装订证据。

## 步骤 3 — 构建清单

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

将 pin-policy 标志调整到您的发布窗口（例如，`--pin-storage-class
对于金丝雀来说是热的）。 JSON 变体是可选的，但便于代码审查。

## 步骤 4 — 使用 Sigstore 签名

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

该包记录了清单摘要、块摘要和 BLAKE3 哈希值
OIDC 令牌，无需保留 JWT。保持捆绑和分离
签名；生产晋升可以重复使用相同的文物而不是辞职。
本地运行可以用 `--identity-token-env` 替换提供程序标志（或设置
`SIGSTORE_ID_TOKEN` 环境中）当外部 OIDC 帮助程序发出
令牌。

## 步骤 5 — 提交至 PIN 注册表

将签名的清单（和块计划）提交到 Torii。始终要求提供摘要
因此生成的注册表项/别名证明是可审计的。

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

推出预览或金丝雀别名 (`docs-preview.sora`) 时，请重复
使用唯一别名提交，以便 QA 可以在生产前验证内容
促销。

别名绑定需要三个字段：`--alias-namespace`、`--alias-name` 和
`--alias-proof`。治理生成证明包（base64 或 Norito 字节）
别名请求何时获得批准；将其存储在 CI 机密中并将其显示为
调用 `manifest submit` 之前的文件。当您
只想固定清单而不触及 DNS。

## 步骤 5b — 生成治理提案

每份清单都应附带议会准备好的提案，以便任何 Sora
公民可以在不借用特权凭证的情况下引入变革。
完成提交/签名步骤后，运行：

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` 捕获规范 `RegisterPinManifest`
指令、块摘要、策略和别名提示。将其附加到治理中
票证或议会门户，以便代表们可以在不重建的情况下区分有效负载
文物。因为该命令从未触及 Torii 权限密钥，所以任何
公民可以在当地起草提案。

## 步骤 6 — 验证证明和遥测

固定后，运行确定性验证步骤：

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- 检查 `torii_sorafs_gateway_refusals_total` 和
  `torii_sorafs_replication_sla_total{outcome="missed"}` 异常。
- 运行 `npm run probe:portal` 来练习 Try-It 代理和记录的链接
  针对新固定的内容。
- 捕获中描述的监控证据
  [发布和监控](./publishing-monitoring.md) 所以 DOCS-3c 的
  可观察性门与发布步骤一起得到满足。帮手
  现在接受多个 `bindings` 条目（站点、OpenAPI、门户 SBOM、OpenAPI
  SBOM）并在目标上强制执行 `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  通过可选的 `hostname` 防护主机。下面的调用写了两个
  单个 JSON 摘要和证据包（`portal.json`、`tryit.json`、
  `binding.json`和`checksums.sha256`）在发布目录下：

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## 步骤 6a — 规划网关证书

在创建 GAR 数据包之前导出 TLS SAN/挑战计划，以便网关
团队和 DNS 审批者审查相同的证据。新助手反映了
DG-3 通过枚举规范通配符主机进行自动化输入，
漂亮主机 SAN、DNS-01 标签和推荐的 ACME 挑战：

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

将 JSON 与发布包一起提交（或将其与更改一起上传）
票证），以便操作员可以将 SAN 值粘贴到 Torii 中
`torii.sorafs_gateway.acme` 配置和 GAR 审阅者可以确认
规范/漂亮的映射，无需重新运行主机派生。添加额外的
`--name` 在同一版本中升级的每个后缀的参数。

## 步骤 6b — 导出规范主机映射

在模板化 GAR 有效负载之前，记录每个的确定性主机映射
别名。 `cargo xtask soradns-hosts` 将每个 `--name` 散列到其规范中
标签 (`<base32>.gw.sora.id`)，发出所需的通配符
(`*.gw.sora.id`)，并派生出漂亮的主机(`<alias>.gw.sora.name`)。坚持
发布工件中的输出，以便 DG-3 审阅者可以区分映射
与 GAR 提交一起：

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

每当 GAR 或网关出现故障时，使用 `--verify-host-patterns <file>` 快速失败
绑定 JSON 省略了所需的主机之一。助手接受多个
验证文件，可以轻松检查 GAR 模板和
在同一调用中装订 `portal.gateway.binding.json`：

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

将摘要 JSON 和验证日志附加到 DNS/网关更改票证中，以便
审核员无需重新运行即可确认规范、通配符和漂亮主机
推导脚本。每当添加新别名时重新运行该命令
捆绑，以便后续 GAR 更新继承相同的证据轨迹。

## 步骤 7 — 生成 DNS 切换描述符

生产切换需要可审核的变更包。成功后
提交（别名绑定），助手发出
`artifacts/sorafs/portal.dns-cutover.json`，捕获：- 别名绑定元数据（命名空间/名称/证明、清单摘要、Torii URL、
  提交纪元、权威）；
- 发布上下文（标签、别名标签、清单/CAR 路径、块计划、Sigstore
  捆绑）；
- 验证指针（探测命令，别名+ Torii端点）；和
- 可选的变更控制字段（票证 ID、切换窗口、操作联系人、
  生产主机名/区域）；
- 源自装订的 `Sora-Route-Binding` 的路线促销元数据
  标头（规范主机/CID、标头+绑定路径、验证命令）、
  确保 GAR 升级和后备演习参考相同的证据；
- 生成的路线规划工件（`gateway.route_plan.json`，
  标头模板和可选的回滚标头），因此更改票证和 CI
  lint hooks 可以验证每个 DG-3 数据包都引用了规范
  批准前的升级/回滚计划；
- 可选的缓存失效元数据（清除端点、身份验证变量、JSON
  有效负载，以及示例 `curl` 命令）；和
- 回滚提示指向先前的描述符（发布标签和清单）
  摘要），因此更改票据捕获确定性后备路径。

当版本需要缓存清除时，生成规范计划以及
切换描述符：

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

将生成的 `portal.cache_plan.json` 附加到 DG-3 数据包，以便运营商
发出时具有确定性的主机/路径（以及匹配的身份验证提示）
`PURGE` 请求。描述符的可选缓存元数据部分可以参考
直接此文件，使变更控制审阅者准确地保持一致
端点在切换期间被刷新。

每个 DG-3 数据包还需要一个升级 + 回滚清单。通过生成它
`cargo xtask soradns-route-plan`，以便变更控制审核人员可以追踪准确的
每个别名的预检、切换和回滚步骤：

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

发出的 `gateway.route_plan.json` 捕获规范/漂亮的主机，上演
健康检查提醒、GAR 绑定更新、缓存清除和回滚操作。
在提交更改之前将其与 GAR/绑定/转换工件捆绑在一起
票，以便运营人员可以按照相同的脚本步骤进行排练和签署。

`scripts/generate-dns-cutover-plan.mjs` 为该描述符提供动力并运行
自动从 `sorafs-pin-release.sh` 开始。重新生成或自定义它
手动：

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

在运行 pin 之前通过环境变量填充可选元数据
帮手：

|变量|目的|
|----------|---------|
| `DNS_CHANGE_TICKET` |票证 ID 存储在描述符中。 |
| `DNS_CUTOVER_WINDOW` | ISO8601 转换窗口（例如 `2026-03-21T15:00Z/2026-03-21T15:30Z`）。 |
| `DNS_HOSTNAME`，`DNS_ZONE` |生产主机名+权威区域。 |
| `DNS_OPS_CONTACT` |待命别​​名或升级联系人。 |
| `DNS_CACHE_PURGE_ENDPOINT` |缓存清除端点记录在描述符中。 |
| `DNS_CACHE_PURGE_AUTH_ENV` |包含清除令牌的环境变量（默认为 `CACHE_PURGE_TOKEN`）。 |
| `DNS_PREVIOUS_PLAN` |回滚元数据的先前切换描述符的路径。 |

将 JSON 附加到 DNS 更改审核，以便审批者可以验证清单
摘要、别名绑定和探测命令，无需抓取 CI 日志。
CLI 标志 `--dns-change-ticket`、`--dns-cutover-window`、`--dns-hostname`、
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` 和 `--previous-dns-plan` 提供相同的覆盖
在 CI 外部运行助手时。

## 步骤 8 — 发出解析器区域文件框架（可选）

当生产切换窗口已知时，发布脚本可以发出
自动 SNS 区域文件框架和解析器片段。传递所需的 DNS
通过环境变量或 CLI 选项记录和元数据；帮手
切换后会立即调用 `scripts/sns_zonefile_skeleton.py`
生成描述符。提供至少一个 A/AAAA/CNAME 值和 GAR
摘要（签名的 GAR 有效负载的 BLAKE3-256）。如果区域/主机名已知
并且 `--dns-zonefile-out` 被省略，帮助程序写入
`artifacts/sns/zonefiles/<zone>/<hostname>.json` 并填充
`ops/soradns/static_zones.<hostname>.json` 作为解析器片段。

|变量/标志|目的|
|----------------|---------|
| `DNS_ZONEFILE_OUT`、`--dns-zonefile-out` |生成的区域文件框架的路径。 |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`，`--dns-zonefile-resolver-snippet` |解析器代码片段路径（省略时默认为 `ops/soradns/static_zones.<hostname>.json`）。 |
| `DNS_ZONEFILE_TTL`，`--dns-zonefile-ttl` |应用于生成记录的 TTL（默认值：600 秒）。 |
| `DNS_ZONEFILE_IPV4`、`--dns-zonefile-ipv4` | IPv4 地址（逗号分隔的 env 或可重复的 CLI 标志）。 |
| `DNS_ZONEFILE_IPV6`、`--dns-zonefile-ipv6` | IPv6 地址。 |
| `DNS_ZONEFILE_CNAME`、`--dns-zonefile-cname` |可选的 CNAME 目标。 |
| `DNS_ZONEFILE_SPKI`、`--dns-zonefile-spki-pin` | SHA-256 SPKI 引脚 (base64)。 |
| `DNS_ZONEFILE_TXT`，`--dns-zonefile-txt` |其他 TXT 条目 (`key=value`)。 |
| `DNS_ZONEFILE_VERSION`、`--dns-zonefile-version` |覆盖计算出的区域文件版本标签。 |
| `DNS_ZONEFILE_EFFECTIVE_AT`、`--dns-zonefile-effective-at` |强制使用 `effective_at` 时间戳 (RFC3339)，而不是切换窗口启动。 |
| `DNS_ZONEFILE_PROOF`、`--dns-zonefile-proof` |覆盖元数据中记录的证明文字。 |
| `DNS_ZONEFILE_CID`，`--dns-zonefile-cid` |覆盖元数据中记录的 CID。 |
| `DNS_ZONEFILE_FREEZE_STATE`、`--dns-zonefile-freeze-state` |守护冻结状态（软、硬、解冻、监控、紧急）。 |
| `DNS_ZONEFILE_FREEZE_TICKET`，`--dns-zonefile-freeze-ticket` |监护人/议会冻结票证参考。 |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`，`--dns-zonefile-freeze-expires-at` |用于解冻的 RFC3339 时间戳。 |
| `DNS_ZONEFILE_FREEZE_NOTES`，`--dns-zonefile-freeze-note` |附加冻结注释（逗号分隔的 env 或可重复标志）。 |
| `DNS_GAR_DIGEST`，`--dns-gar-digest` |签名 GAR 有效负载的 BLAKE3-256 摘要（十六进制）。只要存在网关绑定就需要。 |

GitHub Actions 工作流程从存储库机密中读取这些值，因此每个生产 pin 都会自动发出区域文件工件。配置以下机密（字符串可能包含多值字段的逗号分隔列表）：

|秘密|目的|
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`，`DOCS_SORAFS_DNS_ZONE` |传递给帮助程序的生产主机名/区域。 |
| `DOCS_SORAFS_DNS_OPS_CONTACT` |存储在描述符中的待命别名。 |
| `DOCS_SORAFS_ZONEFILE_IPV4`，`DOCS_SORAFS_ZONEFILE_IPV6` |要发布的 IPv4/IPv6 记录。 |
| `DOCS_SORAFS_ZONEFILE_CNAME` |可选的 CNAME 目标。 |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI 引脚。 |
| `DOCS_SORAFS_ZONEFILE_TXT` |其他 TXT 条目。 |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` |冻结骨架中记录的元数据。 |
| `DOCS_SORAFS_GAR_DIGEST` |已签名 GAR 有效负载的十六进制编码的 BLAKE3 摘要。 |

触发 `.github/workflows/docs-portal-sorafs-pin.yml` 时，提供 `dns_change_ticket` 和 `dns_cutover_window` 输入，以便描述符/区域文件继承正确的更改窗口元数据。仅在进行空运行时才将它们留空。

典型调用（与 SN-7 所有者运行手册匹配）：

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

助手会自动将变更单作为 TXT 条目转入并
继承切换窗口开始作为 `effective_at` 时间戳，除非
被覆盖。有关完整的操作流程，请参阅
`docs/source/sorafs_gateway_dns_owner_runbook.md`。

### 公共 DNS 委托说明

区域文件框架仅定义区域的权威记录。你
仍然需要在您的注册商或 DNS 处配置父区域 NS/DS 委派
提供商，以便常规互联网可以发现名称服务器。

- 对于 apex/TLD 转换，请使用 ALIAS/ANAME（特定于提供商）或发布 A/AAAA
  指向网关任播 IP 的记录。
- 对于子域，将 CNAME 发布到派生的漂亮主机
  （`<fqdn>.gw.sora.name`）。
- 规范主机 (`<hash>.gw.sora.id`) 位于网关域下并且
  未在您的公共区域内发布。

### 网关标头模板

部署助手还会发出 `portal.gateway.headers.txt` 和
`portal.gateway.binding.json`，满足 DG-3 要求的两件文物
网关内容绑定要求：

- `portal.gateway.headers.txt` 包含完整的 HTTP 标头块（包括
  `Sora-Name`、`Sora-Content-CID`、`Sora-Proof`、CSP、HSTS 和
  `Sora-Route-Binding` 描述符），边缘网关必须钉在每个
  回应。
- `portal.gateway.binding.json`以机器可读的方式记录相同的信息
  表单，以便更改票证和自动化可以区分主机/cid 绑定，而无需
  抓取 shell 输出。

它们是通过自动生成的
`cargo xtask soradns-binding-template`
并捕获提供的别名、清单摘要和网关主机名
至 `sorafs-pin-release.sh`。要重新生成或自定义标头块，请运行：

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

通过 `--csp-template`、`--permissions-template` 或 `--hsts-template` 进行覆盖
当特定部署需要额外的默认标头模板
指令；将它们与现有的 `--no-*` 开关组合以删除标头
完全。

将标头片段附加到 CDN 更改请求并提供 JSON 文档
进入网关自动化管道，以便实际主机升级与
公布证据。

发布脚本自动运行验证助手，因此 DG-3 门票
始终包括最近的证据。每当您调整时，请手动重新运行它
手动绑定 JSON：

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

该命令验证绑定包中捕获的 `Sora-Proof` 有效负载，
确保 `Sora-Route-Binding` 元数据与清单 CID + 主机名匹配，
如果任何标头漂移，则会快速失败。将控制台输出存档在
每当您在 CI 外部运行命令时都会产生其他部署工件，因此 DG-3
审阅者有证据表明绑定在切换之前已得到验证。> **DNS 描述符集成：** `portal.dns-cutover.json` 现在嵌入了
> `gateway_binding` 部分指向这些工件（路径、内容 CID、
> 证明状态和文字标头模板）**和** `route_plan` 节
> 引用 `gateway.route_plan.json` 加上主 + 回滚标头
> 模板。将这些块包含在每个 DG-3 变更单中，以便审核者可以
> 比较确切的 `Sora-Name/Sora-Proof/CSP` 值并确认该路由
> 升级/回滚计划与证据包匹配，无需打开构建
> 存档。

## 步骤 9 — 运行发布监视器

路线图任务 **DOCS-3c** 需要持续证据表明门户，尝试一下
代理和网关绑定在发布后保持健康。运行综合
在步骤 7-8 之后立即进行监视并将其连接到您预定的探测器中：

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` 加载配置文件（参见
  `docs/portal/docs/devportal/publishing-monitoring.md` 的架构）和
  执行三项检查：门户路径探测+CSP/权限策略验证，
  尝试代理探针（可以选择访问其 `/metrics` 端点），然后
  网关绑定验证程序 (`cargo xtask soradns-verify-binding`) 检查
  根据预期的别名、主机、证明状态捕获的绑定包，
  并显示 JSON。
- 每当任何探测失败时，该命令都会以非零值退出，因此 CI、cron 作业或
  Runbook 操作员可以在升级别名之前停止发布。
- 传递 `--json-out` 会为每个目标写入一个摘要 JSON 有效负载
  状态； `--evidence-dir` 发出 `summary.json`、`portal.json`、`tryit.json`、
  `binding.json` 和 `checksums.sha256`，以便治理审核者可以区分
  结果无需重新运行监视器。将此目录存档于
  `artifacts/sorafs/<tag>/monitoring/` 以及 Sigstore 捆绑包和 DNS
  切换描述符。
- 包括监视器输出、Grafana 导出 (`dashboards/grafana/docs_portal.json`)、
  和 Alertmanager 钻取发布单中的 ID，以便 DOCS-3c SLO 可以
  后来审核了。专用的发布监控手册位于
  `docs/portal/docs/devportal/publishing-monitoring.md`。

门户探测需要 HTTPS 并拒绝 `http://` 基本 URL，除非
`allowInsecureHttp` 在监视器配置中设置；保持制作/登台
TLS 上的目标，并且仅启用本地预览的覆盖。

通过 Buildkite/cron 中的 `npm run monitor:publishing` 自动化监控
门户网站已上线。指向生产 URL 的同一命令提供正在进行的
SRE/Docs 在版本之间依赖的运行状况检查。

## 使用 `sorafs-pin-release.sh` 实现自动化

`docs/portal/scripts/sorafs-pin-release.sh` 封装了步骤 2-6。它：

1. 将 `build/` 归档到确定性 tarball 中，
2. 运行 `car pack`、`manifest build`、`manifest sign`、`manifest verify-signature`、
   和 `proof verify`，
3.当Torii时可选地执行`manifest submit`（包括别名绑定）
   存在凭据，并且
4.写入`artifacts/sorafs/portal.pin.report.json`，可选
  `portal.pin.proposal.json`，DNS 切换描述符（提交后），
  和网关绑定捆绑包（`portal.gateway.binding.json` 加上
  文本标题块），以便治理、网络和运营团队可以区分
  证据捆绑，无需抓取 CI 日志。

设置 `PIN_ALIAS`、`PIN_ALIAS_NAMESPACE`、`PIN_ALIAS_NAME` 和（可选）
`PIN_ALIAS_PROOF_PATH` 在调用脚本之前。使用 `--skip-submit` 进行干燥
运行；下面描述的 GitHub 工作流程通过 `perform_submit` 切换此功能
输入。

## 步骤 8 — 发布 OpenAPI 规格和 SBOM 捆绑包

DOCS-7 需要门户构建、OpenAPI 规范和 SBOM 工件才能传输
通过相同的确定性管道。现有的助手涵盖了所有三个：

1. **重新生成并签署规范。**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   每当您想要保留某个版本时，请通过 `--version=<label>` 传递发布标签
   历史快照（例如 `2025-q3`）。助手写入快照
   到 `static/openapi/versions/<label>/torii.json`，将其镜像到
   `versions/current`，并记录元数据（SHA-256、清单状态和
   更新的时间戳）在 `static/openapi/versions.json` 中。开发者门户
   读取该索引，以便 Swagger/RapiDoc 面板可以显示版本选择器
   并内联显示相关的摘要/签名信息。省略
   `--version` 保持之前的版本标签不变，仅刷新
   `current` + `latest` 指针。

   清单捕获 SHA-256/BLAKE3 摘要，以便网关可以装订
   `Sora-Proof` `/reference/torii-swagger` 标头。

2. **发出 CycloneDX SBOM。** 发布管道已经期望基于 syft
   根据 `docs/source/sorafs_release_pipeline_plan.md` 的 SBOM。保留输出
   构建工件旁边：

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **将每个有效负载打包到 CAR 中。**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   按照与主站点相同的 `manifest build` / `manifest sign` 步骤，
   调整每个资产的别名（例如，规范的 `docs-openapi.sora` 和
   `docs-sbom.sora` 用于签名的 SBOM 捆绑包）。维护不同的别名
   将 SoraDNS 证明、GAR 和回滚票证的范围限制在确切的有效负载范围内。

4. **提交并绑定。** 复用现有权限+Sigstore捆绑，但是
   在发布清单中记录别名元组，以便审核员可以跟踪哪些别名元组
   Sora 名称映射到哪个清单摘要。

将规范/SBOM 清单与门户构建一起存档可确保每个
发行票包含完整的工件集，无需重新运行加壳器。

### 自动化助手（CI/包脚本）

`./ci/package_docs_portal_sorafs.sh` 编码了步骤 1-8，因此路线图项目
**DOCS-7** 可以使用单个命令来执行。帮手：

- 运行所需的门户准备（`npm ci`、OpenAPI/norito 同步、小部件测试）；
- 通过 `sorafs_cli` 发出门户、OpenAPI 和 SBOM CAR + 清单对；
- 可选择运行 `sorafs_cli proof verify` (`--proof`) 和 Sigstore 签名
  （`--sign`、`--sigstore-provider`、`--sigstore-audience`）；
- 掉落 `artifacts/devportal/sorafs/<timestamp>/` 下的所有文物并且
  写入 `package_summary.json`，以便 CI/发布工具可以摄取该包；和
- 刷新 `artifacts/devportal/sorafs/latest` 以指向最近的运行。

示例（带有 Sigstore + PoR 的完整管道）：

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

值得了解的标志：

- `--out <dir>` – 覆盖工件根（默认保留带时间戳的文件夹）。
- `--skip-build` – 重用现有的 `docs/portal/build`（当 CI 无法
  由于离线镜像而重建）。
- `--skip-sync-openapi` – 当 `cargo xtask openapi` 时跳过 `npm run sync-openapi`
  无法访问 crates.io。
- `--skip-sbom` – 在未安装二进制文件时避免调用 `syft`（
  脚本会打印警告）。
- `--proof` – 为每个 CAR/清单对运行 `sorafs_cli proof verify`。多
  文件有效负载仍然需要 CLI 中的块计划支持，因此保留此标志
  如果遇到 `plan chunk count` 错误，请取消设置，并在出现错误后手动验证
  上游门地。
- `--sign` – 调用 `sorafs_cli manifest sign`。提供一个令牌
  `SIGSTORE_ID_TOKEN`（或 `--sigstore-token-env`）或让 CLI 使用以下命令获取它
  `--sigstore-provider/--sigstore-audience`。

运输生产制品时使用 `docs/portal/scripts/sorafs-pin-release.sh`。
它现在打包门户、OpenAPI 和 SBOM 有效负载，签署每个清单，并
在 `portal.additional_assets.json` 中记录额外的资产元数据。帮手
了解 CI 包装器使用的相同可选旋钮以及新的
`--openapi-*`、`--portal-sbom-*` 和 `--openapi-sbom-*` 开关，以便您可以
为每个工件分配别名元组，通过覆盖 SBOM 源
`--openapi-sbom-source`，跳过某些有效负载（`--skip-openapi`/`--skip-sbom`），
并用 `--syft-bin` 指向非默认 `syft` 二进制文件。

该脚本显示它运行的每个命令；将日志复制到发布票据中
与 `package_summary.json` 一起，以便审阅者可以区分 CAR 摘要、计划
元数据和 Sigstore 捆绑散列，无需深入探索 ad-hoc shell 输出。

## 步骤 9 — 网关 + SoraDNS 验证

在宣布切换之前，请证明新别名通过 SoraDNS 进行解析，并且
网关主要新鲜证明：

1. **运行探测门。** `ci/check_sorafs_gateway_probe.sh` 练习
   `cargo xtask sorafs-gateway-probe` 对比演示装置
   `fixtures/sorafs_gateway/probe_demo/`。对于实际部署，请将探针指向
   在目标主机名处：

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   探头解码 `Sora-Name`、`Sora-Proof` 和 `Sora-Proof-Status`
   `docs/source/sorafs_alias_policy.md` 并在清单摘要时失败，
   TTL 或 GAR 绑定漂移。

   用于轻量级抽查（例如，当仅绑定捆绑包时）
   已更改），运行 `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`。
   帮助器验证捕获的绑定包并方便发布
   只需要绑定确认而不需要进行完整的探测演习的门票。

2. **捕获演练证据。** 对于操作员演练或 PagerDuty 演练，包裹
   带有 `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario 的探针
   devportal-rollout——……`。包装器将标头/日志存储在
   `artifacts/sorafs_gateway_probe/<stamp>/`，更新 `ops/drill-log.md`，以及
   （可选）触发回滚挂钩或 PagerDuty 有效负载。套装
   `--host docs.sora` 用于验证 SoraDNS 路径，而不是硬编码 IP。3. **验证 DNS 绑定。** 当治理发布别名证明时，记录
   探针中引用的 GAR 文件 (`--gar`) 并将其附加到版本中
   证据。解析器所有者可以通过镜像相同的输入
   `tools/soradns-resolver` 以确保缓存条目遵循新清单。
   在附加 JSON 之前，运行
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   因此确定性主机映射、清单元数据和遥测标签是
   离线验证。助手可以发出 `--json-out` 摘要以及
   签署了 GAR，因此审阅者无需打开二进制文件即可获得可验证的证据。
  在起草新的 GAR 时，更喜欢
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  （仅当清单文件不存在时才回退到 `--manifest-cid <cid>`
  可用）。现在，助手直接从 CID ** 和 ** BLAKE3 摘要中得出
  清单 JSON、修剪空白、重复数据删除 `--telemetry-label`
  标志、对标签进行排序并发出默认的 CSP/HSTS/Permissions-Policy
  在编写 JSON 之前使用模板，以便有效负载即使在以下情况下也保持确定性：
  操作员从不同的外壳捕获标签。

4. **观察别名指标。** 保留 `torii_sorafs_alias_cache_refresh_duration_ms`
   和 `torii_sorafs_gateway_refusals_total{profile="docs"}` 出现在屏幕上
   探测器正在运行；两个系列均绘制在
   `dashboards/grafana/docs_portal.json`。

## 步骤 10 — 监控和证据捆绑

- **仪表板。** 导出 `dashboards/grafana/docs_portal.json`（门户 SLO），
  `dashboards/grafana/sorafs_gateway_observability.json`（网关延迟 +
  健康证明）和 `dashboards/grafana/sorafs_fetch_observability.json`
  （协调器健康）每个版本。将 JSON 导出附加到
  发布票证，以便审阅者可以重播 Prometheus 查询。
- **探测档案。** 将 `artifacts/sorafs_gateway_probe/<stamp>/` 保留在 git-annex 中
  或者你的证据桶。包括探测摘要、标头和 PagerDuty
  遥测脚本捕获的有效负载。
- **发布捆绑包。** 存储门户/SBOM/OpenAPI CAR 摘要、清单
  捆绑包、Sigstore 签名、`portal.pin.report.json`、Try-It 探测日志以及
  单个带时间戳的文件夹下的链接检查报告（例如，
  `artifacts/sorafs/devportal/20260212T1103Z/`）。
- **钻孔日志。** 当探头是钻孔的一部分时，让
  `scripts/telemetry/run_sorafs_gateway_probe.sh` 附加到 `ops/drill-log.md`
  因此相同的证据满足 SNNet-5 混沌要求。
- **票证链接。** 参考 Grafana 面板 ID 或附加的 PNG 导出
  变更单以及探测报告路径，因此变更审核者
  无需 shell 访问即可交叉检查 SLO。

## 步骤 11 — 多源获取练习和记分牌证据

发布到 SoraFS 现在需要多源获取证据 (DOCS-7/SF-6)
以及上面的 DNS/网关证明。固定清单后：

1. **针对实时清单运行 `sorafs_fetch`。** 使用相同的计划/清单
   步骤 2-3 中生成的工件以及为每个工件颁发的网关凭证
   提供者。保留每个输出，以便审核员可以重播协调器
   决策轨迹：

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - 首先获取清单引用的提供商广告（例如
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     并通过 `--provider-advert name=path` 传递它们，以便记分板可以
     确定性地评估能力窗口。使用
     `--allow-implicit-provider-metadata` **仅**在重播赛程时
     CI;制作演习必须引用与
     针。
   - 当清单引用其他区域时，请重复该命令
     相应的提供者元组，因此每个缓存/别名都有一个匹配的
     获取文物。

2. **存档输出。** 存储 `scoreboard.json`，
   `providers.ndjson`、`fetch.json` 和 `chunk_receipts.ndjson` 下
   释放证据文件夹。这些文件捕获对等权重，重试
   治理数据包必须的预算、延迟 EWMA 和每块收据
   保留 SF-7。

3. **更新遥测。** 将获取输出导入 **SoraFS Fetch
   可观察性**仪表板 (`dashboards/grafana/sorafs_fetch_observability.json`)，
   观看 `torii_sorafs_fetch_duration_ms`/`_failures_total` 和
   提供者范围异常面板。将 Grafana 面板快照链接到
   沿着记分板路径发布票证。

4. **执行警报规则。** 运行 `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   在关闭版本之前验证 Prometheus 警报包。附加
   promtool 输出到票证，以便 DOCS-7 审阅者可以确认停顿
   慢速提供者警报仍然处于武装状态。

5. **连接到 CI。** 入口引脚工作流程落后于 `sorafs_fetch` 步骤
   `perform_fetch_probe` 输入；启用它进行暂存/生产运行，以便
   获取证据与清单包一起生成，无需手动
   干预。本地演练可以通过导出来重复使用相同的脚本
   网关令牌并将 `PIN_FETCH_PROVIDERS` 设置为逗号分隔
   供应商名单。

## 提升、可观察和回滚

1. **促销：** 保留单独的暂存和生产别名。推广方式
   使用相同的清单/包重新运行 `manifest submit`，交换
   `--alias-namespace/--alias-name` 指向生产别名。这个
   一旦 QA 批准暂存 pin，就可以避免重建或辞职。
2. **监控：**导入pin-registry仪表板
   (`docs/source/grafana_sorafs_pin_registry.json`) 加上特定于门户的
   探头（参见 `docs/portal/docs/devportal/observability.md`）。校验和警报
   漂移、失败的探针或证明重试尖峰。
3. **回滚：** 恢复，重新提交以前的清单（或撤销
   当前别名）使用 `sorafs_cli manifest submit --alias ... --retire`。
   始终保留最后一个已知良好的捆绑包和 CAR 摘要，以便回滚证明可以
   如果 CI 日志轮换，则重新创建。

## CI 工作流程模板

至少，您的管道应该：

1. 构建 + lint（`npm ci`、`npm run build`、校验和生成）。
2. 包 (`car pack`) 和计算清单。
3. 使用作业范围的 OIDC 令牌 (`manifest sign`) 进行签名。
4. 上传工件（CAR、清单、捆绑包、计划、摘要）以供审核。
5. 提交至 PIN 注册表：
   - 拉取请求 → `docs-preview.sora`。
   - 标签/受保护的分支→生产别名升级。
6.退出前运行探测器+证明验证门。

`.github/workflows/docs-portal-sorafs-pin.yml` 将所有这些步骤连接在一起
用于手动发布。工作流程：

- 构建/测试门户，
- 通过 `scripts/sorafs-pin-release.sh` 打包构建，
- 使用 GitHub OIDC 签署/验证清单包，
- 将 CAR/manifest/bundle/plan/proof 摘要作为工件上传，以及
-（可选）在存在机密时提交清单+别名绑定。

在触发作业之前配置以下存储库机密/变量：

|名称 |目的|
|------|---------|
| `DOCS_SORAFS_TORII_URL` |公开 `/v2/sorafs/pin/register` 的 Torii 主机。 |
| `DOCS_SORAFS_SUBMITTED_EPOCH` |与提交一起记录的纪元标识符。 |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` |清单提交的签名授权。 |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` |当 `perform_submit` 为 `true` 时，绑定到清单的别名元组。 |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64 编码的别名证明包（可选；省略跳过别名绑定）。 |
| `DOCS_ANALYTICS_*` |其他工作流程重用的现有分析/探测端点。 |

通过操作 UI 触发工作流程：

1.提供`alias_label`（例如`docs.sora.link`），可选`proposal_alias`，
   以及可选的 `release_tag` 覆盖。
2. 不选中 `perform_submit` 以在不接触 Torii 的情况下生成工件
   （对于空运行有用）或使其能够直接发布到配置的
   别名。

`docs/source/sorafs_ci_templates.md` 仍然记录了通用 CI 助手
此存储库之外的项目，但应首选门户工作流程
用于日常发布。

## 清单

- [ ] `npm run build`、`npm run test:*` 和 `npm run check:links` 为绿色。
- [ ] `build/checksums.sha256` 和 `build/release.json` 在文物中捕获。
- [ ] 在 `artifacts/` 下生成的 CAR、计划、清单和摘要。
- [ ] Sigstore 捆绑 + 与日志一起存储的独立签名。
- [ ] `portal.manifest.submit.summary.json` 和 `portal.manifest.submit.response.json`
      提交发生时捕获。
- [ ] `portal.pin.report.json`（和可选 `portal.pin.proposal.json`）
      与 CAR/清单文物一起存档。
- [ ] `proof verify` 和 `manifest verify-signature` 日志已存档。
- [ ] Grafana 仪表板已更新 + Try-It 探测成功。
- [ ] 回滚注释（先前的清单 ID + 别名摘要）附加到
      释放票。