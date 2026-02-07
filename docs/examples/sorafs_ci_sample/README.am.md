---
lang: am
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-12-29T18:16:35.082870+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS CI ናሙና ቋሚዎች

ይህ ማውጫ ከናሙናው የተፈጠሩትን የሚወስኑ ቅርሶችን ይይዛል
ጭነት በ `fixtures/sorafs_manifest/ci_sample/`. ጥቅሉ ያሳያል
ከጫፍ እስከ ጫፍ I18NT0000002X ማሸግ እና የ CI የስራ ፍሰቶች የሚለማመዱትን የቧንቧ መስመር መፈረም.

## የአርቴፍክት ክምችት

| ፋይል | መግለጫ |
|-------------|
| `payload.txt` | በቋሚ ስክሪፕቶች ጥቅም ላይ የዋለው የምንጭ ክፍያ (የጽሑፍ ናሙና)። |
| `payload.car` | በ`sorafs_cli car pack` የተለቀቀው የCAR ማህደር። |
| `car_summary.json` | በ`car pack` የተፈጠረ ማጠቃለያ ቸንክ ዳይጀስት እና ሜታዳታ። |
| `chunk_plan.json` | አምጣ-ፕላን JSON ቸንክ ክልሎችን እና የአቅራቢዎችን የሚጠበቁ ነገሮችን የሚገልጽ። |
| `manifest.to` | Norito አንጸባራቂ በI18NI0000013X የተሰራ። |
| `manifest.json` | ለማረም በሰው ሊነበብ የሚችል አንጸባራቂ አቀራረብ። |
| `proof.json` | በ`sorafs_cli proof verify` የወጣው የPoR ማጠቃለያ። |
| `manifest.bundle.json` | በ`sorafs_cli manifest sign` የተፈጠረ ቁልፍ የሌለው የፊርማ ቅርቅብ። |
| `manifest.sig` | ከመግለጫው ጋር የሚዛመድ የEd25519 ፊርማ። |
| `manifest.sign.summary.json` | በመፈረም ጊዜ የተለቀቀው የCLI ማጠቃለያ (ሀሽ፣ ጥቅል ዲበ ዳታ)። |
| `manifest.verify.summary.json` | የCLI ማጠቃለያ ከ I18NI0000022X። |

በመልቀቂያ ማስታወሻዎች እና በሰነድ ውስጥ የተጠቀሱ ሁሉም የምግብ አዘገጃጀቶች የተገኙት ከ ነው።
እነዚህ ፋይሎች. የ `ci/check_sorafs_cli_release.sh` የስራ ፍሰት ተመሳሳይ ያድሳል
ቅርሶች እና ከተፈጸሙት ስሪቶች ጋር ይለያቸዋል።

## ቋሚ እድሳት

የቋሚውን ስብስብ እንደገና ለማደስ ከማከማቻ ስር ያሉትን ትዕዛዞችን ያሂዱ።
እነሱ በ`sorafs-cli-fixture` የስራ ፍሰት የሚጠቀሙባቸውን ደረጃዎች ያንፀባርቃሉ፡

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

ማንኛውም እርምጃ የተለያዩ ሃሽዎችን የሚያመጣ ከሆነ እቃዎቹን ከማዘመንዎ በፊት ይመርምሩ።
የCI የስራ ፍሰቶች መመለሻዎችን ለመለየት በሚወስነው ውጤት ላይ ይመሰረታል።

## የወደፊት ሽፋን

ተጨማሪ chunker መገለጫዎች እና የማረጋገጫ ቅርጸቶች ከፍኖተ ካርታው ሲመረቁ፣
ቀኖናዊ መጫዎቻቸዉ በዚህ ማውጫ ስር ይታከላሉ (ለምሳሌ፡-
`sorafs.sf2@1.0.0` (`fixtures/sorafs_manifest/ci_sample_sf2/` ይመልከቱ) ወይም PDP
የዥረት ማረጋገጫዎች)። እያንዳንዱ አዲስ መገለጫ ተመሳሳይ መዋቅር ይከተላል-የክፍያ ጭነት፣ CAR፣
እቅድ፣ ገላጭ፣ ማረጋገጫዎች እና ፊርማ ቅርሶች -ስለዚህ የታችኛው አውቶማቲክ ማድረግ ይችላል።
diff ያለ ብጁ ስክሪፕት ይለቃል።