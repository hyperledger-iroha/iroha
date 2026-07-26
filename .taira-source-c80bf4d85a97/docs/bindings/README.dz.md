---
lang: dz
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK བསྡམས་དང་ བཅོས་བཟོ་གཞུང་སྐྱོང་།

ལམ་གྱི་ས་ཁྲ་གུ་ཡོད་པའི་ WP1-E གིས་ “docs/bindings” འདི་ ཁྲིམས་མཐུན་གྱི་ས་གནས་ཅིག་སྦེ་ འབོཝ་ཨིན།
སྐད་ཡིག་བརྒལ་བའི་བསྡམས་པའི་གནས་སྟངས། ཡིག་ཆ་འདི་གིས་ བཱའིན་ཌིང་ཐོ་གཞུང་འདི་ཐོ་བཀོད་འབདཝ་ཨིན།
བསྐྱར་གསོ་བརྡ་བཀོད་དང་ ཌིཕཊ་སྲུང་སྐྱོབ་ཚུ་ དེ་ལས་ སྒྲུབ་བྱེད་ཀྱི་ས་གནས་ཚུ་ དེ་འབདཝ་ལས་ ཇི་པི་ཡུ་ ཆ་སྙོམས་ཚུ།
གཱེཊསི་ (WP1-E/F/G) དང་ ཕར་སི་ཌི་ཀེ་ གདམ་ཁའི་ཚོགས་སྡེ་ལུ་ གཞི་བསྟུན་གཅིག་ཡོདཔ་ཨིན།

## བརྗེ་སོར་གྱི་སྲུང་སྐྱོབ།
- **ཀེ་ན་ནིག་རྩེད་དེབ་:** `docs/source/norito_binding_regen_playbook.md` ཡིག་སྡེབ་ཚུ་ ཕྱིར་ཐོན་འབདཝ་ཨིན།
  བསྒྱིར་བའི་སྲིད་བྱུས་དང་ རེ་བ་བསྐྱེད་པའི་སྒྲུབ་བྱེད་ དེ་ལས་ Android གི་དོན་ལུ་ ཡར་འཕར་གྱི་ལཱ་གི་རྒྱུན་རིམ་ཚུ་ཨིན།
  Swift, Python, དང་མ་འོངས་པའི་བཱའིན་ཌིང་ཚུ།
- **I18NT0000000X ལས་འཆར་གྱི་ཆ་སྙོམས་:** I18NI000000004X (བརྒྱུད་འཛིན་འབད་ཡོདཔ།
  I18NI000000005X དང་ CI ནང་།
  `ci/check_norito_bindings_sync.sh`) གིས་ རཱསི་ ཇ་བ་ ཡང་ན་ པའི་ཐོན་ཚུ་ བཟོ་བསྐྲུན་འབདཝ་ཨིན།
  schema artfacts འཕྱུལ།
- **ཀེ་ཌེནསི་ལྟ་སྐྱོང་:** I18NI000000007X ལྷག་ཡོད།
  `artifacts/*_fixture_regen_state.json` ཡིག་སྣོད་ཚུ་དང་ ཊུའུ་/ཕིརི་ (ཨེན་ཌོའིཌ་,, , ,
  Python) དང་ Wed (Swift) སྒོ་སྒྲིག་ཚུ་ དེ་འབདཝ་ལས་ ལམ་གྱི་ས་ཁྲ་ཚུ་ལུ་ རྩིས་ཞིབ་འབད་བཏུབ་པའི་དུས་ཚོད་ཀྱི་རྟགས་བཀོད་ཡོདཔ་ཨིན།

## བསྡམས་པའི་མེ་རིགས།

| བསྡམས་པ། | འཛུལ་སའི་ས་ཚིགས། | རིམ་སྒྲིག་/ བསྐྱར་བཟོ་བརྡ་བཀོད་ | འདྲེན་བྱེད་སྲུང་སྐྱོབ་པ་ | སྒྲུབ་བྱེད་ |
|---|-|------------------------------------------------------------------------------------------------------------------- |
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | I18NI000000011X → `artifacts/android_fixture_regen_state.json` | I18NI0000000013X, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| སུའིཕཊ་ (iOS/macOS) | `IrohaSwift/` (I18NI0000018X) | `scripts/swift_fixture_regen.sh` (གདམ་ཁ་ཅན་གྱི་ I18NI0000020X) → `artifacts/swift_fixture_regen_state.json` | I18NI0000002X, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| པའི་ཐོན་ | `python/iroha_python/` (I18NI0000028X) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | I18NI0000031X, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| ཇ་བ་ཨིསི་ཀིརིཔཊི་ | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh`, `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs`, `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, I18NI000000045X, I18NI000000046X |

## ཁ་གསལ་བསྡམས་པ།

### ཨེན་རོལཌ་ (ཇ་ཝ)
Android SDK འདི་ I18NI000000047X གི་འོག་ལུ་སྡོད་ཞིནམ་ལས་ ཀེ་ནོ་ནིག་ I18NT0000001X འདི་ ཟ་སྤྱོད་འབདཝ་ཨིན།
`scripts/android_fixture_regen.sh` གིས་ཐོན་པའི་སྒྲིག་བཀོད་ཚུ། དེ་གིས་གྲོགས་རམ་ཕྱིར་ཚོང་།
རསཊི་ལག་ཆས་གློག་ཆས་ལས་ `.norito` གིས་ དུས་མཐུན་བཟོ་ནི།
I18NI000000050X, དང་ དྲན་ཐོ་ཚུ་ ཚད་གཞིའི་མེ་ཊ་ཌེ་ཊ་ དེ་
I18NI000000051X དང་གཞུང་སྐྱོང་གི་ བརྡ་ཁྱབ་ཚུ་གིས་ བཀོལ་སྤྱོད་འབདཝ་ཨིན། ཌཱའིཊི་འདི་ཨིན།
Norito བརྟག་དཔྱད་འབད་ཡོདཔ་ཨིན།
`ci/check_android_fixtures.sh`) དང་ `java/iroha_android/run_tests.sh` གིས་ .
སྦྱོང་བརྡར་ཚུ་ ཇེ་ཨེན་ཨའི་ བཱའིན་ཌིང་དང་ ཝརཀ་མེ་ནེ་ཇར་ གྱལ་རིམ་བསྐྱར་རྩེད་ དེ་ལས་ ཨིསི་ཊོང་བོགསི་ ཕོལཀ་བེག་ཚུ།
བསྒྱིར་བའི་སྒྲུབ་བྱེད་དང་ འཐུས་ཤོར་གྱི་དྲན་ཐོ་ དེ་ལས་ ཡིག་ཆ་ཚུ་ ལོག་སྤྲོད་དགོཔ་ཨིན།
I18NI0000005X.

### སུའིཕཊ་ (མེཀ་ཨོ་ཨེསི་/ཨའི་ཨོ་ཨེས་)
`IrohaSwift/` གིས་ I18NI000000057X བརྒྱུད་དེ་ I18NT0000002X གི་ པེ་ལོཌ་ཚུ་ གཅིག་མཚུངས་སྦེ་བཟོཝ་ཨིན།
ཡིག་གཟུགས་འདི་གིས་ བསྒྱིར་འཁོར་གྱི་ཇོ་བདག་དང་ ལྡོག་ཕྱོགས་ཁ་ཡིག་ དེ་ལས་ འབྱུང་ཁུངས་ (`live` vs I18NI000000059X) ཚུ་ ཐོ་བཀོད་འབདཝ་ཨིན།
ནང་ན་ I18NI000000060.
ཚད་གཞིའི་ཞིབ་དཔྱད་པ། I18NI000000061X བདག་འཛིན་འཐབ་མི་ཚུ་ལུ་ སྨན་ཁབ་བཙུགས་བཅུགཔ་ཨིན།
རསཊ་གིས་བཟོ་མི་ཡིག་མཛོད་ཚུ། `scripts/check_swift_fixtures.py` དང་།
I18NI000000063X བཱའིཊི་གནས་རིམ་གྱི་ འདྲ་མཉམ་དང་ ཨེསི་ཨེལ་ཨེ་གི་ལོ་ཚད་ཚད་ཚུ་ བརྟན་བཞུགས་འབདཝ་ཨིནམ་དང་ ཨིན།
`scripts/swift_fixture_regen.sh` གིས་ ལག་དེབ་ཀྱི་དོན་ལུ་ `SWIFT_FIXTURE_EVENT_TRIGGER` ལུ་རྒྱབ་སྐྱོར་འབདཝ་ཨིན།
འཁོར་རིམ་ཚུ། ཡར་འཕར་གྱི་ལཱ་གི་རྒྱུན་རིམ་ ཀེ་པི་ཨའི་ དང་ ཌེཤ་བོརཌི་ཚུ་ ཡིག་ཐོག་ལུ་བཀོད་དེ་ཡོད།
`ci/check_norito_bindings_sync.sh` དང་ གདངས་དབྱངས་འདི་ གཤམ་གསལ་གྱི་མདོར་བསྡུས་ཚུ།
I18NI000000067X.

### པའི་ཐན།
པའི་ཐོན་མཁོ་མངགས་འབད་མི་ (`python/iroha_python/`) གིས་ Android གི་སྒྲིག་ཆས་ཚུ་ བརྗེ་སོར་འབདཝ་ཨིན། རྒྱུགས༌ནི
I18NI000000069X གསར་ཤོས་`.norito` པེ་ལོཌ་ཚུ་ འཐེན་དོ་ཡོདཔ་ཨིན།
`python/iroha_python/tests/fixtures/`, དང་ དེ་ལས་ ཚད་གཞིའི་མེ་ཊ་ཌེ་ཊ་ ནང་ལུ་ བཏོན་གཏང་འོང་།
I18NI000000072X ཐེངས་དང་པོ་ལམ་ཐིག་བསྒྱིར་བ།
འདི་བཟུང་ཡོདཔ་ཨིན། I18NI000000073X དང་།
I18NI000000074X གཱེཊ་པི་ཊེཊ་ མའི་པི།
ས་གནས་དང་ CI ནང་ ཆ་སྙོམས། མཇུག་ལས་མཇུག་ཚུན་ཚོད་ (`docs/source/sdk/python/…`) དང་།
བཱའིན་ཌིང་རི་ཇེན་རྩེད་དེབ་འདི་གིས་ ཨེན་ཌོའིཌ་དང་གཅིག་ཁར་ བསྒྱིར་ཐངས་ཚུ་ མཉམ་འབྲེལ་འབད་ཐངས་སྐོར་ལས་ འགྲེལ་བཤད་རྐྱབ་ཨིན།
ཇོ་བདག་ཚུ།

### ཇ་བ་སི་ཀིརིཔ།
I18NI0000000076X འདི་ ས་གནས་ཀྱི་ `scripts/check_fixture_cadence.py` ཡིག་སྣོད་ཚུ་ལུ་ བརྟེན་མི་བཏུབ་ དེ་འབདཝ་ད་ WP1-E གི་རྗེས་འདེད་ཚུ།
འདི་གི་གསར་བཏོན་འབད་མི་སྒྲུབ་བྱེད་འདི་གིས་ GPU CI ལམ་ཚུ་ ཁུངས་གཏུག་ཆ་ཚང་སྦེ་ཡོདཔ་ཨིན། གསར་བཏོན
I18NI000000078X (18NI000000078X བརྒྱུད་དེ་ འབྱུང་ཁུངས་བཟུང་ཡོད།
I18NI000000079X), བཟོ་སྐྲུན།
I18NI000000080X ཡོད་པའི་ SBOM བཱན་ཌལ་ཚུ་གིས་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ གོ་རིམ་ཅན་གྱི་སྐམ་རན་འདི་ གཡོག་བཀོལཝ་ཨིན།
(I18NI0000081X) དང་ ཐོ་བཀོད་ཀྱི་ ཅ་རྙིང་ཚུ་ བདེན་དཔྱད་འབདཝ་ཨིན།
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. གྲུབ་འབྲས་མེ་ཊ་ཌེ་ཊ་འདི།
`artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/`, དང་ `artifacts/js/verification/`,
ལམ་སྟོན་ JS5/JS6 དང་ WP1-F བེན་ཀ་མཱརཀ་རན་ཚུ་གི་དོན་ལུ་ སྒྲུབ་བྱེད་ཚུ། ༢༠༢༠ ནང་དཔེ་སྐྲུན་དེབ་འདི།
I18NI000000087X འཕྲུལ་ཆས་འདི་གཅིག་ཁར་མཐུདཔ་ཨིན།