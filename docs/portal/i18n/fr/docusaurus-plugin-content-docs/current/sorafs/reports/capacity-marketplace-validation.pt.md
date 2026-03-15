---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Validacao do mercado de capacidade SoraFS
balises : [SF-2c, acceptation, liste de contrôle]
résumé : Liste de contrôle de l'acitacao cobrindo onboarding des fournisseurs, flux de litiges et réconciliation de la responsabilité de libérer la disponibilité générale du marché de capacité SoraFS.
---

# Checklist de validation du marché de capacité SoraFS

**Janela de révision:** 2026-03-18 -> 2026-03-24  
**Réponse du programme :** Équipe de stockage (`@storage-wg`), Conseil de gouvernance (`@council`), Guilde du Trésor (`@treasury`)  
**Escopo :** Pipelines d'intégration des fournisseurs, flux de règlement des litiges et processus de réconciliation des comptes requis pour l'AG SF-2c.

La liste de contrôle doit être révisée avant d'autoriser le marché des opérateurs externes. Cada linha connecta les preuves déterministes (tests, montages ou documentation) que les auditeurs peuvent reproduire.

## Checklist de l'huile

### Intégration des fournisseurs

| Chéagem | Validation | Preuve |
|-------|------------|--------------|
| O Registry Aceita Declaracoes Canonicas de Capacidade | Le test d'intégration exercé `/v1/sorafs/capacity/declare` via l'API de l'application, vérifie le traitement des héritages, la capture des métadonnées et le transfert pour le registre du nœud. | `crates/iroha_torii/src/routing.rs:7654` |
| Le contrat intelligent nécessite des charges utiles divergentes | Le test unitaire garantit que les identifiants du fournisseur et les champs GiB compromis correspondent à une déclaration d'assassinat avant de persister. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| La CLI émet des articles canoniques d'intégration | L'exploitation de la CLI écrit comme Norito/JSON/Base64 déterministes et valides les allers-retours pour que les opérateurs puissent préparer des déclarations hors ligne. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Le guide de l'opérateur sur le flux de travail d'admission et les garde-fous de gouvernance | Un document énumérant le schéma de déclaration, les défauts de politique et les étapes de révision pour le conseil. | `../storage-capacity-marketplace.md` |

### Résolution des litiges

| Chéagem | Validation | Preuve |
|-------|------------|--------------|
| Les registres des litiges persistent avec le résumé canonique de la charge utile | Le test unitaire enregistre un litige, décodifie la charge utile armée et confirme le statut en attente pour garantir le déterminisme du grand livre. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Le gestionnaire des litiges de la CLI correspond au schéma canonique | Le test de la CLI utilisant Base64/Norito et les résumés JSON pour `CapacityDisputeV1` garantit que les bundles de preuves ont un hachage déterminé. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Le test de rediffusion prouve le déterminisme du litige/pénalité | Une télémétrie de preuve d'échec reproduit deux fois des instantanés identiques du grand livre, des crédits et des litiges pour que les barres obliques soient déterministes entre pairs. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| La documentation du runbook ou le flux d'escalade et de révision | Le guide des opérations capture le flux de travail du conseil, les exigences en matière de preuves et les procédures de restauration. | `../dispute-revocation-runbook.md` |

### Réconciliation du tesouro| Chéagem | Validation | Preuve |
|-------|------------|--------------|
| L'accumulation du grand livre correspond à un projet de trempage de 30 jours | Le test de trempage a changé cinq fournisseurs en 30 janvier de règlement, en comparant les entrées du grand livre avec une référence de paiement attendue. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Rapprochement des exportations du grand livre et enregistrées à chaque nuit | `capacity_reconcile.py` compare les attentes du grand livre des frais avec les exportations XOR exécutées, émet des mesures Prometheus et donne la porte d'approbation du testeur via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Tableaux de bord de facturation, exposition des pénalités et télémétrie d'accumulation | L'importation du tracé Grafana pour l'accumulation de GiB-heure, les contadores de grèves et les garanties cautionnées pour la visibilité sur appel. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Le rapport publié archive la méthodologie de trempage et les commandes de relecture | Le rapport détaillé sur l'escopo do trempage, les commandes d'exécution et les crochets d'observation pour les auditeurs. | `./sf2c-capacity-soak.md` |

## Notes d'exécution

Réexécutez une suite de validation avant la signature :

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Les opérateurs développent des charges utiles régénératrices de sollicitation d'intégration/de litige avec `sorafs_manifest_stub capacity {declaration,dispute}` et archivent les octets JSON/Norito résultants en même temps que le ticket de gouvernance.

## Artefatos de aprovacao

| Artefato | Chemin | blake2b-256 |
|--------------|------|-------------|
| Pacte d'approbation d'intégration des fournisseurs | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Accord d'approbation pour la résolution des litiges | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacte d'approbation de réconciliation du tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Gardez les copies assassinées de ces artefatos avec le bundle de release e vincule-as no registro de mudancas de gouvernance.

## Approbations

- Chef d'équipe de stockage - @storage-tl (2026-03-24)  
- Secrétaire du Conseil de gouvernance - @council-sec (2026-03-24)  
- Responsable des opérations de trésorerie - @treasury-ops (2026-03-24)