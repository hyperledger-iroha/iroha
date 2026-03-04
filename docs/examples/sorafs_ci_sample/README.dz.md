---
lang: dz
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

I18NH0000004X

# SoraFS སི་ཨའི་དཔེ་ཚད་གཏན་བཟོས།

སྣོད་ཐོ་ཐུམ་སྒྲིལ་འདི་གིས་ དཔེ་ཚད་ལས་བཏོན་ཡོད་པའི་ གཏན་འབེབས་བཟོ་རྙིང་ཚུ།
`fixtures/sorafs_manifest/ci_sample/` གི་འོག་ལུ་ pappack. བཱན་ཌལ་གྱིས་ འདི་སྟོནམ་ཨིན།
མཐའ་མའི་ to-མཐའ་མའི་ SoraFS ཐུམ་སྒྲིལ་དང་ CI ལཱ་གི་རྒྱུན་རིམ་འདི་ མཚན་རྟགས་བཀོད་པའི་ མདོང་ལམ་ཅིག་ཨིན།

## ཅ་རྙིང་ཐོ་ཡིག།

| ཡིག་སྣོད་ | འགྲེལ་བཤད་ |
|------|------------------------------------------------------------------------------------------------------
| I18NI0000006X | སྒྲིག་ཆས་ཡིག་ཚུགས་ཚུ་གིས་ལག་ལེན་འཐབ་མི་ འབྱུང་ཁུངས་པེ་ལོཌི་ (ཚིག་ཡིག་དཔེ་ཚད་ངོ་མ་)། |
| `payload.car` | CAR གཏན་མཛོད་ `sorafs_cli car pack` གིས་བཏོན་ཡོདཔ། |
| I18NI0000009X | `car pack` བཏོན་པའི་ ཅནཀ་བཞུ་ནི་དང་ མེ་ཊ་ཌེ་ཊ་གིས་ བཟོ་བཏོན་འབད་མི་ བཅུད་བསྡུས་དང་ མེ་ཊ་ཌེ་ཊ་ཚུ་ཨིན། |
| `chunk_plan.json` | Fetch-plan-JSON གིས་ ཆ་ཤས་ཁྱབ་ཚད་དང་ བྱིན་མི་གི་རེ་བ་ཚུ་ འགྲེལ་བཤད་རྐྱབ་ཨིན། |
| `manifest.to` | I18NT000000000X `sorafs_cli manifest build` ལས་ཐོན་པའི་མངོན་བྱ། |
| `manifest.json` | རྐྱེན་སེལ་གྱི་དོན་ལུ་ མི་གིས་ལྷག་ཚུགས་པའི་ གསལ་སྟོན་འབད་ནི། |
| `proof.json` | Por བཅུད་བསྡུས་ `sorafs_cli proof verify` གིས་བཏོན་ཡོདཔ། |
| `manifest.bundle.json` | I18NI0000018X བཟོ་བཏོན་འབད་ཡོད་མི་ ལྡེ་མིག་མེད་པའི་མིང་རྟགས་བང་སྒྲིག་ཚུ། |
| `manifest.sig` | མངོན་རྟགས་ལུ་མཐུན་སྒྲིག་ཡོད་པའི་ Ed25519 མཚན་རྟགས་ བཏོན་གཏང་ཡོདཔ། |
| `manifest.sign.summary.json` | མཚན་རྟགས་བཀོད་པའི་སྐབས་ CLI བཅུད་བསྡུས་ (hash, bundle metadata). |
| I18NI0000021X | CLI བཅུད་བསྡུས་ `manifest verify-signature`. |

གསར་བཏོན་དྲན་འཛིན་དང་ ཡིག་ཆ་ཚུ་ གཞི་བསྟུན་འབད་ཡོད་པའི་ ཟས་བཅུད་ཚུ་ཆ་མཉམ་ ལས་ འབྱུང་ཁུངས་བཟོ་ཡོདཔ་ཨིན།
ཡིག་སྣོད་འདི་ཚུ། `ci/check_sorafs_cli_release.sh` ལཱ་གི་རྒྱུན་རིམ་འདི་གིས་ དེ་དང་འདྲ་བའི་བསྐྱར་བཟོ་འབདཝ་ཨིན།
ཅ་རྙིང་དང་ ཁས་བླངས་ཐོན་རིམ་ཚུ་ལུ་ འགལ་བ་འབདཝ་ཨིན།

## སྒྲིག་བཅོས་བསྐྱར་གསོ་།

བརྟན་བཞུགས་ཆ་ཚན་འདི་སླར་བཟོ་འབད་ནི་ལུ་ མཛོད་ཁང་རྩ་བའི་ནང་ལས་ འོག་གི་བརྡ་བཀོད་ཚུ་གཡོག་བཀོལ།
དེ་ཚུ་གིས་ `sorafs-cli-fixture` ལཱ་གི་རྒྱུན་རིམ་གྱིས་ལག་ལེན་འཐབ་མི་ གོ་རིམ་ཚུ་ གཟུགས་བརྙན་བཟོཝ་ཨིན།

```bash
sorafs_cli car pack \
  --input fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)" \
  --issued-at 1700000000 \
  > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature \
  --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
  > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

གལ་སྲིད་ གོ་རིམ་གང་རུང་གིས་ ཧ་ཤེ་སོ་སོ་བཏོན་པ་ཅིན་ བདེ་སྒྲིག་ཚུ་ དུས་མཐུན་བཟོ་མ་ཚར་བའི་ཧེ་མ་ ཞིབ་དཔྱད་འབད།
CI ལཱ་གི་རྒྱུན་རིམ་འདི་ འགྱུར་ལྡོག་ཚུ་ ཤེས་རྟོགས་འབད་ནི་ལུ་ གཏན་འབེབས་ཀྱི་ཐོན་འབྲས་ལུ་ བརྟེན་དོ་ཡོདཔ་ཨིན།

## མ་འོངས་ཁྱབ་ཁོངས།

ཁ་སྐོང་ཆ་ཤས་གསལ་སྡུད་དང་ བདེན་ཁུངས་ཀྱི་རྩ་སྒྲིག་ཚུ་ ལམ་སྟོན་ལས་ མཐོ་རིམ་ཤེས་ཚད་མཐར་འཁྱོལ་ཡོདཔ་ཨིན།
སྣོད་ཐོ་འདི་གི་འོག་ལུ་ དེ་ཚུ་གི་ཚད་ལྡན་སྒྲིག་བཀོད་ཚུ་ཁ་སྐོང་འབད་འོང་། (དཔེར་ན་,
I18NI0000025X (I18NI0000026X ལ་གཟིགས།) ཡང་ན་ PDP ལ་གཟིགས།
རྒྱུན་སྤེལ་བདེན་དཔང་ཚུ།) གསལ་སྡུད་གསརཔ་རེ་རེ་གིས་ གཞི་བཀོད་གཅིག་པ་ལུ་རྗེས་སུ་འཇུག་འོང་—payload, CAR,
འཆར་གཞི་དང་ གསལ་སྟོན་དང་ བདེན་ཁུངས་ དེ་ལས་ མཚན་རྟགས་ཀྱི་ཅ་ཆས་ཚུ་གིས་ མར་ལས་ འཕྲུལ་ཆས་ཚུ་ འབད་ཚུགས།
སྲོལ་སྒྲིག་ཡིག་གཟུགས་མེད་པར་ diff གསར་བཏོན་འབདཝ་ཨིན།