---
lang: dz
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

I18NH0000002X

---
slug: /norito/དཔེར་ན་ སྤོ་བཤུད་-རྒྱུ་དངོས་ཚུ།
title: རྩིས་ཐོ་ཚུའི་བར་ན་སྤོ་བཤུད་ཀྱི་བདོག་གཏད།
འགྲེལ་བཤད་: ཨེསི་ཌི་ཀེ་ མགྱོགས་དྲགས་འགོ་བཙུགས་དང་ ལག་དེབ་ཀྱི་ འགྲུལ་བསྐྱོད་ཚུ་ མེ་ལོང་ནང་ འབད་མི་ ཕྲང་ཏང་ཏ་ རྒྱུ་དངོས་སྤོ་བཤུད་ཀྱི་ ལཱ་གི་རྒྱུན་རིམ་ ཕྲང་ཏང་ཏ་ཨིན།
source: དཔེར་བརྗོད/སྤོ་བཤུད་/སྤོ་བཤུད་.ཀོ།
---

ཨེསི་ཌི་ཀེ་ མགྱོགས་དྲགས་འགོ་བཙུགས་མི་དང་ ལེཌ་ཇར་གྱི་ འགྲུལ་བསྐྱོད་ཚུ་ མེ་ལོང་བཟོ་མི་ རྒྱུ་དངོས་སྤོ་བཤུད་ལཱ་གི་རྒྱུན་རིམ་ ཕྲང་ཏང་ཏ་ཨིན།

##

- སྔོན་དངུལ་ཨེ་ལིསི་འདི་དམིགས་གཏད་རྒྱུ་དངོས་དང་གཅིག་ཁར་ཨེ་ལིསི་ (དཔེར་ན་ “ཐོ་བཀོད་དང་ཨེམ་ཊི་” པར་ཆས་ ཡང་ན་ ཨེསི་ཌི་ཀེ་ མགྱོགས་དྲགས་ཀྱི་རྒྱུན་རིམ་ཚུ་བརྒྱུད་དེ་) བརྒྱུད་དེ་)།
- `do_transfer` འཛུལ་སྒོ་འདི་ ཨེ་ལིསི་ལས་ བོབ་ལུ་སྤོ་བཤུད་འབད་ནིའི་དོན་ལུ་ I18NI000000008X གནང་བ་འདི་ བསྒྲུབ་ཚུགསཔ་ཨིན།
- འདྲི་དཔྱད་ལྷག་ལུས་ཚུ་ (I18NI0000009X, `iroha_cli ledger assets list`) ཡང་ན་ སྤོ་བཤུད་གྲུབ་འབྲས་བལྟ་ནིའི་དོན་ལུ་ མདོང་ལམ་བྱུང་རིམ་ཚུ་ལུ་ མིང་རྟགས་བཀོད།

## འབྲེལ་བའི་ཨེས་ཌི་ཀེ་ལམ་སྟོན།

- [ལམ་ལུགས་ ཨེསི་ཌི་ཀེ་ མགྱོགས་མྱུར་](I18NU0000003X)
- [པའི་ཐོན་ཨེས་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་།](I18NU0000004X)
- [ཇ་བ་ཨིསི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་](I18NU0000005X)

[Kotodama འབྱུང་ཁུངས་](/norito-snippets/transfer-asset.ko) ཕབ་ལེན།

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```