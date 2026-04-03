<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 270c9c1079659b7c0f66d20b22a4453620d87bdb2877e66600db4a4b844c7924
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
slug: /norito/examples/threshold-escrow
title: عتبة الضمان
description: ضمان دافع واحد يقبل عمليات زيادة المبلغ إلى المبلغ المستهدف المحدد، ثم يقوم بتحرير الأموال أو ردها.
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

ضمان دافع واحد يقبل عمليات زيادة المبلغ إلى المبلغ المستهدف المحدد، ثم يقوم بتحرير الأموال أو ردها.

## تجول دفتر الأستاذ

- قم بإنشاء حساب الضمان مسبقًا وتعريف الأصول الرقمية، ثم قم بتمويل حساب الدافع الذي سيرسل طلبات العقد. تقوم العينة بربط الدافع تلقائيًا بـ `authority()` أثناء `open_escrow`.
- اتصل بـ `open_escrow(recipient, escrow_account, asset_definition, target_amount)` مرة واحدة لتسجيل الدافع والمستلم وحساب الضمان وتعريف الأصول والهدف الدقيق والإشارات المفتوحة/المحررة/المستردة في حالة العقد الدائم.
- اتصل بـ `deposit(amount)` من نفس الدافع حتى `funded_amount_value == target_amount_value`؛ يجب أن تظل الودائع إيجابية ويتم رفض أي زيادة من شأنها أن تزيد من تمويل الضمان.
- اتصل بـ `release_if_ready()` لنقل أموال الضمان إلى المستلم بمجرد تحقيق الهدف، أو اتصل بـ `refund()` بينما لا يزال حساب الضمان مفتوحًا لإعادة المبلغ الممول إلى الدافع.
- فحص الأرصدة باستخدام `FindAssetById` / `iroha_cli ledger asset list` وفحص حالة العقد باستخدام `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json`.

## أدلة SDK ذات الصلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/threshold-escrow.ko)

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