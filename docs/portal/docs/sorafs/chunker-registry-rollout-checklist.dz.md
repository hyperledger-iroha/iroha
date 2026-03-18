---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a117889e81f876c00129ade76a9a04aa39181add2378ef5c19110b7be30f9d6f
source_last_modified: "2026-01-05T09:28:11.859335+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry-rollout-checklist
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

# I18NT0000000X ཐོ་འགོད་ཐོ་ཡིག དཔྱད་གཞི།

ཞིབ་དཔྱད་ཐོ་ཡིག་འདི་གིས་ ཅཱན་ཀར་གསལ་སྡུད་གསརཔ་ ཁྱབ་སྤེལ་འབད་ནི་ལུ་ དགོ་པའི་ གོ་རིམ་ཚུ་ འཛིན་བཟུང་འབདཝ་ཨིན།
གཞུང་སྐྱོང་གི་ཤུལ་ལས་ བསྐྱར་ཞིབ་ལས་ ཐོན་སྐྱེད་ཚུན་ཚོད་ བྱིན་མི་ འཛུལ་ཞུགས་ཀྱི་ བང་རིམ་ཚུ།
ཆོག་ཐམ་ཆ་འཇོག་འབད་ཡོདཔ།

> **Scope:** ལེགས་བཅོས་འབད་མི་གསར་བཏོན་ཆ་མཉམ་ལུ་འཇུག་སྤྱོད་འབདཝ་ཨིན།
> `sorafs_manifest::chunker_registry`, བྱིན་མི་འཛུལ་ཞུགས་ཡིག་ཆ་ཚུ་, ཡང་ན་
> ཀེ་ནོ་ནིག་སྒྲིག་ཆས་བཱན་ཌལ་ (`fixtures/sorafs_chunker/*`).

## 1. འཕུར་འགྲུལ་སྔོན་གྱི་བདེན་དཔང་།

༡ བདེ་སྒྲིག་ཚུ་ ལོག་སྟེ་བཟོ་སྟེ་ གཏན་འབེབས་བཟོ་ནི།
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. 2 2 2012
   I18NI000000011X (ཡང་ན་འབྲེལ་ཡོད་གསལ་སྡུད།
   སྙན་ཞུ།) བསྐྱར་བཟོ་བྱས་པའི་དངོས་པོ་དང་མཐུན་པ།
3. དང་བཅས་ `sorafs_manifest::chunker_registry` བསྡུ་སྒྲིག།
   I18NI000000013X གཡོག་བཀོལ་ཐོག་ལས་:
   I18NF0000004X
༤ གྲོས་འཆར་ཡིག་ཆ་འདི་དུས་མཐུན་བཟོ་ནི།
   - `docs/source/sorafs/proposals/<profile>.json`
   - ལྷན་ཚོགས་ཀྱི་སྐར་མ་ཚུ་ I18NI0000015X གི་འོག་ལུ་ཨིན།
   - གཏན་འཁེལ་གྱི་སྙན་ཐོ།

## 2. གཞུང་སྐྱོང་མཚན་རྟགས།

1. ལག་ཆས་ལས་བྱེད་ཚོགས་ཆུང་གི་སྙན་ཞུ་དང་ གྲོས་འཆར་སའྱི་ སོ་ར་ལུ་ འཇུ་ནི།
   སྤྱི་ཚོགས་གཞི་རྟེན་གྱི་ ཚོགས་ཆུང་།
2. ཆ་འཇོག་གནང་བའི་ཁ་གསལ་ ༢༠༠༨ ནང་།
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
༣ སྤྱི་ཚོགས་ཀྱི་ ཡིག་ཤུབས་དེ་ སྒྲིག་ཆས་ཀྱི་ མཉམ་དུ་ དཔར་བསྐྲུན་འབད་ནི།
   `fixtures/sorafs_chunker/manifest_signatures.json`.
༤ གཞུང་སྐྱོང་ལས་ཁུངས་ཀྱི་གྲོགས་རམ་བརྒྱུད་དེ་ ཡིག་ཤུབས་འདི་ འཐོབ་ཚུགསཔ་སྦེ་ བདེན་དཔྱད་འབད།
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. གོ་རིམ་གྱི་ཐོ་གཞུང་།

[འཁྲབ་སྟོན
ཁ་གསལ་གྱི་གོམ་པ་འདི་དག་གི་སྐོར་རོགས།

1. `torii.sorafs` དང་མཉམ་དུ་ཉོ་ཚོང་ནུས་པ་དང་འཛུལ་ཞུགས་བྱེད་པ།
   བསྟར་དཔྱད་འབད་ཡོདཔ་ཨིན་ (`enforce_admission = true`).
༢ ཆ་འཇོག་གྲུབ་པའི་ མཁོ་སྤྲོད་འབད་མི་ འཛུལ་ཞུགས་ཡིག་ཆ་ཚུ་ གནས་རིམ་ཐོ་བཀོད་ལུ་ བཀལ་དགོ།
   `torii.sorafs.discovery.admission.envelopes_dir` གིས་ གཞི་བསྟུན་འབད་ཡོདཔ།
༣ འཚོལ་ཞིབ་ཨེ་པི་ཨའི་བརྒྱུད་དེ་ བྱིན་མི་ ཁྱབ་བསྒྲགས་ཚུ་ བདེན་དཔྱད་འབད།
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
༤ ལུས་རྩལ་གྱི་མགོ་ཡིག་ཚུ་དང་གཅིག་ཁར་ གསལ་སྟོན་/འཆར་གཞི་གི་མཇུག་བསྡུ།
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
༥ ཊེ་ལི་མི་ཊི་ ཌེཤ་བོརཌ་ (I18NI0000021X) དང་ ཉེན་བརྡ་ལམ་ལུགས་ཚུ་ སྙན་ཞུ་འབད།
   འཛོལ་བ་མེད་པའི་གསལ་སྡུད་གསརཔ།

## 4.ཐོན་ལས་ཀྱི་ཐོ་ཡིག།

༡ ཐོན་སྐྱེད་ I18NT0000002X མཛུབ་གནོན་ལུ་ གོ་རིམ་གྱི་གོ་རིམ་ཚུ་ བསྐྱར་ལོག་འབད།
2. ཤུགས་ལྡན་གྱི་སྒོ་སྒྲིག་ (ཚེས་གྲངས་/དུས་ཚོད་ བྱིན་རླབས་དུས་ཡུན་, བཤུད་འཆར་གཞི་) ལུ་གསལ་བསྒྲགས་འབད།
   བཀོལ་སྤྱོད་པ་དང་ཨེསི་ཌི་ཀེ་རྒྱུ་ལམ་ཚུ།
3. གསར་བཏོན་ PR མཉམ་བསྡོམས་འབད།
   - དུས་མཐུན་བཟོས་པའི་སྒྲིག་ཆས་དང་ ཡིག་ཤུགས།
   - ཡིག་ཆ་བསྒྱུར་བཅོས་ (ཡིག་ཆའི་གཞི་བསྟུན་ གཏན་འབེབས་སྙན་ཐོ།)
   - ལམ།/གནས་ལུགས་གསར་བཞེངས།
༤ ཁུངས་གཏུག་འབད་ནིའི་དོན་ལུ་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ ཅ་རྙིང་ཚུ་ གསར་བཏོན་དང་ གཏན་མཛོད་ལུ་ རྟགས་བཀལ།

## 5. འཁོར་ལོའི་རྗེས་སུ་རྩིས་ཞིབ་པ།

1. མཐའ་མའི་མེ་ཊིག་ཚུ་ བསྡུ་ལེན་འབད་ (གསར་འཚོལ་གྲངས་འབོར, མཐར་འཁྱོལ་གྱི་ཚད་, འཛོལ་བ་འཛོལ་བ།
   histograms) ༢༤h གི་ཤུལ་ལས་ ༢༤h.
༢ བཅུད་བསྡུས་ཐུང་ཀུ་ཅིག་དང་གཅིག་ཁར་ I18NI0000002X དུས་མཐུན་དང་ གཏན་འབེབས་སྙན་ཞུ་ལུ་འབྲེལ་མཐུད་འབད།
༣ རྗེས་འཇུག་ལས་ཀ་གང་རུང་བཙུགས་ (དཔེར་ན་ གསལ་སྡུད་རྩོམ་སྒྲིག་ལམ་སྟོན་) ནང་།
   `roadmap.md`.