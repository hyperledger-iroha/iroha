---
lang: zh-hant
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 域名背書

域名背書讓運營商可以根據委員會簽署的聲明來控制域名的創建和重用。背書有效負載是記錄在鏈上的 Norito 對象，因此客戶端可以審核誰在何時證明了哪個域。

## 有效負載形狀

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`：規範域標識符
- `committee_id`：人類可讀的委員會標籤
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`：區塊高度邊界有效性
- `scope`：可選數據空間加上可選 `[block_start, block_end]` 窗口（包含），**必須**覆蓋接受塊高度
- `signatures`：`body_hash()` 上的簽名（用 `signatures = []` 背書）
- `metadata`：可選的 Norito 元數據（提案 ID、審核鏈接等）

## 執行

- 當啟用 Nexus 和 `nexus.endorsement.quorum > 0` 時，或者當每個域策略將域標記為必需時，需要認可。
- 驗證強制執行域/語句哈希綁定、版本、塊窗口、數據空間成員資格、到期/年齡和委員會法定人數。簽名者必須擁有具有 `Endorsement` 角色的實時共識密鑰。重播被 `body_hash` 拒絕。
- 域註冊附加的認可使用元數據密鑰 `endorsement`。 `SubmitDomainEndorsement` 指令使用相同的驗證路徑，該指令記錄審核背書，而無需註冊新域。

## 委員會和政策

- 委員會可以在鏈上註冊（`RegisterDomainCommittee`）或從配置默認值派生（`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`，id = `default`）。
- 通過 `SetDomainEndorsementPolicy`（委員會 ID、`max_endorsement_age`、`required` 標誌）配置每個域的策略。如果不存在，則使用 Nexus 默認值。

## CLI 助手

- 構建/簽署背書（將 Norito JSON 輸出到標準輸出）：

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- 提交認可：

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- 管理治理：
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

驗證失敗返回穩定的錯誤字符串（仲裁不匹配、過時/過期的背書、範圍不匹配、未知的數據空間、缺少委員會）。