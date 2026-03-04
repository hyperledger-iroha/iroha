---
id: developer-cli
lang: am
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

የተጠናከረው I18NI0000013X ወለል (በI18NI0000014X ሣጥን የቀረበ
የ `cli` ባህሪ ነቅቷል) SoraFS ለማዘጋጀት የሚያስፈልገውን እያንዳንዱን እርምጃ ያጋልጣል
ቅርሶች. ወደ የተለመዱ የስራ ሂደቶች በቀጥታ ለመዝለል ይህን የምግብ አዘገጃጀት መመሪያ ይጠቀሙ; ጋር አጣምሩት
አንጸባራቂው የቧንቧ መስመር እና ኦርኬስትራ runbooks ለአሰራር አውድ።

## ጥቅል ጭነት

ቆራጥ የሆኑ የCAR ማህደሮችን እና ቸንክ እቅዶችን ለማምረት `car pack` ይጠቀሙ። የ
እጀታ ካልተሰጠ በቀር ትዕዛዝ የ SF-1 chunkerን በራስ-ሰር ይመርጣል።

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- ነባሪ chunker እጀታ: `sorafs.sf1@1.0.0`.
- የማውጫ ግብዓቶች የሚራመዱት በቃላት ቅደም ተከተል ነው ስለዚህ ቼኮች ተረጋግተው ይቆያሉ።
  በመድረኮች ላይ.
- የJSON ማጠቃለያ የደመወዝ ጭነት መጨመሪያዎችን፣ በቸንክ ዲበዳታ እና ሥሩን ያካትታል
  CID በመዝገብ ቤት እና ኦርኬስትራ የታወቀ።

## የግንባታ መግለጫዎች

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` አማራጮች ካርታ በቀጥታ ወደ I18NI0000019X መስኮች ውስጥ
  `sorafs_manifest::ManifestBuilder`.
- CLI የSHA3 ቻንክን እንደገና እንዲያሰላስል ሲፈልጉ `--chunk-plan` ያቅርቡ
  ከማቅረቡ በፊት መፈጨት; አለበለዚያ በ ውስጥ የተካተተውን መፍጨት እንደገና ይጠቀማል
  ማጠቃለያ
- የJSON ውፅዓት የ Norito ክፍያን በቀጥታ ለሚፈጠሩ ልዩነቶች ያንጸባርቃል
  ግምገማዎች.

## ምልክት ያለ ረጅም ዕድሜ ቁልፎች ይገለጻል።

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- የመስመር ላይ ቶከኖችን፣ የአካባቢ ተለዋዋጮችን ወይም በፋይል ላይ የተመሰረቱ ምንጮችን ይቀበላል።
- የፕሮቬንሽን ሜታዳታ ይጨምራል (`token_source`፣ `token_hash_hex`፣ chunk diest)
  `--include-token=true` ካልሆነ በስተቀር ጥሬውን JWT ሳይቀጥል.
- በCI ውስጥ በደንብ ይሰራል፡ ከ GitHub Actions I18NT0000006X ጋር በማቀናበር ያጣምሩ
  `--identity-token-provider=github-actions`.

## ለTorii ይገለጣል

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- ለተለዋጭ ስም ማረጋገጫዎች Norito መፍታትን ያከናውናል እና ከሚከተሉት ጋር የሚዛመዱ መሆናቸውን ያረጋግጣል ።
  ወደ Torii ከመለጠፍ በፊት አንጸባራቂ መፍጨት።
- የማይዛመዱ ጥቃቶችን ለመከላከል የ SHA3 ዲጀስትን ከእቅዱ እንደገና ያሰላል።
- የምላሽ ማጠቃለያዎች የኤችቲቲፒ ሁኔታን፣ ራስጌዎችን እና የመመዝገቢያ ጭነቶችን ይይዛሉ
  በኋላ ኦዲት ማድረግ.

## የ CAR ይዘቶችን እና ማረጋገጫዎችን ያረጋግጡ

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- የPoR ዛፉን እንደገና ይገነባል እና የተጫኑትን ጨረሮች ከማንፀባረቂያው ማጠቃለያ ጋር ያወዳድራል።
- የማባዛት ማረጋገጫዎችን በሚያስገቡበት ጊዜ የሚፈለጉትን ቆጠራዎች እና መለያዎችን ይይዛል
  ወደ አስተዳደር.

## የዥረት ማረጋገጫ ቴሌሜትሪ

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- ለእያንዳንዱ የዥረት ማረጋገጫ የNDJSON ንጥሎችን ያወጣል (እንደገና መጫወትን ያሰናክሉ።
  `--emit-events=false`)።
- የስኬት/የሽንፈት ቆጠራዎችን፣ የቆይታ ሂስቶግራምን እና የናሙና ውድቀቶችን ያጠቃልላል
  የ JSON ማጠቃለያ ዳሽቦርዶች የምዝግብ ማስታወሻዎችን ሳይቧጠጡ ውጤቶችን ማቀድ ይችላሉ።
- መግቢያው አለመሳካቱን ወይም የአካባቢ የPoR ማረጋገጫን ሲዘግብ ከዜሮ ውጭ ይወጣል
  (በ`--por-root-hex` በኩል) ማረጋገጫዎችን ውድቅ ያደርጋል። ጣራዎቹን በ ጋር ያስተካክሉ
  ለልምምድ ሩጫዎች `--max-failures` እና `--max-verification-failures`።
- ዛሬ PoR ይደግፋል; ፒዲፒ እና ፖትአር SF-13/SF-14 አንድ ጊዜ ተመሳሳይ ፖስታ እንደገና ይጠቀማሉ
  መሬት.
- `--governance-evidence-dir` የተሰራውን ማጠቃለያ፣ ሜታዳታ (የጊዜ ማህተም፣
  የCLI ስሪት፣ መግቢያ ዩአርኤል፣ አንጸባራቂ መፍጨት) እና የገለጻው ቅጂ ወደ ውስጥ
  የአስተዳደር እሽጎች የማረጋገጫ ዥረቱን በማህደር እንዲቀመጡ የቀረበው ማውጫ
  ሩጫውን እንደገና ሳይጫወት ማስረጃ.

## ተጨማሪ ማጣቀሻዎች

- `docs/source/sorafs_cli.md` - የተሟላ ባንዲራ ሰነድ።
- `docs/source/sorafs_proof_streaming.md` - ማረጋገጫ የቴሌሜትሪ ንድፍ እና Grafana
  ዳሽቦርድ አብነት.
- `docs/source/sorafs/manifest_pipeline.md` - በመቁረጥ ላይ ጥልቅ ጠልቆ ፣ አንጸባራቂ
  ቅንብር, እና የ CAR አያያዝ.