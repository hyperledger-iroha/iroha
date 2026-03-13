---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2025-12-29T18:16:35.911952+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook-dz
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
slug: /sorafs/staging-manifest-playbook-dz
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
མེ་ལོང་ `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. འདྲ་བཤུས་གཉིས་ཆ་ར་ གསར་བཏོན་ཚུ་ནང་ ཕྲང་སྒྲིག་འབད་བཞག།
:::

## སྤྱི་མཐོང་།

འདི་ཡང་ བཟོ་བསྐྲུན་གྱི་བསྒྱུར་བཅོས་འདི་ ཁྱབ་སྤེལ་མ་འབད་བའི་ཧེ་མ་ Torii བཀོལ་སྤྱོད་འབད་དེ་ སྤྱི་ཚོགས་ཀྱི་ཆ་འཇོག་འབད་དེ་ཡོད་པའི་ ཆ་ཤས་གསལ་སྡུད་འདི་ ལྕོགས་ཅན་བཟོ་སྟེ་ རྩེད་དེབ་འདི་གིས་ འགྱོཝ་ཨིན། འདི་གིས་ SoraFS གཞུང་སྐྱོང་བཅའ་ཁྲིམས་འདི་ ཆ་འཇོག་འབད་ཡོདཔ་དང་ མཛོད་ཁང་ནང་ལུ་ ཁྲིམས་མཐུན་གྱི་ བརྟན་ཏོག་ཏོ་ཚུ་ འཐོབ་ཚུགསཔ་ཨིན།

## 1. སྔོན་འགྲོ།

1. ཚད་ལྡན་གྱི་སྒྲིག་བཀོད་དང་མིང་རྟགས་ཚུ་མཉམ་འབྱུང་འབད།

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

༢ Torii གིས་ འགོ་བཙུགས་པའི་སྐབས་ (དཔེ་ཚད་ཀྱི་འགྲུལ་ལམ་): `/var/lib/iroha/admission/sorafs`.
3. Torii རིམ་སྒྲིག་འདི་གིས་ གསར་ཐོབ་ཀྱི་འདྲ་མཛོད་དང་ འཛུལ་ཞུགས་བཀག་དམ་ཚུ་ ལྕོགས་ཅན་བཟོཝ་ཨིན།

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

## 2. འཛུལ་ཞུགས་ཡིག་ཆ།

༡ ཆ་འཇོག་འབད་ཡོད་པའི་ བྱིན་མི་འཛུལ་ཞུགས་ཡིག་ཆ་ཚུ་ `torii.sorafs.discovery.admission.envelopes_dir` གིས་ རྒྱབ་རྟེན་འབད་ཡོད་པའི་ སྣོད་ཐོ་ནང་ འདྲ་བཤུས་རྐྱབས།

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. ཁྱོད་ཀྱིས་ མངོན་གསལ་འབད་མི་འདི་ འཕུར་ལྡིང་ནང་ལུ་ བསྐྱར་མངོན་གསལ་འབད་དེ་ བཀབ་བཞག་པ་ཅིན་ ཨེསི་ཨའི་ཇི་ཨེཆ་ཨཔ་ཅིག་ སླར་གཏང་འབད།)
༣ འཛུལ་ཞུགས་འཕྲིན་དོན་ཚུ་གི་དོན་ལུ་ དྲན་ཐོ་ཚུ་ མཇུག་བསྡུ།

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. བདེན་པའི་གསལ་བསྒྲགས་དར་ཁྱབ་

1. མཚན་རྟགས་བཀོད་པའི་བརྡ་ཁྱབ་སྤྲོད་ལེན་པ་ (Norito bytes) གིས་ཁྱོད་ཀྱིས་བཏོན་ཡོད།
   བྱིན་མི་ ཆུ་མཛོད་ཀྱི་ :

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. གསར་རྙེད་ཀྱི་མཇུག་སྣོད་འདི་འདྲི་དཔྱད་འབད་དེ་ ཁྱབ་བསྒྲགས་འདི་ ཀེ་ནོ་ནིག་མིང་གཞན་ཚུ་དང་གཅིག་ཁར་ འབྱུངམ་ཨིན་ ངེས་གཏན་བཟོ།

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   `profile_aliases` ནང་ འཛུལ་ཞུགས་འགོ་དང་པ་སྦེ་ `"sorafs.sf1@1.0.0"` ཚུ་ཚུདཔ་ཨིན།

## 4. ལུས་སྦྱོང་མངོན་གསལ་དང་འཆར་གཞི་མཇུག་བསྡུ།

༡ གསལ་སྟོན་མེ་ཊ་ཌེ་ཊ་ལུ་ལེན་ (འཛུལ་ཞུགས་བསྟར་སྤྱོད་འབད་བ་ཅིན་ རྒྱུན་ལམ་བརྡ་མཚོན་དགོཔ་ཨིན་):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON ཐོན་འབྲས་འདི་བརྟག་དཔྱད་འབད་དེ་ བདེན་དཔྱད་འབད།
   - `chunk_profile_handle` ནི་ `sorafs.sf1@1.0.0` ཡིན།
   - `manifest_digest_hex` གཏན་འབེབས་སྙན་ཞུ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
   - `chunk_digests_blake3` བསྐྱར་བཟོ་འབད་ཡོད་པའི་སྒྲིག་བཀོད་ཚུ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།

## 5. བརྒྱུད་འཕྲིན་བརྟག་དཔྱད།

- Prometheus གིས་གསལ་སྡུད་མེ་ཊིག་གསརཔ་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ཌེཤ་བོརཌི་ཚུ་གིས་ རེ་བ་བསྐྱེད་མི་ མིང་གཞན་གྱི་འོག་ལུ་ གནས་རིམ་བྱིན་མི་འདི་སྟོན་དགོཔ་དང་ གསལ་སྡུད་འདི་ཤུགས་ལྡན་སྦེ་ཡོད་པའི་སྐབས་ བཱརའོན་ཨའུཊི་གྱངས་ཁ་ཚུ་ ཀླད་ཀོར་ནང་བཞག་དགོ།

## 6. འགྲེམས་སྒྲུབ།

༡ ཡུ་ཨར་ཨེལ་ཚུ་དང་གཅིག་ཁར་ སྙན་ཞུ་ཐུང་ཀུ་ཅིག་ བཟུང་ཞིནམ་ལས་ ཨའི་ཌི་ གསལ་སྟོན་འབད་ནི་ དེ་ལས་ ཊེ་ལི་མི་ཊི་པར་ཚུ་ པར་ལེན་འབད།
2. འཆར་གཞིའི་ཐོན་སྐྱེད་ཤུགས་ལྡན་སྒོ་སྒྲིག་མཉམ་དུ་ Nexus བསྐོར་ཐེངས་ནང་སྙན་ཞུ་འདི་བགོ་བཤའ་རྐྱབས།
༣ ཐོན་སྐྱེད་བརྟག་ཞིབ་ཐོ་ཡིག་ནང་ འགྱོ། (`chunker_registry_rollout_checklist.md` ནང་ དོན་ཚན་༤)

རྩེད་དེབ་འདི་དུས་མཐུན་བཟོ་སྟེ་བཞག་མི་འདི་གིས་ ཆ་ཤས་ག་ར་/འཛུལ་ཞུགས་འགོ་བཙུགས་པའི་སྐབས་ལུ་ གོ་རིམ་དང་ཐོན་སྐྱེད་ནང་ལུ་ གོ་རིམ་ཅོག་འཐདཔ་ཚུ་ ངེས་གཏན་བཟོཝ་ཨིན།
