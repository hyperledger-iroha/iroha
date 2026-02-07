---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Validation du marché des marchandises SoraFS
balises : [SF-2c, acceptation, liste de contrôle]
Sommaire : Liste des fournisseurs, fournisseurs de services d'importation, fournisseurs de produits et services de caisse, qui sont en contact avec les acheteurs du marché. SoraFS à GA.
---

# Liste de validation des marchandises du marché SoraFS

**Prouvés :** 2026-03-18 -> 2026-03-24  
**Programmes suivants :** Équipe de stockage (`@storage-wg`), Conseil de gouvernance (`@council`), Guilde du Trésor (`@treasury`)  
**Область:** Les fournisseurs de services embarqués, les fournisseurs de services de transfert et les processus de mise en œuvre des services, non disponibles pour le GA SF-2c.

Il n'y a qu'un seul client qui a fait ses preuves auprès des opérateurs du marché. Chaque travail effectué sur les documents de détermination (tests, montages ou documentation), les auditeurs peuvent vous fournir des informations.

## Liste des commandes

### Fournisseurs d'embarquement

| Proverbe | Validation | Documentation |
|-------|------------|--------------|
| Registre des déclarations canoniques | Le test d'intégration utilise `/v1/sorafs/capacity/declare` avec l'API de l'application, en vérifiant l'état des lieux, en mettant les métadonnées et avant dans les registres. | `crates/iroha_torii/src/routing.rs:7654` |
| Le contrat intelligent отклоняет несовпадающие les charges utiles | Unit-test garantit que les fournisseurs d'identifiants et les GiB engagés sont fournis par une déclaration préalable à l'achat. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI выпускает канонические artefacts онбординга | Le harnais CLI permet de déterminer les allers-retours Norito/JSON/Base64, les opérateurs pouvant obtenir des déclarations hors ligne. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| L'opérateur local examine le flux de travail et la mise en place des garde-corps | La documentation concerne les déclarations de régime, la politique par défaut et les rapports du conseil. | `../storage-capacity-marketplace.md` |

### Разрешение споров

| Proverbe | Validation | Documentation |
|-------|------------|--------------|
| Les informations relatives à la charge utile du résumé canonique | Un test unitaire enregistre les événements, décode la charge utile réelle et met à jour le statut en attente pour la détermination des garanties du grand livre. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Générateur de liens CLI pour le schéma canonique | Le test CLI fournit Base64/Norito et les fichiers JSON pour `CapacityDisputeV1`, garantissant la détermination des ensembles de preuves de hachage. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Replay-test доказывает детерминизм споров/пенализаций | Preuve d'échec de la télémétrie, vérification des données, identification du grand livre des instantanés, du crédit et des litiges, des barres obliques pour déterminer les pairs. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook documente les mises à jour et les révocations | Le fonctionnement du conseil de flux de travail, le travail de documentation et la procédure de restauration. | `../dispute-revocation-runbook.md` |

### Сверка казначейства| Proverbe | Validation | Documentation |
|-------|------------|--------------|
| Grand livre d'accumulation совпадает с 30-дневной проекцией trempage | Trempez le test pour les fournisseurs de règlement sur 30 heures, en prenant en compte le grand livre avec votre référence de référence. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Le grand livre des exportations s'ouvre chaque jour | `capacity_reconcile.py` gère le grand livre des frais lors de l'exportation XOR, en publiant les mesures Prometheus et la porte d'entrée d'Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Tableaux de bord de facturation pour les pénalités et l'accumulation de télémétrie | Importez Grafana pour l'accumulation de GiB-heure, les grèves et les garanties cautionnées pour les vidéos de garde. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Les archives disponibles sur la méthode de trempage et les commandes replay | Nous proposons également un trempage, des commandes de déverrouillage et des crochets d'observabilité pour les auditeurs. | `./sf2c-capacity-soak.md` |

## Примечания по выполнению

Перезапустите набор проверок перед signature :

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Les opérateurs doivent générer des charges utiles pour les embarquements/activités à partir de `sorafs_manifest_stub capacity {declaration,dispute}` et archiver les octets JSON/Norito. вместе с ticket de gouvernance.

## Signature des articles

| Artefact | Chemin | blake2b-256 |
|--------------|------|-------------|
| Fournisseurs de paquets de transport à bord | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquet de détails sur les événements | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquet d'articles à prix abordables | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Achetez des copies de ces objets dans le bundle de versions et achetez les éléments nécessaires à la mise en œuvre de la restauration.

## Подписи

- Chef d'équipe de stockage — @storage-tl (2026-03-24)  
- Secrétaire du Conseil de gouvernance — @council-sec (2026-03-24)  
- Responsable des opérations de trésorerie — @treasury-ops (2026-03-24)