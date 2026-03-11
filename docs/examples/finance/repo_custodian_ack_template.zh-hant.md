---
lang: zh-hant
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 回購託管人確認模板

當回購協議（雙邊或三方）引用託管人時使用此模板
通過 `RepoAgreement::custodian`。目標是記錄託管SLA、路由
賬戶，並在資產轉移之前鑽取聯繫人。將模板複製到您的
證據目錄（例如
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`），填寫
佔位符，並將文件作為治理數據包的一部分進行哈希處理，如
`docs/source/finance/repo_ops.md` §2.8。

## 1. 元數據

|領域|價值|
|--------|--------|
|協議標識符| `<repo-yyMMdd-XX>` |
|託管賬戶 ID | `<i105...>` |
|準備時間/日期 | `<custodian ops lead>` |
|已確認桌面聯繫人 | `<desk lead + counterparty>` |
|證據目錄| ``artifacts/finance/repo/<slug>/`` |

## 2. 託管範圍

- **收到的附帶定義：** `<list of asset definition ids>`
- **現金支線貨幣/結算軌：** `<xor#sora / other>`
- **保管窗口：** `<start/end timestamps or SLA summary>`
- **常規說明：** `<hash + path to standing instruction document>`
- **自動化先決條件：** `<scripts, configs, or runbooks custodian will invoke>`

## 3. 路由和監控

|項目 |價值|
|------|--------|
|託管錢包/賬本賬戶 | `<asset ids or ledger path>` |
|監控通道| `<Slack/phone/on-call rotation>` |
|聯繫鑽頭 | `<primary + backup>` |
|所需警報 | `<PagerDuty service, Grafana board, etc.>` |

## 4. 聲明

1. *託管準備情況：*“我們審查了分階段的 `repo initiate` 有效負載，
   上面的標識符並準備接受列出的 SLA 下的抵押品
   在§2中。”
2. *回滾承諾：*“我們將執行上面指定的回滾劇本，如果
   由事件指揮官指揮，並將提供 CLI 日誌以及哈希值
   `governance/drills/<timestamp>.log`”。
3. *證據保留：*“我們將保留該確認書，
   至少 `<duration>` 的指令和 CLI 日誌，並將它們提供給
   財務委員會應要求。”

在下面簽名（通過治理時可接受電子簽名）
跟踪器）。

|名稱 |角色 |簽名/日期|
|------|------|------------------|
| `<custodian ops lead>` |託管運營商| `<signature>` |
| `<desk lead>` |書桌| `<signature>` |
| `<counterparty>` |交易對手 | `<signature>` |

> 簽名後，對文件進行哈希處理（例如：`sha256sum custodian_ack_<cust>.md`）並
> 將摘要記錄在治理數據包表中，以便審閱者可以驗證
> 投票期間引用的確認字節。