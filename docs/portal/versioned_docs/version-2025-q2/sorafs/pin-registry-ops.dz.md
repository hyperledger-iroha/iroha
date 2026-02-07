---
lang: dz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
མེ་ལོང་ `docs/source/sorafs/runbooks/pin_registry_ops.md`. ཐོན་རིམ་གཉིས་ཆ་ར་ གསར་བཏོན་ཚུ་ནང་ ཕྲང་སྒྲིག་འབད་བཞག།
:::

## སྤྱི་མཐོང་།

རན་བུ་འདི་གིས་ SoraFS པིན་ཐོ་བཀོད་དང་ དེ་གི་འདྲ་བཤུས་ཞབས་ཏོག་གནས་རིམ་ཆིངས་ཡིག་ (SLAs) ཚུ་ ག་དེ་སྦེ་ ལྟ་རྟོག་དང་ འབད་ནི་ཨིན་ན་ ཡིག་ཆ་བཟོཝ་ཨིན། མེ་ཊིག་ཚུ་ `iroha_torii` ལས་འབྱུང་ཡོདཔ་དང་ `torii_sorafs_*` གི་འོག་ལུ་ `torii_sorafs_*` གི་འོག་ལུ་ ཕྱིར་འདྲེན་འབདཝ་ཨིན། Torii རྒྱབ་གཞི་ནང་ ༣༠ སྐར་ཆ་གཅིག་གི་བར་མཚམས་ལུ་ ཐོ་བཀོད་གནས་སྟངས་ཀྱི་དཔེ་ཚད་ཚུ་ དཔེ་ཚད་འབདཝ་ལས་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ `/v1/sorafs/pin/*` མཐའ་མཚམས་ཚུ་ འོས་འདེམས་འཐབ་པའི་སྐབས་ ཌེཤ་བོརཌི་ཚུ་ ད་ལྟོའི་གནས་སྟངས་ནང་ ལུས་ཡོདཔ་ཨིན། འོག་གི་དབྱེ་ཚན་ཚུ་ལུ་ཐད་ཀར་དུ་ སབ་ཁྲ་བཟོ་མི་ Grafana བཀོད་སྒྲིག་དོན་ལུ་ གཅེས་སྐྱོང་འབད་ཡོད་པའི་ ཌེཤ་བོརཌ་ (`docs/source/grafana_sorafs_pin_registry.json`) ནང་འདྲེན་འབད།

## མེཊིག་གི་རྒྱབ་རྟེན།

| མེ་ཊིག་ | ཁ་ཡིག་ཚུ། | འགྲེལ་བཤད་ |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \|`approved` \|`retired`) | མི་ཚེའི་འཁོར་རིམ་གྱི་གནས་སྟངས་ཀྱིས་ རིམ་སྒྲིག་གསལ་སྟོན་ཐོ་བཀོད། |
| `torii_sorafs_registry_aliases_total` | — | ཐོ་བཀོད་ནང་ཐོ་བཀོད་འབད་ཡོད་པའི་ ཤུགས་ལྡན་གྱི་གསལ་སྟོན་གྱི་གྱངས་ཁ་བཀོད། |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \|`expired`) | གནས་རིམ་གྱིས་ ཆ་ཤས་འབད་ཡོད་མི་ འདྲ་བཤུས་གོ་རིམ་འདི་ རྒྱབ་གཞི་བཀོད་ནི། |
| `torii_sorafs_replication_backlog_total` | — | སྟབས་བདེ་བའི་ཚད་གཞི་མེ་ལོང་ `pending` བཀའ་རྒྱ་ཚུ། |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \|`missed` \|`pending`) | ཨེསི་ཨེལ་ཨེ་རྩིས་ཁྲ་: `met` གྱངས་ཁ་ཚུ་གིས་ དུས་ཚོད་ཀྱི་ནང་འཁོད་ལུ་ བཀའ་རྒྱ་ཚུ་མཇུག་བསྡུ་ཡོདཔ་ཨིན། `missed` གིས་ མཇུག་བསྡུའི་མཇུག་བསྡུའི་མཇུག་བསྡུ་ + དུས་ཡུན་ཚུ་ བསྡུ་སྒྲིག་འབད་ཡོདཔ་ཨིན། |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \|`p95` \|`max` \|`count`) | བསྡོམས་རྩིས་མཇུག་བསྡུ་བའི་ འཕྲོ་མཐུད་ (བཏོན་ནི་དང་ མཇུག་བསྡུ་བའི་བར་ན་ འགོ་བཙུགས་མི)། |
| `torii_sorafs_replication_deadline_slack_epochs` | (`stat` (`avg` \| `p95` \| `max` \| `count`) | Pround-གོ་རིམ་གྱི་ སྒོ་སྒྲིག་ཚུ་ (དུས་ཚོད་ལས་ ཕབ་རྩིས་འདི་ epoch བཏོན་ཡོདཔ་ཨིན།) |

འཇལ་ཚད་ཆ་མཉམ་རང་ པར་རིས་འཐེན་མི་རེ་ལུ་ སླར་སྒྲིག་འབདཝ་ལས་ ཌེཤ་བོརཌི་ཚུ་གིས་ `1m` གི་ཚད་དང་ ཡང་ན་ མགྱོགས་དྲག་སྦེ་ དཔེ་ཚད་བཟོ་དགོ།

## Grafana ཌེཀསི་བོཌ།

ཌེཤ་བོརཌ་ཇེ་ཨེསི་ཨོ་ཨེན་གྱིས་ བཀོལ་སྤྱོད་པའི་ལཱ་ཚུ་ ཁྱབ་ཚུགས་པའི་ པེ་ནཱལ་བདུན་ཡོད་པའི་ གྲུ་ཚུ་ བཏངམ་ཨིན། འདྲི་དཔྱད་ཚུ་ ཁྱོད་ཀྱིས་ བེསི་པོཀ་ཐིག་ཁྲམ་ཚུ་བཟོ་བསྐྲུན་འབད་ནི་ལུ་དགའ་བ་ཅིན་ མགྱོགས་དྲགས་སྦེ་ གཞི་བསྟུན་འབད་ནིའི་དོན་ལུ་ འོག་ལུ་ཐོ་བཀོད་འབད་ཡོདཔ་ཨིན།

༡.
2. **ཨ་ལི་ཡས་ཐོ་གཞུང་འགྲོས་** – `torii_sorafs_registry_aliases_total`.
༣. **order གྱལ་འདི་ གནས་རིམ་** – `torii_sorafs_registry_orders_total` (`status` གིས་སྡེ་ཚན་བཟོ་ཡོདཔ།)།
༤. **Backlog vs དུས་ཚོད་རྫོགས་པའི་བཀའ་རྒྱ་** – ཁ་ཐོག་གི་ ཚད་གཞི་ལུ་ `torii_sorafs_replication_backlog_total` དང་ `torii_sorafs_registry_orders_total{status="expired"}` མཉམ་སྡེབ་འབདཝ་ཨིན།
5. **SLA མཐར་འཁྱོལ་གྱི་ཆ་སྙོམས་** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

༦. **དུས་ཚོད་དང་དུས་ཚོད་ཀྱི་ slack** – བཀབ་སྟེ་ `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` དང་ `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. ཁྱོད་ལུ་ དཔེར་ན་ ཆ་ཚང་སྦེ་ ལཱ་འབད་སའི་ ཐོག་ཁར་དགོ་པའི་སྐབས་ Grafana བསྒྱུར་བཅོས་ཚུ་ ལག་ལེན་འཐབ།

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **བཀའ་རྒྱ་ (1h ཚད་)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## ཉེན་བརྡ་- **SLA མཐར་འཁྱོལ་  ༠**
  - ཐེརེ་ཤོལཌི་: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - བྱ་བ་: བྱིན་མི་ཚུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ གཞུང་སྐྱོང་བརྟག་དཔྱད་འབདཝ་ཨིན།
- **མཇུག་བསྡུ་ p95 > དུས་ཚོད་ཀྱི་བཀག་ཆ་ avg**
  - ཐེརེ་ཤོལཌི་: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - བྱ་བ་: དུས་ཚོད་མ་འགྱོ་བའི་ཧེ་མ་ བཀྲམ་སྤེལ་འབད་མི་ཚུ་ བདེན་དཔྱད་འབད་དོ་ཡོདཔ་ཨིན་; བསྐྱར་འགན་ཚུ་བཏོན་ནི་ལུ་བརྩི་འཇོག་འབད།

### དཔེར་བརྗོད། Prometheus བཅའ་ཁྲིམས།

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## གཟེངས་བསྟོད་ལས་རིམ།

༡ **རྒྱུ་རྐྱེན་ངོས་འཛིན་འབད།**
   - ཨེསི་ཨེལ་ཨེ་གིས་ རྒྱབ་ལོག་འདི་ དམའ་ཤོས་སྦེ་ལུས་པའི་སྐབས་ བརླག་སྟོར་ཤོར་པ་ཅིན་ བྱིན་མི་ལཱ་ཤུགས་ལུ་གཙོ་བོར་བསྟེན་དོ་ཡོདཔ་ཨིན།
   - གལ་སྲིད་ རྒྱབ་ལོག་འདི་ བརྟན་ཏོག་ཏོ་སྦེ་ ཉམས་འགྱོ་བ་ཅིན་ ཚོགས་སྡེ་གི་ཆ་འཇོག་ལུ་ བསྒུག་སྡོད་པའི་ རྟགས་མཚན་ཚུ་ ངེས་གཏན་བཟོ་ནི་གི་དོན་ལུ་ འཛུལ་ཞུགས་ (`/v1/sorafs/pin/*`) ཞིབ་དཔྱད་འབད།
2. **བྱིན་མཁན་གྱི་གནས་རིམ་ནུས་ཅན་**།
   - `iroha app sorafs providers list` གཡོག་བཀོལ་ཞིནམ་ལས་ ཁྱབ་བསྒྲགས་འབད་ཡོད་པའི་ལྕོགས་གྲུབ་མཐུན་སྒྲིག་འདྲ་བཤུས་དགོས་མཁོ་ཚུ་ བདེན་དཔྱད་འབད།
   - `torii_sorafs_capacity_*` གི་འཇལ་ཚད་ཚུ་ བཀོད་སྒྲིག་འབད་ཡོད་པའི་ GiB དང་ POR གི་མཐར་འཁྱོལ་ཚུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ ཞིབ་དཔྱད་འབད།
3. **འདྲ་བཤུས་བསྐྱར་བཟོ་འབད།**
   - རྒྱབ་ལོག་བཀག་ཆ་ (`stat="avg"`) གིས་ དུས་སྐབས་ ༥ གི་འོག་ལུ་ མར་ཕབ་འགྱོ་བའི་སྐབས་ Grafana བརྒྱུད་དེ་ བཀའ་རྒྱ་གསརཔ་ཚུ་ བཏོན་ཡོདཔ་ཨིན།
   - གལ་སྲིད་ མིང་ཚིག་ཚུ་ལུ་ གསལ་སྟོན་གྱི་ གསལ་སྟོན་ཚུ་ ཤུགས་ཅན་མེད་པ་ཅིན་ གཞུང་སྐྱོང་ལུ་ བརྡ་དོན་སྤྲོད་དགོ། (`torii_sorafs_registry_aliases_total` རེ་བ་མེད་པར་ མར་ཕབ་)།
༤ **ཡིག་ཆ་གྲུབ་འབྲས།**
   - དུས་ཚོད་མཚོན་རྟགས་ཚུ་དང་གཅིག་ཁར་ SoraFS བཀོལ་སྤྱོད་དྲན་ཐོ་ནང་ བྱུང་རྐྱེན་དྲན་ཐོ་ཚུ་ ཐོ་བཀོད་དང་ གནོད་སྐྱོན་བྱུང་མི་ གསལ་སྟོན་ཚུ་ བཟུམ་འབདཝ་ཨིན།
   - འཐུས་ཤོར་ཐབས་ལམ་གསརཔ་ ཡང་ན་ ཌེཤ་བོརཌི་ཚུ་ འགོ་བཙུགས་པ་ཅིན་ རན་བུ་འདི་ དུས་མཐུན་བཟོ་དགོ།

## ལས་འཆར་གྱི་འཆར་གཞི།

བཟོ་བསྐྲུན་ནང་ མིང་གཞན་འདྲ་མཛོད་སྲིད་བྱུས་འདི་ ལྕོགས་ཅན་དང་ ཡང་ན་ བཀག་ཆ་འབད་བའི་སྐབས་ གོ་རིམ་ཅན་གྱི་བྱ་རིམ་འདི་ རྗེས་སུ་འཇུག་དགོ།1. **སྒྲིག་བཀོད་གྲ་སྒྲིག་**།
   - ཆ་འཇོག་གྲུབ་པའི་ TTLs དང་ བྱིན་རླབས་སྒོ་སྒྲིག་ཚུ་དང་གཅིག་ཁར་ `iroha_config` (user → five) ནང་ Prometheus དུས་མཐུན་བཟོ་ནི། `revocation_ttl`, `rotation_max_age`, `successor_grace`, དང་ `governance_grace`. སྔོན་སྒྲིག་ཚུ་གིས་ `docs/source/sorafs_alias_policy.md` ནང་ལུ་ སྲིད་བྱུས་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
   - ཨེསི་ཌི་ཀེ་ཨེསི་གི་དོན་ལུ་ གནས་གོང་ཚུ་ ཁོང་རའི་རིམ་སྒྲིག་བང་རིམ་ཚུ་བརྒྱུད་དེ་བཀྲམ་སྤེལ་འབད་ (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` Rust / NAPI / Python binds) དེ་འབདཝ་ལས་ མཁོ་སྤྲོད་འབད་མི་འདི་གིས་ འཛུལ་སྒོ་དང་མཐུན་སྒྲིག་འབདཝ་ཨིན།
༢ **འཁྲབ་སྟོན་ནང་སྐམ་བསྐྱོད་**།
   - བཟོ་བསྐྲུན་གྱི་ཊོ་པོ་ལོ་ཇི་ལུ་ མེ་ལོང་ནང་ རིམ་སྒྲིག་བསྒྱུར་བཅོས་འདི་ བཀོད་སྒྲིག་འབད།
   - Prometheus འདི་ ཀེན་ནོ་ནིག་ཨེ་ལི་ཡསི་སྒྲིག་ཆས་ཚུ་ ད་ལྟོ་ཡང་ ཌི་ཀོཌ་དང་ སྒོར་རིམ་འགྲུལ་སྐྱོད་ཚུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ གཡོག་བཀོལ། མཐུན་སྒྲིག་མེད་པའི་ ཡར་འཕེལ་གྱི་གསལ་སྟོན་འདི་ དང་པ་རང་ ཐག་གཅད་དགོཔ་ཨིན།
   - `/v1/sorafs/pin/{digest}` དང་ `/v1/sorafs/aliases` མཐའ་མཇུག་ཚུ་ བཅོས་མའི་བདེན་ཁུངས་ཚུ་ གསརཔ་དང་ གསརཔ་སྦེ་ སྒོ་སྒྲིག་ཚུ་ ཁྱབ་སྟེ་ དུས་ཚོད་རྫོགས་ཏེ་ དུས་ཚོད་རྫོགས་མི་ དེ་ལས་ དཀའ་ངལ་ཅན་གྱི་གནས་སྟངས་ཚུ་ ལག་ལེན་འཐབ་ཨིན། ཨེཆ་ཊི་ཊི་པི་གནས་རིམ་ཨང་རྟགས་ཚུ་ མགོ་ཡིག་ཚུ་ (`Sora-Proof-Status`, `Retry-After`, `Warning`) དང་ རན་བུཀ་འདི་དང་འགལ་བའི་ ཇེ་ཨེསི་ཨོན་གཟུགས་ཀྱི་ས་སྒོ་ཚུ་ བདེན་དཔྱད་འབད།
3. **ཐོན་སྐྱེད་ནང་ལྕོགས་ཅན་**།
   - ཚད་ལྡན་བསྒྱུར་བཅོས་སྒོ་སྒྲིག་བརྒྱུད་དེ་ རིམ་སྒྲིག་གསརཔ་འདི་ བཤུད་འབད། དེ་ དང་པ་ར་ Torii ལུ་འཇུག་སྤྱོད་འབད་ཞིནམ་ལས་ མཐུད་མཚམས་དེ་གིས་ དྲན་དེབ་ནང་ སྲིད་བྱུས་གསརཔ་འདི་ ངེས་དཔྱད་འབད་ཚརཝ་ད་ འཛུལ་སྒོ་/ཨེསི་ཌི་ཀེ་ཞབས་ཏོག་ཚུ་ ལོག་འགོ་བཙུགས།
   - `docs/source/grafana_sorafs_pin_registry.json` ནང་འདྲེན་ Grafana (ཡང་ན་ ད་ལྟོ་ཡོད་པའི་ ཌེཤ་བོརཌི་ཚུ་དུས་མཐུན་བཟོ་ནི་) ནང་འདྲེན་འབད་ཞིནམ་ལས་ ཨེན་ཨོ་སི་ལཱ་གི་ས་སྒོ་ལུ་ མིང་གཞན་འདྲ་མཛོད་གསརཔ་གི་པེ་ནཱལ་ཚུ་ འཕྲི་གཏང་།
༤ **བཀོད་རྒྱ་སྤྲོད་པའི་བདེན་དཔང་**།
   - སྐར་མ་ ༣༠ རིང་ `torii_sorafs_alias_cache_refresh_total` དང་ `torii_sorafs_alias_cache_age_seconds` བལྟ་རྟོག་འབད། `error`/`expired` ནང་ལུ་ སྤྱང་ཀི་ཚུ་གིས་ སྲིད་བྱུས་གསརཔ་གསརཔ་གི་སྒོ་སྒྲིག་ཚུ་དང་ འབྲེལ་བ་འཐབ་དགོཔ་ཨིན། རེ་བ་མེད་པའི་ཡར་རྒྱས་ཟེར་མི་འདི་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ འཕྲོ་མཐུད་མ་འབད་བའི་ཧེ་མ་ མིང་གཞན་བདེན་ཁུངས་དང་ གསོ་བའི་འཕྲོད་བསྟེན་ཚུ་ བརྟག་དཔྱད་འབད་དགོཔ་ཨིན།
   - མཁོ་སྤྲོད་པ་-ཕྱོགས་དྲན་ཐོ་ཚུ་ ངེས་དཔྱད་ཀྱིས་ སྲིད་བྱུས་གྲོས་ཐག་གཅིགཔོ་སྟོནམ་ཨིན། (བདེན་ཁུངས་འདི་ སྒྲིང་སྒྲི་ཡང་ན་ དུས་ཡུན་ཚང་བའི་སྐབས་ འཛོལ་བ་ཚུ་ ཁ་ཐོག་ལུ་ཐོན་འོང་)། མཁོ་སྤྲོད་པའི་ཉེན་བརྡ་མེད་མི་འདི་གིས་ རིམ་སྒྲིག་ལོག་སྤྱོད་འབད་བའི་བརྡ་སྟོནམ་ཨིན།
༥ **ཕོལ་བེག་**
   - གལ་སྲིད་ མིང་གཞན་སྤྲོད་ལེན་འདི་ རྒྱབ་ཁར་འགྱོ་སྟེ་ སྒོ་སྒྲིག་འགྲུལ་བསྐྱོད་འདི་ འཕྲལ་འཕྲལ་སྦེ་ར་ གསརཔ་བཟོ་ཞིནམ་ལས་ གནས་སྐབས་ཅིག་གི་དོན་ལུ་ Prometheus དང་ `positive_ttl` ཡར་སེང་འབད་དེ་ རིམ་སྒྲིག་འབད་དེ་ བསྐྱར་གསོ་འབདཝ་ཨིན། `hard_expiry` དེ་ ངོ་མ་སྦེ་ བདེན་ཁུངས་མེད་པའི་ བདེན་ཁུངས་ཚུ་ ད་ལྟོ་ཡང་ ངོས་ལེན་མ་འབད་བར་ བཞག་དགོ།
   - ཧེ་མའི་རིམ་སྒྲིག་ལུ་ ཕྱིར་ལོག་འབད་དེ་ ཧེ་མའི་ `iroha_config` པར་ལེན་འདི་ འཕྲོ་མཐུད་དེ་རང་ `error` གྱངས་ཁ་ཚུ་སྟོན་པ་ཅིན་ དེ་ལས་ མིང་གཞན་བཏོན་ནིའི་དོན་ལུ་ བྱུང་རྐྱེན་ཅིག་ཁ་ཕྱེ་དགོ།

## འབྲེལ་བའི་རྒྱུ་ཆ།

- `docs/source/sorafs/pin_registry_plan.md` — ལག་ལེན་འཐབ་སའི་ས་ཁྲ་དང་ གཞུང་སྐྱོང་སྐབས་དོན།
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — བསག་བཞག་གི་ལས་བཀོད། ཐོ་བཀོད་ཀྱི་རྩེད་དེབ་འདི་མཐུན་སྒྲིག་འབདཝ་ཨིན།