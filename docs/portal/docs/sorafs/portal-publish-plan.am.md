---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd5fff7302924f71ca19593cbbcc29352c00f286ab5bc555d4654e2dc43c3daa
source_last_modified: "2026-01-22T16:26:46.525444+00:00"
translation_last_reviewed: 2026-02-07
id: portal-publish-plan
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
translator: machine-google-reviewed
---

::: ማስታወሻ ቀኖናዊ ምንጭ
መስተዋቶች `docs/source/sorafs/portal_publish_plan.md`. የስራ ፍሰቱ ሲቀየር ሁለቱንም ቅጂዎች ያዘምኑ።
::

የመንገድ ካርታ ንጥል DOCS-7 እያንዳንዱን የሰነዶች ጥበብ ይፈልጋል (ፖርታል ግንባታ ፣ OpenAPI ዝርዝር ፣
SBOMs) በSoraFS አንጸባራቂ ቧንቧ በኩል እንዲፈስ እና በ`docs.sora` በኩል ለማገልገል
ከ `Sora-Proof` ራስጌዎች ጋር። ይህ የማረጋገጫ ዝርዝር ያሉትን ረዳቶች አንድ ላይ ይሰፋል
ስለዚህ ሰነዶች/ዴቭሬል፣ ማከማቻ እና ኦፕስ ልቀቱን ሳያድኑ ማስኬድ ይችላሉ።
በርካታ runbooks.

## 1. ይገንቡ እና ጥቅል ክፍያ

የማሸጊያ ረዳትን ያሂዱ (ለደረቅ ሩጫዎች መዝለል አማራጮች ይገኛሉ)

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- CI አስቀድሞ ካመረተው `--skip-build` `docs/portal/build` እንደገና ይጠቀማል።
- `syft` በማይገኝበት ጊዜ `--skip-sbom` ይጨምሩ (ለምሳሌ የአየር ክፍተት ያለው ልምምድ)።
- ስክሪፕቱ የፖርታል ሙከራዎችን ያካሂዳል፣ CAR + ጥንዶችን ለ`portal` ያወጣል።
  `openapi`፣ `portal-sbom`፣ እና `openapi-sbom`፣ እያንዳንዱን መኪና ሲያረጋግጡ
  `--proof` ተቀናብሯል፣ እና `--sign` ሲዘጋጅ Sigstore ጥቅሎችን ይጥላል።
- የውጤት መዋቅር;

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

መላውን አቃፊ (ወይም ሲምሊንክ በ `artifacts/devportal/sorafs/latest`) ያስቀምጡ
የአስተዳደር ገምጋሚዎች የግንባታ ቅርሶችን መከታተል ይችላሉ።

## 2. የፒን መግለጫዎች + ተለዋጭ ስሞች

መግለጫዎችን ወደ I18NT0000005X ለመግፋት እና ተለዋጭ ስሞችን ለማሰር `sorafs_cli manifest submit` ይጠቀሙ።
`${SUBMITTED_EPOCH}`ን ወደ የቅርብ ጊዜው የጋራ ስምምነት ዘመን (ከ
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` ወይም የእርስዎ ዳሽቦርድ)።

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="ih58..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- ለI18NI0000029X ይድገሙ እና SBOM መገለጫዎች (የቅጽል ባንዲራዎችን ለ
  አስተዳደር የስም ቦታ ካልሰጠ በስተቀር SBOM ጥቅሎች)።
- አማራጭ፡ I18NI0000030X ከማስረከቢያው ጋር አብሮ ይሰራል
  ሁለትዮሽ አስቀድሞ ከተጫነ ማጠቃለያ።
- የመመዝገቢያ ሁኔታን በ ጋር ያረጋግጡ
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- ለማየት ዳሽቦርዶች፡ `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  መለኪያዎች)።

## 3. የጌትዌይ ራስጌዎች እና ማረጋገጫዎች

የኤችቲቲፒ አርዕስት አግድ + አስገዳጅ ዲበ ዳታ ይፍጠሩ፡

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- አብነቱ `Sora-Name`፣ `Sora-CID`፣ `Sora-Proof` እና
  `Sora-Proof-Status` ራስጌዎች እና ነባሪ CSP/HSTS/ፍቃዶች-መመሪያ።
- የተጣመረ ጥቅልል ​​የራስጌ ስብስብ ለመስራት `--rollback-manifest-json` ይጠቀሙ።

ትራፊክን ከማጋለጥዎ በፊት፣ ያሂዱ፡-

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- ፍተሻው የGAR ፊርማ ትኩስነትን፣ ተለዋጭ ፖሊሲን እና የTLS የምስክር ወረቀትን ያስፈጽማል
  የጣት አሻራዎች.
- የእራስ ሰርተፍኬት ማሰሪያው አንጸባራቂውን በ`sorafs_fetch` እና በሱቆች ያወርዳል።
  የ CAR መልሶ ማጫወት ምዝግብ ማስታወሻዎች; ለኦዲት ማስረጃዎች ውጤቱን ያስቀምጡ.

## 4. ዲ ኤን ኤስ እና ቴሌሜትሪ የጥበቃ መንገዶች

1. አስተዳደር አስገዳጅነቱን ማረጋገጥ እንዲችል የዲ ኤን ኤስ አጽሙን ያድሱ፡-

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. በታቀደ ጊዜ ተቆጣጠር፡-

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   ዳሽቦርዶች፡ I18NI0000044X፣
   `sorafs_fetch_observability.json`፣ እና የፒን መመዝገቢያ ሰሌዳ።

3. የማንቂያ ደንቦችን ያጨሱ (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) እና
   ለመልቀቂያ ማህደር ምዝግብ ማስታወሻዎችን/ቅጽበታዊ ገጽ እይታዎችን ያንሱ።

## 5. የማስረጃ ጥቅል

በሚለቀቁበት ትኬት ወይም የአስተዳደር ፓኬጅ ውስጥ የሚከተሉትን ያካትቱ፡

- `artifacts/devportal/sorafs/<stamp>/` (መኪናዎች፣ መግለጫዎች፣ SBoms፣ ማረጋገጫዎች፣
  Sigstore ጥቅሎች፣ ማጠቃለያዎችን ያስገቡ)።
- የጌትዌይ ፍተሻ + የራስ ሰር ማረጋገጫ ውጤቶች
  (`artifacts/sorafs_gateway_probe/<stamp>/`፣
  `artifacts/sorafs_gateway_self_cert/<stamp>/`)።
- የዲ ኤን ኤስ አጽም + ራስጌ አብነቶች (`portal.gateway.headers.txt`፣
  `portal.gateway.plan.json`፣ `portal.dns-cutover.json`)።
- ዳሽቦርድ ቅጽበታዊ ገጽ እይታዎች + ማንቂያ እውቅናዎች።
- `status.md` ማሻሻያ አንጸባራቂ መፍጨት እና ቅጽል አስገዳጅ ጊዜን በመጥቀስ።

ይህንን የማረጋገጫ ዝርዝር በመከተል DOCS-7 ያቀርባል፡ የፖርታል/OpenAPI/SBOM ጭነት
በቆራጥነት የታሸገ፣ በቅጥያ ስሞች የተሰካ፣ በ`Sora-Proof` የተጠበቀ
ራስጌዎች፣ እና ከጫፍ እስከ ጫፍ ባለው ተመልካች ቁልል በኩል ክትትል የሚደረግበት።