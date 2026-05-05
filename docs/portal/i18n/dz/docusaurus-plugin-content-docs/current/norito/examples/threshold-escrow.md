<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 270c9c1079659b7c0f66d20b22a4453620d87bdb2877e66600db4a4b844c7924
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
slug: /norito/examples/threshold-escrow
title: ཐེམ་ཐོ་ཨེས་ཀོརོ།
description: དམིགས་གཏད་ཅན་གྱི་དངུལ་འབོར་ངེས་བདེན་ལུ་ ཁ་སྐོང་བཀལ་མི་ཚུ་ ངོས་ལེན་འབད་མི་ དངུལ་སྤྲོད་མི་རྐྱང་པའི་ བཀག་ཆ་ དེ་ལས་ མ་དངུལ་ཚུ་ བཏོན་གཏང་ནི་དང་ ཡང་ན་ ལོག་སྤྲོདཔ་ཨིན།
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

དམིགས་གཏད་ཅན་གྱི་དངུལ་འབོར་ངེས་བདེན་ལུ་ ཁ་སྐོང་བཀལ་མི་ཚུ་ ངོས་ལེན་འབད་མི་ དངུལ་ཕོགས་སྤྲོད་མི་རྐྱང་པ་གིས་ མ་དངུལ་ཚུ་ བཏོན་གཏང་ནི་དང་ ཡང་ན་ ལོག་སྤྲོདཔ་ཨིན།

## ལེ་ཇར་འགྲུལ་བཞུད་

- ཨེསི་ཀོརོ་རྩིས་ཁྲ་དང་ ཨང་གྲངས་རྒྱུ་དངོས་ངེས་ཚིག་ཚུ་ སྔོན་སྒྲིག་འབད་ཞིནམ་ལས་ གན་རྒྱ་འབོད་བརྡ་ཚུ་ བཙུགས་མི་ དངུལ་སྤྲོད་མི་རྩིས་ཁྲ་ལུ་ མ་དངུལ་བཙུགས། དཔེ་ཚད་འདི་གིས་ དངུལ་སྤྲོད་མི་འདི་ `open_escrow` གི་སྐབས་ལུ་ `authority()` དང་ཅིག་ཁར་ རང་བཞིན་གྱིས་ བསྡམ་བཞགཔ་ཨིན།
- གླ་ཆ་སྤྲོད་མི་དང་ ཐོབ་མི་ བཀག་ཆ་རྩིས་ཁྲ་ རྒྱུ་དངོས་ངེས་ཚིག་ དམིགས་ཚད་ངེས་བདེན་ དེ་ལས་ ཐུབ་ཚད་ཅན་གྱི་གན་རྒྱ་གནས་སྟངས་ནང་ ཁ་ཕྱེ་/བཏོན་བཏང་མི་/ལོག་སྤྲོད་མི་ དར་ཆ་ཚུ་ ཐོ་བཀོད་འབད་ནིའི་དོན་ལུ་ `open_escrow(recipient, escrow_account, asset_definition, target_amount)` ལུ་ ཚར་གཅིག་ཁ་པར་གཏང་།
- དངུལ་སྤྲོད་མི་གཅིག་ལས་ `funded_amount_value == target_amount_value` ཚུན་ཚོད་ `deposit(amount)` ལུ་ཁ་པར་གཏང་། དངུལ་བཙུགས་ཚུ་ ལེགས་ཤོམ་སྦེ་རང་ བཞག་དགོཔ་དང་ མ་དངུལ་མང་དྲགས་སྦེ་ བཙུགས་མི་ཚུ་ ངོས་ལེན་མི་འབད།
- དམིགས་གཏད་གྲུབ་ཚར་བའི་ཤུལ་ལས་ བཀག་ཆ་འབད་མི་མ་དངུལ་ཚུ་ ཐོབ་མི་ལུ་སྤོ་བཤུད་འབད་ནིའི་དོན་ལུ་ `release_if_ready()` ལུ་ཁ་པར་གཏང་ ཡང་ན་ བཀག་ཆ་འབད་མི་འདི་ ད་ལྟོ་ཡང་ ཁ་ཕྱེ་སྟེ་ཡོད་པའི་སྐབས་ `refund()` ལུ་ཁ་པར་གཏང་སྟེ་ མ་དངུལ་སྤྲོད་མི་ལུ་ ལོག་སྤྲོད་དགོ།
- `FindAssetById` / `iroha_cli ledger asset list` དང་ཅིག་ཁར་ ལྷག་ལུས་ཚུ་བརྟག་དཔྱད་འབད་ཞིནམ་ལས་ `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json` དང་ཅིག་ཁར་ གན་རྒྱ་གནས་སྟངས་བརྟག་དཔྱད་འབད།

## འབྲེལ་ཡོད་ཨེསི་ཌི་ཀེ་ལམ་སྟོན།

- [རསཊ་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་](/sdks/rust)
- [པའི་ཐོན་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས](/sdks/python)
- [ཇ་བ་སི་ཀིརིཔ་ཊི་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་](/sdks/javascript)

[Kotodama འབྱུང་ཁུངས་ཕབ་ལེན་བྱོས།](/norito-snippets/threshold-escrow.ko)

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