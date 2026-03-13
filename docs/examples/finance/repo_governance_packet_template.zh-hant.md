---
lang: zh-hant
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 回購協議治理包模板（路線圖 F1）

準備路線圖項所需的工件包時使用此模板
F1（存儲庫生命週期文檔和工具）。目標是向審稿人提供
列出每個輸入、哈希值和證據包的單個 Markdown 文件，以便
治理委員會可以重播提案中引用的字節。

> 將模板複製到您自己的證據目錄中（例如
> `artifacts/finance/repo/2026-03-15/packet.md`)，替換佔位符，並且
> 將其提交/上傳到下面引用的散列工件旁邊。

## 1. 元數據

|領域|價值|
|--------|--------|
|協議/變更標識符 | `<repo-yyMMdd-XX>` |
|準備時間/日期 | `<desk lead> – 2026-03-15T10:00Z` |
|評論者 | `<dual-control reviewer(s)>` |
|更改類型 | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
|託管人 | `<custodian id(s)>` |
|鏈接提案/公投 | `<governance ticket id or GAR link>` |
|證據目錄| ``artifacts/finance/repo/<slug>/`` |

## 2. 指令有效負載

記錄各服務台通過以下方式簽署的分階段 Norito 指令
`iroha app repo ... --output`。每個條目應包含發出的哈希值
文件以及投票後將提交的操作的簡短描述
通過。

|行動|文件| SHA-256 |筆記|
|--------|------|---------|--------|
|發起| `instructions/initiate.json` | `<sha256>` |包含經服務台+交易對手批准的現金/抵押品。 |
|追加保證金通知 | `instructions/margin_call.json` | `<sha256>` |捕獲觸發呼叫的節奏 + 參與者 ID。 |
|放鬆 | `instructions/unwind.json` | `<sha256>` |一旦條件滿足，反向腿的證明。 |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 託管人確認（僅限三方）

每當存儲庫使用 `--custodian` 時，請完成此部分。治理包
必須包含每個託管人的簽名確認以及哈希值
`docs/source/finance/repo_ops.md` §2.8 中引用的文件。

|託管人 |文件| SHA-256 |筆記|
|------------|------|---------|--------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` |簽署的 SLA 涵蓋託管窗口、路由賬戶和鑽探聯繫人。 |

> 將確認信息存儲在其他證據旁邊 (`artifacts/finance/repo/<slug>/`)
> 所以 `scripts/repo_evidence_manifest.py` 將文件記錄在同一棵樹中
> 分階段說明和配置片段。參見
> `docs/examples/finance/repo_custodian_ack_template.md` 表示可立即灌裝
> 與治理證據合約相匹配的模板。

## 3. 配置片段

粘貼將登陸集群的 `[settlement.repo]` TOML 塊（包括
`collateral_substitution_matrix`）。將哈希存儲在代碼片段旁邊，以便
審計員可以確認回購預訂時處於活動狀態的運行時策略
獲得批准。

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 批准後配置快照

公投或治理投票完成後，`[settlement.repo]`
更改已推出，從每個對等方捕獲 `/v2/configuration` 快照，以便
審計員可以證明批准的政策在整個集群中有效（參見
`docs/source/finance/repo_ops.md` §2.9 證據工作流程）。

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v2/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

|同行/來源|文件| SHA-256 |區塊高度 |筆記|
|----------------|------|---------|----------------|--------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` |配置推出後立即捕獲的快照。 |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` |確認 `[settlement.repo]` 與暫存的 TOML 匹配。 |

將摘要與對等 ID 一起記錄在 `hashes.txt`（或等效的
摘要），以便審閱者可以跟踪哪些節點吸收了更改。快照
位於 TOML 片段旁邊的 `config/peers/` 下，並將被拾取
由 `scripts/repo_evidence_manifest.py` 自動生成。

## 4. 確定性測試工件

附上最新的輸出：

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

記錄 CI 生成的日誌包或 JUnit XML 的文件路徑 + 哈希值
系統。

|文物|文件| SHA-256 |筆記|
|----------|------|---------|--------|
|生命週期證明日誌| `tests/repo_lifecycle.log` | `<sha256>` |使用 `--nocapture` 輸出捕獲。 |
|集成測試日誌| `tests/repo_integration.log` | `<sha256>` |包括替代+邊際節奏覆蓋。 |

## 5. 生命週期證明快照

每個數據包必須包含從以下位置導出的確定性生命週期快照
`repo_deterministic_lifecycle_proof_matches_fixture`。運行線束
啟用導出旋鈕，以便審閱者可以區分 JSON 框架並摘要
`crates/iroha_core/tests/fixtures/` 中跟踪的夾具（參見
`docs/source/finance/repo_ops.md` §2.7)。

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

或者使用固定的助手重新生成燈具並將它們複製到您的
一步收集證據：

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

|文物 |文件| SHA-256 |筆記|
|----------|------|---------|--------|
|快照 JSON | `repo_proof_snapshot.json` | `<sha256>` |由證明線束髮出的規範生命週期框架。 |
|摘要文件 | `repo_proof_digest.txt` | `<sha256>` |大寫十六進制摘要鏡像自 `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`；即使未更改也要附加。 |

## 6. 證據清單

生成整個證據目錄的清單，以便審核員可以驗證
散列而不解壓存檔。助手反映了所描述的工作流程
在 `docs/source/finance/repo_ops.md` §3.2 中。

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

|文物|文件| SHA-256 |筆記|
|----------|------|---------|--------|
|證據清單 | `manifest.json` | `<sha256>` |將校驗和包含在治理票/公投註釋中。 |

## 7. 遙測和事件快照

導出相關的 `AccountEvent::Repo(*)` 條目和任何儀表板或 CSV
`docs/source/finance/repo_ops.md` 中引用的導出。記錄文件+
這裡有哈希值，以便審閱者可以直接跳到證據。

|出口|文件| SHA-256 |筆記|
|--------|------|---------|--------|
|回購事件 JSON | `evidence/repo_events.ndjson` | `<sha256>` |原始 Torii 事件流已過濾到桌面帳戶。 |
|遙測 CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` |使用回購保證金面板從 Grafana 導出。 |

## 8. 批准和簽名

- **雙控簽名者：** `<names + timestamps>`
- **GAR / 分鐘摘要：** `<sha256>` 簽名的 GAR PDF 或分鐘上傳。
- **存儲位置：** `governance://finance/repo/<slug>/packet/`

## 9. 清單

完成後標記每個項目。

- [ ] 指令有效負載已暫存、散列並附加。
- [ ] 記錄配置片段哈希值。
- [ ] 捕獲+散列的確定性測試日誌。
- [ ] 生命週期快照 + 導出摘要。
- [ ] 生成證據清單並記錄哈希值。
- [ ] 事件/遙測導出捕獲+散列。
- [ ] 雙控制確認已存檔。
- [ ] GAR/分鐘上傳；上面記錄了摘要。

與每個數據包一起維護此模板可以保留治理 DAG
確定性，並為審計人員提供回購生命週期的便攜式清單
決定。