---
lang: dz
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

I18NH0000002X

---
slug: /norito/དཔེར་ན་/ཨེན་ཊི་-ཕོལོ།
title: Mint, སྤོ་བཤུད་དང་ NFT ཅིག་འཚིག་ནི།
འགྲེལ་བཤད་: མཇུག་ཚུན་ཚོད་ NFT གི་མི་ཚེ་འཁོར་རིམ་བརྒྱུད་དེ་ ལམ་འགྱོ་མི་ཚུ་: ཇོ་བདག་ལུ་ བརྡ་བཀོད་འབད་ནི་དང་ སྤོ་བཤུད་འབད་ནི་ མེ་ཊ་ཌེ་ཊ་ དེ་ལས་ མེ་བཏང་ནི།
འབྱུང་ཁུངས་: ཀེརེ་ཊི/ཨཝ་ཨེམ་/ཡིག་ཆ་/དཔེ་/12_nf_low.ko.
---

མཇུག་ཚུན་ཚོད་ NFT གི་མི་ཚེ་འཁོར་རིམ་བརྒྱུད་དེ་ འགྱོ་:

##

- ཨེན་ཨེཕ་ཊི་ངེས་ཚིག་ (དཔེར་ན་ I18NI000000007X) འདི་ ཇོ་བདག་/ཐོབ་མཁན་རྩིས་ཐོ་ཚུ་ འཚོལ་ཞིབ་ནང་ལག་ལེན་འཐབ་མི་ མཉམ་སྦྲགས་ (`i105...`, I18NI000000009X) དང་ཅིག་ཁར་ ངེས་གཏན་བཟོ།
- ཨེན་ཨེཕ་ཊི་ལུ་ `nft_issue_and_transfer` འཛུལ་སྒོ་འདི་ ཨེ་ལིསི་ལུ་སྤོ་བཤུད་འབད་ཞིནམ་ལས་ བཏོན་གཏང་ནི་དང་ བཏོན་གཏང་ནིའི་གསལ་བཀོད་འབད་མི་ མེ་ཊ་ཌེ་ཊ་ དར་ཆ་འདི་ མཉམ་སྦྲགས་འབད།
- ཨེན་ཨེཕ་ཊི་ ལེཌི་ཇར་མངའ་སྡེ་འདི་ I18NI000000011X ཡང་ན་ SDK འདྲ་མཉམ་ཚུ་དང་གཅིག་ཁར་ སྤོ་བཤུད་བདེན་དཔྱད་འབད་ནི་ལུ་ བདེན་དཔྱད་འབད་ཞིནམ་ལས་ རྒྱུ་དངོས་འདི་ བདེན་དཔྱད་འབད།

## འབྲེལ་བའི་ཨེས་ཌི་ཀེ་ལམ་སྟོན།

- [ལམ་ལུགས་ ཨེསི་ཌི་ཀེ་ མགྱོགས་མྱུར་](I18NU0000003X)
- [པའི་ཐོན་ཨེས་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་།](I18NU0000004X)
- [ཇ་བ་ཨིསི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་](I18NU0000005X)

[Kotodama འབྱུང་ཁུངས་](/norito-snippets/nft-flow.ko) ཕབ་ལེན།

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("soraカタカナ...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("soraカタカナ...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```