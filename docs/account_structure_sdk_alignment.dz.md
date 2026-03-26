---
lang: dz
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# I105 ཨེསི་ཌི་ཀེ་དང་ཨང་རྟགས་བདག་པོ་ཚུ་གི་དོན་ལུ་ ཐོ་ཡིག་དྲན་ཐོ།

སྡེ་ཚན་: རཱསི་ཨེསི་ཌི་ཀེ་ ཊའིཔ་སི་ཀིརིཔཊི་/ཇ་བ་ཨིསི་ཀིརིཔ་ཨེསི་ཌི་ཀེ་ པའི་ཐོན་ཨེསི་ཌི་ཀེ་ ཀོཊ་ལིན་ཨེསི་ཌི་ཀེ་ ཀོཌེཀ་ལག་ཆས་ཚུ།

སྐབས་དོན་: I18NI000000000X ད་ལྟ་ I105 རྩིས་ཁྲའི་ ID གྲུ་གཟིངས་བཏང་བའི་ I105 རྩིས་ཁྲའི་ ID བསྟན་ཡོད།
ལག་ལེན་འཐབ་ནི། ཨེསི་ཌི་ཀེ་ སྤྱོད་ལམ་དང་ བརྟག་དཔྱད་ཚུ་ ཀེ་ནོ་ནིག་ སི་ཊིབ་དང་གཅིག་ཁར་ ཕྲང་སྒྲིག་འབད་གནང་།

ལྡེ་མིག་གཞི་བསྟུན་ཚུ།
- ཁ་བྱང་ ཀོ་ཌེཀ་ + མགོ་ཡིག་སྒྲིག་བཀོད་ — I18NI000000001X §2
- གུག་གུག — `docs/source/references/address_curve_registry.md`
- Norm v1 མངའ་ཁོངས་འཛིན་སྐྱོང་ — `docs/source/references/address_norm_v1.md`
- ཕིག་ཅར་བེག་ཊར་ — `fixtures/account/address_vectors.json`

བྱ་བའི་རྣམ་གྲངས།
1. **ཀེ་ནོ་ནིག་ཨའུཊི་པུཊི་:** `AccountId::to_string()`/བཀྲམ་སྟོན་འབད་དགོཔ་ I105 རྐྱངམ་གཅིག
   (No I18NI0000006X རྗེས་འཇུག་)། ཀེ་ནོ་ནིག་ཧེགསི་འདི་ རྐྱེན་སེལ་འབད་ནིའི་དོན་ལུ་ཨིན། (I18NI000000007X)
2. **Accepted inputs:** parsers MUST accept only canonical Katakana i105 account literals. Reject non-canonical Katakana i105 literals, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105 ཅེག་སམ་:** `I105PRE || prefix || payload` ལས་ Blake2b-512 ལག་ལེན་འཐབ།
   the དང་པོ་ ༢ བཱའིཊི། བསྡམས་ཡོད་པའི་ཡི་གུ་གཞི་རྟེན་འདི་ **105** ཨིན།
༥ ** གུག་གུགཔ་གི་སྒྲ་སྒྲིག་:** ཨེསི་ཌི་ཀེ་ཚུ་ Ed25519-only ལུ་སྔོན་སྒྲིག་འབད། དོན་གསལ་གསལ་སྟོན་འབད་ནི།
   ML‐DSA/GOST/SM (Swift བཟོ་བསྐྲུན་གྱི་རྒྱལ་དར་; JS/Android I18NI0000016X). འབད
   secp256k1 འདི་ རཱསི་ཕྱི་ཁར་ སྔོན་སྒྲིག་གིས་ ལྕོགས་ཅན་བཟོ་ཡོདཔ་སྦེ་ མནོ་བསམ་མ་གཏང་།
6. **CAIP-10:** ད་ལྟའི་བར་དུ་ CAIP‐10 སབ་ཁྲ་ད་དུང་བཏང་མེད། མ་བཏོན།
   fult on CAIP‐10 བསྒྱུར་བཅོས་ཚུ།

ཀོ་ཌེཀ་/བརྟག་དཔྱད་ཚུ་དུས་མཐུན་བཟོ་ཚར་བའི་ཤུལ་ལས་ ངེས་དཔྱད་འབད་གནང་། ཁ་ཕྱེ་བའི་དྲི་བ་ཚུ་ བརྟག་ཞིབ་འབད་ཚུགས།
རྩིས་ཐོ་-ཁ་བྱང་ RFC ཐགསཔ་ནང་།