---
id: deploy-guide
lang: am
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## አጠቃላይ እይታ

ይህ የመጫወቻ መጽሐፍ የመንገድ ካርታ ንጥሎችን ይለውጣል **DOCS-7** (SoraFS ማተም) እና **DOCS-8**
(CI/ሲዲ ፒን አውቶሜሽን) ለገንቢው ፖርታል ሊተገበር ወደሚችል ሂደት።
የግንባታ/ሊንት ደረጃን፣ SoraFS ማሸግን፣ Sigstore-የተደገፈ አንጸባራቂን ይሸፍናል።
ፊርማ፣ ቅጽል ማስተዋወቂያ፣ ማረጋገጫ፣ እና የድጋሚ ልምምዶች እያንዳንዱ ቅድመ እይታ እና
የመልቀቂያ ጥበብ እንደገና ሊባዛ የሚችል እና ሊመረመር የሚችል ነው።

ፍሰቱ `sorafs_cli` ባለ ሁለትዮሽ (የተሰራው በ
`--features cli`)፣ የTorii የመጨረሻ ነጥብ ከፒን መዝገብ ፍቃዶች ጋር መድረስ፣ እና
የOIDC ምስክርነቶች ለI18NT0000004X። የረጅም ጊዜ ሚስጥሮችን ያከማቹ (`IROHA_PRIVATE_KEY` ፣
`SIGSTORE_ID_TOKEN`፣ Torii ማስመሰያዎች) በእርስዎ CI ቮልት ውስጥ; የአካባቢ ሩጫዎች እነሱን ሊያገኙ ይችላሉ።
ከሼል ኤክስፖርት.

## ቅድመ ሁኔታዎች

- መስቀለኛ መንገድ 18.18+ ከ `npm` ወይም I18NI0000090X ጋር።
- `sorafs_cli` ከ I18NI0000092X.
- Torii URL የሚያጋልጥ I18NI0000093X እና የባለስልጣን መለያ/የግል ቁልፍ
  መግለጫዎችን እና ተለዋጭ ስሞችን ማቅረብ የሚችል።
- OIDC ሰጪ (GitHub Actions፣ GitLab፣ የስራ ጫና መታወቂያ፣ ወዘተ)
  `SIGSTORE_ID_TOKEN`.
- አማራጭ፡ I18NI0000095X ለደረቅ ሩጫዎች እና
  `docs/source/sorafs_ci_templates.md` ለ GitHub/GitLab የስራ ፍሰት ስካፎልዲንግ።
- ይሞክሩት OAuth ተለዋዋጮችን (`DOCS_OAUTH_*`) ያዋቅሩ እና ያሂዱ
  ግንባታን ከማስተዋወቅዎ በፊት [የደህንነት ማጠንከሪያ ዝርዝር](./security-hardening.md)
  ከላብራቶሪ ውጭ. እነዚህ ተለዋዋጮች ሲጠፉ የፖርታል ግንባታው አሁን አይሳካም።
  ወይም የቲቲኤል/የድምጽ መስጫ ቁልፎች ከግዳጅ መስኮቶች ውጭ ሲወድቁ; ወደ ውጭ መላክ
  `DOCS_OAUTH_ALLOW_INSECURE=1` ሊጣሉ ለሚችሉ የአካባቢ ቅድመ እይታዎች ብቻ። ያያይዙት።
  የመልቀቂያ ቲኬቱ የብዕር ሙከራ ማስረጃ።

## ደረጃ 0 — ተኪ ጥቅል ሞክር

ቅድመ እይታን ወደ Netlify ወይም ጌትዌይ ከማስተዋወቅዎ በፊት ይሞክሩት ፕሮክሲውን ማህተም ያድርጉ
ምንጮች እና የተፈረመ OpenAPI አንጸባራቂ ወደ የሚወስን ጥቅል ይዋሃዳሉ፡-

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` ተኪ/መመርመሪያ/ተመለስ አጋዥዎችን ይገለበጣል፣
የOpenAPI ፊርማ ያረጋግጣል እና `release.json` ፕላስ ይጽፋል
`checksums.sha256`. ይህን ቅርቅብ ከNetlify/SoraFS ጌትዌይ ማስተዋወቂያ ጋር አያይዘው
ቲኬት ስለዚህ ገምጋሚዎች ትክክለኛውን የተኪ ምንጮች እና የ Torii ዒላማ ፍንጮችን እንደገና ማጫወት ይችላሉ
እንደገና ሳይገነባ. ጥቅሉ በደንበኛ ያቀረቧቸው ተሸካሚዎች እንደነበሩ ይመዘግባል
የነቃ (`allow_client_auth`) የታቀደ ልቀት እቅዱን እና የሲኤስፒ ህጎችን በአንድ ላይ ለማቆየት።

## ደረጃ 1 - ፖርታሉን ይገንቡ እና ያስገቧቸው

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` `scripts/write-checksums.mjs`ን በራስ ሰር ያስፈጽማል፡

- `build/checksums.sha256` — SHA256 አንጸባራቂ ለ`sha256sum -c` ተስማሚ።
- `build/release.json` — ሜታዳታ (`tag`፣ `generated_at`፣ `source`) ተሰክቷል።
  እያንዳንዱ CAR / አንጸባራቂ.

ገምጋሚዎች ቅድመ እይታን እንዲለዩ ሁለቱንም ፋይሎች ከCAR ማጠቃለያ ጋር በማህደር ያስቀምጡ
እንደገና ሳይገነቡ ቅርሶች.

## ደረጃ 2 - የማይንቀሳቀሱ ንብረቶችን ያሽጉ

የCAR ማሸጊያውን ከI18NT0000002X የውጤት ማውጫ ጋር ያሂዱ። ከታች ያለው ምሳሌ
ሁሉንም ቅርሶች በ `artifacts/devportal/` ይጽፋል።

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

ማጠቃለያው JSON የዚያን ፍንጭ ፍንጮችን ይይዛል፣ ይፈጫል፣ እና የማረጋገጫ እቅድ ማውጣት
`manifest build` እና CI ዳሽቦርዶች በኋላ እንደገና ጥቅም ላይ ይውላሉ።

## ደረጃ 2 ለ - ጥቅል OpenAPI እና SBOM አጋሮች

DOCS-7 የፖርታል ጣቢያውን፣ OpenAPI ቅጽበታዊ ገጽ እይታን እና የSBOM ጭነት ማተምን ይጠይቃል።
እንደ ልዩ መገለጫዎች መግቢያ መንገዶች `Sora-Proof`/`Sora-Content-CID` ዋና ሊሆኑ ይችላሉ
ለእያንዳንዱ artefact ራስጌዎች. የተለቀቀው ረዳት
(`scripts/sorafs-pin-release.sh`) የOpenAPI ማውጫን አስቀድሞ ጠቅልሏል
(`static/openapi/`) እና SBoms በ`syft` ወደ ተለያዩ የወጡ
`openapi.*`/`*-sbom.*` መኪናዎች እና ሜታዳታውን በ ውስጥ ይመዘግባል
`artifacts/sorafs/portal.additional_assets.json`. በእጅ የሚሰራውን ፍሰት በሚሰራበት ጊዜ,
ለእያንዳንዱ ጭነት የራሱ ቅድመ ቅጥያዎች እና የሜታዳታ መለያዎች ደረጃዎች 2-4 ን ይድገሙ
(ለምሳሌ `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). እያንዳንዱን መግለጫ/ተለዋጭ ስም ይመዝገቡ
ዲ ኤን ኤስ ከመቀየርዎ በፊት በ Torii (ጣቢያ ፣ OpenAPI ፣ portal SBOM ፣ OpenAPI SBOM) ያጣምሩ ።
የመግቢያ መንገዱ ለሁሉም የታተሙ ቅርሶች የተደረደሩ ማስረጃዎችን ማቅረብ ይችላል።

## ደረጃ 3 - አንጸባራቂውን ይገንቡ

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

የፒን ፖሊሲ ባንዲራዎችን በመልቀቂያ መስኮትዎ ላይ ያስተካክሉ (ለምሳሌ `--pin-storage-class
ሙቅ` ለካናሪዎች)። የJSON ተለዋጭ አማራጭ ግን ለኮድ ግምገማ ምቹ ነው።

## ደረጃ 4 - በ Sigstore ይመዝገቡ

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

ጥቅሉ አንጸባራቂ መፍጨትን፣ ቁርጥራጭ መፍጨትን እና የ BLAKE3 ሃሽ ይመዘግባል
OIDC ማስመሰያ JWT ሳይቀጥል። ሁለቱንም ጥቅል እና ተለያይተው ያስቀምጡ
ፊርማ; የምርት ማስተዋወቂያዎች ስራ ከመልቀቃቸው ይልቅ ተመሳሳይ ቅርሶችን እንደገና መጠቀም ይችላሉ።
የአካባቢ ሩጫዎች የአቅራቢውን ባንዲራዎች በ`--identity-token-env` (ወይም አዘጋጅ
`SIGSTORE_ID_TOKEN` በአከባቢው) ውጫዊ OIDC ረዳት ሲያወጣ
ማስመሰያ

## ደረጃ 5 - ወደ ፒን መዝገብ ያስገቡ

የተፈረመውን ዝርዝር መግለጫ (እና ቸንክ እቅድ) ለTorii ያቅርቡ። ሁልጊዜ ማጠቃለያ ይጠይቁ
ስለዚህ የተገኘው የመዝገብ ቤት መግቢያ/ተለዋጭ ማስረጃ ኦዲት ይደረጋል።

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

ቅድመ እይታን ወይም የካናሪ ተለዋጭ ስም (`docs-preview.sora`) ሲያወጡ፣ ይድገሙት
QA ከምርት በፊት ይዘትን ማረጋገጥ እንዲችል በልዩ ተለዋጭ ስም ማስረከብ
ማስተዋወቅ.

ተለዋጭ ማስያዣ ሶስት መስኮችን ይፈልጋል፡ `--alias-namespace`፣ `--alias-name`፣ እና
`--alias-proof`. አስተዳደር የማረጋገጫ ጥቅል (ቤዝ64 ወይም Norito ባይት) ያዘጋጃል።
የቅጽል ጥያቄው ሲፈቀድ; በ CI ሚስጥሮች ውስጥ ያከማቹ እና እንደ ሀ
`manifest submit` ከመጥራትዎ በፊት ፋይል ያድርጉ። እርስዎ ሲሆኑ የቅጽል ባንዲራዎቹ እንዳልተዋቀሩ ይተዉት።
ዲ ኤን ኤስ ሳይነኩ አንጸባራቂውን ብቻ ለመሰካት አስበዋል::

## ደረጃ 5 ለ - የአስተዳደር ፕሮፖዛል ይፍጠሩ

ማንኛውም ሶራ እንዲችል እያንዳንዱ መግለጫ ለፓርላማ ዝግጁ በሆነ ፕሮፖዛል መጓዝ አለበት።
ዜጋ ለውጡን ማስተዋወቅ የሚችለው ልዩ የትምህርት ማስረጃዎችን ሳይበደር ነው።
ከማስገባት/የምልክት ደረጃዎች በኋላ፣ አሂድ፡-

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` ቀኖናዊውን `RegisterPinManifest` ይይዛል
መመሪያ፣ ቸንክ ዳይጀስት፣ ፖሊሲ እና ተለዋጭ ስም ፍንጭ። ከአስተዳደር ጋር አያይዘው
ቲኬት ወይም የፓርላማ ፖርታል ስለዚህ ልዑካኑ እንደገና ሳይገነቡ ክፍያውን እንዲቀይሩ
ቅርሶቹ ። ምክንያቱም ትዕዛዙ የTorii የስልጣን ቁልፍን በጭራሽ አይነካውም
ዜጋ ሀሳቡን በአገር ውስጥ ማርቀቅ ይችላል።

## ደረጃ 6 - ማስረጃዎችን እና ቴሌሜትሪዎችን ያረጋግጡ

ከተሰካ በኋላ፣ የሚወስን የማረጋገጫ ደረጃዎችን ያሂዱ፡-

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- `torii_sorafs_gateway_refusals_total` እና ይመልከቱ
  `torii_sorafs_replication_sla_total{outcome="missed"}` ለአናማዎች።
- የ Try-It proxy እና የተቀዳ ሊንኮችን ለመጠቀም `npm run probe:portal` ን ያሂዱ
  አዲስ በተሰካው ይዘት ላይ።
- የተገለጹትን የክትትል ማስረጃዎች ይያዙ
  [ማተም እና ክትትል](./publishing-monitoring.md) ስለዚህ DOCS-3c
  የታዛቢነት በር ከህትመት ደረጃዎች ጎን ለጎን ረክቷል። ረዳቱ
  አሁን በርካታ የ`bindings` ግቤቶችን ይቀበላል (ጣቢያ ፣ OpenAPI ፣ portal SBOM ፣ OpenAPI
  SBOM) እና `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` በዒላማው ላይ ያስፈጽማል።
  በአማራጭ I18NI0000139X ጠባቂ በኩል አስተናጋጅ. ከዚህ በታች ያለው ጥሪ ሁለቱንም ሀ
  ነጠላ JSON ማጠቃለያ እና የማስረጃ ጥቅል (`portal.json`፣ `tryit.json`፣
  `binding.json`፣ እና `checksums.sha256`) በተለቀቀው ማውጫ፡-

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## ደረጃ 6 ሀ - የመግቢያ የምስክር ወረቀቶችን ያቅዱ

የ GAR ፓኬቶችን ከመፍጠርዎ በፊት የ TLS SAN/የፈተና እቅድን ያግኙ
ቡድን እና ዲ ኤን ኤስ አጽዳቂዎች ተመሳሳይ ማስረጃን ይገመግማሉ። አዲሱ ረዳት ያንጸባርቃል
ቀኖናዊ የዱር ካርድ አስተናጋጆችን በመዘርዘር DG-3 አውቶሜሽን ግብዓቶች፣
ቆንጆ አስተናጋጅ SANs፣ DNS-01 መለያዎች እና የሚመከሩ የኤሲኤምኢ ፈተናዎች፡-

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON ን ከመልቀቂያ ቅርቅቡ ጋር ያስፈጽሙ (ወይም ከለውጡ ጋር ይስቀሉት
ቲኬት) ኦፕሬተሮች የ SAN እሴቶችን ወደ Torii መለጠፍ ይችላሉ
`torii.sorafs_gateway.acme` ውቅር እና GAR ገምጋሚዎች ማረጋገጥ ይችላሉ።
ቀኖናዊ/ቆንጆ ካርታዎች ያለ ዳግም ሩጫ አስተናጋጅ ተዋጽኦዎች። ተጨማሪ ያክሉ
`--name` ነጋሪ እሴቶች ለእያንዳንዱ ቅጥያ በተመሳሳይ ልቀት ላይ አስተዋውቀዋል።

## ደረጃ 6 ለ - ቀኖናዊ አስተናጋጅ ካርታዎችን ያግኙ

የGAR ክፍያ ጭነቶችን ከመቅረጽዎ በፊት ለእያንዳንዱ የሚወስነውን አስተናጋጅ ካርታ ይመዝግቡ
ተለዋጭ ስም `cargo xtask soradns-hosts` እያንዳንዱ `--name` ወደ ቀኖናዊው ሃሽ
መለያ (`<base32>.gw.sora.id`)፣ አስፈላጊውን የዱር ካርድ ያወጣል።
(`*.gw.sora.id`)፣ እና ቆንጆውን አስተናጋጅ (`<alias>.gw.sora.name`) ያገኛል። ጽና
የዲጂ-3 ገምጋሚዎች የካርታ ስራውን ሊለያዩት በሚችሉት የተለቀቁ ቅርሶች ውስጥ ያለው ውጤት
ከ GAR ማስረከቢያ ጋር፡-

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

በ GAR ወይም በመግቢያው ላይ በፍጥነት ላለመሳካት `--verify-host-patterns <file>` ይጠቀሙ
አስገዳጅ JSON ከሚያስፈልጉት አስተናጋጆች አንዱን ይተዋል. ረዳቱ ብዙ ይቀበላል
የማረጋገጫ ፋይሎች, ሁለቱንም የGAR አብነት እና የ
stapled `portal.gateway.binding.json` በተመሳሳዩ ጥሪ፡-

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

የማጠቃለያ JSON እና የማረጋገጫ ምዝግብ ማስታወሻውን ከዲኤንኤስ/የበረንዳ ለውጥ ትኬት ጋር ያያይዙ
ኦዲተሮች ዳግመኛ ሳይሮጡ ቀኖናዊውን፣ የዱር ካርድ እና ቆንጆ አስተናጋጆችን ማረጋገጥ ይችላሉ።
የመነጩ ስክሪፕቶች. አዲስ ተለዋጭ ስሞች በተጨመሩ ቁጥር ትዕዛዙን እንደገና ያሂዱ
ጥቅል ስለዚህ ተከታይ የGAR ዝማኔዎች ተመሳሳይ የማስረጃ ዱካ ይወርሳሉ።

## ደረጃ 7 - የዲ ኤን ኤስ መቁረጫ ገላጭ ይፍጠሩ

የምርት ቆራጮች ኦዲት ሊደረግ የሚችል የለውጥ ፓኬት ያስፈልጋቸዋል። ከተሳካ በኋላ
መገዛት (ተለዋጭ ስም ማሰር) ፣ ረዳቱ ይለቃል
`artifacts/sorafs/portal.dns-cutover.json`፣ በመያዝ፡- ተለዋጭ ስም ማሰር ሜታዳታ (ስም/ስም/ማስረጃ፣አንጸባራቂ መፍጨት፣Torii URL፣
  የተረከበው ዘመን, ስልጣን);
- የመልቀቂያ አውድ (መለያ፣ ተለዋጭ መለያ፣ አንጸባራቂ/CAR ዱካዎች፣ ቁርጥራጭ ዕቅድ፣ Sigstore
  ጥቅል);
- የማረጋገጫ ጠቋሚዎች (የመመርመሪያ ትዕዛዝ, ተለዋጭ ስም + Torii የመጨረሻ ነጥብ); እና
- አማራጭ የለውጥ መቆጣጠሪያ መስኮች (የቲኬት መታወቂያ ፣ የመቁረጫ መስኮት ፣ የኦፕስ አድራሻ ፣
  የምርት አስተናጋጅ ስም / ዞን);
- የመንገድ ማስተዋወቂያ ሜታዳታ ከስታፕለር `Sora-Route-Binding` የተገኘ
  ራስጌ (ቀኖናዊ አስተናጋጅ/CID፣ ራስጌ + አስገዳጅ መንገዶች፣ የማረጋገጫ ትዕዛዞች)፣
  የ GAR ማስተዋወቅ እና የመውደቅ ልምምድ ማረጋገጥ ተመሳሳይ ማስረጃዎችን ማመልከቱ;
- የመነጨው የመንገድ እቅድ ቅርሶች (`gateway.route_plan.json`፣
  የራስጌ አብነቶች፣ እና አማራጭ የመመለሻ ራስጌዎች) ስለዚህ ቲኬቶችን እና CI ይቀይሩ
  የሊንት መንጠቆዎች እያንዳንዱ ዲጂ-3 ፓኬት ቀኖናዊውን እንደሚጠቅስ ማረጋገጥ ይችላል።
  ከማጽደቁ በፊት የማስተዋወቅ/የመመለሻ ዕቅዶች;
- የአማራጭ መሸጎጫ ትክክለኛ ያልሆነ ሜታዳታ (የማብቂያ ነጥብ፣ የዳይ ተለዋዋጭ፣ JSON
  ክፍያ, እና ምሳሌ `curl` ትዕዛዝ); እና
- ወደ ቀዳሚው ገላጭ የሚጠቁሙ የመልስ ፍንጮች (መለቀቅ እና አንጸባራቂ ይልቀቁ
  ዲጀስት) ስለዚህ ቲኬቶችን መቀየር ወሳኙን የመውደቅ መንገድን ይይዛሉ።

መልቀቂያው መሸጎጫ ማጽዳትን ሲፈልግ፣ ከ ጋር አብሮ ቀኖናዊ እቅድ ያመነጫል።
መቁረጫ ገላጭ፡

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

የተገኘውን `portal.cache_plan.json` ከዲጂ-3 ፓኬት ጋር ያያይዙ ስለዚህ ኦፕሬተሮች
በሚሰጡበት ጊዜ ቆራጥ አስተናጋጆች/ዱካዎች (እና ተዛማጅ ማረጋገጫ ፍንጮች) ይኑርዎት
`PURGE` ጥያቄዎች። የገላጭው አማራጭ መሸጎጫ ዲበ ውሂብ ክፍል ሊያመለክት ይችላል።
ይህ ፋይል በቀጥታ፣ የለውጥ መቆጣጠሪያ ገምጋሚዎችን በትክክል በየትኛው ላይ እንዲሰለፉ ያደርጋል
የመጨረሻ ነጥቦች በቆራጥነት ጊዜ ይታጠባሉ።

እያንዳንዱ DG-3 ፓኬት እንዲሁ የማስተዋወቂያ + የመልስ ማረጋገጫ ዝርዝር ያስፈልገዋል። በኩል አመንጭተው
`cargo xtask soradns-route-plan` ስለዚህ የለውጥ መቆጣጠሪያ ገምጋሚዎች ትክክለኛውን መከታተል ይችላሉ።
ቅድመ በረራ፣ መቁረጥ እና የመመለሻ እርምጃዎች በተለዋጭ ስም፡-

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

የተለቀቀው `gateway.route_plan.json` ቀኖናዊ/ቆንጆ አስተናጋጆችን ይይዛል
የጤና ማረጋገጫ አስታዋሾች፣ የGAR አስገዳጅ ዝመናዎች፣ መሸጎጫ ማጽጃዎች እና የመመለሻ እርምጃዎች።
ለውጡን ከማቅረቡ በፊት ከ GAR/ቢንዲንግ/ቆርጦ ቅርሶች ጋር ያገናኙት።
ቲኬት Ops እንዲለማመዱ እና በተመሳሳዩ ስክሪፕት ደረጃዎች ላይ መፈረም ይችላሉ።

`scripts/generate-dns-cutover-plan.mjs` ይህን ገላጭ ያንቀሳቅሳል እና ይሰራል
በራስ-ሰር ከ `sorafs-pin-release.sh`. እሱን ለማደስ ወይም ለማበጀት።
በእጅ:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

ፒኑን ከማስኬድዎ በፊት የአማራጭ ሜታዳታውን በአካባቢ ተለዋዋጮች በኩል ይሙሉ
ረዳት፡

| ተለዋዋጭ | ዓላማ |
|-------|--------|
| `DNS_CHANGE_TICKET` | የቲኬት መታወቂያ በገላጭ ውስጥ ተከማችቷል። |
| `DNS_CUTOVER_WINDOW` | ISO8601 የመቁረጫ መስኮት (ለምሳሌ `2026-03-21T15:00Z/2026-03-21T15:30Z`)። |
| `DNS_HOSTNAME`, `DNS_ZONE` | የምርት አስተናጋጅ ስም + ባለሥልጣን ዞን። |
| `DNS_OPS_CONTACT` | በጥሪ ላይ ተለዋጭ ስም ወይም ጭማሪ ግንኙነት። |
| `DNS_CACHE_PURGE_ENDPOINT` | በመግለጫው ውስጥ የተመዘገበ የመሸጎጫ ማጽጃ የመጨረሻ ነጥብ። |
| `DNS_CACHE_PURGE_AUTH_ENV` | የጽዳት ማስመሰያ (የ`CACHE_PURGE_TOKEN` ነባሪዎች) የያዘ Env var። |
| `DNS_PREVIOUS_PLAN` | የመመለሻ ሜታዳታ ወደ ቀዳሚው መቁረጫ ገላጭ መንገድ። |

አጽዳቂዎች አንጸባራቂን ማረጋገጥ እንዲችሉ JSONን ከዲኤንኤስ ለውጥ ግምገማ ጋር ያያይዙት።
የCI ምዝግብ ማስታወሻዎችን ሳይቧጭ መፍጨት፣ ተለዋጭ ስም ማሰር እና የምርመራ ትዕዛዞች።
CLI ባንዲራዎች `--dns-change-ticket`፣ I18NI0000174X፣ `--dns-hostname`፣
`--dns-zone`፣ `--ops-contact`፣ `--cache-purge-endpoint`፣
`--cache-purge-auth-env`፣ እና `--previous-dns-plan` ተመሳሳይ መሻርዎችን ያቀርባሉ።
ረዳቱን ከ CI ውጭ ሲያሄዱ.

## ደረጃ 8 - የመፍትሄውን ዞን ፋይል አጽም ያውጡ (አማራጭ)

የምርት መቁረጫ መስኮቱ በሚታወቅበት ጊዜ, የመልቀቂያው ስክሪፕት ሊያወጣ ይችላል
የኤስኤንኤስ የዞንፋይል አጽም እና ፈቺ በራስ-ሰር ይቀንሳሉ። የተፈለገውን ዲ ኤን ኤስ ይለፉ
መዛግብት እና ሜታዳታ በሁለቱም የአካባቢ ተለዋዋጮች ወይም CLI አማራጮች; ረዳቱ
ከተቆረጠ በኋላ ወዲያውኑ `scripts/sns_zonefile_skeleton.py` ይደውላል
ገላጭ ተፈጠረ። ቢያንስ አንድ የA/AAAA/CNAME ዋጋ እና GAR ያቅርቡ
መፍጨት (BLAKE3-256 የተፈረመው GAR ጭነት)። የዞኑ/የአስተናጋጅ ስም የሚታወቅ ከሆነ
እና `--dns-zonefile-out` ተትቷል, ረዳቱ ይጽፋል
`artifacts/sns/zonefiles/<zone>/<hostname>.json` እና ይሞላል
`ops/soradns/static_zones.<hostname>.json` እንደ መፍቻ ቅንጣቢ።

| ተለዋዋጭ / ባንዲራ | ዓላማ |
|-----------------|--------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | ለተፈጠረው የዞን ፋይል አጽም መንገድ። |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | የመፍትሄ ቅንጣቢ መንገድ (በ `ops/soradns/static_zones.<hostname>.json` ነባሪዎች ሲቀሩ)። |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | ቲቲኤል ለተፈጠሩ መዝገቦች ተተግብሯል (ነባሪ፡ 600 ሰከንድ)። |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 አድራሻዎች (በነጠላ ነጠላ ሰረዝ የተለየ ኢንቪ ወይም ሊደገም የሚችል CLI ባንዲራ)። |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 አድራሻዎች። |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | አማራጭ የCNAME ኢላማ። |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI ፒን (ቤዝ64)። |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | ተጨማሪ የTXT ግቤቶች (`key=value`)። |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | የተሰላው የዞን ፋይል ሥሪት መለያውን ይሽሩት። |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | ከመቁረጫ መስኮቱ ጅምር ይልቅ `effective_at` የጊዜ ማህተም (RFC3339) ያስገድዱ። |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | በሜታዳታ ውስጥ የተመዘገበውን ማስረጃ ይሽሩ። |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | በሜታዳታ ውስጥ የተቀዳውን CID ይሽሩት። |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | ጠባቂው የቀዘቀዘ ሁኔታ (ለስላሳ፣ ጠንካራ፣ ማቅለጥ፣ ክትትል፣ ድንገተኛ)። |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | ጠባቂ/የካውንስል ትኬት ማጣቀሻ። |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 የጊዜ ማህተም ለማቅለጥ። |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | ተጨማሪ የቀዘቀዙ ማስታወሻዎች (በነጠላ በነጠላ ነጠላ ሰረዝ ወይም ሊደገም የሚችል ባንዲራ)። |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 መፍጨት (ሄክስ) የተፈረመው GAR ጭነት። የመተላለፊያ መንገድ ማያያዣዎች በሚገኙበት ጊዜ ሁሉ ያስፈልጋል። |

የ GitHub ድርጊቶች የስራ ፍሰት እነዚህን እሴቶች ከማጠራቀሚያ ሚስጥሮች ያነባል ስለዚህ እያንዳንዱ የምርት ፒን የዞን ፋይል ቅርሶችን በራስ-ሰር ያወጣል። የሚከተሉትን ምስጢሮች አዋቅር (ሕብረቁምፊዎች ለባለብዙ እሴት መስኮች በነጠላ ሰረዞች የተከፋፈሉ ዝርዝሮችን ሊይዙ ይችላሉ)

| ሚስጥር | ዓላማ |
|-------|--------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | የምርት አስተናጋጅ ስም/ዞን ለረዳት ተላልፏል። |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | በጥሪ ላይ ተለዋጭ ስም በገላጭ ውስጥ ተከማችቷል። |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 መዝገቦች ለማተም |
| `DOCS_SORAFS_ZONEFILE_CNAME` | አማራጭ የCNAME ኢላማ። |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI ካስማዎች. |
| `DOCS_SORAFS_ZONEFILE_TXT` | ተጨማሪ የTXT ግቤቶች። |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | በአጽም ውስጥ የተቀዳውን ዲበ ውሂብ እሰር። |
| `DOCS_SORAFS_GAR_DIGEST` | የተፈረመበት GAR የክፍያ ጭነት ሄክስ-የተመሰጠረ BLAKE3 መፍጨት። |

`.github/workflows/docs-portal-sorafs-pin.yml` ሲቀሰቀስ፣ የ`dns_change_ticket` እና `dns_cutover_window` ግብዓቶችን ያቅርቡ ስለዚህ ገላጭ/ዞንፋይል ትክክለኛውን የለውጥ መስኮት ሜታዳታ ይወርሳሉ። ደረቅ ሩጫዎች ሲሮጡ ብቻ ባዶ ይተዉዋቸው።

የተለመደ ጥሪ (ከSN-7 ባለቤት መሮጫ ጋር የሚዛመድ)፦

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

ረዳቱ የለውጡን ቲኬት እንደ TXT ግቤት እና በራስ ሰር ይሸከማል
በቀር የመቁረጫ መስኮቱን እንደ `effective_at` የጊዜ ማህተም ይወርሳል
የተሻረ። ለሙሉ የስራ ሂደት፣ ይመልከቱ
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### የህዝብ ዲ ኤን ኤስ ውክልና ማስታወሻ

የዞንፋይል አጽም ለዞኑ ሥልጣናዊ መዝገቦችን ብቻ ይገልጻል። አንተ
አሁንም የወላጅ-ዞን NS/DS ውክልና በእርስዎ ሬጅስትራር ወይም ዲኤንኤስ ማዋቀር ያስፈልጋል
መደበኛው ኢንተርኔት ስም ሰርቨሮችን እንዲያገኝ አቅራቢ።

- ለከፍተኛ/TLD መቁረጫዎች፣ ALIAS/ANAME (አቅራቢ-ተኮር) ይጠቀሙ ወይም A/AAAA ያትሙ
  በመግቢያው ላይ የሚጠቁሙ መዝገቦች anycast IPs።
- ለንዑስ ጎራዎች፣ CNAMEን ለመጣው ቆንጆ አስተናጋጅ ያትሙ
  (`<fqdn>.gw.sora.name`)።
- ቀኖናዊው አስተናጋጅ (`<hash>.gw.sora.id`) በመግቢያው ጎራ ስር ይቆያል እና
  በእርስዎ የህዝብ ዞን ውስጥ አልታተመም።

### ጌትዌይ ራስጌ አብነት

አሰማራው አጋዥ እንዲሁ `portal.gateway.headers.txt` እና ያመነጫል።
`portal.gateway.binding.json`፣ DG-3ን የሚያረኩ ሁለት ቅርሶች
መግቢያ-ይዘት ማሰሪያ መስፈርት፡-

- `portal.gateway.headers.txt` ሙሉውን የኤችቲቲፒ አርዕስት ማገጃ ይዟል (ጨምሮ
  `Sora-Name`፣ `Sora-Content-CID`፣ `Sora-Proof`፣ CSP፣ HSTS፣ እና
  `Sora-Route-Binding` ገላጭ) ያ የጠርዝ በሮች በእያንዳንዱ ላይ መቆም አለባቸው
  ምላሽ.
- `portal.gateway.binding.json` በማሽን-ሊነበብ የሚችል ተመሳሳይ መረጃ ይመዘግባል
  ቅጽ ስለዚህ ቲኬቶችን ይቀይሩ እና አውቶሜሽን ያለ አስተናጋጅ/ሲድ ማሰሪያ ሊለያይ ይችላል።
  የሼል ውፅዓት መቧጨር.

እነሱ በራስ-ሰር የሚመነጩት በ
`cargo xtask soradns-binding-template`
እና የቀረበውን ተለዋጭ ስም፣ ገላጭ መፍቻ እና መግቢያ አስተናጋጅ ስም ይያዙ
ወደ `sorafs-pin-release.sh`. የራስጌ ማገጃውን ለማደስ ወይም ለማበጀት ያሂዱ፡-

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

ለመሻር `--csp-template`፣ `--permissions-template`፣ ወይም `--hsts-template` ይለፉ
አንድ የተወሰነ ማሰማራት ተጨማሪ ሲፈልግ ነባሪውን የራስጌ አብነቶች ያዘጋጃል።
መመሪያዎች; ራስጌ ለመጣል አሁን ካሉት I18NI0000252X መቀየሪያዎች ጋር ያዋህዷቸው
ሙሉ በሙሉ።

የራስጌ ቅንጣቢውን ከCDN ለውጥ ጥያቄ ጋር ያያይዙ እና የJSON ሰነዱን ይመግቡ
የጌትዌይ አውቶሜሽን ቧንቧው ውስጥ መግባት ስለዚህ ትክክለኛው የአስተናጋጅ ማስተዋወቂያ ከ
ማስረጃ መልቀቅ.

የመልቀቂያው ስክሪፕት የማረጋገጫ ረዳትን በራስ ሰር ይሰራል ስለዚህ DG-3 ትኬቶች
ሁልጊዜ የቅርብ ጊዜ ማስረጃዎችን ያካትቱ. በሚስተካከሉበት ጊዜ እራስዎ እንደገና ያሂዱት
JSON በእጅ ማሰር፡

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

ትዕዛዙ በማሰሪያው ጥቅል ውስጥ የተያዘውን I18NI0000253X ክፍያ ያረጋግጣል።
የ`Sora-Route-Binding` ሜታዳታ ከማንፀባረቂያው CID + አስተናጋጅ ስም ጋር እንደሚዛመድ ያረጋግጣል፣
እና ማንኛውም ራስጌ የሚንሸራተት ከሆነ በፍጥነት አይሳካም። ቀጥሎ ያለውን የኮንሶል ውፅዓት በማህደር ያስቀምጡ
ትዕዛዙን ከ CI ውጭ በሚያሄዱበት ጊዜ እና DG-3 ሌሎች የማሰማራት ቅርሶች
ገምጋሚዎች ማሰሪያው ከመቋረጡ በፊት እንደተረጋገጠ ማረጋገጫ አላቸው።> ** ዲ ኤን ኤስ ገላጭ ውህደት፡** `portal.dns-cutover.json` አሁን አንድን ያካትታል
> የ`gateway_binding` ክፍል ወደ እነዚህ ቅርሶች (መንገዶች፣ የይዘት CID፣
> የማረጋገጫ ሁኔታ፣ እና ቀጥተኛው ራስጌ አብነት) ** እና** አንድ `route_plan` ስታንዛ
> `gateway.route_plan.json` እና ዋናውን + ጥቅልል ራስጌን በመጥቀስ
> አብነቶች. ገምጋሚዎች እንዲችሉ እነዚያን ብሎኮች በእያንዳንዱ DG-3 የለውጥ ትኬት ውስጥ ያካትቱ
> ትክክለኛውን `Sora-Name/Sora-Proof/CSP` እሴቶችን ይለያዩ እና መንገዱን ያረጋግጡ
> የማስተዋወቂያ/የመመለሻ ዕቅዶች ግንባታውን ሳይከፍቱ ከማስረጃ ጥቅል ጋር ይዛመዳሉ
> ማህደር.

## ደረጃ 9 - የህትመት ማሳያዎችን ያሂዱ

የመንገድ ካርታ ተግባር **DOCS-3c** ፖርታሉ፣ ይሞክሩት የሚለውን ቀጣይነት ያለው ማስረጃ ያስፈልገዋል
ፕሮክሲ እና የጌትዌይ ማሰሪያዎች ከተለቀቀ በኋላ ጤናማ ሆነው ይቆያሉ። የተጠናከረውን ያሂዱ
ከደረጃ 7–8 በኋላ ወዲያውኑ ይቆጣጠሩ እና በታቀዱት መመርመሪያዎችዎ ውስጥ ያስገቡት፡

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` የውቅር ፋይሉን ይጭናል (ይመልከቱ
  `docs/portal/docs/devportal/publishing-monitoring.md` ለዕቅዱ) እና
  ሶስት ቼኮችን ያከናውናል፡ የፖርታል መንገድ መመርመሪያዎች + CSP/ፍቃዶች-የመመሪያ ማረጋገጫ፣
  ተኪ መመርመሪያዎችን ይሞክሩት (በአማራጭ የ `/metrics` የመጨረሻ ነጥቡን በመምታት) እና
  የጌትዌይ ማሰሪያ አረጋጋጭ (`cargo xtask soradns-verify-binding`) የሚያረጋግጥ
  የተያዘው ማሰሪያ ጥቅል ከሚጠበቀው ተለዋጭ ስም ፣ አስተናጋጅ ፣ የማረጋገጫ ሁኔታ ፣
  እና JSON አሳይ።
- ትዕዛዙ ከዜሮ ውጭ ይወጣል ማንኛውም መጠይቅ ሳይሳካ ሲቀር CI፣ ክሮን ስራዎች ወይም
  runbook ኦፕሬተሮች ተለዋጭ ስሞችን ከማስተዋወቅዎ በፊት ልቀትን ማቆም ይችላሉ።
- ማለፍ `--json-out` ነጠላ ማጠቃለያ JSON ክፍያ በእያንዳንዱ ዒላማ ይጽፋል
  ሁኔታ; `--evidence-dir` `summary.json`፣ `portal.json`፣ `tryit.json`፣
  `binding.json` እና `checksums.sha256` ስለዚህ የአስተዳደር ገምጋሚዎች ሊለያዩ ይችላሉ
  ተቆጣጣሪዎቹን እንደገና ሳያስኬዱ ውጤቶች. ይህን ማውጫ ስር በማህደር አስቀምጥ
  `artifacts/sorafs/<tag>/monitoring/` ከ I18NT0000007X ጥቅል እና ዲ ኤን ኤስ ጋር
  መቁረጫ ገላጭ.
- የተቆጣጣሪውን ውጤት ያካትቱ፣ Grafana ወደ ውጭ መላክ (`dashboards/grafana/docs_portal.json`) ፣
  እና Alertmanager መሰርሰሪያ መታወቂያ በመልቀቂያ ትኬት ውስጥ ስለዚህ DOCS-3c SLO ሊሆን ይችላል
  በኋላ ኦዲት ተደርጓል። የተወሰነው የሕትመት ማሳያ መጫወቻ መጽሐፍ የሚኖረው በ
  `docs/portal/docs/devportal/publishing-monitoring.md`.

የፖርታል መመርመሪያዎች HTTPS ያስፈልጋቸዋል እና `http://` መሰረት ዩአርኤሎችን ውድቅ ያድርጉ ካልሆነ በስተቀር
`allowInsecureHttp` በ ማሳያ ውቅረት ውስጥ ተዘጋጅቷል; ምርትን / ዝግጅትን ጠብቅ
በTLS ላይ ያነጣጠረ እና መሻርን ለአካባቢያዊ ቅድመ እይታዎች ብቻ ያንቁ።

ሞኒተሩን አንዴ በBuildkite/cron በI18NI0000276X በኩል ሰርት
ፖርታል ቀጥታ ነው። በምርት ዩአርኤሎች ላይ የተጠቆመው ተመሳሳይ ትዕዛዝ ቀጣይ የሆነውን ይመገባል።
በተለቀቁት መካከል SRE/ሰነዶች የሚተማመኑባቸውን የጤና ምርመራዎች።

## በ`sorafs-pin-release.sh` አውቶማቲክ ማድረግ

`docs/portal/scripts/sorafs-pin-release.sh` ደረጃዎች2–6ን ያጠቃልላል። እሱ፡-

1. `build/`ን ወደ ወሳኙ ታርቦል ያስቀምጣል።
2. `car pack`፣ `manifest build`፣ `manifest sign`፣ `manifest verify-signature`፣
   እና `proof verify`፣
3. Torii ሲኖር `manifest submit` (ተለዋጭ ስም ማሰርን ጨምሮ) በአማራጭ ያስፈጽማል።
   ምስክርነቶች አሉ, እና
4. `artifacts/sorafs/portal.pin.report.json` ይጽፋል, አማራጭ
  `portal.pin.proposal.json`፣ የዲ ኤን ኤስ መቁረጫ ገላጭ (ከገባ በኋላ)
  እና የጌትዌይ ማሰሪያ ጥቅል (`portal.gateway.binding.json` እና የ
  የጽሑፍ ራስጌ ብሎክ) ስለዚህ የአስተዳደር፣ የኔትወርክ እና የኦፕስ ቡድኖች ሊለያዩ ይችላሉ።
  የCI ምዝግብ ማስታወሻዎችን ሳይቧጭ የማስረጃ ጥቅል።

`PIN_ALIAS`፣ `PIN_ALIAS_NAMESPACE`፣ `PIN_ALIAS_NAME`፣ እና (በአማራጭ) አዘጋጅ
ስክሪፕቱን ከመጥራትዎ በፊት `PIN_ALIAS_PROOF_PATH`። ለደረቅ `--skip-submit` ይጠቀሙ
ይሮጣል; ከዚህ በታች የተገለፀው የ GitHub የስራ ፍሰት ይህንን በ `perform_submit` በኩል ይቀየራል
ግቤት.

## ደረጃ 8 — የOpenAPI ዝርዝሮችን እና የኤስቢኦኤም ቅርቅቦችን ያትሙ

DOCS-7 ለመጓዝ የፖርታል ግንባታን፣ OpenAPI spec እና SBOM ቅርሶችን ይፈልጋል።
በተመሳሳዩ የመወሰን ቧንቧ መስመር. ያሉት ረዳቶች ሶስቱን ይሸፍናሉ፡-

1. ** እንደገና ማመንጨት እና መግለጫውን ይፈርሙ።**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   ሀ ማቆየት በሚፈልጉበት ጊዜ የመልቀቂያ መለያን በ`--version=<label>` በኩል ያስተላልፉ
   ታሪካዊ ቅጽበታዊ ገጽ እይታ (ለምሳሌ `2025-q3`)። ረዳቱ ቅጽበተ-ፎቶውን ይጽፋል
   ወደ `static/openapi/versions/<label>/torii.json`፣ ያንጸባርቃል
   `versions/current`፣ እና ሜታዳታውን ይመዘግባል (SHA-256፣ አንጸባራቂ ሁኔታ እና
   የዘመነ የጊዜ ማህተም) በ `static/openapi/versions.json` ውስጥ። የገንቢ ፖርታል
   የ Swagger/RapiDoc ፓነሎች የስሪት መራጭ እንዲያቀርቡ ያንን ኢንዴክስ ያነባል
   እና የተጎዳኘውን የምግብ መፍጫ/የፊርማ መረጃ በመስመር ላይ አሳይ። መተው
   `--version` የቀደሙትን የመልቀቂያ መለያዎች እንደተጠበቀ ያቆያል እና የሚያድስ ብቻ ነው
   `current` + `latest` ጠቋሚዎች።

   አንጸባራቂው SHA-256/BLAKE3 የምግብ መፈጨትን ስለሚይዝ የመግቢያ መንገዱ ዋና ክፍል ሊሆን ይችላል።
   `Sora-Proof` ራስጌዎች ለ`/reference/torii-swagger`።

2. **CycloneDX SBOMs አስወጣ።** የመልቀቂያ ቧንቧው አስቀድሞ በሲፍት ላይ የተመሰረተ ይጠብቃል።
   SBoms በ `docs/source/sorafs_release_pipeline_plan.md`። ውጤቱን ያስቀምጡ
   ከግንባታው ቅርሶች ቀጥሎ፡-

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. ** እያንዳንዱን ጭነት በመኪና ውስጥ ያሽጉ።**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   እንደ ዋናው ጣቢያ ተመሳሳይ `manifest build` / `manifest sign` ደረጃዎችን ይከተሉ።
   ተለዋጭ ስሞችን በንብረት ማስተካከል (ለምሳሌ፣ `docs-openapi.sora` ለዝርዝሩ እና
   `docs-sbom.sora` ለተፈረመው SBOM ጥቅል)። የተለዩ ተለዋጭ ስሞችን መጠበቅ
   የSoraDNS ማረጋገጫዎች፣ GARs እና የመመለሻ ትኬቶችን ለትክክለኛው የክፍያ መጠን ያቆያል።

4. **አስረክብ እና አስረው።** ያለውን ባለስልጣን + I18NT0000008X ጥቅልን እንደገና ተጠቀም፣ ግን
   ኦዲተሮች የትኛውን መከታተል እንዲችሉ በተለቀቀው የማረጋገጫ ዝርዝር ውስጥ ተለዋጭ ስም ይመዝግቡ
   የሶራ ስም ካርታዎች ለየትኛዎቹ መገለጫዎች መፈጨት።

ልዩነቱን/SBOMን ከፖርታል ግንባታው ጎን ለጎን ማስቀመጥ እያንዳንዱን ያረጋግጣል
የመልቀቂያ ትኬት ማሸጊያውን እንደገና ሳያስኬድ ሙሉውን የጥበብ ስራ ይዟል።

### አውቶሜሽን አጋዥ (CI/የጥቅል ስክሪፕት)

`./ci/package_docs_portal_sorafs.sh` የፍኖተ ካርታ ንጥሉን በደረጃ 1–8 ኮድ ያደርጋል
**DOCS-7** በአንድ ትእዛዝ ሊተገበር ይችላል። ረዳቱ፡-

- አስፈላጊውን የፖርታል ዝግጅት ያካሂዳል (`npm ci`, OpenAPI/norito sync, widget tests);
- መግቢያውን፣ OpenAPI፣ እና SBOM CARs + manifest ጥንዶችን በ`sorafs_cli` በኩል ያወጣል።
- እንደ አማራጭ `sorafs_cli proof verify` (`--proof`) እና Sigstore ፊርማ ይሰራል
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- እያንዳንዱን ቅርስ በ `artifacts/devportal/sorafs/<timestamp>/` እና ይጥላል
  `package_summary.json` ይጽፋል ስለዚህ CI / የተለቀቀው መሣሪያ ጥቅሉን ወደ ውስጥ ማስገባት ይችላል; እና
- በጣም የቅርብ ጊዜ ሩጫ ላይ ለመጠቆም `artifacts/devportal/sorafs/latest` ያድሳል።

ምሳሌ (ሙሉ የቧንቧ መስመር ከSigstore + PoR ጋር)

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

ሊታወቁ የሚገባቸው ባንዲራዎች፡-

- `--out <dir>` - የአርቴፋክት ሥሩን ይሽሩ (በነባሪ ጊዜ የታተሙ አቃፊዎችን ያቆያል)።
- `--skip-build` - ያለውን `docs/portal/build` እንደገና ይጠቀሙ (CI በማይችልበት ጊዜ ምቹ ነው
  ከመስመር ውጭ መስተዋቶች ምክንያት እንደገና መገንባት).
- `--skip-sync-openapi` - `npm run sync-openapi` ይዝለሉ `cargo xtask openapi` ጊዜ
  crates.io መድረስ አይችልም.
- `--skip-sbom` - ሁለትዮሽ ካልተጫነ `syft` ከመደወል ይቆጠቡ (የ
  በምትኩ ስክሪፕት ማስጠንቀቂያ ያትማል)።
- `--proof` - ለእያንዳንዱ CAR/የተገለጠ ጥንድ `sorafs_cli proof verify` ን ያሂዱ። ባለብዙ-
  የፋይል ጭነት አሁንም በCLI ውስጥ የቸንክ-ፕላን ድጋፍ ይፈልጋል፣ ስለዚህ ይህን ባንዲራ ይተውት።
  የ `plan chunk count` ስህተቶችን ካጋጠሙ እና አንድ ጊዜ እራስዎ ካረጋገጡ አይዋቀሩም።
  የላይኛው ተፋሰስ በር መሬቶች.
- `--sign` - `sorafs_cli manifest sign` ይደውሉ። ጋር ማስመሰያ ያቅርቡ
  `SIGSTORE_ID_TOKEN` (ወይም `--sigstore-token-env`) ወይም CLI ተጠቅመው እንዲያመጣው ይፍቀዱለት
  `--sigstore-provider/--sigstore-audience`.

የምርት ቅርሶችን በሚላክበት ጊዜ `docs/portal/scripts/sorafs-pin-release.sh` ይጠቀሙ።
አሁን ፖርታሉን፣ OpenAPIን፣ እና SBom ክፍያን ያጠቃልላል፣ እያንዳንዱን መግለጫ ይፈርማል፣ እና
ተጨማሪ የንብረት ሜታዳታ በ`portal.additional_assets.json` ይመዘግባል። ረዳቱ
በCI packr እና አዲሱ ጥቅም ላይ የሚውሉትን ተመሳሳይ አማራጭ ቁልፎችን ይረዳል
`--openapi-*`፣ `--portal-sbom-*`፣ እና `--openapi-sbom-*` መቀየሪያ
ቅጽል tuples በእያንዳንዱ artefact መድብ, በኩል SBOM ምንጭ መሻር
`--openapi-sbom-source`፣ የተወሰኑ የክፍያ ጭነቶችን (`--skip-openapi`/`--skip-sbom`) ዝለል፣
እና ነባሪ ያልሆነ I18NI0000345X ሁለትዮሽ ከ `--syft-bin` ጋር ያመልክቱ።

ስክሪፕቱ የሚሠራውን እያንዳንዱን ትዕዛዝ ይሸፍናል; መዝገቡን ወደ መልቀቂያ ቲኬቱ ይቅዱ
ከ `package_summary.json` ጋር ገምጋሚዎች የ CAR መፈጨትን ፣ እቅድ ማውጣትን ሊለያዩ ይችላሉ
ሜታዳታ፣ እና Sigstore የጥቅል hashes የማስታወቂያ ሆክ ሼል ውፅዓትን ሳያስወጡ።

## ደረጃ 9 — ጌትዌይ + SoraDNS ማረጋገጫ

መቁረጡን ከማስታወቅዎ በፊት አዲሱን ተለዋጭ ስም በሶራዲኤንኤስ እና ያንን ያረጋግጡ
የጌትዌይ ዋና ዋና ማስረጃዎች፡-

1. ** የመመርመሪያውን በር ያሂዱ።** `ci/check_sorafs_gateway_probe.sh` ልምምዶች
   `cargo xtask sorafs-gateway-probe` በ ውስጥ ካለው ማሳያ ማሳያዎች ጋር
   `fixtures/sorafs_gateway/probe_demo/`. ለእውነተኛ ማሰማራት፣ መፈተሻውን ይጠቁሙ
   በዒላማ አስተናጋጅ ስም:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   መርማሪው `Sora-Name`፣ `Sora-Proof`፣ እና `Sora-Proof-Status` በአንድ
   `docs/source/sorafs_alias_policy.md` እና አንጸባራቂው ሲፈጭ አይሳካም።
   ቲቲኤሎች፣ ወይም የጋር ማሰሪያዎች ተንሳፋፊ።

   ለቀላል ክብደት የቦታ ፍተሻዎች (ለምሳሌ፣ ማሰሪያው ጥቅል ሲደረግ
   ተቀይሯል)፣ `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` አሂድ።
   ረዳቱ የተያዘውን ማሰሪያ ጥቅል ያረጋግጣል እና ለመልቀቅ ምቹ ነው።
   ከሙሉ የምርመራ መሰርሰሪያ ይልቅ አስገዳጅ ማረጋገጫ ብቻ የሚያስፈልጋቸው ቲኬቶች።

2. ** የመሰርሰሪያ ማስረጃን ይያዙ።** ለኦፕሬተር ልምምዶች ወይም ፔጀርዱቲ ደረቅ ሩጫዎች፣ ጥቅል
   ምርመራው `ስክሪፕት/ቴሌሜትሪ/አሂድ_sorafs_gateway_probe.sh --scenario
   ዴቭፖርታል-መለቀቅ --…`። መጠቅለያው የራስጌ ምዝግብ ማስታወሻዎችን ያከማቻል
   `artifacts/sorafs_gateway_probe/<stamp>/`፣ ዝማኔዎች `ops/drill-log.md`፣ እና
   (በአማራጭ) የመመለሻ መንጠቆዎችን ወይም የፔጀርዱቲ ክፍያ ጭነቶችን ያነሳሳል። አዘጋጅ
   `--host docs.sora` አይፒን በጠንካራ ኮድ ከማስቀመጥ ይልቅ የሶራዲኤንኤስን መንገድ ለማረጋገጥ።3. **የዲኤንኤስ ማሰሪያዎችን ያረጋግጡ።** አስተዳደር ተለዋጭ ማስረጃውን ሲያትም ይመዝገቡ
   በምርመራው ውስጥ የተጠቀሰው የ GAR ፋይል (`--gar`) እና ከተለቀቀው ጋር ያያይዙት
   ማስረጃ. የመፍትሄው ባለቤቶች ተመሳሳይ ግብአትን ማንጸባረቅ ይችላሉ።
   `tools/soradns-resolver` የተሸጎጡ ግቤቶች አዲሱን አንጸባራቂ እንደሚያከብሩ ለማረጋገጥ።
   JSON ን ከማያያዝዎ በፊት፣ ያሂዱ
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   ስለዚህ ወሳኙ አስተናጋጅ ካርታ፣ አንጸባራቂ ሜታዳታ እና የቴሌሜትሪ መለያዎች ናቸው።
   ከመስመር ውጭ የተረጋገጠ. ረዳቱ የ `--json-out` ማጠቃለያ ከጎኑ ሊያወጣ ይችላል።
   የተፈረመ GAR ስለዚህ ገምጋሚዎች ሁለትዮሽ ሳይከፍቱ የሚረጋገጥ ማስረጃ አላቸው።
  አዲስ GAR ሲያዘጋጁ, ይመርጣሉ
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (ወደ `--manifest-cid <cid>` ተመለስ አንጸባራቂ ፋይል ካልሆነ ብቻ
  ይገኛል)። ረዳቱ አሁን CID ** እና ** BLAKE3 መፍጨት በቀጥታ ከ ያገኛል
  አንጸባራቂው JSON፣ ነጭ ቦታን ይከርክማል፣ የተደገመ `--telemetry-label`
  ይጠቁማል፣ መለያዎቹን ይመድባል እና ነባሪውን CSP/HSTS/ፍቃዶች-መመሪያን ያወጣል።
  JSON ን ከመጻፍዎ በፊት አብነቶችን ያዘጋጁ ስለዚህ ጭነቱ በሚታወቅበት ጊዜ እንኳን የሚቆይ ይሆናል።
  ኦፕሬተሮች መለያዎችን ከተለያዩ ዛጎሎች ይይዛሉ።

4. ** ተለዋጭ መለኪያዎችን ይመልከቱ።** `torii_sorafs_alias_cache_refresh_duration_ms` አቆይ
   እና `torii_sorafs_gateway_refusals_total{profile="docs"}` በስክሪኑ ላይ ሳለ
   መፈተሻ እየሄደ ነው; ሁለቱም ተከታታዮች በሠንጠረዥ ውስጥ ተቀምጠዋል
   `dashboards/grafana/docs_portal.json`.

## ደረጃ 10 - ክትትል እና ማስረጃ ማያያዝ

- ** ዳሽቦርዶች።** `dashboards/grafana/docs_portal.json` (ፖርታል SLOs) ወደ ውጪ ላክ
  `dashboards/grafana/sorafs_gateway_observability.json` (የጌትዌይ መዘግየት +
  ማረጋገጫ ጤና), እና `dashboards/grafana/sorafs_fetch_observability.json`
  (የኦርኬስትራ ጤና) ለእያንዳንዱ ልቀት። የJSON ወደ ውጭ የሚላኩትን ያያይዙ
  ገምጋሚዎች የPrometheus መጠይቆችን እንደገና ማጫወት እንዲችሉ ትኬት የመልቀቅ።
- ** የመመርመሪያ ማህደሮች።** `artifacts/sorafs_gateway_probe/<stamp>/` በgit-annex ውስጥ አቆይ
  ወይም ማስረጃችሁን ባልዲ። የፍተሻ ማጠቃለያን፣ ራስጌዎችን እና PagerDutyን ያካትቱ
  ክፍያ በቴሌሜትሪ ስክሪፕት ተይዟል።
- **የልቀት ጥቅል
  ቅርቅቦች፣ I18NT0000012X ፊርማዎች፣ `portal.pin.report.json`፣ ይሞክሩ-It መመርመሪያ ምዝግብ ማስታወሻዎች፣ እና
  በአንድ የጊዜ ማህተም በተሰየመ አቃፊ ስር ሪፖርቶችን አገናኝ-ቼክ (ለምሳሌ ፣
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- ** የቁፋሮ መዝገብ።** መመርመሪያዎች የአንድ መሰርሰሪያ አካል ሲሆኑ፣ ይልቀቁ
  `scripts/telemetry/run_sorafs_gateway_probe.sh` ወደ `ops/drill-log.md` ተያይዟል
  ስለዚህ ተመሳሳይ ማስረጃ የ SNNet-5 ትርምስ መስፈርትን ያሟላል።
** የቲኬት ማያያዣዎች።** የGrafana ፓነል መታወቂያዎችን ወይም የፒኤንጂ ወደ ውጭ የሚላኩትን አያይዘው ይመልከቱ።
  የለውጥ ትኬቱ፣ ከመርማሪው ሪፖርት መንገድ ጋር፣ ስለዚህ ለውጥ-ገምጋሚዎች
  የሼል መዳረሻ ሳይኖር SLO ን መፈተሽ ይችላል።

## ደረጃ 11 - ባለብዙ ምንጭ ማምጣት መሰርሰሪያ እና የውጤት ሰሌዳ ማስረጃ

ወደ SoraFS ማተም አሁን የብዝሃ-ምንጭ ማምጣት ማስረጃ ያስፈልገዋል (DOCS-7/SF-6)
ከላይ ካለው የዲ ኤን ኤስ/የበረንዳ ማረጋገጫዎች ጋር። አንጸባራቂውን ከተሰካ በኋላ፡-

1. ** `sorafs_fetch`ን ከቀጥታ አንጸባራቂው ጋር ያሂዱ።** ተመሳሳይ እቅድ ይጠቀሙ/አሳይ።
   በደረጃ 2-3 የተሰሩ ቅርሶች እና ለእያንዳንዳቸው የተሰጡ የጌትዌይ ምስክርነቶች
   አቅራቢ. ኦዲተሮች ኦርኬስትራውን እንደገና እንዲጫወቱ እያንዳንዱን ውፅዓት ቀጥል
   የውሳኔ መንገድ;

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - በቅድሚያ በማንፀባረቂያው የተጠቀሱ የአቅራቢውን ማስታወቂያዎች ያውጡ (ለምሳሌ፡
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     እና የውጤት ሰሌዳው እንዲችል በ`--provider-advert name=path` በኩል ይልፋቸው
     የችሎታ መስኮቶችን በቆራጥነት ይገምግሙ። ተጠቀም
     `--allow-implicit-provider-metadata` **ብቻ** ውስጥ የቤት ዕቃዎችን ሲደግሙ
     CI; የምርት ልምምዶች ከ ጋር ያረፉ የተፈረሙ ማስታወቂያዎችን መጥቀስ አለባቸው
     ፒን.
   - አንጸባራቂው ተጨማሪ ክልሎችን ሲያመለክት ትዕዛዙን ይድገሙት
     ሁሉም መሸጎጫ/ተለዋጭ ስም ተዛማጅ አለው።
     artefact አምጣ.

2. **ውጤቶቹን በማህደር ያስቀምጡ።** `scoreboard.json` ማከማቻ፣
   `providers.ndjson`፣ `fetch.json`፣ እና `chunk_receipts.ndjson` በ
   የማስረጃ ማህደርን መልቀቅ። እነዚህ ፋይሎች የአቻውን ክብደት ይይዛሉ፣ እንደገና ይሞክሩ
   የበጀት፣ የዘገየ EWMA እና የአስተዳደር እሽግ የግድ የግድ ደረሰኞች
   ለ SF-7 ማቆየት.

3. **ቴሌሜትሪ አዘምን።** የማምጣት ውጤቶችን ወደ **SoraFS ፈልሳ አስገባ
   ታዛቢነት** ዳሽቦርድ (`dashboards/grafana/sorafs_fetch_observability.json`)፣
   `torii_sorafs_fetch_duration_ms`/`_failures_total` እና
   ለ anomalies አቅራቢ-ክልል ፓነሎች. የGrafana ፓነል ቅጽበተ-ፎቶዎችን ከ
   የመልቀቅ ትኬት ከውጤት ሰሌዳው መንገድ ጋር።

4. ** የማንቂያ ደንቦችን ያጨሱ።** `scripts/telemetry/test_sorafs_fetch_alerts.sh` ን ያሂዱ
   ልቀቱን ከመዘጋቱ በፊት የI18NT0000001X ማንቂያ ቅርቅቡን ለማረጋገጥ። ያያይዙ
   DOCS-7 ገምጋሚዎች ድንኳኑን ማረጋገጥ እንዲችሉ የፕሮምቶል ውፅዓት ወደ ቲኬቱ ይደርሳል
   እና ዘገምተኛ አቅራቢዎች ማንቂያዎች እንደታጠቁ ይቀራሉ።

5. ** ሽቦ ወደ CI።** የፖርታል ፒን የስራ ፍሰቱ የ`sorafs_fetch` እርምጃ ወደ ኋላ ይይዛል።
   የ `perform_fetch_probe` ግቤት; ለዝግጅቱ/ምርት ስራዎች እንዲሰራ ያድርጉት
   ማስረጃዎችን ማምጣት ከማንዋል ውጪ ከማንፀባረቂያ ጥቅል ጋር አብሮ ይወጣል
   ጣልቃ ገብነት. የአካባቢ ልምምዶች ያንኑ ስክሪፕት ወደ ውጭ በመላክ እንደገና መጠቀም ይችላሉ።
   ጌትዌይ ቶከኖች እና `PIN_FETCH_PROVIDERS` ወደ በነጠላ ሰረዝ ተለይተዋል
   የአቅራቢዎች ዝርዝር.

## ማስተዋወቅ፣ ታዛቢነት እና መመለስ

1. ** ማስተዋወቅ: ** የተለየ የዝግጅት እና የምርት ስሞችን ያስቀምጡ። ያስተዋውቁ በ
   `manifest submit` እንደገና በማስኬድ ከተመሳሳዩ አንጸባራቂ/ጥቅል ጋር፣ በመቀያየር
   `--alias-namespace/--alias-name` ወደ ምርት ተለዋጭ ስም ለመጠቆም። ይህ
   QA የማስተዳደሪያ ፒን ካጸደቀ በኋላ እንደገና ከመገንባቱ ወይም ከሥራ መልቀቁን ያስወግዳል።
2. **ክትትል፡** የፒን መዝገብ ዳሽቦርዱን አስመጣ
   (`docs/source/grafana_sorafs_pin_registry.json`) በተጨማሪም ፖርታል-ተኮር
   መመርመሪያዎች (`docs/portal/docs/devportal/observability.md` ይመልከቱ)። በቼክሰም ላይ ማንቂያ
   ተንሳፋፊ፣ ያልተሳኩ መመርመሪያዎች ወይም ማስረጃዎችን እንደገና ይሞክሩ።
3. ** መመለስ፡** ወደነበረበት ለመመለስ፣ የቀደመውን አንጸባራቂ እንደገና ያስገቡ (ወይም ጡረታ መውጣት
   የአሁኑ ቅጽል) `sorafs_cli manifest submit --alias ... --retire` በመጠቀም።
   የመመለሻ ማረጋገጫዎች እንዲችሉ ሁልጊዜ የመጨረሻውን ጥሩ ጥቅል እና የ CAR ማጠቃለያ ያቆዩ
   የ CI ምዝግብ ማስታወሻዎች ከተሽከረከሩ እንደገና ይፈጠሩ.

## CI የስራ ፍሰት አብነት

ቢያንስ የቧንቧ መስመርዎ የሚከተሉትን ማድረግ አለበት:

1. Build + lint (`npm ci`, `npm run build`, checksum ትውልድ).
2. ጥቅል (`car pack`) እና መግለጫዎችን ያሰሉ.
3. የስራ ወሰን ያለው OIDC token (`manifest sign`) በመጠቀም ይመዝገቡ።
4. ለኦዲት ስራ ቅርሶችን (CAR፣ መግለጫ፣ ጥቅል፣ እቅድ፣ ማጠቃለያ) ይስቀሉ።
5. ለፒን መዝገብ ያቅርቡ፡-
   - ጥያቄዎችን ይጎትቱ → `docs-preview.sora`።
   - መለያዎች / የተጠበቁ ቅርንጫፎች → የምርት ስም ማስተዋወቅ.
6. ከመውጣትዎ በፊት መፈተሻዎችን + የማረጋገጫ በሮች ያሂዱ።

`.github/workflows/docs-portal-sorafs-pin.yml` ሁሉንም እነዚህን ደረጃዎች አንድ ላይ ያገናኛል።
በእጅ ልቀቶች. የሥራ ሂደት;

- ፖርታሉን ይገነባል/ይሞክራል።
- ግንባታውን በ `scripts/sorafs-pin-release.sh` በኩል ያጠቃልላል ፣
- GitHub OIDC ን በመጠቀም የአንጸባራቂ ቅርቅቡን ይፈርማል/ ያረጋግጣል፣
- የ CAR/ማሳያ/ጥቅል/ዕቅድ/ማስረጃ ማጠቃለያዎችን እንደ ቅርስ ሰቀላ፣ እና
- (በአማራጭ) ሚስጥሮች በሚኖሩበት ጊዜ የማሳያውን + ተለዋጭ ስም ያቀርባል።

ስራውን ከመቀስቀስዎ በፊት የሚከተሉትን የማከማቻ ሚስጥሮች/ተለዋዋጮች ያዋቅሩ፡

| ስም | ዓላማ |
|------|--------|
| `DOCS_SORAFS_TORII_URL` | `/v2/sorafs/pin/register` የሚያጋልጥ Torii አስተናጋጅ። |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | ኢፖክ ለዪ ከማስገባት ጋር ተመዝግቧል። |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | ለአንጸባራቂው ማስረከቢያ ስልጣን መፈረም። |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` `true` ሲሆን ከማንፀባረቁ ጋር የታሰረ ተለዋጭ ስም። |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-encoded ቅጽል ማረጋገጫ ጥቅል (አማራጭ፣ ተለዋጭ ስም ማሰርን መዝለልን ተወው)። |
| `DOCS_ANALYTICS_*` | በሌሎች የስራ ፍሰቶች እንደገና ጥቅም ላይ የሚውሉ ነባር ትንታኔዎች/የፍተሻ ነጥቦች። |

የስራ ፍሰቱን በድርጊት UI በኩል ያስነሱ፡

1. `alias_label` (ለምሳሌ `docs.sora.link`)፣ አማራጭ `proposal_alias` ያቅርቡ፣
   እና አማራጭ `release_tag` መሻር።
2. Torii ሳይነኩ ቅርሶችን ለማምረት `perform_submit` ሳይፈተሽ ይተዉት።
   (ለደረቅ ሩጫዎች ጠቃሚ ነው) ወይም በቀጥታ ወደ የተዋቀረው እንዲታተም ያንቁት
   ተለዋጭ ስም

`docs/source/sorafs_ci_templates.md` አሁንም ለአጠቃላይ CI አጋዥዎችን ይመዘግባል
ከዚህ ማከማቻ ውጭ ያሉ ፕሮጀክቶች፣ ግን የፖርታል የስራ ፍሰት ተመራጭ መሆን አለበት።
ለቀን-ቀን ልቀቶች.

#የማረጋገጫ ዝርዝር

- [ ] `npm run build`፣ `npm run test:*`፣ እና `npm run check:links` አረንጓዴ ናቸው።
- [ ] `build/checksums.sha256` እና I18NI0000425X በሥነ ጥበባት የተያዙ።
- [ ] CAR፣ እቅድ፣ አንጸባራቂ እና ማጠቃለያ በ`artifacts/`።
- [ ] Sigstore ጥቅል + የተነጠለ ፊርማ በምዝግብ ማስታወሻዎች ተከማችቷል።
- [ ] `portal.manifest.submit.summary.json` እና `portal.manifest.submit.response.json`
      ማቅረቢያዎች ሲከሰቱ ተይዟል.
- [ ] `portal.pin.report.json` (እና አማራጭ `portal.pin.proposal.json`)
      ከCAR/የተገለጡ ቅርሶች ጋር በማህደር ተቀምጧል።
- [ ] `proof verify` እና I18NI0000432X ምዝግብ ማስታወሻዎች ተቀምጠዋል።
- [ ] Grafana ዳሽቦርዶች ተዘምነዋል + ይሞክሩት በተሳካ ሁኔታ ይፈትሻል።
- [ ] የጥቅልል ማስታወሻዎች (የቀድሞ አንጸባራቂ መታወቂያ + ተለዋጭ ስም መፍጨት) ከ
      የመልቀቅ ትኬት.