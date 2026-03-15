---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Validation du marché de capacité SoraFS
balises : [SF-2c, acceptation, liste de contrôle]
résumé : Liste de contrôle de l'acceptation pour l'intégration des fournisseurs, les flux de litiges et la conciliation des finances qui permettent la disponibilité générale du marché de capacité SoraFS.
---

# Liste de vérification de validation du marché de capacité SoraFS

**Vente de révision :** 2026-03-18 -> 2026-03-24  
**Responsables du programme :** Équipe de stockage (`@storage-wg`), Conseil de gouvernance (`@council`), Guilde du Trésor (`@treasury`)  
**Chance :** Pipelines d'intégration des fournisseurs, flux de règlement des litiges et processus de conciliation des résultats requis pour l'AG SF-2c.

La liste de contrôle suivante doit être révisée avant d'autoriser le marché pour les opérateurs externes. Cada fila enlaza evidencia deterministica (tests, montages ou documentation) que les auditeurs peuvent reproduire.

## Checklist d'acceptation

### Intégration des fournisseurs

| Chèque | Validation | Preuve |
|-------|------------|--------------|
| Le registre accepte les déclarations canoniques de capacité | Le test d'intégration est exécuté `/v2/sorafs/capacity/declare` via l'API de l'application, en vérifiant le fonctionnement des entreprises, la capture des métadonnées et le transfert vers le registre du nœud. | `crates/iroha_torii/src/routing.rs:7654` |
| Le contrat intelligent rechaza charges utiles desalineados | Le test unitaire garantit que les identifiants du fournisseur et les champs de compromis GiB coïncident avec la déclaration ferme avant de persister. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| La CLI émet des artefacts canoniques d’intégration | Le harnais de CLI décrit les étapes Norito/JSON/Base64 déterministes et valides les allers-retours pour que les opérateurs puissent préparer des déclarations hors ligne. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Le guide des opérateurs capture le flux d'admission et les garde-corps de gouvernement | La documentation énumère l'esquema de déclaration, les défauts de politique et les étapes de révision pour le conseil. | `../storage-capacity-marketplace.md` |

### Résolution des litiges

| Chèque | Validation | Preuve |
|-------|------------|--------------|
| Les registres des litiges persistent avec le résumé canonique de la charge utile | Le test unitaire enregistre un litige, décodifie la charge utile enregistrée et confirme l'état pendant pour garantir le déterminisme du grand livre. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Le générateur de litiges de la CLI coïncide avec l’esquema canonique | Le test de la CLI cubré en Base64/Norito et reprend JSON pour `CapacityDisputeV1`, garantissant que les bundles de preuves hashean de forme déterministe. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Le test de relecture vérifie le déterminisme du litige/pénalisation | La télémétrie des échecs de preuve reproduite deux fois produit des instantanés identiques du grand livre, des crédits et des litiges pour que les barres obliques soient déterministes entre pairs. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Le runbook documente le flux d'escalade et de révocation | Le guide d'opérations capture les flux du conseil, les exigences en matière de preuves et les procédures de restauration. | `../dispute-revocation-runbook.md` |### Conciliación de tesoreria

| Chèque | Validation | Preuve |
|-------|------------|--------------|
| L'accumulation du grand livre coïncide avec la projection de trempage de 30 jours | Le test d'immersion s'effectue auprès de cinq fournisseurs sur 30 ventanas de règlement, en comparant les entrées du grand livre avec la référence de paiement attendue. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La conciliation des exportations du grand livre est enregistrée chaque nuit | `capacity_reconcile.py` compare les attentes du grand livre des frais avec les exportations XOR effectuées, émet des mesures Prometheus et déclenche l'approbation du trésor via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Les tableaux de bord de facturation exposent les pénalités et la télémétrie de cumul | El import de Grafana grafica acumulacion GiB-hour, contadores de strikes y collatéraux cautionnés pour la visibilité sur appel. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Le rapport publié archive la méthodologie du trempage et les commandes de relecture | Le rapport détaille l'altitude du trempage, les commandes d'éjection et les crochets d'observabilité pour les auditeurs. | `./sf2c-capacity-soak.md` |

## Notes d'éjection

Rejeter la suite de validation avant la signature :

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Les opérateurs doivent régénérer les charges utiles de la demande d'intégration/de litige avec `sorafs_manifest_stub capacity {declaration,dispute}` et archiver les octets JSON/Norito résultants en même temps que le ticket d'administration.

## Artefacts d'approbation

| Artefact | Itinéraire | blake2b-256 |
|--------------|------|-------------|
| Paquet d'approbation d'intégration des fournisseurs | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquet d'approbation de résolution de litiges | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquet d'approbation de conciliation de trésorerie | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Gardez les copies confirmées de ces artefacts avec le bundle de release et inlazalas dans le registre des changements de gouvernement.

## Approbations

- Responsable de l'équipe de stockage — @storage-tl (2026-03-24)  
- Secrétariat du Conseil de gouvernance — @council-sec (2026-03-24)  
- Directeur des Opérations de Tesoreria — @treasury-ops (2026-03-24)