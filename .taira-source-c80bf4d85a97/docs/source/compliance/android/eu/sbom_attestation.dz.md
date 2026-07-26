---
lang: dz
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM & Provenance Action — Android SDK

| ཕིལཌ་ | གནས་གོང་ |
|-------|--|-------------------------------------------------------------------------
| ཁྱབ་ཁོངས། | Android SDK (`java/iroha_android`) + དཔེ་ཚད་གློག་རིམ་ (`examples/android/*`) |
| ལཱ་གི་རྒྱུན་རིམ་གྱི་ཇོ་བདག་ | གསར་བཏོན་བཟོ་རིག་ (Alexei Morozov) |
| མཐའ་མའི་བདེན་དཔྱད་ | ༢༠༢༦-༠༢-༡༡ (སྒྲིང་ཁྱིམ་ `android-sdk-release#4821`) |

## 1. མི་རབས་ལས་རིམ།

གྲོགས་རམ་ཡིག་ཚུགས་ (AND6 རང་བཞིན་གྱི་དོན་ལུ་ཁ་སྐོང་འབད་ཡོདཔ་) འདི་གཡོག་བཀོལ།

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

ཡིག་གཟུགས་འདི་གིས་ གཤམ་གསལ་ཚུ་འབདཝ་ཨིན།

1. `ci/run_android_tests.sh` དང་ `scripts/check_android_samples.sh` ལག་ལེན་འཐབ་ཡོད།
2. CycloneDX SBOMs བཟོ་ནིའི་དོན་ལུ་ `examples/android/` གི་འོག་ལུ་ Gradle wrapper འབོཝ་ཨིན།
   `:android-sdk`, `:operator-console`, དང་ `:retail-wallet` བཅས་བཀྲམ་སྤེལ་འབད་ཡོདཔ།
   `-PversionName`.
༣ ཨེསི་བི་ཨོ་ཨེམ་རེ་རེ་ལུ་ ཀེ་ནོ་ནིག་མིང་ཚུ་དང་གཅིག་ཁར་ `artifacts/android/sbom/<sdk-version>/` ནང་ལུ་འདྲ་བཤུས་འབདཝ་ཨིན།
   (`iroha-android.cyclonedx.json` ལ་སོགས་པ།).

## 2. སྐུལ་འདེད་དང་མཚན་རྟགས།

དེ་བཟུམ་མའི་ཡིག་ཚུགས་འདི་གིས་ SBOM རེ་རེ་ལུ་ `cosign sign-blob --bundle <file>.sigstore --yes` དང་གཅིག་ཁར་ མཚན་རྟགས་བཀོདཔ་ཨིན།
དང་ འགྲོ་ཡུལ་སྣོད་ཐོ་ནང་ `checksums.txt` (SHA-256) བཏོནམ་ཨིན། `COSIGN` གཞི་སྒྲིག་འབད།
གཉིས་ལྡན་འདི་ གཉིས་ལྡན་འདི་ `$PATH` གི་ཕྱི་ཁར་སྡོད་པ་ཅིན། ཡིག་ཆ་འདི་མཇུག་བསྡུ་བའི་ཤུལ་ལས་།
བཱན་ཌལ་/ཅེག་སམ་འགྲུལ་ལམ་ཚུ་ དང་ བཱའིཊི་ཀི་ཊི་ རན་ཨའི་ཌི་ནང་ ཐོ་བཀོད་འབད།
`docs/source/compliance/android/evidence_log.csv`.

## 3. བདེན་དཔང་།

དཔར་བསྐྲུན་འབད་ཡོད་པའི་ SBOM བདེན་དཔྱད་འབད་ནི་ལུ་:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

ཐོན་འབྲས་ SHA འདི་ `checksums.txt` ནང་ཐོ་བཀོད་འབད་ཡོད་པའི་གནས་གོང་ལུ་ག་བསྡུར་འབད། བསྐྱར་ཞིབ་འབད་མི་ཚུ་གིས་ བརྟེན་པའི་ཌེལ་ཊ་ཚུ་ ཤེས་བཞིན་དུ་ཡོདཔ་ངེས་གཏན་བཟོ་ནི་ལུ་ ཧེ་མའི་གསར་བཏོན་ལུ་ ཨེསི་བི་ཨོ་ཨེམ་འདི་ བཀྲམ་སྤེལ་འབདཝ་ཨིན།

## 4. སྒྲུབ་བྱེད་ཀྱི་པར་ཆས་ (༢༠༢༦-༠༢-༡༡)

| ཆ་ཤས་ | SBOM | SHA-256 | Sigstore བུན་ནལ་ |
|---------------------------------------------------- -|
| ཨེན་ཌོརཌི་ཨེསི་ཌི་ཀེ་ (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` SBOM གི་སྦོ་ལོགས་ཁར་ གསོག་འཇོག་འབད་ཡོད་པའི་ བཱན་ཌལ་ |
| བཀོལ་སྤྱོད་པ་ཀོན་སོལ་དཔེ་ཚད་ | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| ཚོང་འབྲེལ་དངུལ་ཁང་གི་དཔེ་ཚད་ | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite from `android-sdk-release#4821` ལས་བཟུང་ཡོདཔ་ཨིན། གོང་འཁོད་བརྡ་བཀོད་བརྒྱུད་དེ་ བསྐྱར་བཟོ་འབད་ཡོདཔ།)*

## 5. ལས་འདས་པའི་ལས་ནི།

- ཇི་ཨེ་གི་ཧེ་མ་ གསར་བཏོན་པའིཔ་ལའིན་ནང་ལུ་ རིམ་པ་ཚུ་ རང་བཞིན་གྱིས་ SBOM + བསྒྲིགས་དགོ།
- མི་མང་རྙིང་པའི་ཅ་རྙིང་གི་བཱ་ཀེཊ་ལུ་ མེ་ལོང་ཨེསི་བི་ཨོ་ཨེམ་ཚུ་ ཨེ་ཨེན་ཌི་༦ གིས་ བརྟག་ཞིབ་ཐོ་ཡིག་འདི་ མཇུག་བསྡུ་ཡོདཔ་ཨིན།
- མཉམ་འབྲེལ་ཐོག་ལས་ གསར་བཏོན་འབད་ཡོད་པའི་ དྲན་ཐོ་ཚུ་ལས་ ཨེསི་བི་ཨོ་ཨེམ་ཕབ་ལེན་འབད་སའི་ས་ཁོངས་ཚུ་ འབྲེལ་མཐུད་འབད་ནི་ལུ་ ཌོཀསི་ཚུ་དང་གཅིག་ཁར་ མཉམ་འབྲེལ་འབད།