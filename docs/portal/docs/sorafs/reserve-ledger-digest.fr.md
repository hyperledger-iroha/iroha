<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 72f24d0456a52deb36ed5801fbd0c2c0a97c0daee4419be8d4a49546d7cf758c
source_last_modified: "2025-11-20T08:27:52.686493+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Digest du registre de reserve et tableaux de bord
description: Comment transformer la sortie de `sorafs reserve ledger` en telemetrie, tableaux de bord et alertes pour la politique Reserve+Rent.
---

La politique Reserve+Rent (element de roadmap **SFM-6**) livre maintenant les helpers CLI `sorafs reserve`
et le traducteur `scripts/telemetry/reserve_ledger_digest.py` afin que les executions de tresorerie emettent
des transferts deterministes de loyer/reserve. Cette page reprend le workflow defini dans
`docs/source/sorafs_reserve_rent_plan.md` et explique comment cabler le nouveau flux de transferts dans
Grafana + Alertmanager afin que les relecteurs economiques et de gouvernance puissent auditer chaque cycle
de facturation.

## Flux de bout en bout

1. **Devis + projection du registre**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account ih58... \
     --treasury-account ih58... \
     --reserve-account ih58... \
     --asset-definition xor#sora \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Le helper du registre attache un bloc `ledger_projection` (loyer du, manque de reserve,
   delta de top-up, booleens d'underwriting) ainsi que les ISIs Norito `Transfer` necessaires
   pour deplacer du XOR entre les comptes de tresorerie et de reserve.

2. **Generer le digest + sorties Prometheus/NDJSON**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Le helper de digest normalise les totaux micro-XOR en XOR, enregistre si la projection
   respecte l'underwriting et emet les metriques du flux de transferts
   `sorafs_reserve_ledger_transfer_xor` et `sorafs_reserve_ledger_instruction_total`.
   Lorsque plusieurs ledgers doivent etre traites (par exemple, un lot de fournisseurs),
   repetez les paires `--ledger`/`--label` et le helper ecrit un unique fichier NDJSON/Prometheus
   contenant chaque digest afin que les dashboards ingerent tout le cycle sans glue sur mesure.
   Le fichier `--out-prom` cible un textfile collector node-exporter - deposez le fichier `.prom`
   dans le repertoire surveille par l'exporter ou chargez-le dans le bucket de telemetrie
   consomme par le job du dashboard Reserve - tandis que `--ndjson-out` alimente les memes payloads
   dans les pipelines de donnees.

3. **Publier les artefacts + preuves**
   - Stockez les digests dans `artifacts/sorafs_reserve/ledger/<provider>/` et liez le resume
     Markdown depuis votre rapport economique hebdomadaire.
   - Attachez le digest JSON au burn-down de loyer (pour que les auditeurs puissent rejouer
     les calculs) et incluez le checksum dans le paquet de preuves de gouvernance.
   - Si le digest signale un top-up ou une rupture d'underwriting, referencez les IDs d'alerte
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) et notez
     quelles ISIs de transfert ont ete appliquees.

## Metriques → dashboards → alertes

| Metrique source | Panneau Grafana | Alerte / crochet de politique | Notes |
|----------------|-----------------|-------------------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” dans `dashboards/grafana/sorafs_capacity_health.json` | Alimente le digest hebdomadaire de tresorerie; les pics de flux de reserve se propagent vers `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (meme dashboard) | A associer au digest du registre pour prouver que le stockage facture correspond aux transferts XOR. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + cartes de statut dans `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` se declenche quand `requires_top_up=1`; `SoraFSReserveLedgerUnderwritingBreach` se declenche quand `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown” et les cartes de couverture dans `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` et `SoraFSReserveLedgerTopUpTransferMissing` avertissent lorsque le flux de transferts est absent ou nul alors que le loyer/top-up est requis; les cartes de couverture tombent a 0% dans les memes cas. |

Quand un cycle de loyer se termine, rafraichissez les snapshots Prometheus/NDJSON, confirmez que
les panneaux Grafana recuperent le nouveau `label`, et joignez des captures + IDs Alertmanager au
paquet de gouvernance du loyer. Cela prouve que la projection CLI, la telemetrie et les artefacts
de gouvernance proviennent du **meme** flux de transferts et maintient l'alignement des dashboards
economiques du roadmap avec l'automatisation Reserve+Rent. Les cartes de couverture doivent afficher
100% (ou 1.0) et les nouvelles alertes doivent se lever une fois que les transferts de loyer et de
remise de reserve sont presents dans le digest.
