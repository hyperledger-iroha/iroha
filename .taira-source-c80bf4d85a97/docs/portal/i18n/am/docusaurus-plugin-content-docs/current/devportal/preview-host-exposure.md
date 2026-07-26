---
id: preview-host-exposure
lang: am
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

የ DOCS-SORA ፍኖተ ካርታ በተመሳሳይ ለመሳፈር እያንዳንዱን የህዝብ ቅድመ እይታ ይፈልጋል
ገምጋሚዎች በአካባቢው የሚለማመዱ በቼክሰም የተረጋገጠ ጥቅል። ይህን የሩጫ መጽሐፍ ተጠቀም
ገምጋሚ ተሳፍሮ (እና የግብዣ ማጽደቂያ ትኬት) ለማስቀመጥ ከተጠናቀቀ በኋላ
የቤታ ቅድመ እይታ አስተናጋጅ በመስመር ላይ።

## ቅድመ ሁኔታዎች

- ገምጋሚ የቦርዲንግ ሞገድ አጽድቆ ወደ ቅድመ እይታ መከታተያ ገብቷል።
- የቅርብ ጊዜ ፖርታል ግንባታ በI18NI0000013X እና በቼክሰም ስር ይገኛል።
  የተረጋገጠ (`build/checksums.sha256`)።
- SoraFS ቅድመ እይታ ምስክርነቶች (Torii URL፣ ባለስልጣን፣ የግል ቁልፍ፣ ገብቷል
  epoch) በአከባቢው ተለዋዋጮች ወይም በ JSON ውቅር ውስጥ ተከማችቷል
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)።
- የዲ ኤን ኤስ ለውጥ ትኬት በተፈለገው የአስተናጋጅ ስም (`docs-preview.sora.link` ፣
  `docs.iroha.tech`፣ ወዘተ.) እንዲሁም በጥሪ ላይ ያሉ እውቂያዎች።

## ደረጃ 1 - ጥቅሉን ይገንቡ እና ያረጋግጡ

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

የማረጋገጫው ስክሪፕት የፍተሻ ሰነድ ሲጠፋ ወይም ለመቀጠል ፈቃደኛ አይሆንም
መስተጓጎል፣ እያንዳንዱን የቅድመ እይታ ቅርስ ኦዲት እንዲደረግ ማድረግ።

## ደረጃ 2 - የ SoraFS ቅርሶችን ያሽጉ

የማይንቀሳቀስ ቦታን ወደ የሚወስን CAR/የግልፅ ጥንድ ይለውጡ። `ARTIFACT_DIR`
ለ `docs/portal/artifacts/` ነባሪዎች።

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

የመነጨውን I18NI0000020X፣ `portal.manifest.*`፣ ገላጭ እና ቼክ ያያይዙ
ለቅድመ እይታ ሞገድ ቲኬት አንጸባራቂ።

## ደረጃ 3 - የቅድመ እይታ ተለዋጭ ስም ያትሙ

ለማጋለጥ ዝግጁ ከሆኑ በኋላ የፒን አጋዥውን እንደገና ያሂዱ ** ያለ *** `--skip-submit`
አስተናጋጁ. የJSON ውቅር ወይም ግልጽ የCLI ባንዲራዎችን ያቅርቡ፡

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

ትዕዛዙ `portal.pin.report.json` ይጽፋል፣
`portal.manifest.submit.summary.json`፣ እና `portal.submit.response.json`፣ ይህም
ከግብዣው ማስረጃ ጥቅል ጋር መላክ አለበት።

## ደረጃ 4 - የዲ ኤን ኤስ መቁረጫ እቅድ ይፍጠሩ

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

የዲ ኤን ኤስ መቀየሪያው በትክክል እንዲጠቅስ የተገኘውን JSON ለኦፕስ ያጋሩ
አንጸባራቂ መፍጨት. የቀድሞ ገላጭን እንደ የመመለሻ ምንጭ እንደገና ሲጠቀሙ፣
አባሪ `--previous-dns-plan path/to/previous.json`.

## ደረጃ 5 - የተዘረጋውን አስተናጋጅ መርምር

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

መርማሪው የቀረበውን የመልቀቂያ መለያ፣ የCSP ራስጌዎችን እና የፊርማ ዲበ ውሂብን ያረጋግጣል።
ኦዲተሮች ማየት እንዲችሉ ከሁለት ክልሎች ትዕዛዙን ይድገሙት (ወይም የከርል ውፅዓት ያያይዙ)
የጠርዝ መሸጎጫ ሞቃት እንደሆነ.

## የማስረጃ ጥቅል

በቅድመ-እይታ ማዕበል ትኬት ውስጥ የሚከተሉትን ቅርሶች ያካትቱ እና ወደ ውስጥ ያመልክቱ
የግብዣ ኢሜል፡-

| Artefact | ዓላማ |
|-------|--------|
| `build/checksums.sha256` | ጥቅሉ ከCI ግንባታው ጋር እንደሚመሳሰል ያረጋግጣል። |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | ቀኖናዊ SoraFS ጭነት + አንጸባራቂ። |
| `portal.pin.report.json`፣ `portal.manifest.submit.summary.json`፣ `portal.submit.response.json` | አንጸባራቂ ማስረከብ + ተለዋጭ ስም ማሰር እንደተሳካ ያሳያል። |
| `artifacts/sorafs/portal.dns-cutover.json` | የዲኤንኤስ ሜታዳታ (ቲኬት፣ መስኮት፣ አድራሻዎች)፣ የመንገድ ማስተዋወቂያ (`Sora-Route-Binding`) ማጠቃለያ፣ የ`route_plan` ጠቋሚ (የእቅድ JSON + አርዕስት አብነቶች)፣ የመሸጎጫ ማጽዳት መረጃ እና ለኦፕስ የመመለሻ መመሪያዎች። |
| `artifacts/sorafs/preview-descriptor.json` | የተፈረመ ገላጭ ማህደሩን + ቼክ ድምርን አንድ ላይ እያሰረ ነው። |
| `probe` ውፅዓት | የቀጥታ አስተናጋጁ የሚጠበቀውን የመልቀቂያ መለያ እንደሚያስተዋውቅ ያረጋግጣል። |

አንዴ አስተናጋጁ በቀጥታ ከተለቀቀ፣ [የግብዣ ጫወታ መጽሐፍን ቅድመ እይታ](./public-preview-invite.md) ይከተሉ።
ሊንኩን ለማሰራጨት፣ ግብዣዎችን ለመመዝገብ እና ቴሌሜትሪ ለመቆጣጠር።