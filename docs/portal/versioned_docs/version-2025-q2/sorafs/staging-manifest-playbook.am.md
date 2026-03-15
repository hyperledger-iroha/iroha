---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2025-12-29T18:16:35.911952+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook-am
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
slug: /sorafs/staging-manifest-playbook-am
---

::: ማስታወሻ ቀኖናዊ ምንጭ
መስተዋቶች `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. ሁለቱንም ቅጂዎች በመልቀቂያዎች ላይ ያቆዩ።
::

## አጠቃላይ እይታ

ይህ የመጫወቻ መጽሐፍ ወደ ምርት ለውጡን ከማስተዋወቁ በፊት በፓርላማ የተረጋገጠውን የቻንከር ፕሮፋይል በዝግጅት Torii ማሰማራቱ ላይ ያልፋል። የ SoraFS የአስተዳደር ቻርተር እንደፀደቀ እና ቀኖናዊው ቋሚዎች በማከማቻው ውስጥ ይገኛሉ ብሎ ያስባል።

## 1. ቅድመ ሁኔታዎች

1. ቀኖናዊ ቋሚዎችን እና ፊርማዎችን ያመሳስሉ፡

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Torii ጅምር ላይ የሚያነበውን የመግቢያ ኤንቨሎፕ ማውጫ ያዘጋጁ (ለምሳሌ መንገድ)፡ `/var/lib/iroha/admission/sorafs`።
3. የTorii ውቅረት የግኝት መሸጎጫ እና የመግቢያ ማስፈጸሚያ ማብቃቱን ያረጋግጡ፡-

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

1. በ`torii.sorafs.discovery.admission.envelopes_dir` በተጠቀሰው ማውጫ ውስጥ የተፈቀደውን የአቅራቢ ኤንቨሎፕ ይቅዱ፡-

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii እንደገና ያስጀምሩ (ወይም ጫኚውን በበረራ ላይ ዳግም ከጫኑ) SIGUP ይላኩ።
3. የመግቢያ መልእክቶችን መዝገቦችን በጅራት ይያዙ፡

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. የግኝት ስርጭትን ያረጋግጡ

1. በእርስዎ የተፈረመውን አቅራቢ የማስታወቂያ ክፍያ (Norito ባይት) ይለጥፉ
   አቅራቢ ቧንቧ መስመር;

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. የግኝቱን የመጨረሻ ነጥብ ይጠይቁ እና ማስታወቂያው በቀኖናዊ ተለዋጭ ስሞች መከሰቱን ያረጋግጡ።

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
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
   - `chunk_profile_handle` `sorafs.sf1@1.0.0` ነው።
   - `manifest_digest_hex` ከቆራጥነት ዘገባ ጋር ይዛመዳል።
   - `chunk_digests_blake3` እንደገና ከተፈጠሩት እቃዎች ጋር ይጣጣማል.

## 5. ቴሌሜትሪ ቼኮች

- ያረጋግጡ Prometheus አዲሱን የመገለጫ መለኪያዎች ያጋልጣል፡

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ዳሽቦርዶች የዝግጅት አቅራቢውን በሚጠበቀው ተለዋጭ ስም ማሳየት እና ፕሮፋይሉ ንቁ በሚሆንበት ጊዜ ቡናማ መውጫ ቆጣሪዎችን በዜሮ ማቆየት አለበት።

## 6. የመልቀቅ ዝግጁነት

1. አጭር ሪፖርት በዩአርኤሎች፣ የሰነድ መታወቂያ እና በቴሌሜትሪ ቅጽበተ ፎቶ ያንሱ።
2. ሪፖርቱን በNexus ልቀት ቻናል ከታቀደው የምርት ማስነሻ መስኮት ጋር ያካፍሉ።
3. ባለድርሻ አካላት ከፈረሙ በኋላ ወደ የምርት ማረጋገጫ ዝርዝር (ክፍል 4 በ `chunker_registry_rollout_checklist.md`) ይቀጥሉ።

ይህን የመጫወቻ ደብተር ማዘመን እያንዳንዱ ቸንከር/የመግቢያ ልቀት በማዘጋጀት እና በምርት ላይ ተመሳሳይ ቆራጥ እርምጃዎችን እንደሚከተል ያረጋግጣል።
