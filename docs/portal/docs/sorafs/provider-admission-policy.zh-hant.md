---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-29T18:16:35.197199+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

> 改編自 [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md)。

# SoraFS 提供者准入和身份政策（SF-2b 草案）

本說明捕獲了 **SF-2b** 的可操作交付成果：定義和
執行准入工作流程、身份要求和證明
SoraFS 存儲提供商的有效負載。它擴展了高層流程
SoraFS 架構 RFC 中概述了剩餘工作
可跟踪的工程任務。

## 政策目標

- 確保只有經過審查的運營商才能發布 `ProviderAdvertV1` 記錄
  網絡會接受。
- 將每個廣告密鑰綁定到經政府批准的身份文件，
  經證明的端點和最低股權貢獻。
- 提供確定性驗證工具，以便 Torii、網關和
  `sorafs-node` 強制執行相同的檢查。
- 支持續訂和緊急撤銷，而不破壞確定性或
  工裝工效學。

## 身份和權益要求

|要求 |描述 |可交付成果 |
|----------|-------------|----------|
|廣告關鍵出處|提供商必須註冊用於簽署每個廣告的 Ed25519 密鑰對。准入包將公鑰與治理簽名一起存儲。 |使用 `advert_key`（32 字節）擴展 `ProviderAdmissionProposalV1` 架構，並從註冊表 (`sorafs_manifest::provider_admission`) 引用它。 |
|樁指針|入場需要一個指向活躍質押池的非零 `StakePointer`。 |在 `sorafs_manifest::provider_advert::StakePointer::validate()` 中添加驗證並在 CLI/測試中顯示錯誤。 |
|司法管轄區標籤 |提供商聲明管轄權+法律聯繫方式。 |使用 `jurisdiction_code` (ISO 3166-1 alpha-2) 和可選的 `contact_uri` 擴展提案架構。 |
|端點證明 |每個公佈的端點必須由 mTLS 或 QUIC 證書報告支持。 |定義 `EndpointAttestationV1` Norito 有效負載並將每個端點存儲在准入捆綁包內。 |

## 入學流程

1. **提案創建**
   - CLI：添加 `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …`
     生成 `ProviderAdmissionProposalV1` + 證明包。
   - 驗證：確保必填字段、權益 > 0、`profile_id` 中的規範分塊器句柄。
2. **治理認可**
   - 理事會使用現有的 `blake3("sorafs-provider-admission-v1" || canonical_bytes)` 簽名
     信封工具（`sorafs_manifest::governance` 模塊）。
   - 信封保留為 `governance/providers/<provider_id>/admission.json`。
3. **註冊中心攝取**
   - 實現共享驗證器（`sorafs_manifest::provider_admission::validate_envelope`）
     Torii/gateways/CLI 重用。
   - 更新 Torii 准入路徑以拒絕摘要或到期時間與信封不同的廣告。
4. **續訂和撤銷**
   - 添加 `ProviderAdmissionRenewalV1` 以及可選端點/權益更新。
   - 公開記錄吊銷原因並推送治理事件的 `--revoke` CLI 路徑。

## 實施任務

|面積 |任務|所有者 |狀態 |
|------|------|----------|--------|
|架構|在 `crates/sorafs_manifest/src/provider_admission.rs` 下定義 `ProviderAdmissionProposalV1`、`ProviderAdmissionEnvelopeV1`、`EndpointAttestationV1` (Norito)。在 `sorafs_manifest::provider_admission` 中實現，帶有驗證助手。 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 |存儲/治理| ✅ 已完成 |
| CLI 工具 |使用子命令擴展 `sorafs_manifest_stub`：`provider-admission proposal`、`provider-admission sign`、`provider-admission verify`。 |工具工作組 | ✅ |

CLI 流程現在接受中間證書包 (`--endpoint-attestation-intermediate`)，發出
規範提案/信封字節，並在 `sign`/`verify` 期間驗證理事會簽名。運營商可以
直接提供廣告主體，或重複使用簽名廣告，簽名文件可以通過配對提供
`--council-signature-public-key` 和 `--council-signature-file` 實現自動化友好。

### CLI 參考

通過 `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …` 運行每個命令。

- `proposal`
  - 所需標誌：`--provider-id=<hex32>`、`--chunker-profile=<namespace.name@semver>`、
    `--stake-pool-id=<hex32>`、`--stake-amount=<amount>`、`--advert-key=<hex32>`、
    `--jurisdiction-code=<ISO3166-1>`，以及至少一個 `--endpoint=<kind:host>`。
  - 每個端點認證需要 `--endpoint-attestation-attested-at=<secs>`，
    `--endpoint-attestation-expires-at=<secs>`，證書通過
    `--endpoint-attestation-leaf=<path>`（加上可選的 `--endpoint-attestation-intermediate=<path>`
    對於每個鏈元素）和任何協商的 ALPN ID
    （`--endpoint-attestation-alpn=<token>`）。 QUIC 端點可以提供傳輸報告
    `--endpoint-attestation-report[-hex]=…`。
  - 輸出：規範的 Norito 提案字節 (`--proposal-out`) 和 JSON 摘要
    （默認標準輸出或 `--json-out`）。
- `sign`
  - 輸入：提案 (`--proposal`)、簽名廣告 (`--advert`)、可選廣告正文
    (`--advert-body`)、保留紀元和至少一個理事會簽名。可提供簽名
    內聯 (`--council-signature=<signer_hex:signature_hex>`) 或通過文件組合
    `--council-signature-public-key` 與 `--council-signature-file=<path>`。
  - 生成經過驗證的信封 (`--envelope-out`) 和指示摘要綁定的 JSON 報告，
    簽名者計數和輸入路徑。
- `verify`
  - 驗證現有信封 (`--envelope`)，可選擇檢查匹配的提案，
    廣告，或廣告正文。 JSON 報告突出顯示摘要值、簽名驗證狀態、
    以及哪些可選工件相匹配。
- `renewal`
  - 將新批准的信封鏈接到先前批准的摘要。需要
    `--previous-envelope=<path>` 和後繼 `--envelope=<path>`（均為 Norito 有效負載）。
    CLI 驗證配置文件別名、功能和廣告鍵是否保持不變，同時
    允許權益、端點和元數據更新。輸出規範
    `ProviderAdmissionRenewalV1` 字節 (`--renewal-out`) 加上 JSON 摘要。
- `revoke`
  - 向提供商發出緊急 `ProviderAdmissionRevocationV1` 捆綁包，其信封必須
    被撤回。需要 `--envelope=<path>`、`--reason=<text>`，至少一個
    `--council-signature`，以及可選的 `--revoked-at`/`--notes`。 CLI 簽署並驗證
    撤銷摘要，通過 `--revocation-out` 寫入 Norito 有效負載，並打印 JSON 報告
    捕獲摘要和簽名計數。
|驗證|實現 Torii、網關和 `sorafs-node` 使用的共享驗證器。提供單元+CLI集成測試。 【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 |網絡 TL / 存儲 | ✅ 已完成 |
| Torii 集成 |將驗證程序引入 Torii 廣告攝取，拒絕不符合策略的廣告，發出遙測數據。 |網絡 TL | ✅ 已完成 | Torii 現在加載治理信封 (`torii.sorafs.admission_envelopes_dir`)，在攝取期間驗證摘要/簽名匹配，並顯示准入遙測。 【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
|續訂 |添加續訂/撤銷架構 + CLI 幫助程序，在文檔中發布生命週期指南（請參閱下面的運行手冊和中的 CLI 命令） `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 |存儲/治理| ✅ 已完成 |
|遙測|定義 `provider_admission` 儀表板和警報（缺少續訂、信封到期）。 |可觀察性| 🟠 進行中 |計數器 `torii_sorafs_admission_total{result,reason}` 存在；儀表板/警報待處理。 【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### 續訂和撤銷操作手冊

#### 預定更新（權益/拓撲更新）
1. 使用 `provider-admission proposal` 和 `provider-admission sign` 構建後續提案/廣告對，增加 `--retention-epoch` 並根據需要更新權益/端點。
2. 執行  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   該命令通過以下方式驗證未更改的功能/配置文件字段
   `AdmissionRecord::apply_renewal`，發出 `ProviderAdmissionRenewalV1`，並打印摘要
   治理日誌。 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. 替換 `torii.sorafs.admission_envelopes_dir` 中的先前信封，將更新 Norito/JSON 提交到治理存儲庫，並將更新哈希 + 保留紀元附加到 `docs/source/sorafs/migration_ledger.md`。
4. 通知操作員新信封已生效並監控 `torii_sorafs_admission_total{result="accepted",reason="stored"}` 以確認攝入。
5. 通過 `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` 重新生成並提交規範裝置； CI (`ci/check_sorafs_fixtures.sh`) 驗證 Norito 輸出保持穩定。

#### 緊急撤銷
1. 識別受損的信封並發出撤銷：
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI 對 `ProviderAdmissionRevocationV1` 進行簽名，通過以下方式驗證簽名集
   `verify_revocation_signatures`，並報告撤銷摘要。 【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. 從 `torii.sorafs.admission_envelopes_dir` 中取出信封，將撤銷 Norito/JSON 分發到准入緩存，並在治理分鐘中記錄原因哈希。
3.觀看`torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`以確認緩存刪除了已撤銷的廣告；將撤銷工件保留在事件回顧中。

## 測試和遙測- 在下面添加錄取建議和信封的黃金裝置
  `fixtures/sorafs_manifest/provider_admission/`。
- 擴展 CI (`ci/check_sorafs_fixtures.sh`) 以重新生成提案並驗證信封。
- 生成的賽程包括帶有規範摘要的 `metadata.json`；下游測試斷言
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`。
- 提供集成測試：
  - Torii 拒絕錄取信封丟失或過期的廣告。
  - CLI 往返提案 → 信封 → 驗證。
  - 治理續訂輪換端點證明而不更改提供商 ID。
- 遙測要求：
  - 在 Torii 中發出 `provider_admission_envelope_{accepted,rejected}` 計數器。 ✅ `torii_sorafs_admission_total{result,reason}` 現在顯示接受/拒絕的結果。
  - 在可觀察性儀表板中添加到期警告（7 天內到期更新）。

## 後續步驟

1. ✅ 完成了 Norito 架構更改並在
   `sorafs_manifest::provider_admission`。不需要功能標誌。
2. ✅ CLI 工作流程（`proposal`、`sign`、`verify`、`renewal`、`revoke`）通過集成測試進行記錄和執行；使治理腳本與運行手冊保持同步。
3. ✅ Torii 入場/發現攝取信封並暴露遙測計數器以進行接受/拒絕。
4. 注重可觀察性：完成准入儀表板/警報，以便在 7 天內到期的續訂引發警告（`torii_sorafs_admission_total`，到期儀表）。