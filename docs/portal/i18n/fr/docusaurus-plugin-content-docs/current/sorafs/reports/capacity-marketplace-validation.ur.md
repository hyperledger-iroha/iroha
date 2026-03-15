---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Validation du marché de capacité SoraFS
résumé : Liste de contrôle d'acceptation, intégration du fournisseur, flux de travail des litiges, rapprochement de la trésorerie, et SoraFS, marché de capacité, GA et porte d'entrée.
balises : [SF-2c, acceptation, liste de contrôle]
---

# SoraFS Liste de contrôle de validation du marché de capacité

**Période de révision :** 2026-03-18 -> 2026-03-24  
**Propriétaires du programme :** Équipe de stockage (`@storage-wg`), Conseil de gouvernance (`@council`), Guilde du Trésor (`@treasury`)  
**Portée :** Pipelines d'intégration des fournisseurs, flux de règlement des litiges, processus de rapprochement de trésorerie et SF-2c GA pour les paiements en ligne

Il y a une liste de contrôle pour les opérateurs de marché et l'activation du marché et l'examen des avis sur les achats en ligne. ہر lignes de preuves déterministes (tests, montages, etc. documentation) et les auditeurs replay کر سکتے ہیں۔

## Liste de contrôle d'acceptation

### Intégration des fournisseurs

| Vérifier | Validation | Preuve |
|-------|------------|--------------|
| Déclarations canoniques de capacité du Registre قبول کرتا ہے | API de l'application de test d'intégration pour `/v2/sorafs/capacity/declare` pour la gestion des signatures, la capture des métadonnées, le registre des nœuds, le transfert et la vérification | `crates/iroha_torii/src/routing.rs:7654` |
| Les charges utiles incompatibles avec les contrats intelligents sont rejetées | Test unitaire pour les ID de fournisseur et les champs GiB validés, déclaration signée et pour la persistance | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Les artefacts d'intégration canoniques CLI émettent کرتا ہے | CLI exploite les sorties déterministes Norito/JSON/Base64 et les allers-retours valident les déclarations hors ligne des opérateurs et les déclarations hors ligne des opérateurs. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Flux de travail d'admission du guide de l'opérateur et garde-corps de gouvernance et couverture | Schéma de déclaration de documentation, valeurs par défaut de la politique, étapes d'examen du conseil et énumération des éléments | `../storage-capacity-marketplace.md` |

### Résolution des litiges

| Vérifier | Validation | Preuve |
|-------|------------|--------------|
| Les dossiers de litiges résument canoniquement la charge utile et persistent ہوتے ہیں | Registre des litiges de tests unitaires pour décoder la charge utile stockée et statut en attente d'affirmation pour le déterminisme du grand livre | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Le schéma canonique du générateur de litige CLI correspond à la description | Test CLI `CapacityDisputeV1` et sorties Base64/Norito et résumés JSON couvrant les différents faisceaux de preuves de hachage déterministe. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay test litige/déterminisme des pénalités کو ثابت کرتا ہے | Télémétrie de preuve d'échec pour la relecture du grand livre, du crédit et des litiges instantanés, pour les coupures des pairs et pour les instantanés déterministes | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Escalade du runbook et flux de révocation et document | Guide des opérations flux de travail du conseil, exigences en matière de preuves, procédures de restauration et capture des informations | `../dispute-revocation-runbook.md` |

### Rapprochement de la trésorerie| Vérifier | Validation | Preuve |
|-------|------------|--------------|
| Comptabilité du grand livre Projection de trempage sur 30 jours et match کرتا ہے | Test d'immersion pour les fournisseurs avec 30 fenêtres de règlement pour les écritures du grand livre et la référence de paiement attendue pour les différences de paiement | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Rapprochement des exportations du grand livre ہر رات record ہوتا ہے | Les attentes du grand livre des frais `capacity_reconcile.py` et les exportations de transferts XOR exécutées sont comparées aux métriques Prometheus émises par Alertmanager et la porte d'approbation du Trésor. ہے۔ | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Tableaux de bord de facturation pénalités et surface de télémétrie d'accumulation | Grafana importer l'accumulation de GiB-heures, les compteurs d'exercices et le tracé de garanties cautionnées ainsi que la visibilité sur appel | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Méthodologie de trempage du rapport publié et archives des commandes de relecture ici | Portée de l'absorption du rapport, commandes d'exécution, crochets d'observabilité, auditeurs, détails et détails | `./sf2c-capacity-soak.md` |

## Notes d'exécution

Signature de la suite de validation de la suite de validation :

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Les opérateurs avec `sorafs_manifest_stub capacity {declaration,dispute}` chargent les charges utiles des demandes d'intégration/de contestation et génèrent des octets et des tickets de gouvernance JSON/Norito. ساتھ archives کرنا چاہیے۔

## Artefacts de signature

| Artefact | Chemin | blake2b-256 |
|--------------|------|-------------|
| Dossier d'approbation d'intégration du fournisseur | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Dossier d'approbation pour la résolution des litiges | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Dossier d'approbation du rapprochement du Trésor | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Il s'agit d'artefacts, de copies signées et d'un lot de versions, d'un dossier de changement de gouvernance et d'un enregistrement de changement de gouvernance.

## Approbations

- Chef d'équipe de stockage — @storage-tl (2026-03-24)  
- Secrétaire du Conseil de gouvernance — @council-sec (2026-03-24)  
- Responsable des opérations de trésorerie — @treasury-ops (2026-03-24)