<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: fr
direction: ltr
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-11-21T14:07:47.531238+00:00"
translation_last_reviewed: 2026-01-01
---

# FAQ settlement Nexus

**Lien roadmap :** NX-14 - documentation Nexus et runbooks operateurs  
**Statut :** Brouillon 2026-03-24 (reflete les specs settlement router et playbook CBDC)  
**Audience :** Operateurs, auteurs SDK et reviewers de gouvernance preparant le lancement
Nexus (Iroha 3).

Cette FAQ repond aux questions remontees lors de la revue NX-14 sur le routage de settlement, la
conversion XOR, la telemetrie et les preuves d'audit. Consultez
`docs/source/settlement_router.md` pour la specification complete et
`docs/source/cbdc_lane_playbook.md` pour les boutons de policy propres au CBDC.

> **TL;DR:** Tous les flux de settlement passent par Settlement Router, qui
> debite les buffers XOR sur les lanes publiques et applique des frais par lane. Les operateurs
> doivent garder la config de routage (`config/config.toml`), les dashboards de telemetrie et
> les logs d'audit en sync avec les manifests publies.

## Questions frequentes

### Quelles lanes gerent le settlement, et comment savoir ou se place mon DS ?

- Chaque dataspace declare un `settlement_handle` dans son manifest. Les handles par defaut se
  repartissent ainsi :
  - `xor_global` pour les lanes publiques par defaut.
  - `xor_lane_weighted` pour les lanes publiques custom qui sourcent la liquidite ailleurs.
  - `xor_hosted_custody` pour les lanes privees/CBDC (buffer XOR en escrow).
  - `xor_dual_fund` pour les lanes hybrides/confidentielles qui mixent des flux shielded + publics.
- Consultez `docs/source/nexus_lanes.md` pour les classes de lane et
  `docs/source/project_tracker/nexus_config_deltas/*.md` pour les dernieres approvals de catalogue.
  `irohad --sora --config ... --trace-config` imprime le catalogue effectif en runtime pour les
  audits.

### Comment Settlement Router determine les taux de conversion ?

- Le router impose un chemin unique avec un pricing deterministe :
  - Pour les lanes publiques, on utilise le pool de liquidite XOR on-chain (DEX publique). Les
    oracles de prix retombent sur le TWAP approuve par la gouvernance quand la liquidite est faible.
  - Les lanes privees pre-financent des buffers XOR. Quand elles debitent un settlement, le router
    log la tuile de conversion `{lane_id, source_token, xor_amount, haircut}` et applique les
    haircuts approuves par la gouvernance (`haircut.rs`) si les buffers derivent.
- La configuration vit sous `[settlement]` dans `config/config.toml`. Evitez les edits custom sauf
  instruction de la gouvernance. Consultez `docs/source/settlement_router.md` pour la description
  des champs.

### Comment les frais et rebates sont appliques ?

- Les frais sont exprimes par lane dans le manifest :
  - `base_fee_bps` - s'applique a chaque debit de settlement.
  - `liquidity_haircut_bps` - compense les fournisseurs de liquidite partagee.
  - `rebate_policy` - optionnel (ex. rebates promotionnels CBDC).
- Le router emet des evenements `SettlementApplied` (format Norito) avec le detail des frais pour
  que les SDKs et les auditeurs puissent reconciler les ecritures du ledger.

### Quelle telemetrie prouve que les settlements sont sains ?

- Metriques Prometheus (exportees via `iroha_telemetry` et settlement router) :
  - `nexus_settlement_latency_seconds{lane_id}` - P99 doit rester sous 900 ms pour les lanes
    publiques / 1200 ms pour les lanes privees.
  - `settlement_router_conversion_total{source_token}` - confirme les volumes de conversion par
    token.
  - `settlement_router_haircut_total{lane_id}` - alerter si non-zero sans note de gouvernance.
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` - montre les debits XOR en live par lane
    (micro unites). Alerter quand <25 %/10 % de Bmin.
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` - variance de haircut realisee pour
    reconciliation avec le P&L de tresorerie.
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` - epsilon effectif applique dans le dernier
    bloc; les auditeurs comparent avec la policy du router.
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` - usage de la ligne de
    credit sponsor/MM; alerter au-dessus de 80 %.
- `nexus_lane_block_height{lane,dataspace}` - derniere hauteur de bloc observee pour le pair
  lane/dataspace; gardez les peers voisins a quelques slots.
- `nexus_lane_finality_lag_slots{lane,dataspace}` - slots entre le head global et le bloc le plus
  recent pour cette lane; alerter quand >12 hors drills.
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` - backlog en attente de settlement en XOR;
  gatez les charges CBDC/privees avant de depasser les seuils reglementaires.

`settlement_router_conversion_total` porte les labels `lane_id`, `dataspace_id` et `source_token`
pour prouver quel actif de gas a pilote chaque conversion. `settlement_router_haircut_total`
accumule des unites XOR (pas des micro montants bruts), ce qui permet a la Tresorerie de reconciler
le ledger haircut directement depuis Prometheus.
- `lane_settlement_commitments[*].swap_metadata.volatility_class` montre si le router a applique le
  bucket de marge `stable`, `elevated` ou `dislocated`. Les entrees elevated/dislocated doivent
  lier le log d'incident ou la note de gouvernance.
- Dashboards : `dashboards/grafana/nexus_settlement.json` plus l'overview
  `nexus_lanes.json`. Liez les alertes a `dashboards/alerts/nexus_audit_rules.yml`.
- Quand la telemetrie de settlement degrade, loggez l'incident selon le runbook dans
  `docs/source/nexus_operations.md`.

### Comment exporter la telemetrie de lane pour les regulateurs ?

Lancez le helper ci-dessous quand un regulateur demande la table des lanes :

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` accepte le blob JSON renvoye par `iroha status --format json`.
* `--json-out` capture un array JSON canonique par lane (aliases, dataspace, hauteur de bloc,
  finality lag, capacite/utilisation TEU, compteurs de scheduler trigger + utilisation, RBC
  throughput, backlog, metadonnees de gouvernance, etc.).
* `--parquet-out` ecrit le meme payload en Parquet (schema Arrow), pret pour les regulateurs qui
  exigent une preuve columnar.
* `--markdown-out` emet un resume lisible qui signale les lanes en retard, backlog non-zero,
  evidence de compliance manquante et manifests en attente; par defaut
  `artifacts/nexus_lane_audit.md`.
* `--lane-compliance` est optionnel; si fourni, il doit pointer vers le manifest JSON decrit dans le
  doc compliance afin que les lignes exportees embarquent la policy de lane correspondante, les
  signatures des reviewers, le snapshot de metriques et les extraits d'audit log.

Archivez les deux outputs sous `artifacts/` avec la preuve routed-trace (captures de
`nexus_lanes.json`, etat Alertmanager, et `nexus_lane_rules.yml`).

### Quelles preuves attendent les auditeurs ?

1. **Snapshot de config** - capturez `config/config.toml` avec la section `[settlement]` et le
   catalogue de lanes reference par le manifest courant.
2. **Logs du router** - archivez `settlement_router.log` chaque jour; il contient des IDs de
   settlement haches, des debits XOR et les haircuts appliques.
3. **Exports telemetrie** - snapshot hebdo des metriques mentionnees ci-dessus.
4. **Rapport de reconciliation** - optionnel mais recommande : exportez les entrees
   `SettlementRecordV1` (voir `docs/source/cbdc_lane_playbook.md`) et comparez avec le ledger de
   tresorerie.

### Les SDKs ont-ils besoin d'un traitement special pour le settlement ?

- Les SDKs doivent :
  - Fournir des helpers pour interroger les evenements de settlement (`/v1/settlement/records`) et
    interpreter les logs `SettlementApplied`.
  - Exposer les IDs de lane + settlement handles dans la configuration client pour que les
    operateurs routent correctement les transactions.
  - Miroiter les payloads Norito definis dans `docs/source/settlement_router.md` (ex.
    `SettlementInstructionV1`) avec des tests end-to-end.
- Le quickstart du SDK Nexus (section suivante) detaille les snippets par langage pour l'onboarding
  au reseau public.

### Comment les settlements interagissent-ils avec la gouvernance ou les freins d'urgence ?

- La gouvernance peut mettre en pause des handles de settlement specifiques via des updates de
  manifest. Le router respecte le flag `paused` et rejette les nouveaux settlements avec une erreur
  deterministe (`ERR_SETTLEMENT_PAUSED`).
- Les "haircut clamps" d'urgence limitent les debits XOR maximum par bloc pour eviter de drainer les
  buffers partages.
- Les operateurs doivent monitorer `governance.settlement_pause_total` et suivre le template
  d'incident dans `docs/source/nexus_operations.md`.

### Ou signaler des bugs ou demander des changements ?

- Gaps de fonctionnalite -> ouvrez une issue taggee `NX-14` et reliez a la roadmap.
- Incidents settlement urgents -> pagez le Nexus primary (voir `docs/source/nexus_operations.md`) et
  joignez les logs du router.
- Corrections de documentation -> ouvrez des PRs contre ce fichier et ses equivalents portail
  (`docs/portal/docs/nexus/overview.md`, `docs/portal/docs/nexus/operations.md`).

### Pouvez-vous montrer des exemples de flux de settlement ?

Les snippets suivants montrent ce que les auditeurs attendent pour les types de lane les plus
courants. Capturez le log du router, les hashes de ledger et l'export telemetrie correspondant pour
chaque scenario afin que les reviewers puissent rejouer la preuve.

#### Lane CBDC privee (`xor_hosted_custody`)

Ci-dessous un log du router tronque pour une lane CBDC privee utilisant le handle hosted custody.
Le log prouve des debits XOR deterministes, la composition des frais et les IDs de telemetrie :

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

Dans Prometheus vous devriez voir les metriques correspondantes :

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

Archivez le log, le hash de transaction ledger et l'export de metriques ensemble afin que les
 auditeurs puissent reconstruire le flux. Les exemples suivants montrent comment enregistrer la
preuve pour les lanes publiques et hybrides/confidentielles.

#### Lane publique (`xor_global`)

Les data spaces publics routent via `xor_global`, donc le router debite le buffer DEX partage et
registre le TWAP live qui a price le transfert. Attachez le hash TWAP ou la note de gouvernance
quand l'oracle retombe sur une valeur cachee.

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

Les metriques prouvent le meme flux :

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

Sauvegardez l'enregistrement TWAP, le log router, le snapshot telemetrie et le hash ledger dans le
meme bundle de preuve. Quand les alertes se declenchent pour la latence de la lane 0 ou la
fraicheur TWAP, reliez le ticket d'incident a ce bundle.

#### Lane hybride/confidentielle (`xor_dual_fund`)

Les lanes hybrides melangent des buffers shielded avec des reserves XOR publiques. Chaque
settlement doit montrer quel bucket a source le XOR et comment la policy haircut a split les frais.
Le log du router expose ces details via le bloc metadata dual-fund :

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

Archivez le log du router avec la policy dual-fund (extrait du catalogue de gouvernance), l'export
`SettlementRecordV1` pour la lane et le snippet telemetrie afin que les auditeurs confirment que le
split shielded/public respecte les limites de gouvernance.

Gardez cette FAQ a jour quand le comportement du settlement router change ou quand la gouvernance
introduit de nouvelles classes de lane/policies de frais.
