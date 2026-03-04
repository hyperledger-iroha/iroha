---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Validation du marche de capacité SoraFS
balises : [SF-2c, acceptation, liste de contrôle]
Sommaire : Checklist d'acceptation couvrant l'onboarding des prestataires, les flux de litiges et la réconciliation du trésor conditionnant la disponibilité générale du marché de capacité SoraFS.
---

# Checklist de validation du marche de capacité SoraFS

**Fenêtre de revue:** 2026-03-18 -> 2026-03-24  
**Responsables du programme :** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Portée :** Pipelines d'onboarding des fournisseurs, flux d'arbitrage des litiges et processus de réconciliation du trésor requis pour la GA SF-2c.

La liste de contrôle ci-dessous doit être revue avant d'activer le marché pour des opérateurs externes. Chaque ligne renvoie vers une preuve déterministe (tests, montages ou documentation) que les auditeurs peuvent rejouer.

## Checklist d'acceptation

### Onboarding des prestataires

| Vérifier | Validation | Preuve |
|-------|------------|--------------|
| Le registre accepte les déclarations canoniques de capacité | Le test d'intégration exerce `/v1/sorafs/capacity/declare` via l'app API, en vérifiant la gestion des signatures, la capture de métadonnées et le hand-off vers le registre du nœud. | `crates/iroha_torii/src/routing.rs:7654` |
| Le smart contract rejette les payloads incohérents | Le test unitaire garantit que les ID du fournisseur et les champs GiB s'engagent correspondant à la déclaration signée avant persistance. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Le CLI emet des artefacts d'onboarding canoniques | Le harnais CLI écrit des sorties Norito/JSON/Base64 déterministes et valide les allers-retours pour que les opérateurs puissent préparer les déclarations hors ligne. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Le guide opérateur couvre le workflow d'admission et les garde-fous de gouvernance | La documentation énumère le schéma de déclaration, les défauts de politique et les étapes de revue pour le conseil. | `../storage-capacity-marketplace.md` |

### Résolution des litiges

| Vérifier | Validation | Preuve |
|-------|------------|--------------|
| Les enregistrements de litige persistant avec un résumé canonique du payload | Le test unitaire enregistre un litige, décode le stock de charge utile et affirme le statut en attente pour garantir le déterminisme du grand livre. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Le générateur de litiges du CLI correspond au schéma canonique | Le test CLI couvre les sorties Base64/Norito et les reprends JSON pour `CapacityDisputeV1`, en garantissant que les bundles de preuves hashent de manière déterministe. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Le test de replay prouve le déterminisme litige/pénalité | La télémétrie de preuve-échec rejouée deux fois produit des instantanés identiques de grand livre, de crédit et de litige afin que les barres obliques soient déterministes entre pairs. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Le runbook documente le flux d'escalade et de révocation | Le guide d'opérations capture le workflow du conseil, les exigences de preuve et les procédures de rollback. | `../dispute-revocation-runbook.md` |

### Réconciliation du trésor| Vérifier | Validation | Preuve |
|-------|------------|--------------|
| L'accumulation du grand livre correspond à la projection de trempage sur 30 jours | Le test couvre cinq fournisseurs sur 30 fenêtres de règlement, en comparant les entrées du grand livre à la référence de paiement attendue. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La réconciliation des exportations de grand livre est enregistrée chaque nuit | `capacity_reconcile.py` compare les attentes du registre des frais aux exportations XOR exécute, met les métriques Prometheus et porte l'approbation du trésor via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Les tableaux de bord de facturation exposent les pénalités et la télémétrie d'accumulation | L'import Grafana trace l'accumulation GiB-heure, les compteurs de grèves et le collatéral engagé pour la visibilité d'astreinte. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Le rapport public archive la méthodologie de trempage et les commandes de replay | Le rapport détaille la porte du trempage, les commandes d'exécution et les crochets d'observabilité pour les auditeurs. | `./sf2c-capacity-soak.md` |

## Notes d'exécution

Relancez la suite de validation avant le sign-off :

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Les opérateurs doivent régénérer les charges utiles de demande d'onboarding/litige avec `sorafs_manifest_stub capacity {declaration,dispute}` et archiver les octets JSON/Norito résultants aux cotes du ticket de gouvernance.

## Artefacts de signature

| Artefact | Chemin | blake2b-256 |
|--------------|------|-------------|
| Paquet d'approbation d'onboarding des fournisseurs | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquet d'approbation de résolution des litiges | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquet d'approbation de réconciliation du trésor | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Conservez les copies signées de ces artefacts avec le bundle de release et reliez-les dans le registre de changement de gouvernance.

## Approbations

- Chef d'équipe de stockage — @storage-tl (2026-03-24)  
- Secrétaire du Conseil de gouvernance — @council-sec (2026-03-24)  
- Responsable des opérations de trésorerie — @treasury-ops (2026-03-24)