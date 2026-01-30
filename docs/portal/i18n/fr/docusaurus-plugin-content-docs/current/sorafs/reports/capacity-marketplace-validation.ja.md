---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e2c87bc1691e8e88ad28942e7b1d63a832aac8306dc58cceaa895f1db6efe528
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Checklist de validation du marche de capacite SoraFS

**Fenetre de revue:** 2026-03-18 -> 2026-03-24  
**Responsables du programme:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Portee:** Pipelines d'onboarding des providers, flux d'adjudication des litiges et processus de reconciliation du tresor requis pour la GA SF-2c.

La checklist ci-dessous doit etre revue avant d'activer le marche pour des operateurs externes. Chaque ligne renvoie vers une evidence deterministe (tests, fixtures ou documentation) que les auditeurs peuvent rejouer.

## Checklist d'acceptation

### Onboarding des providers

| Check | Validation | Evidence |
|-------|------------|----------|
| Le registry accepte les declarations canoniques de capacite | Le test d'integration exerce `/v1/sorafs/capacity/declare` via l'app API, en verifiant la gestion des signatures, la capture de metadata et le hand-off vers le registry du noeud. | `crates/iroha_torii/src/routing.rs:7654` |
| Le smart contract rejette les payloads incoherents | Le test unitaire garantit que les IDs de provider et les champs GiB engages correspondent a la declaration signee avant persistance. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Le CLI emet des artefacts d'onboarding canoniques | Le harness CLI ecrit des sorties Norito/JSON/Base64 deterministes et valide les round-trips pour que les operateurs puissent preparer les declarations offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Le guide operateur couvre le workflow d'admission et les garde-fous de gouvernance | La documentation enumere le schema de declaration, les defaults de policy et les etapes de revue pour le council. | `../storage-capacity-marketplace.md` |

### Resolution des litiges

| Check | Validation | Evidence |
|-------|------------|----------|
| Les enregistrements de litige persistent avec un digest canonique du payload | Le test unitaire enregistre un litige, decode le payload stocke et affirme le statut pending pour garantir le determinisme du ledger. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Le generateur de litiges du CLI correspond au schema canonique | Le test CLI couvre les sorties Base64/Norito et les resumes JSON pour `CapacityDisputeV1`, en assurant que les evidence bundles hashent de maniere deterministe. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Le test de replay prouve le determinisme litige/penalite | La telemetry de proof-failure rejouee deux fois produit des snapshots identiques de ledger, credit et litige afin que les slashes soient deterministes entre peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Le runbook documente le flux d'escalade et de revocation | Le guide d'operations capture le workflow du council, les exigences de preuve et les procedures de rollback. | `../dispute-revocation-runbook.md` |

### Reconciliation du tresor

| Check | Validation | Evidence |
|-------|------------|----------|
| L'accumulation du ledger correspond a la projection de soak sur 30 jours | Le test soak couvre cinq providers sur 30 fenetres de settlement, en comparant les entrees du ledger a la reference de payout attendue. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La reconciliation des exports de ledger est enregistree chaque nuit | `capacity_reconcile.py` compare les attentes du fee ledger aux exports XOR executes, emet des metriques Prometheus et gate l'approbation du tresor via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Les dashboards de billing exposent les penalites et la telemetry d'accumulation | L'import Grafana trace l'accumulation GiB-hour, les compteurs de strikes et le collateral engage pour la visibilite on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Le rapport publie archive la methodologie de soak et les commandes de replay | Le rapport detaille la portee du soak, les commandes d'execution et les hooks d'observabilite pour les auditeurs. | `./sf2c-capacity-soak.md` |

## Notes d'execution

Relancez la suite de validation avant le sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Les operateurs doivent regenerer les payloads de demande d'onboarding/litige avec `sorafs_manifest_stub capacity {declaration,dispute}` et archiver les bytes JSON/Norito resultants aux cotes du ticket de gouvernance.

## Artefacts de sign-off

| Artefact | Path | blake2b-256 |
|----------|------|-------------|
| Paquet d'approbation d'onboarding des providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquet d'approbation de resolution des litiges | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquet d'approbation de reconciliation du tresor | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Conservez les copies signees de ces artefacts avec le bundle de release et reliez-les dans le registre de changement de gouvernance.

## Approbations

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
