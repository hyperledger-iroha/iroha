---
lang: dz
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19babc3c24608a2eaaff8d205392c10a46f044feb91dd22c3cff4d7a0d12d542
source_last_modified: "2026-01-22T16:26:46.501854+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

I18NH0000002X

---
slug: /norito/དཔེར་ན་ ཐོ་བཀོད་དང་མིན་ཊི།
title: ཐོ་འགོད་མངའ་ཁོངས་དང་ མིནཊི་རྒྱུ་དངོས།
འགྲེལ་བཤད་: གནང་བ་ཡོད་པའི་མངའ་ཁོངས་གསར་བསྐྲུན་དང་ རྒྱུ་དངོས་ཐོ་བཀོད་ དེ་ལས་ གཏན་འབེབས་བཟོ་ནིའི་ མིན་ཊིང་ཚུ་ བརྡ་སྟོན་འབདཝ་ཨིན།
sours: ཀེརེ་ཊི/ཨཝ་ཨེམ་/ཡིག་ཆ་/དཔེ་/༡༣_ཐོ་བཀོད་འབད་མི་_དང་_མིན་ཊི་.ཀོ།
---

གནང་བ་ཡོད་པའི་མངའ་ཁོངས་གསར་བསྐྲུན་དང་ རྒྱུ་དངོས་ཐོ་བཀོད་ དེ་ལས་ གཏན་འབེབས་བཟོ་མི་ མིན་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།

##

- འགྲོ་ཡུལ་རྩིས་ཐོ་ (དཔེར་ན་ I18NI000000007X) གནས་ཏེ་ཡོདཔ་ངེས་གཏན་བཟོ་ཞིནམ་ལས་ ཨེསི་ཌི་ཀེ་མགྱོགས་དྲགས་རེ་རེ་ནང་ གཞི་སྒྲིག་གནས་རིམ་འདི་ མེ་ལོང་བཟོཝ་ཨིན།
- ROSE རྒྱུ་དངོས་ངེས་ཚིག་དང་ མིན་ཊི་ ༢༥༠ ཆ་ཚན་ཚུ་ ཚོང་འབྲེལ་གཅིག་ནང་ ཨེ་ལིསི་ལུ་ གསར་བསྐྲུན་འབད་ནི་ལུ་ `register_and_mint` འཛུལ་སྒོ་འདི་ འབོ་དགོ།
- མེ་ཏོག་འདི་ མཐར་འཁྱོལ་བྱུང་ཡོད་མི་ ངེས་གཏན་བཟོ་ནིའི་དོན་ལུ་ I18NI000000009X ཡང་ན་ I18NI000000010X བརྒྱུད་དེ་ ལྷག་ལུས་ཚུ་ བདེན་དཔྱད་འབད།

## འབྲེལ་བའི་ཨེས་ཌི་ཀེ་ལམ་སྟོན།

- [ལམ་ལུགས་ ཨེསི་ཌི་ཀེ་ མགྱོགས་མྱུར་](I18NU0000003X)
- [པའི་ཐོན་ཨེས་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་།](I18NU0000004X)
- [ཇ་བ་ཨིསི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་](I18NU0000005X)

[Kotodama འབྱུང་ཁུངས་](/norito-snippets/register-and-mint.ko) ཕབ་ལེན།

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("<katakana-i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```