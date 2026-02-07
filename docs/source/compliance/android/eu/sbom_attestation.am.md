---
lang: am
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

# SBOM እና ፕሮቨንስ ማረጋገጫ - አንድሮይድ ኤስዲኬ

| መስክ | ዋጋ |
|-------|------|
| ወሰን | አንድሮይድ ኤስዲኬ (`java/iroha_android`) + የናሙና መተግበሪያዎች (`examples/android/*`) |
| የስራ ፍሰት ባለቤት | መልቀቂያ ምህንድስና (አሌክሲ ሞሮዞቭ) |
| መጨረሻ የተረጋገጠ | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. የትውልድ የስራ ፍሰት

የረዳት ስክሪፕቱን ያሂዱ (ለ AND6 አውቶማቲክ የተጨመረ)

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

ስክሪፕቱ የሚከተሉትን ያከናውናል፡

1. `ci/run_android_tests.sh` እና `scripts/check_android_samples.sh` ያስፈጽማል።
2. CycloneDX SBOMs ለመገንባት በ`examples/android/` ስር ያለውን የግራድል መጠቅለያ ጠርቶታል ለ
   `:android-sdk`፣ `:operator-console`፣ እና `:retail-wallet` ከቀረበው ጋር
   `-PversionName`.
3. እያንዳንዱን SBOM ወደ `artifacts/android/sbom/<sdk-version>/` ከቀኖናዊ ስሞች ጋር ይገለበጣል
   (`iroha-android.cyclonedx.json`, ወዘተ.)

## 2. ፕሮቬንሽን እና መፈረም

ተመሳሳይ ስክሪፕት እያንዳንዱን SBOM በ `cosign sign-blob --bundle <file>.sigstore --yes` ይፈርማል
እና በመድረሻ ማውጫው ውስጥ `checksums.txt` (SHA-256) ያወጣል። `COSIGN` ያዘጋጁ
የአካባቢ ተለዋዋጭ ሁለትዮሽ ከ `$PATH` ውጭ የሚኖር ከሆነ። ስክሪፕቱ ካለቀ በኋላ፣
የቅርቅብ/የቼክተም ዱካዎችን እና የBuildkite አሂድ መታወቂያውን ይመዝግቡ
`docs/source/compliance/android/evidence_log.csv`.

## 3. ማረጋገጥ

የታተመ SBOM ለማረጋገጥ፡-

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

ውጤቱን SHA በ`checksums.txt` ከተዘረዘረው እሴት ጋር ያወዳድሩ። ገምጋሚዎች ጥገኝነት ዴልታዎች ሆን ብለው መሆናቸውን ለማረጋገጥ SBOMን ከቀዳሚው ልቀት ጋር ይለያሉ።

## 4. የማስረጃ ቅጽበታዊ ገጽ እይታ (2026-02-11)

| አካል | SBOM | SHA-256 | Sigstore ቅርቅብ |
|-------|-------|--------|------|
| አንድሮይድ ኤስዲኬ (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` ጥቅል ከ SBOM አጠገብ ተከማችቷል |
| ኦፕሬተር ኮንሶል ናሙና | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| የችርቻሮ ቦርሳ ናሙና | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

* (Hashes ከBuildkite run `android-sdk-release#4821` ተይዟል፤ ከዚህ በላይ ባለው የማረጋገጫ ትእዛዝ ይባዛሉ።)*

## 5. የላቀ ስራ

- በመልቀቂያ ቧንቧው ውስጥ የ SBOM + የኮሲንግ ደረጃዎችን ከጂኤ በፊት ያሂዱ።
- አንድ ጊዜ AND6 የማረጋገጫ ዝርዝሩ እንደተጠናቀቀ SBOMs ወደ ህዝባዊው የቅርስ ጥበብ ባልዲ ያንጸባርቁ።
- SBOM የሚወርዱ ቦታዎችን ከአጋር-ከፊት የመልቀቂያ ማስታወሻዎች ለማገናኘት ከሰነዶች ጋር ያስተባበሩ።