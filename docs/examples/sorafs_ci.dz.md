---
lang: dz
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e17a07b8d98725e24b75710496b0b69b6d8878160c7f12884e6e1eef0c0a4af7
source_last_modified: "2026-01-03T19:37:11.140795+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CI Cookbook
summary: Reference GitHub Actions workflow bundling sign + verify steps with review notes.
translator: machine-google-reviewed
---

# I18NT00000000 CI བག་ལེབ་དེབ།

འདིའི་ནང་དུ་ `docs/source/sorafs_ci_templates.md` དང་ནང་དུ་ལམ་སྟོན་བཟོས།
མཚན་རྟགས་དང་བདེན་དཔྱད་ དེ་ལས་ བདེན་ཁུངས་ཞིབ་དཔྱད་ཚུ་ ག་དེ་སྦེ་ མཉམ་བསྡོམས་འབད་ནི་ཨིན་ན་ སྟོནམ་ཨིན།
reghtHub Actions ལས་ཀ།

I18NF0000002X

## དྲན་ཐོ།

- I18NI000000005X འདི་ རྒྱུག་མི་གུ་ཐོབ་དགོཔ་ཨིན་ (དཔེར་ན་ SoraFSX ཚུ་ གོ་རིམ་འདི་ཚུ་གི་ཧེ་མ་)
- ལཱ་གི་རྒྱུན་རིམ་འདི་གིས་ གསལ་ཏོག་ཏོ་ I18NT0000001X གི་ལྟདམོ་ལྟ་མི་ཅིག་ (ནཱ་ལུ་ I18NI000000007X); ཁྱོད་རའི་ཕུལ་སིའོ་སྲིད་བྱུས་དང་མཐུན་སྒྲིག་འབད་ནིའི་དོན་ལུ་ `--identity-token-audience` བདེ་སྒྲིག་འབད།
- གསར་བཏོན་འབད་མི་ མདོང་ལམ་འདི་གིས་ གཞུང་སྐྱོང་བསྐྱར་ཞིབ་ཀྱི་དོན་ལུ་ I18NI0000009X, `artifacts/manifest.sig`, དང་ I18NI000000011X ཚུ་ གཏན་མཛོད་འབད་དགོ།
- གཏན་འབེབས་ཀྱི་དཔེ་ཚད་ཀྱི་ཅ་རྙིང་ཚུ་ I18NI000000012X ནང་ལུ་སྡོད་དོ་ཡོདཔ་ཨིན། ཁྱོད་ལུ་ གསེར་གྱི་གསལ་སྟོན་དང་ ཆ་ཤས་འཆར་གཞི་ ཡང་ན་ ཆུ་དུང་འདི་ ལོག་རྩིས་མ་རྐྱབ་པར་ ཇེ་ཨེསི་ཨོ་ཨེན་ དགོ་པའི་སྐབས་ བརྟག་དཔྱད་ཚུ་ནང་ འདྲ་བཤུས་རྐྱབས།

## བསྒྲིགས་བཅོས་བདེན་དཔང་།

ལཱ་གི་རྒྱུན་རིམ་འདི་གི་དོན་ལུ་ གཏན་འབེབས་བཟོ་རྙིང་ཚུ་ འོག་ལུ་སྡོད་དོ་ཡོདཔ་ཨིན།
`fixtures/sorafs_manifest/ci_sample`. Pipelines གིས་ གོང་གི་རིམ་པ་ཚུ་ ལོག་གཏང་ཚུགས།
diff གི་ཐོན་འབྲས་ཚུ་ དཔེར་ན་ ཀེན་ནོ་ནིག་ཡིག་སྣོད་ཚུ་ལུ་རྒྱབ་འགལ་འབདཝ་ཨིན།

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

སྟོང་པའི་ཁྱད་པར་གྱིས་ བཟོ་བསྐྲུན་འབད་བའི་ བཱའིཊ་འདྲ་མཚུངས་ཀྱི་རྟགས་མཚན་དང་ འཆར་གཞི་ དེ་ལས་ དེ་ལས་ ངེས་གཏན་བཟོཝ་ཨིན།
མཚན་རྟགས་བསྡམས་པ། ཆ་ཚང་ཅིག་གི་དོན་ལུ་ `fixtures/sorafs_manifest/ci_sample/README.md` ལུ་བལྟ།
འཛིན་བཟུང་འབད་ཡོད་མི་ལས་ ཊེམ་པེལེཊི་གསར་བཏོན་དྲན་ཐོ་ཚུ་གུ་ཡོད་པའི་སྣོད་ཐོ་ཐོ་ཡིག་དང་ བསླབ་བྱ་ཚུ།
བཅུད་བསྡུས།