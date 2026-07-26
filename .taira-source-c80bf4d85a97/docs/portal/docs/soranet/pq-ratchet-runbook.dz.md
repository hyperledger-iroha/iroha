---
lang: dz
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 370733f000ffa7022cab18931c2697031a225d2ce3ae83382896c0d61f9fe6e2
source_last_modified: "2026-01-05T09:28:11.912569+00:00"
translation_last_reviewed: 2026-02-07
id: pq-ratchet-runbook
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

## དགོས༌དོན

འ་ནི་རན་དེབ་འདི་གིས་ SoraNet གི་གནས་རིམ་གྱི་ ཚད་གཞི་ (PQ) མིང་མ་བཀོད་པའི་སྲིད་བྱུས་ཀྱི་དོན་ལུ་ མེ་བཏེགས་ཀྱི་གོ་རིམ་ལམ་སྟོན་འབདཝ་ཨིན། བཀོལ་སྤྱོད་པ་ཚུ་གིས་ ཡར་འཕེལ་ (གནས་རིམ་ཀ་ -> གནས་རིམ་བི་ -> གནས་རིམ་ ག) གཉིས་ཆ་ར་ བསྐྱར་སྦྱོང་འབད་ཞིནམ་ལས་ པི་ཀིའུ་བཀྲམ་སྤེལ་གྱི་ མར་ཕབ་ཚུ་ གནས་རིམ་བི་/ཨེ་ལུ་ ལོག་ཚད་འཛིན་འབདཝ་ཨིན། དམག་སྦྱོང་འདི་གིས་ ཊེ་ལི་མི་ཊི་རི་ཧུཀ་ (I18NI000000014X, `sorafs_orchestrator_brownouts_total`, I18NI000000016X) བྱུང་རྐྱེན་གྱི་ བསྐྱར་སྦྱོང་དྲན་དེབ་ཀྱི་དོན་ལུ་ ཅ་རྙིང་ཚུ་ བསྡུ་ལེན་འབདཝ་ཨིན།

## སྔོན་འགྲོའི་ཆ་རྐྱེན།

- ལེ་ཊེཊ་ `sorafs_orchestrator` ལྕོགས་གྲུབ་ཀྱི་ལྗིད་ཚད་དང་གཅིག་ཁར་ གཉིས་ལྡན་ (`docs/source/soranet/reports/pq_ratchet_validation.md` ནང་སྟོན་ཡོད་པའི་ དྲིལ་གཞི་བསྟུན་ནང་ ཡང་ན་ ཤུལ་ལས་ བཀོདཔ་ཨིན།)
- I18NT00000000000X/I18NT000000002X ལུ་འཛུལ་སྤྱོད་འབད་ནི། `dashboards/grafana/soranet_pq_ratchet.json`.
- མིང་ཚིག་སྲུང་ཆས་སྣོད་ཐོ་པར་བཀོད། དམག་སྦྱོང་མ་འབད་བའི་ཧེ་མ་ འདྲ་བཤུས་ཅིག་ བཀལ་ཞིནམ་ལས་ བདེན་དཔྱད་འབད།

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

འབྱུང་ཁུངས་སྣོད་ཐོ་འདི་གིས་ JSON རྐྱངམ་ཅིག་དཔར་བསྐྲུན་འབད་བ་ཅིན་ བསྒྱིར་མི་གྲོགས་རམ་པ་ཚུ་ གཡོག་བཀོལ་བའི་ཧེ་མ་ I18NI000000000X དང་གཅིག་ཁར་ I18NT000000004X གཉིས་ལྡན་ལུ་ ལོག་གཏང་དགོ།

- སི་ཨེལ་ཨའི་དང་གཅིག་ཁར་ མེ་ཊ་ཌེ་ཊ་དང་ སྔོན་རིམ་བཏོན་མི་ བསྐོར་འཁོར་གྱི་ ཅ་ཆས་ཚུ་ བསྡུ་ལེན་འབད་ནི།

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- འབོད་བརྡ་གུ་ཡོད་པའི་སྡེ་ཚན་ཚུ་གིས་ ཆ་འཇོག་འབད་མི་ སྒོ་སྒྲིག་བསྒྱུར་བཅོས་འབད།

## འཕེལ་རྒྱས་ཀྱི་གོམ་པ།

1. **གནས་རིམ་རྩིས་ཞིབ་**

   འགོ་བཙུགས་གནས་རིམ་ཐོ་བཀོད་འབད།

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   ཁྱབ་སྤེལ་མ་འབད་བའི་ཧེ་མ་ `anon-guard-pq` ལུ་རེ་བ་བསྐྱེད།

2. **གནས་རིམ་ཁ་ (Majority PQ)** ལུ་ཁྱབ་སྤེལ་འབད།

   I18NF0000008X

   - གསལ་སྟོན་གྱི་དོན་ལུ་ གསལ་སྟོན་ཚུ་གི་དོན་ལུ་ བསྒུགས་ >སྐར་མ་༥།
   - Grafana (I18NI000002X dashboard) གིས་ "སྲིད་བྱུས་བྱུང་ལས་" པེ་ནཱལ་གྱིས་ I18NI000000023X གི་དོན་ལུ་ I18NI00000000024X འདི་སྟོནམ་ཨིན།
   - གསལ་གཞི་པར་བཏབ་ནི་ཡང་ན་ པེ་ནཱལ་ཇེ་ཨེསི་ཨོ་ཨེན་ཅིག་ བསྡུ་སྒྲིག་འབད་ཞིནམ་ལས་ བྱུང་རྐྱེན་དྲན་ཐོ་ལུ་མཉམ་སྦྲགས་འབད།

3. **གནས་རིམ་ C (Scrit PQ)** ལུ་ཁྱབ་སྤེལ་འབད།

   I18NF0000009X

   - `sorafs_orchestrator_pq_ratio_*` ཧིསི་ཊོ་གཱརམ་ཚུ་ ༡.༠ ལུ་བདེན་དཔྱད་འབད།
   - བཱརའུ་ཨའུཊ་ཀའུན་ཊར་འདི་ ཕྲང་ཏང་ཏ་སྦེ་ལུསཔ་ཨིན། དེ་མེན་པ་ཅིན་ མར་ཕབ་ཀྱི་གོ་རིམ་ཚུ་ རྗེས་སུ་འཇུགཔ་ཨིན།

## བརླག་གཏོར་ / རྒྱ་སྨུག་སྦྱོང་བརྡར།

1. **བཅོས་མའི་ PQ མ་ལང་པའི་དཀའ་ངལ་**།

   སྲུང་སྐྱོབ་སྣོད་ཐོ་འདི་ སྔར་ལུགས་ཀྱི་ཐོ་བཀོད་ཚུ་ལུ་རྐྱངམ་ཅིག་ བཏོག་བཏང་ཞིནམ་ལས་ རྩེད་ཐང་གི་མཐའ་འཁོར་ནང་ པི་ཀིའུ་ རི་ལེ་ཚུ་ ལྕོགས་ཅན་བཟོ་ཞིནམ་ལས་ རོལ་དབྱངས་འདྲ་མཛོད་འདི་ ལོག་མངོན་གསལ་འབད།

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **བགྲོད་ལམ་བརྙན་བའི་བརྡ་འཕྲིན་**།

   - ཌེཤ་བོརཌི་: པེ་ནཱལ་ "Brownout Rate" འདི་ ༠ ལས་ལྷག་པའི་ སྤེཝ་ཨིན།
   - པྲོམཀིའུ་ཨེལ་: I18NI0000026X
   - I18NI000000027X གིས་ `anonymity_outcome="brownout"` གིས་ `anonymity_reason="missing_majority_pq"` དང་ཅིག་ཁར་སྙན་ཞུ་འབད་དགོ།

3. **Demote B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   PQ བཀྲམ་སྤེལ་འདི་ད་ལྟོ་ཡང་ལངམ་སྦེ་མེད་པ་ཅིན་ I18NI000000030X ལུ་མར་ཕབ་འབདཝ་ཨིན། སྦྱོང་བརྡར་འདི་ བཱརའོན་ཨའུཊ་ཀའུན་ཊར་ཚུ་ གཞི་བཅག་ཞིནམ་ལས་ ཁྱབ་སྤེལ་ཚུ་ ལོག་སྟེ་བཙུགས་ཚུགས་པའི་སྐབས་ མཇུག་བསྡུཝ་ཨིན།

༤. **སྲུང་སྐྱོབ་སྣོད་ཐོ་སླར་གསོ་འབད་ནི།**།

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## བརྒྱུད་འཕྲིན་དང་ཅ་ཆས།

- **ཌེཤ་བོརཌ་:** I18NI0000031X
- **I18NT0000001X དྲན་སྐུལ་ཚུ་:** རིམ་སྒྲིག་འབད་ཡོད་པའི་ཨེསི་ཨེལ་ཨོ་གི་འོག་ལུ་སྡོད་ནི་ཨིན་ (<5% སྒོ་སྒྲིག་གང་རུང་ནང་)
- **རྐྱེན་ངན་དྲན་ཐོ་:** བཟུང་ཡོད་པའི་ ཊེ་ལི་མི་ཊི་ པར་ཆས་དང་ བཀོལ་སྤྱོད་པ་ཚུ་ I18NI000000033X ལུ་ མཉམ་སྦྲགས་འབད།
- **མཚན་རྟགས་བཀོད་མི་:** དམག་སྦྱོང་དང་ སྐུགས་བཀོད་སྒྲིག་འདི་ I18NI000000035X, copite BLAKE3 ནང་ལུ་འདྲ་བཤུས་རྐྱབ་ནི་དང་ མཚན་རྟགས་བཀོད་ཡོད་པའི་ I18NI000000036X བཟོ་བསྐྲུན་འབདཝ་ཨིན།

དཔེ:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

བཟོ་བཏོན་འབད་ཡོད་པའི་མེ་ཊ་ཌེ་ཊ་དང་ གཞུང་སྐྱོང་སྦུང་ཚན་ལུ་ མཚན་རྟགས་མཉམ་སྦྲགས་འབད།

## རོལ་རིམ།

གལ་སྲིད་ དམག་སྦྱོང་འདི་གིས་ PQ མ་ལང་པའི་དཀའ་ངལ་ངོ་མ་ཚུ་ གསལ་སྟོན་འབད་བ་ཅིན་ གནས་རིམ་ཀ་གུ་སྡོད་ཞིནམ་ལས་ ཡོངས་འབྲེལ་ TL ལུ་ བརྡ་དོན་སྤྲོད་ཞིནམ་ལས་ བསྡུ་སྒྲིག་འབད་ཡོད་པའི་ མེ་ཊིག་ཚུ་ དང་ སྲུང་རྒྱབ་སྣོད་ཐོ་ཚུ་ བྱུང་རྐྱེན་འཚོལ་ཞིབ་པ་ལུ་ མཐུད་དགོ། སྤྱིར་བཏང་ཞབས་ཏོག་སླར་གསོ་འབད་ནི་ལུ་ ཧེ་མ་ལས་བཟུང་ཡོད་པའི་ ཉེན་སྲུང་སྣོད་ཐོ་ཕྱིར་འདྲེན་འདི་ལག་ལེན་འཐབ།

:::tip རི་རིམ་ཁྱོན་ཁྱབ་ཁོངས།
I18NI000000037X གིས་ བཅོས་མའི་བདེན་དཔྱད་རྒྱབ་ཐག་འདི་ དྲིལ་འདི་བྱིནམ་ཨིན།
:::