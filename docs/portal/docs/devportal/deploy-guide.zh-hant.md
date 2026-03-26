---
lang: zh-hant
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

此劇本轉換路線圖項目 **DOCS-7** （SoraFS 發布）和 **DOCS-8**
（CI/CD 引腳自動化）轉化為開發人員門戶的可操作程序。
它涵蓋了構建/lint 階段、SoraFS 打包、Sigstore 支持的清單
簽名、別名升級、驗證和回滾演練，以便每次預覽和
發布工件是可重現和可審計的。

該流程假設您有 `sorafs_cli` 二進製文件（使用
`--features cli`)，使用 PIN 註冊權限訪問 Torii 端點，以及
OIDC Sigstore 的憑據。存儲長期秘密（`IROHA_PRIVATE_KEY`，
`SIGSTORE_ID_TOKEN`、Torii 代幣）在您的 CI 金庫中；本地運行可以獲取它們
來自殼出口。

## 先決條件

- 節點 18.18+ 具有 `npm` 或 `pnpm`。
- `sorafs_cli` 來自 `cargo run -p sorafs_car --features cli --bin sorafs_cli`。
- Torii 公開 `/v1/sorafs/*` 的 URL 以及授權帳戶/私鑰
  可以提交清單和別名。
- OIDC 發行者（GitHub Actions、GitLab、工作負載身份等）來鑄造
  `SIGSTORE_ID_TOKEN`。
- 可選：`examples/sorafs_cli_quickstart.sh` 用於試運行和
  `docs/source/sorafs_ci_templates.md` 用於 GitHub/GitLab 工作流程腳手架。
- 配置 Try it OAuth 變量 (`DOCS_OAUTH_*`) 並運行
  升級構建之前的[安全強化清單](./security-hardening.md)
  實驗室外。當這些變量丟失時，門戶構建現在會失敗
  或者當 TTL/輪詢旋鈕落在強制窗口之外時；出口
  `DOCS_OAUTH_ALLOW_INSECURE=1` 僅適用於一次性本地預覽。附上
  釋放票的滲透測試證據。

## 步驟 0 — 捕獲 Try it 代理包

在將預覽版推廣到 Netlify 或網關之前，請標記 Try it 代理
源並簽名 OpenAPI 清單摘要到確定性捆綁包中：

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` 複製代理/探針/回滾助手，
驗證 OpenAPI 簽名，並寫入 `release.json` plus
`checksums.sha256`。將此捆綁包附加到 Netlify/SoraFS 網關促銷
票證，以便審閱者可以重播確切的代理源和 Torii 目標提示
無需重建。該捆綁包還記錄客戶端提供的承載是否
啟用 (`allow_client_auth`) 以保持部署計劃和 CSP 規則同步。

## 步驟 1 — 構建並檢查門戶

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

`npm run build` 自動執行 `scripts/write-checksums.mjs`，產生：

- `build/checksums.sha256` — SHA256 清單適用於 `sha256sum -c`。
- `build/release.json` — 元數據（`tag`、`generated_at`、`source`）固定到
  每輛車/艙單。

將這兩個文件與 CAR 摘要一起存檔，以便審閱者可以比較預覽
文物無需重建。

## 步驟 2 — 打包靜態資源

針對 Docusaurus 輸出目錄運行 CAR 加殼程序。下面的例子
將所有工件寫入 `artifacts/devportal/` 下。

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

摘要 JSON 捕獲塊計數、摘要和證明計劃提示
`manifest build` 和 CI 儀表板稍後重用。

## 步驟 2b — 軟件包 OpenAPI 和 SBOM 同伴

DOCS-7 需要發布門戶網站、OpenAPI 快照和 SBOM 有效負載
作為不同的清單，因此網關可以裝訂 `Sora-Proof`/`Sora-Content-CID`
每個工件的標題。發布助手
(`scripts/sorafs-pin-release.sh`)已經打包了OpenAPI目錄
(`static/openapi/`) 和通過 `syft` 發出的 SBOM 到單獨的
`openapi.*`/`*-sbom.*` CAR 並將元數據記錄在
`artifacts/sorafs/portal.additional_assets.json`。運行手動流程時，
對每個具有自己的前綴和元數據標籤的有效負載重複步驟 2-4
（例如 `--car-out "$OUT"/openapi.car` 加
`--metadata alias_label=docs.sora.link/openapi`）。註冊每個清單/別名
在切換 DNS 之前在 Torii（站點、OpenAPI、門戶 SBOM、OpenAPI SBOM）中進行配對
該網關可以為所有已發布的文物提供裝訂證據。

## 步驟 3 — 構建清單

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

將 pin-policy 標誌調整到您的發布窗口（例如，`--pin-storage-class
對於金絲雀來說是熱的）。 JSON 變體是可選的，但便於代碼審查。

## 步驟 4 — 使用 Sigstore 簽名

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

該包記錄了清單摘要、塊摘要和 BLAKE3 哈希值
OIDC 令牌，無需保留 JWT。保持捆綁和分離
簽名；生產晉升可以重複使用相同的文物而不是辭職。
本地運行可以用 `--identity-token-env` 替換提供程序標誌（或設置
`SIGSTORE_ID_TOKEN` 環境中）當外部 OIDC 幫助程序發出
令牌。

## 步驟 5 — 提交至 PIN 註冊表

將簽名的清單（和塊計劃）提交到 Torii。始終要求提供摘要
因此生成的註冊表項/別名證明是可審計的。

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority soraカタカナ... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

推出預覽或金絲雀別名 (`docs-preview.sora`) 時，請重複
使用唯一別名提交，以便 QA 可以在生產前驗證內容
促銷。

別名綁定需要三個字段：`--alias-namespace`、`--alias-name` 和
`--alias-proof`。治理生成證明包（base64 或 Norito 字節）
別名請求何時獲得批准；將其存儲在 CI 機密中並將其顯示為
調用 `manifest submit` 之前的文件。當您
只想固定清單而不觸及 DNS。

## 步驟 5b — 生成治理提案

每份清單都應附帶議會準備好的提案，以便任何 Sora
公民可以在不借用特權憑證的情況下引入變革。
完成提交/簽名步驟後，運行：

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` 捕獲規範 `RegisterPinManifest`
指令、塊摘要、策略和別名提示。將其附加到治理中
票證或議會門戶，以便代表們可以在不重建的情況下區分有效負載
文物。因為該命令從未觸及 Torii 權限密鑰，所以任何
公民可以在當地起草提案。

## 步驟 6 — 驗證證明和遙測

固定後，運行確定性驗證步驟：

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

- 檢查 `torii_sorafs_gateway_refusals_total` 和
  `torii_sorafs_replication_sla_total{outcome="missed"}` 異常。
- 運行 `npm run probe:portal` 來練習 Try-It 代理和記錄的鏈接
  針對新固定的內容。
- 捕獲中描述的監控證據
  [發布和監控](./publishing-monitoring.md) 所以 DOCS-3c 的
  可觀察性門與發布步驟一起得到滿足。幫手
  現在接受多個 `bindings` 條目（站點、OpenAPI、門戶 SBOM、OpenAPI
  SBOM）並在目標上強制執行 `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  通過可選的 `hostname` 防護主機。下面的調用寫了兩個
  單個 JSON 摘要和證據包（`portal.json`、`tryit.json`、
  `binding.json`和`checksums.sha256`）在發布目錄下：

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## 步驟 6a — 規劃網關證書

在創建 GAR 數據包之前導出 TLS SAN/挑戰計劃，以便網關
團隊和 DNS 審批者審查相同的證據。新助手反映了
DG-3 通過枚舉規範通配符主機進行自動化輸入，
漂亮主機 SAN、DNS-01 標籤和推薦的 ACME 挑戰：

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

將 JSON 與發布包一起提交（或將其與更改一起上傳）
票證），以便操作員可以將 SAN 值粘貼到 Torii 中
`torii.sorafs_gateway.acme` 配置和 GAR 審閱者可以確認
規範/漂亮的映射，無需重新運行主機派生。添加額外的
`--name` 在同一版本中升級的每個後綴的參數。

## 步驟 6b — 導出規範主機映射

在模板化 GAR 有效負載之前，記錄每個的確定性主機映射
別名。 `cargo xtask soradns-hosts` 將每個 `--name` 散列到其規範中
標籤 (`<base32>.gw.sora.id`)，發出所需的通配符
(`*.gw.sora.id`)，並派生出漂亮的主機(`<alias>.gw.sora.name`)。堅持
發布工件中的輸出，以便 DG-3 審閱者可以區分映射
與 GAR 提交一起：

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

每當 GAR 或網關出現故障時，使用 `--verify-host-patterns <file>` 快速失敗
綁定 JSON 省略了所需的主機之一。助手接受多個
驗證文件，可以輕鬆檢查 GAR 模板和
在同一調用中裝訂 `portal.gateway.binding.json`：

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

將摘要 JSON 和驗證日誌附加到 DNS/網關更改票證中，以便
審核員無需重新運行即可確認規範、通配符和漂亮主機
推導腳本。每當添加新別名時重新運行該命令
捆綁，以便後續 GAR 更新繼承相同的證據軌跡。

## 步驟 7 — 生成 DNS 切換描述符

生產切換需要可審核的變更包。成功後
提交（別名綁定），助手發出
`artifacts/sorafs/portal.dns-cutover.json`，捕獲：- 別名綁定元數據（命名空間/名稱/證明、清單摘要、Torii URL、
  提交紀元、權威）；
- 發布上下文（標籤、別名標籤、清單/CAR 路徑、塊計劃、Sigstore
  捆綁）；
- 驗證指針（探測命令，別名+ Torii端點）；和
- 可選的變更控製字段（票證 ID、切換窗口、操作聯繫人、
  生產主機名/區域）；
- 源自裝訂的 `Sora-Route-Binding` 的路線促銷元數據
  標頭（規範主機/CID、標頭+綁定路徑、驗證命令）、
  確保 GAR 升級和後備演習參考相同的證據；
- 生成的路線規劃工件（`gateway.route_plan.json`，
  標頭模板和可選的回滾標頭），因此更改票證和 CI
  lint hooks 可以驗證每個 DG-3 數據包都引用了規範
  批准前的升級/回滾計劃；
- 可選的緩存失效元數據（清除端點、身份驗證變量、JSON
  有效負載，以及示例 `curl` 命令）；和
- 回滾提示指向先前的描述符（發布標籤和清單）
  摘要），因此更改票據捕獲確定性後備路徑。

當版本需要緩存清除時，生成規範計劃以及
切換描述符：

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

將生成的 `portal.cache_plan.json` 附加到 DG-3 數據包，以便運營商
發出時具有確定性的主機/路徑（以及匹配的身份驗證提示）
`PURGE` 請求。描述符的可選緩存元數據部分可以參考
直接此文件，使變更控制審閱者準確地保持一致
端點在切換期間被刷新。

每個 DG-3 數據包還需要一個升級 + 回滾清單。通過生成它
`cargo xtask soradns-route-plan`，以便變更控制審核人員可以追踪準確的
每個別名的預檢、切換和回滾步驟：

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

發出的 `gateway.route_plan.json` 捕獲規範/漂亮的主機，上演
健康檢查提醒、GAR 綁定更新、緩存清除和回滾操作。
在提交更改之前將其與 GAR/綁定/轉換工件捆綁在一起
票，以便運營人員可以按照相同的腳本步驟進行排練和簽署。

`scripts/generate-dns-cutover-plan.mjs` 為該描述符提供動力並運行
自動從 `sorafs-pin-release.sh` 開始。重新生成或自定義它
手動：

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

在運行 pin 之前通過環境變量填充可選元數據
幫手：

|變量|目的|
|----------|---------|
| `DNS_CHANGE_TICKET` |票證 ID 存儲在描述符中。 |
| `DNS_CUTOVER_WINDOW` | ISO8601 轉換窗口（例如 `2026-03-21T15:00Z/2026-03-21T15:30Z`）。 |
| `DNS_HOSTNAME`，`DNS_ZONE` |生產主機名+權威區域。 |
| `DNS_OPS_CONTACT` |待命別​​名或升級聯繫人。 |
| `DNS_CACHE_PURGE_ENDPOINT` |緩存清除端點記錄在描述符中。 |
| `DNS_CACHE_PURGE_AUTH_ENV` |包含清除令牌的環境變量（默認為 `CACHE_PURGE_TOKEN`）。 |
| `DNS_PREVIOUS_PLAN` |回滾元數據的先前切換描述符的路徑。 |

將 JSON 附加到 DNS 更改審核，以便審批者可以驗證清單
摘要、別名綁定和探測命令，無需抓取 CI 日誌。
CLI 標誌 `--dns-change-ticket`、`--dns-cutover-window`、`--dns-hostname`、
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` 和 `--previous-dns-plan` 提供相同的覆蓋
在 CI 外部運行助手時。

## 步驟 8 — 發出解析器區域文件框架（可選）

當生產切換窗口已知時，發布腳本可以發出
自動 SNS 區域文件框架和解析器片段。傳遞所需的 DNS
通過環境變量或 CLI 選項記錄和元數據；幫手
切換後會立即調用 `scripts/sns_zonefile_skeleton.py`
生成描述符。提供至少一個 A/AAAA/CNAME 值和 GAR
摘要（簽名的 GAR 有效負載的 BLAKE3-256）。如果區域/主機名已知
並且 `--dns-zonefile-out` 被省略，幫助程序寫入
`artifacts/sns/zonefiles/<zone>/<hostname>.json` 並填充
`ops/soradns/static_zones.<hostname>.json` 作為解析器片段。

|變量/標誌|目的|
|----------------|---------|
| `DNS_ZONEFILE_OUT`、`--dns-zonefile-out` |生成的區域文件框架的路徑。 |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`，`--dns-zonefile-resolver-snippet` |解析器代碼片段路徑（省略時默認為 `ops/soradns/static_zones.<hostname>.json`）。 |
| `DNS_ZONEFILE_TTL`，`--dns-zonefile-ttl` |應用於生成記錄的 TTL（默認值：600 秒）。 |
| `DNS_ZONEFILE_IPV4`、`--dns-zonefile-ipv4` | IPv4 地址（逗號分隔的 env 或可重複的 CLI 標誌）。 |
| `DNS_ZONEFILE_IPV6`、`--dns-zonefile-ipv6` | IPv6 地址。 |
| `DNS_ZONEFILE_CNAME`、`--dns-zonefile-cname` |可選的 CNAME 目標。 |
| `DNS_ZONEFILE_SPKI`、`--dns-zonefile-spki-pin` | SHA-256 SPKI 引腳 (base64)。 |
| `DNS_ZONEFILE_TXT`，`--dns-zonefile-txt` |其他 TXT 條目 (`key=value`)。 |
| `DNS_ZONEFILE_VERSION`、`--dns-zonefile-version` |覆蓋計算出的區域文件版本標籤。 |
| `DNS_ZONEFILE_EFFECTIVE_AT`、`--dns-zonefile-effective-at` |強制使用 `effective_at` 時間戳 (RFC3339)，而不是切換窗口啟動。 |
| `DNS_ZONEFILE_PROOF`、`--dns-zonefile-proof` |覆蓋元數據中記錄的證明文字。 |
| `DNS_ZONEFILE_CID`，`--dns-zonefile-cid` |覆蓋元數據中記錄的 CID。 |
| `DNS_ZONEFILE_FREEZE_STATE`、`--dns-zonefile-freeze-state` |守護凍結狀態（軟、硬、解凍、監控、緊急）。 |
| `DNS_ZONEFILE_FREEZE_TICKET`，`--dns-zonefile-freeze-ticket` |監護人/議會凍結票證參考。 |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`，`--dns-zonefile-freeze-expires-at` |用於解凍的 RFC3339 時間戳。 |
| `DNS_ZONEFILE_FREEZE_NOTES`，`--dns-zonefile-freeze-note` |附加凍結註釋（逗號分隔的 env 或可重複標誌）。 |
| `DNS_GAR_DIGEST`，`--dns-gar-digest` |簽名 GAR 有效負載的 BLAKE3-256 摘要（十六進制）。只要存在網關綁定就需要。 |

GitHub Actions 工作流程從存儲庫機密中讀取這些值，因此每個生產 pin 都會自動發出區域文件工件。配置以下機密（字符串可能包含多值字段的逗號分隔列表）：

|秘密|目的|
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`，`DOCS_SORAFS_DNS_ZONE` |傳遞給幫助程序的生產主機名/區域。 |
| `DOCS_SORAFS_DNS_OPS_CONTACT` |存儲在描述符中的待命別名。 |
| `DOCS_SORAFS_ZONEFILE_IPV4`，`DOCS_SORAFS_ZONEFILE_IPV6` |要發布的 IPv4/IPv6 記錄。 |
| `DOCS_SORAFS_ZONEFILE_CNAME` |可選的 CNAME 目標。 |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI 引腳。 |
| `DOCS_SORAFS_ZONEFILE_TXT` |其他 TXT 條目。 |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` |凍結骨架中記錄的元數據。 |
| `DOCS_SORAFS_GAR_DIGEST` |已簽名 GAR 有效負載的十六進制編碼的 BLAKE3 摘要。 |

觸發 `.github/workflows/docs-portal-sorafs-pin.yml` 時，提供 `dns_change_ticket` 和 `dns_cutover_window` 輸入，以便描述符/區域文件繼承正確的更改窗口元數據。僅在進行空運行時才將它們留空。

典型調用（與 SN-7 所有者運行手冊匹配）：

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

助手會自動將變更單作為 TXT 條目轉入並
繼承切換窗口開始作為 `effective_at` 時間戳，除非
被覆蓋。有關完整的操作流程，請參閱
`docs/source/sorafs_gateway_dns_owner_runbook.md`。

### 公共 DNS 委託說明

區域文件框架僅定義區域的權威記錄。你
仍然需要在您的註冊商或 DNS 處配置父區域 NS/DS 委派
提供商，以便常規互聯網可以發現名稱服務器。

- 對於 apex/TLD 轉換，請使用 ALIAS/ANAME（特定於提供商）或發布 A/AAAA
  指向網關任播 IP 的記錄。
- 對於子域，將 CNAME 發佈到派生的漂亮主機
  （`<fqdn>.gw.sora.name`）。
- 規範主機 (`<hash>.gw.sora.id`) 位於網關域下並且
  未在您的公共區域內發布。

### 網關標頭模板

部署助手還會發出 `portal.gateway.headers.txt` 和
`portal.gateway.binding.json`，滿足 DG-3 要求的兩件文物
網關內容綁定要求：

- `portal.gateway.headers.txt` 包含完整的 HTTP 標頭塊（包括
  `Sora-Name`、`Sora-Content-CID`、`Sora-Proof`、CSP、HSTS 和
  `Sora-Route-Binding` 描述符），邊緣網關必須釘在每個
  回應。
- `portal.gateway.binding.json`以機器可讀的方式記錄相同的信息
  表單，以便更改票證和自動化可以區分主機/cid 綁定，而無需
  抓取 shell 輸出。

它們是通過自動生成的
`cargo xtask soradns-binding-template`
並捕獲提供的別名、清單摘要和網關主機名
至 `sorafs-pin-release.sh`。要重新生成或自定義標頭塊，請運行：

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

通過 `--csp-template`、`--permissions-template` 或 `--hsts-template` 進行覆蓋
當特定部署需要額外的默認標頭模板
指令；將它們與現有的 `--no-*` 開關組合以刪除標頭
完全。

將標頭片段附加到 CDN 更改請求並提供 JSON 文檔
進入網關自動化管道，以便實際主機升級與
公佈證據。

發布腳本自動運行驗證助手，因此 DG-3 門票
始終包括最近的證據。每當您調整時，請手動重新運行它
手動綁定 JSON：

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

該命令驗證綁定包中捕獲的 `Sora-Proof` 有效負載，
確保 `Sora-Route-Binding` 元數據與清單 CID + 主機名匹配，
如果任何標頭漂移，則會快速失敗。將控制台輸出存檔在
每當您在 CI 外部運行命令時都會產生其他部署工件，因此 DG-3
審閱者有證據表明綁定在切換之前已得到驗證。> **DNS 描述符集成：** `portal.dns-cutover.json` 現在嵌入了
> `gateway_binding` 部分指向這些工件（路徑、內容 CID、
> 證明狀態和文字標頭模板）**和** `route_plan` 節
> 引用 `gateway.route_plan.json` 加上主 + 回滾標頭
> 模板。將這些塊包含在每個 DG-3 變更單中，以便審核者可以
> 比較確切的 `Sora-Name/Sora-Proof/CSP` 值並確認該路由
> 升級/回滾計劃與證據包匹配，無需打開構建
> 存檔。

## 步驟 9 — 運行發布監視器

路線圖任務 **DOCS-3c** 需要持續證據表明門戶，嘗試一下
代理和網關綁定在發布後保持健康。運行綜合
在步驟 7-8 之後立即進行監視並將其連接到您預定的探測器中：

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` 加載配置文件（參見
  `docs/portal/docs/devportal/publishing-monitoring.md` 的架構）和
  執行三項檢查：門戶路徑探測+CSP/權限策略驗證，
  嘗試代理探針（可以選擇訪問其 `/metrics` 端點），然後
  網關綁定驗證程序 (`cargo xtask soradns-verify-binding`) 檢查
  根據預期的別名、主機、證明狀態捕獲的綁定包，
  並顯示 JSON。
- 每當任何探測失敗時，該命令都會以非零值退出，因此 CI、cron 作業或
  Runbook 操作員可以在升級別名之前停止發布。
- 傳遞 `--json-out` 會為每個目標寫入一個摘要 JSON 有效負載
  狀態； `--evidence-dir` 發出 `summary.json`、`portal.json`、`tryit.json`、
  `binding.json` 和 `checksums.sha256`，以便治理審核者可以區分
  結果無需重新運行監視器。將此目錄存檔於
  `artifacts/sorafs/<tag>/monitoring/` 以及 Sigstore 捆綁包和 DNS
  切換描述符。
- 包括監視器輸出、Grafana 導出 (`dashboards/grafana/docs_portal.json`)、
  和 Alertmanager 鑽取發佈單中的 ID，以便 DOCS-3c SLO 可以
  後來審核了。專用的發布監控手冊位於
  `docs/portal/docs/devportal/publishing-monitoring.md`。

門戶探測需要 HTTPS 並拒絕 `http://` 基本 URL，除非
`allowInsecureHttp` 在監視器配置中設置；保持製作/登台
TLS 上的目標，並且僅啟用本地預覽的覆蓋。

通過 Buildkite/cron 中的 `npm run monitor:publishing` 自動化監控
門戶網站已上線。指向生產 URL 的同一命令提供正在進行的
SRE/Docs 在版本之間依賴的運行狀況檢查。

## 使用 `sorafs-pin-release.sh` 實現自動化

`docs/portal/scripts/sorafs-pin-release.sh` 封裝了步驟 2-6。它：

1. 將 `build/` 歸檔到確定性 tarball 中，
2. 運行 `car pack`、`manifest build`、`manifest sign`、`manifest verify-signature`、
   和 `proof verify`，
3.當Torii時可選地執行`manifest submit`（包括別名綁定）
   存在憑據，並且
4.寫入`artifacts/sorafs/portal.pin.report.json`，可選
  `portal.pin.proposal.json`，DNS 切換描述符（提交後），
  和網關綁定捆綁包（`portal.gateway.binding.json` 加上
  文本標題塊），以便治理、網絡和運營團隊可以區分
  證據捆綁，無需抓取 CI 日誌。

設置 `PIN_ALIAS`、`PIN_ALIAS_NAMESPACE`、`PIN_ALIAS_NAME` 和（可選）
`PIN_ALIAS_PROOF_PATH` 在調用腳本之前。使用 `--skip-submit` 進行乾燥
運行；下面描述的 GitHub 工作流程通過 `perform_submit` 切換此功能
輸入。

## 步驟 8 — 發布 OpenAPI 規格和 SBOM 捆綁包

DOCS-7 需要門戶構建、OpenAPI 規範和 SBOM 工件才能傳輸
通過相同的確定性管道。現有的助手涵蓋了所有三個：

1. **重新生成並簽署規範。 **

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   每當您想要保留某個版本時，請通過 `--version=<label>` 傳遞發布標籤
   歷史快照（例如 `2025-q3`）。助手寫入快照
   到 `static/openapi/versions/<label>/torii.json`，將其鏡像到
   `versions/current`，並記錄元數據（SHA-256、清單狀態和
   更新的時間戳）在 `static/openapi/versions.json` 中。開發者門戶
   讀取該索引，以便 Swagger/RapiDoc 面板可以顯示版本選擇器
   並內聯顯示相關的摘要/簽名信息。省略
   `--version` 保持之前的版本標籤不變，僅刷新
   `current` + `latest` 指針。

   清單捕獲 SHA-256/BLAKE3 摘要，以便網關可以裝訂
   `Sora-Proof` `/reference/torii-swagger` 標頭。

2. **發出 CycloneDX SBOM。 ** 發布管道已經期望基於 syft
   根據 `docs/source/sorafs_release_pipeline_plan.md` 的 SBOM。保留輸出
   構建工件旁邊：

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **將每個有效負載打包到 CAR 中。 **

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

   按照與主站點相同的 `manifest build` / `manifest sign` 步驟，
   調整每個資產的別名（例如，規範的 `docs-openapi.sora` 和
   `docs-sbom.sora` 用於簽名的 SBOM 捆綁包）。維護不同的別名
   將 SoraDNS 證明、GAR 和回滾票證的範圍限制在確切的有效負載範圍內。

4. **提交並綁定。 ** 復用現有權限+Sigstore捆綁，但是
   在發布清單中記錄別名元組，以便審核員可以跟踪哪些別名元組
   Sora 名稱映射到哪個清單摘要。

將規範/SBOM 清單與門戶構建一起存檔可確保每個
發行票包含完整的工件集，無需重新運行加殼器。

### 自動化助手（CI/包腳本）

`./ci/package_docs_portal_sorafs.sh` 編碼了步驟 1-8，因此路線圖項目
**DOCS-7** 可以使用單個命令來執行。幫手：

- 運行所需的門戶準備（`npm ci`、OpenAPI/norito 同步、小部件測試）；
- 通過 `sorafs_cli` 發出門戶、OpenAPI 和 SBOM CAR + 清單對；
- 可選擇運行 `sorafs_cli proof verify` (`--proof`) 和 Sigstore 簽名
  （`--sign`、`--sigstore-provider`、`--sigstore-audience`）；
- 掉落 `artifacts/devportal/sorafs/<timestamp>/` 下的所有文物並且
  寫入 `package_summary.json`，以便 CI/發布工具可以攝取該包；和
- 刷新 `artifacts/devportal/sorafs/latest` 以指向最近的運行。

示例（帶有 Sigstore + PoR 的完整管道）：

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

值得了解的標誌：

- `--out <dir>` – 覆蓋工件根（默認保留帶時間戳的文件夾）。
- `--skip-build` – 重用現有的 `docs/portal/build`（當 CI 無法
  由於離線鏡像而重建）。
- `--skip-sync-openapi` – 當 `cargo xtask openapi` 時跳過 `npm run sync-openapi`
  無法訪問 crates.io。
- `--skip-sbom` – 在未安裝二進製文件時避免調用 `syft`（
  腳本會打印警告）。
- `--proof` – 為每個 CAR/清單對運行 `sorafs_cli proof verify`。多
  文件有效負載仍然需要 CLI 中的塊計劃支持，因此保留此標誌
  如果遇到 `plan chunk count` 錯誤，請取消設置，並在出現錯誤後手動驗證
  上游門地。
- `--sign` – 調用 `sorafs_cli manifest sign`。提供一個令牌
  `SIGSTORE_ID_TOKEN`（或 `--sigstore-token-env`）或讓 CLI 使用以下命令獲取它
  `--sigstore-provider/--sigstore-audience`。

運輸生產製品時使用 `docs/portal/scripts/sorafs-pin-release.sh`。
它現在打包門戶、OpenAPI 和 SBOM 有效負載，簽署每個清單，並
在 `portal.additional_assets.json` 中記錄額外的資產元數據。幫手
了解 CI 包裝器使用的相同可選旋鈕以及新的
`--openapi-*`、`--portal-sbom-*` 和 `--openapi-sbom-*` 開關，以便您可以
為每個工件分配別名元組，通過覆蓋 SBOM 源
`--openapi-sbom-source`，跳過某些有效負載（`--skip-openapi`/`--skip-sbom`），
並用 `--syft-bin` 指向非默認 `syft` 二進製文件。

該腳本顯示它運行的每個命令；將日誌複製到發布票據中
與 `package_summary.json` 一起，以便審閱者可以區分 CAR 摘要、計劃
元數據和 Sigstore 捆綁散列，無需深入探索 ad-hoc shell 輸出。

## 步驟 9 — 網關 + SoraDNS 驗證

在宣布切換之前，請證明新別名通過 SoraDNS 進行解析，並且
網關主要新鮮證明：

1. **運行探測門。 ** `ci/check_sorafs_gateway_probe.sh` 練習
   `cargo xtask sorafs-gateway-probe` 對比演示裝置
   `fixtures/sorafs_gateway/probe_demo/`。對於實際部署，請將探針指向
   在目標主機名處：

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   探頭解碼 `Sora-Name`、`Sora-Proof` 和 `Sora-Proof-Status`
   `docs/source/sorafs_alias_policy.md` 並在清單摘要時失敗，
   TTL 或 GAR 綁定漂移。

   用於輕量級抽查（例如，當僅綁定捆綁包時）
   已更改），運行 `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`。
   幫助器驗證捕獲的綁定包並方便發布
   只需要綁定確認而不需要進行完整的探測演習的門票。

2. **捕獲演練證據。 ** 對於操作員演練或 PagerDuty 演練，包裹
   帶有 `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario 的探針
   devportal-rollout——……`。包裝器將標頭/日誌存儲在
   `artifacts/sorafs_gateway_probe/<stamp>/`，更新 `ops/drill-log.md`，以及
   （可選）觸發回滾掛鉤或 PagerDuty 有效負載。套裝
   `--host docs.sora` 用於驗證 SoraDNS 路徑，而不是硬編碼 IP。3. **驗證 DNS 綁定。 ** 當治理髮布別名證明時，記錄
   探針中引用的 GAR 文件 (`--gar`) 並將其附加到版本中
   證據。解析器所有者可以通過鏡像相同的輸入
   `tools/soradns-resolver` 以確保緩存條目遵循新清單。
   在附加 JSON 之前，運行
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   因此確定性主機映射、清單元數據和遙測標籤是
   離線驗證。助手可以發出 `--json-out` 摘要以及
   簽署了 GAR，因此審閱者無需打開二進製文件即可獲得可驗證的證據。
  在起草新的 GAR 時，更喜歡
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  （僅當清單文件不存在時才回退到 `--manifest-cid <cid>`
  可用）。現在，助手直接從 CID ** 和 ** BLAKE3 摘要中得出
  清單 JSON、修剪空白、重複數據刪除 `--telemetry-label`
  標誌、對標籤進行排序並發出默認的 CSP/HSTS/Permissions-Policy
  在編寫 JSON 之前使用模板，以便有效負載即使在以下情況下也保持確定性：
  操作員從不同的外殼捕獲標籤。

4. **觀察別名指標。 ** 保留 `torii_sorafs_alias_cache_refresh_duration_ms`
   和 `torii_sorafs_gateway_refusals_total{profile="docs"}` 出現在屏幕上
   探測器正在運行；兩個系列均繪製在
   `dashboards/grafana/docs_portal.json`。

## 步驟 10 — 監控和證據捆綁

- **儀表板。 ** 導出 `dashboards/grafana/docs_portal.json`（門戶 SLO），
  `dashboards/grafana/sorafs_gateway_observability.json`（網關延遲 +
  健康證明）和 `dashboards/grafana/sorafs_fetch_observability.json`
  （協調器健康）每個版本。將 JSON 導出附加到
  發布票證，以便審閱者可以重播 Prometheus 查詢。
- **探測檔案。 ** 將 `artifacts/sorafs_gateway_probe/<stamp>/` 保留在 git-annex 中
  或者你的證據桶。包括探測摘要、標頭和 PagerDuty
  遙測腳本捕獲的有效負載。
- **發布捆綁包。 ** 存儲門戶/SBOM/OpenAPI CAR 摘要、清單
  捆綁包、Sigstore 簽名、`portal.pin.report.json`、Try-It 探測日誌以及
  單個帶時間戳的文件夾下的鏈接檢查報告（例如，
  `artifacts/sorafs/devportal/20260212T1103Z/`）。
- **鑽孔日誌。 ** 當探頭是鑽孔的一部分時，讓
  `scripts/telemetry/run_sorafs_gateway_probe.sh` 附加到 `ops/drill-log.md`
  因此相同的證據滿足 SNNet-5 混沌要求。
- **票證鏈接。 ** 參考 Grafana 面板 ID 或附加的 PNG 導出
  變更單以及探測報告路徑，因此變更審核者
  無需 shell 訪問即可交叉檢查 SLO。

## 步驟 11 — 多源獲取練習和記分牌證據

發佈到 SoraFS 現在需要多源獲取證據 (DOCS-7/SF-6)
以及上面的 DNS/網關證明。固定清單後：

1. **針對實時清單運行 `sorafs_fetch`。 ** 使用相同的計劃/清單
   步驟 2-3 中生成的工件以及為每個工件頒發的網關憑證
   提供者。保留每個輸出，以便審核員可以重播協調器
   決策軌跡：

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

   - 首先獲取清單引用的提供商廣告（例如
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     並通過 `--provider-advert name=path` 傳遞它們，以便記分板可以
     確定性地評估能力窗口。使用
     `--allow-implicit-provider-metadata` **僅**在重播賽程時
     CI;製作演習必須引用與
     針。
   - 當清單引用其他區域時，請重複該命令
     相應的提供者元組，因此每個緩存/別名都有一個匹配的
     獲取文物。

2. **存檔輸出。 ** 存儲 `scoreboard.json`，
   `providers.ndjson`、`fetch.json` 和 `chunk_receipts.ndjson` 下
   釋放證據文件夾。這些文件捕獲對等權重，重試
   治理數據包必須的預算、延遲 EWMA 和每塊收據
   保留 SF-7。

3. **更新遙測。 ** 將獲取輸出導入 **SoraFS Fetch
   可觀察性**儀表板 (`dashboards/grafana/sorafs_fetch_observability.json`)，
   觀看 `torii_sorafs_fetch_duration_ms`/`_failures_total` 和
   提供者範圍異常面板。將 Grafana 面板快照鏈接到
   沿著記分板路徑發布票證。

4. **執行警報規則。 ** 運行 `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   在關閉版本之前驗證 Prometheus 警報包。附加
   promtool 輸出到票證，以便 DOCS-7 審閱者可以確認停頓
   慢速提供者警報仍然處於武裝狀態。

5. **連接到 CI。 ** 入口引腳工作流程落後於 `sorafs_fetch` 步驟
   `perform_fetch_probe` 輸入；啟用它進行暫存/生產運行，以便
   獲取證據與清單包一起生成，無需手動
   干預。本地演練可以通過導出來重複使用相同的腳本
   網關令牌並將 `PIN_FETCH_PROVIDERS` 設置為逗號分隔
   供應商名單。

## 提升、可觀察和回滾

1. **促銷：** 保留單獨的暫存和生產別名。推廣方式
   使用相同的清單/包重新運行 `manifest submit`，交換
   `--alias-namespace/--alias-name` 指向生產別名。這個
   一旦 QA 批准暫存 pin，就可以避免重建或辭職。
2. **監控：**導入pin-registry儀表板
   (`docs/source/grafana_sorafs_pin_registry.json`) 加上特定於門戶的
   探頭（參見 `docs/portal/docs/devportal/observability.md`）。校驗和警報
   漂移、失敗的探針或證明重試尖峰。
3. **回滾：** 恢復，重新提交以前的清單（或撤銷
   當前別名）使用 `sorafs_cli manifest submit --alias ... --retire`。
   始終保留最後一個已知良好的捆綁包和 CAR 摘要，以便回滾證明可以
   如果 CI 日誌輪換，則重新創建。

## CI 工作流程模板

至少，您的管道應該：

1. 構建 + lint（`npm ci`、`npm run build`、校驗和生成）。
2. 包 (`car pack`) 和計算清單。
3. 使用作業範圍的 OIDC 令牌 (`manifest sign`) 進行簽名。
4. 上傳工件（CAR、清單、捆綁包、計劃、摘要）以供審核。
5. 提交至 PIN 註冊表：
   - 拉取請求 → `docs-preview.sora`。
   - 標籤/受保護的分支→生產別名升級。
6.退出前運行探測器+證明驗證門。

`.github/workflows/docs-portal-sorafs-pin.yml` 將所有這些步驟連接在一起
用於手動發布。工作流程：

- 構建/測試門戶，
- 通過 `scripts/sorafs-pin-release.sh` 打包構建，
- 使用 GitHub OIDC 簽署/驗證清單包，
- 將 CAR/manifest/bundle/plan/proof 摘要作為工件上傳，以及
-（可選）在存在機密時提交清單+別名綁定。

在觸發作業之前配置以下存儲庫機密/變量：

|名稱 |目的|
|------|---------|
| `DOCS_SORAFS_TORII_URL` |公開 `/v1/sorafs/pin/register` 的 Torii 主機。 |
| `DOCS_SORAFS_SUBMITTED_EPOCH` |與提交一起記錄的紀元標識符。 |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` |清單提交的簽名授權。 |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` |當 `perform_submit` 為 `true` 時，綁定到清單的別名元組。 |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64 編碼的別名證明包（可選；省略跳過別名綁定）。 |
| `DOCS_ANALYTICS_*` |其他工作流程重用的現有分析/探測端點。 |

通過操作 UI 觸發工作流程：

1.提供`alias_label`（例如`docs.sora.link`），可選`proposal_alias`，
   以及可選的 `release_tag` 覆蓋。
2. 不選中 `perform_submit` 以在不接觸 Torii 的情況下生成工件
   （對於空運行有用）或使其能夠直接發佈到配置的
   別名。

`docs/source/sorafs_ci_templates.md` 仍然記錄了通用 CI 助手
此存儲庫之外的項目，但應首選門戶工作流程
用於日常發布。

## 清單

- [ ] `npm run build`、`npm run test:*` 和 `npm run check:links` 為綠色。
- [ ] `build/checksums.sha256` 和 `build/release.json` 在文物中捕獲。
- [ ] 在 `artifacts/` 下生成的 CAR、計劃、清單和摘要。
- [ ] Sigstore 捆綁 + 與日誌一起存儲的獨立簽名。
- [ ] `portal.manifest.submit.summary.json` 和 `portal.manifest.submit.response.json`
      提交發生時捕獲。
- [ ] `portal.pin.report.json`（和可選 `portal.pin.proposal.json`）
      與 CAR/清單文物一起存檔。
- [ ] `proof verify` 和 `manifest verify-signature` 日誌已存檔。
- [ ] Grafana 儀表板已更新 + Try-It 探測成功。
- [ ] 回滾註釋（先前的清單 ID + 別名摘要）附加到
      釋放票。