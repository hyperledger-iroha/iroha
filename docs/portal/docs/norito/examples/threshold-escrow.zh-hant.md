<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54b6d543cff8df6e8fd50632cfed6265770edc33855f06912be603457c5b517e
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/threshold-escrow
title: 門檻託管
description: 單一付款人託管，接受精確目標金額的充值，然後釋放或退還資金。
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

單一付款人託管，接受準確目標金額的充值，然後釋放或退還資金。

## 帳本演練

- 預先建立託管帳戶和數位資產定義，然後為將提交合約呼叫的付款人帳戶提供資金。此範例在 `open_escrow` 期間自動將該付款人與 `authority()` 綁定。
- 呼叫 `open_escrow(recipient, escrow_account, asset_definition, target_amount)` 一次，記錄持久合約狀態下的付款人、收款人、託管帳戶、資產定義、確切目標以及開啟/釋放/退款標誌。
- 從同一付款人撥打 `deposit(amount)` 直至 `funded_amount_value == target_amount_value`；存款必須保持正值，任何超出託管資金的充值都會被拒絕。
- 一旦達到目標，請致電 `release_if_ready()` 將託管資金轉移給收款人，或在託管仍處於開放狀態時致電 `refund()` 將資金金額退還給付款人。
- 使用 `FindAssetById` / `iroha_cli ledger asset list` 檢查餘額，並使用 `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json` 檢查合約狀態。

## 相關SDK指南

- [Rust SDK 快速入門](/sdks/rust)
- [Python SDK 快速入門](/sdks/python)
- [JavaScript SDK 快速入門](/sdks/javascript)

[下載Kotodama原始碼](/norito-snippets/threshold-escrow.ko)

```text
// Threshold escrow sample for a single payer and an exact funding target.
// The payer is bound to authority() when the escrow is opened.
seiyaku ThresholdEscrow {
  meta { abi_version: 1; }

  state AccountId payer_account;
  state AccountId recipient_account;
  state AccountId escrow_account_id;
  state AssetDefinitionId escrow_asset_definition;
  state int target_amount_value;
  state int funded_amount_value;
  state bool is_open;
  state bool is_released;
  state bool is_refunded;

  fn assert_unopened() {
    assert(!is_open, "escrow already open");
    assert(!is_released, "escrow already released");
    assert(!is_refunded, "escrow already refunded");
  }

  fn assert_open() {
    assert(is_open, "escrow is not open");
    assert(!is_released, "escrow already released");
    assert(!is_refunded, "escrow already refunded");
  }

  fn assert_payer() {
    assert(authority() == payer_account, "only the payer may call this entrypoint");
  }

  kotoage fn main() {}

  // NOTE:
  // This sample uses permission(Admin) because it releases and refunds funds
  // from the configured escrow account.
  #[access(read="*", write="*")]
  kotoage fn open_escrow(recipient: AccountId,
                         escrow_account: AccountId,
                         asset_definition: AssetDefinitionId,
                         target_amount: int) permission(Admin) {
    assert_unopened();
    assert(target_amount > 0, "target_amount must be positive");

    payer_account = authority();
    recipient_account = recipient;
    escrow_account_id = escrow_account;
    escrow_asset_definition = asset_definition;
    target_amount_value = target_amount;
    funded_amount_value = 0;
    is_open = true;
    is_released = false;
    is_refunded = false;
  }

  #[access(read="*", write="*")]
  kotoage fn deposit(amount: int) permission(Admin) {
    assert_open();
    assert_payer();
    assert(amount > 0, "amount must be positive");

    let next_funded = funded_amount_value + amount;
    assert(next_funded <= target_amount_value, "deposit exceeds target_amount");

    transfer_asset(payer_account, escrow_account_id, escrow_asset_definition, amount);
    funded_amount_value = next_funded;
  }

  #[access(read="*", write="*")]
  kotoage fn release_if_ready() permission(Admin) {
    assert_open();
    assert(funded_amount_value == target_amount_value, "escrow is not fully funded");

    transfer_asset(
      escrow_account_id,
      recipient_account,
      escrow_asset_definition,
      funded_amount_value
    );
    is_open = false;
    is_released = true;
  }

  #[access(read="*", write="*")]
  kotoage fn refund() permission(Admin) {
    assert_open();
    assert_payer();

    let funded = funded_amount_value;
    if (funded > 0) {
      transfer_asset(
        escrow_account_id,
        payer_account,
        escrow_asset_definition,
        funded
      );
    }
    is_open = false;
    is_refunded = true;
  }
}
```