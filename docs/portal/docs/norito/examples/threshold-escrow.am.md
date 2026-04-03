<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
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
title: የመግቢያ ገደብ
description: ነጠላ ከፋይ ክፍያን በትክክል የሚቀበል እና ገንዘቡን የሚለቀቅ ወይም የሚመለስ።
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

ነጠላ ከፋይ ክፍያን በትክክል የሚቀበል እና ገንዘቡን የሚለቀቅ ወይም የሚመለስ።

## የመመዝገቢያ መመሪያ

- የ escrow ሒሳቡን እና የቁጥር ንብረት ፍቺን አስቀድመው ይፍጠሩ፣ ከዚያም የኮንትራት ጥሪዎችን የሚያቀርበውን ከፋይ ሂሳብ ገንዘብ ይስጡ። ናሙናው ያንን ከፋይ በ`authority()` በ `open_escrow` ጊዜ በራስ-ሰር ያስራል።
- ከፋይ፣ ተቀባይ፣ የተጨበጠ መለያ፣ የንብረት ትርጉም፣ ትክክለኛ ኢላማ እና ክፍት/የተለቀቁ/የተመለሱ ባንዲራዎችን ዘላቂ በሆነ የኮንትራት ሁኔታ ለመመዝገብ አንድ ጊዜ `open_escrow(recipient, escrow_account, asset_definition, target_amount)` ይደውሉ።
- ከተመሳሳይ ከፋይ እስከ `funded_amount_value == target_amount_value` ድረስ `deposit(amount)` ይደውሉ; የተቀማጭ ገንዘብ አወንታዊ መሆን አለበት እና ማንኛውም ተጨማሪ ገንዘብ ማሸግ ውድቅ ይሆናል።
- ዒላማው ከተፈጸመ በኋላ የተመደበውን ገንዘብ ወደ ተቀባዩ ለማዘዋወር ወደ `release_if_ready()` ይደውሉ ወይም በገንዘብ የተደገፈውን ገንዘብ ለከፋዩ ለመመለስ አሁንም ክፍት ሆኖ እያለ ወደ `refund()` ይደውሉ።
- ሚዛኖችን ከ`FindAssetById`/`iroha_cli ledger asset list` ጋር ይፈትሹ እና ከ `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json` ጋር የውል ሁኔታን ይፈትሹ።

## ተዛማጅ የኤስዲኬ መመሪያዎች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[የKotodama ምንጭ ያውርዱ](/norito-snippets/threshold-escrow.ko)

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