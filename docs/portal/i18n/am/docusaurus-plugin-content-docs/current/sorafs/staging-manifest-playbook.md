---
id: staging-manifest-playbook
lang: am
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

## አጠቃላይ እይታ

ይህ የመጫወቻ መጽሐፍ ወደ ምርት ለውጡን ከማስተዋወቁ በፊት በፓርላማ የተረጋገጠውን የቻንከር ፕሮፋይል በዝግጅት Torii ማሰማራቱ ላይ ያልፋል። የ I18NT0000002X የአስተዳደር ቻርተር እንደፀደቀ እና ቀኖናዊው ቋሚዎች በማከማቻው ውስጥ ይገኛሉ ብሎ ያስባል።

## 1. ቅድመ ሁኔታዎች

1. ቀኖናዊ ቋሚዎችን እና ፊርማዎችን ያመሳስሉ፡

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii ጅምር ላይ የሚያነበውን የመግቢያ ኤንቨሎፕ ማውጫ ያዘጋጁ (ለምሳሌ መንገድ)፡ `/var/lib/iroha/admission/sorafs`።
3. የI18NT0000006X ውቅረት የግኝት መሸጎጫ እና የመግቢያ ማስፈጸሚያ ማብቃቱን ያረጋግጡ፡-

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. የመግቢያ ኤንቨሎፕ ያትሙ

1. በ`torii.sorafs.discovery.admission.envelopes_dir` የተጠቀሰውን የተፈቀደውን የአቅራቢ ኤንቨሎፕ ይቅዱ፡

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. I18NT0000007X እንደገና ያስጀምሩ (ወይም ጫኚውን በበረራ ላይ ዳግም ከጫኑ) SIGUP ይላኩ።
3. የመግቢያ መልእክቶችን መዝገቦችን በጅራት ይያዙ፡

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. የግኝት ስርጭትን ያረጋግጡ

1. በእርስዎ የተፈረመውን አቅራቢ የማስታወቂያ ክፍያ (Norito ባይት) ይለጥፉ
   አቅራቢ ቧንቧ መስመር;

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. የግኝቱን የመጨረሻ ነጥብ ይጠይቁ እና ማስታወቂያው በቀኖናዊ ተለዋጭ ስሞች መከሰቱን ያረጋግጡ።

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   `profile_aliases` `"sorafs.sf1@1.0.0"`ን እንደ መጀመሪያው ግቤት ማካተቱን ያረጋግጡ።

## 4. የአካል ብቃት እንቅስቃሴ መግለጫ እና የመጨረሻ ነጥቦችን ያቅዱ

1. የአንጸባራቂውን ሜታዳታ አምጡ (መግቢያ ከተተገበረ የዥረት ማስመሰያ ያስፈልገዋል)

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. የJSON ውፅዓትን ይፈትሹ እና ያረጋግጡ፡-
   - `chunk_profile_handle` I18NI0000021X ነው።
   - `manifest_digest_hex` ከመወሰኛ ዘገባ ጋር ይዛመዳል።
   - `chunk_digests_blake3` እንደገና ከተፈጠሩት እቃዎች ጋር ይጣጣማል.

## 5. ቴሌሜትሪ ቼኮች

- ያረጋግጡ I18NT0000000X አዲሱን የመገለጫ መለኪያዎች ያጋልጣል፡

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ዳሽቦርዶች የዝግጅት አቅራቢውን በሚጠበቀው ተለዋጭ ስም ማሳየት እና ፕሮፋይሉ ንቁ በሚሆንበት ጊዜ ቡናማ መውጫ ቆጣሪዎችን በዜሮ ማቆየት አለበት።

## 6. የመልቀቅ ዝግጁነት

1. አጭር ሪፖርት በዩአርኤሎች፣ የሰነድ መታወቂያ እና በቴሌሜትሪ ቅጽበተ ፎቶ ያንሱ።
2. ሪፖርቱን በI18NT0000003X ልቀት ቻናል ከታቀደው የምርት ማስነሻ መስኮት ጋር ያካፍሉ።
3. ባለድርሻ አካላት ከፈረሙ በኋላ ወደ የምርት ማረጋገጫ ዝርዝር (ክፍል 4 በ `chunker_registry_rollout_checklist.md`) ይቀጥሉ።

ይህን የመጫወቻ ደብተር ማዘመን እያንዳንዱ ቸንከር/የመግቢያ ልቀት በማዘጋጀት እና በምርት ላይ ተመሳሳይ ቆራጥ እርምጃዎችን እንደሚከተል ያረጋግጣል።