---
lang: zh-hant
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 回購結算操作手冊

本指南記錄了 Iroha 中回購協議和逆回購協議的確定性流程。
它涵蓋 CLI 編排、SDK 幫助程序以及預期的治理旋鈕，以便操作員能夠
無需編寫原始 Norito 有效負載即可啟動、保證金和解除協議。用於治理
檢查表、證據捕獲和欺詐/回滾程序請參閱
[`repo_ops.md`](./repo_ops.md)，滿足路線圖項 F1。

## CLI 命令

`iroha app repo` 命令對特定於存儲庫的幫助程序進行分組：

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
  --custodian soraカタカナ... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` 和 `repo unwind` 尊重 `--input/--output` 因此生成的 `InstructionBox`
  有效負載可以通過管道傳輸到其他 CLI 流中或立即提交。
* 通過 `--custodian <account>` 將抵押品傳送給三方託管人。當省略時，
  交易對手直接收到質押（雙邊回購）。
* `repo margin` 通過 `FindRepoAgreements` 查詢賬本並報告下一個預期保證金
  時間戳（以毫秒為單位）以及當前是否到期保證金回調。
* `repo margin-call`附加一條`RepoMarginCallIsi`指令，記錄margin檢查點和
  為所有參與者發出事件。如果節奏尚未過去或如果
  指示由非參與者提交。

## Python SDK 幫助器

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="soraカタカナ...",
    counterparty="soraカタカナ...",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* 兩個幫助器在調用 PyO3 綁定之前都會標準化數字數量和元數據字段。
* `RepoAgreementRecord` 鏡像運行時計劃計算，因此賬外自動化可以
  確定回調何時到期，而無需手動重新計算節奏。

## DvP / PvP 結算

`iroha app settlement` 命令階段交付與付款和付款與付款指令：

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from soraカタカナ... \
  --delivery-to soraカタカナ... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from soraカタカナ... \
  --payment-to soraカタカナ... \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from soraカタカナ... \
  --primary-to soraカタカナ... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from soraカタカナ... \
  --counter-to soraカタカナ... \
  --iso-xml-out trade_pvp.xml
```

* 支線數量接受整數或小數值，並根據資產精度進行驗證。
* `--atomicity` 接受 `all-or-nothing`、`commit-first-leg` 或 `commit-second-leg`。使用這些模式
  使用 `--order` 來表示如果後續處理失敗，哪個分支仍保持提交（`commit-first-leg`
  保持第一條腿處於應用狀態； `commit-second-leg` 保留第二個）。
* CLI 調用今天發出空指令元數據；在結算級別時使用Python助手
  需要附加元數據。
* 請參閱 [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) 了解 ISO 20022 字段映射
  支持這些指令（`sese.023`、`sese.025`、`colr.007`、`pacs.009`、`camt.054`）。
* 傳遞 `--iso-xml-out <path>` 以使 CLI 與 Norito 一起發出規範的 XML 預覽
  指示；該文件遵循上面的映射（DvP 為 `sese.023`，PvP`為 `sese.025`）。配對
  帶有 `--iso-reference-crosswalk <path>` 標記，以便 CLI 對照 `--delivery-instrument-id` 進行驗證
  Torii 在運行時准入期間使用相同的快照。

Python 助手鏡像 CLI 界面：

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
)
```

## 決定論和治理期望

Repo 指令完全依賴於 Norito 編碼的數字類型和共享的
`RepoGovernance::with_defaults` 邏輯。請記住以下不變量：* 數量按確定性 `NumericSpec` 值進行序列化：現金支線使用
  `fractional(2)`（兩位小數），側支腿使用 `integer()`。不提交
  更精確的值——運行時守衛會拒絕它們，並且同行會產生分歧。
* 三方存儲庫將託管賬戶 ID 保留在 `RepoAgreement` 中。生命週期和保證金事件
  發出 `RepoAccountRole::Custodian` 有效負載，以便保管人可以訂閱和協調庫存。
* 理髮率被限制為 10000bps (100%)，邊際頻率為整秒。提供
  這些規範單元中的治理參數與運行時期望保持一致。
* 時間戳始終為 unix 毫秒。所有助手將其原樣轉發至 Norito
  有效負載，以便對等方得出相同的時間表。
* 啟動和展開指令重用相同的協議標識符。運行時拒絕
  重複 ID 並解除未知協議； CLI/SDK 幫助程序會儘早發現這些錯誤。
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` 返回規範節奏。總是
  在觸發回調之前查閱此快照以避免重播過時的計劃。