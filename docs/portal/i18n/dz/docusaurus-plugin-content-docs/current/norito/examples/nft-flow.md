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

- ཨེན་ཨེཕ་ཊི་ངེས་ཚིག་ (དཔེར་ན་ I18NI000000007X) འདི་ ཇོ་བདག་/ཐོབ་མཁན་རྩིས་ཐོ་ཚུ་ འཚོལ་ཞིབ་ནང་ལག་ལེན་འཐབ་མི་ མཉམ་སྦྲགས་ (`<i105-account-id>`, I18NI000000009X) དང་ཅིག་ཁར་ ངེས་གཏན་བཟོ།
- ཨེན་ཨེཕ་ཊི་ལུ་ `nft_issue_and_transfer` འཛུལ་སྒོ་འདི་ ཨེ་ལིསི་ལུ་སྤོ་བཤུད་འབད་ཞིནམ་ལས་ བཏོན་གཏང་ནི་དང་ བཏོན་གཏང་ནིའི་གསལ་བཀོད་འབད་མི་ མེ་ཊ་ཌེ་ཊ་ དར་ཆ་འདི་ མཉམ་སྦྲགས་འབད།
- ཨེན་ཨེཕ་ཊི་ ལེཌི་ཇར་མངའ་སྡེ་འདི་ I18NI000000011X ཡང་ན་ SDK འདྲ་མཉམ་ཚུ་དང་གཅིག་ཁར་ སྤོ་བཤུད་བདེན་དཔྱད་འབད་ནི་ལུ་ བདེན་དཔྱད་འབད་ཞིནམ་ལས་ རྒྱུ་དངོས་འདི་ བདེན་དཔྱད་འབད།

## འབྲེལ་བའི་ཨེས་ཌི་ཀེ་ལམ་སྟོན།

- [ལམ་ལུགས་ ཨེསི་ཌི་ཀེ་ མགྱོགས་མྱུར་](I18NU0000003X)
- [པའི་ཐོན་ཨེས་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་།](I18NU0000004X)
- [ཇ་བ་ཨིསི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་ མགྱོགས་འགོ་བཙུགས་](I18NU0000005X)

[Kotodama འབྱུང་ཁུངས་](/norito-snippets/nft-flow.ko) ཕབ་ལེན།

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", );
        ledger::nft::transfer(source: owner, nft: nft, destination: to);
        ledger::nft::set_metadata(
            nft: nft,
            key: Name::parse("issued"),
            value: Json::parse("{\"issued\":\"demo\"}"),
        );
        ledger::nft::burn(nft);
    }
}
```
