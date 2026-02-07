---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : Plan de réalisation du registre des broches SoraFS
sidebar_label : Planifier le registre des épingles
description : Plan de réalisation du SF-4, du registre de la machine, du modèle Torii, de l'outillage et du démarrage.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/pin_registry_plan.md`. Veuillez sélectionner les copies synchronisées lorsque la documentation est active.
:::

# Plan de réalisation du registre des broches SoraFS (SF-4)

SF-4 fournit un registre de broches de contrat et fournit des services pour votre entreprise
en créant le manifeste, en introduisant la broche politique et en prévoyant l'API pour Torii,
шлюзов и оркестраторов. Ce document précise le plan de validation des bétons
réalisations, logistique en chaîne, services de stockage d'hébergement,
luminaires et fonctionnement.

## Область

1. ** Registre des fichiers ** : indique Norito pour les manifestes, les alias,
   Il y a des préparatifs, des périodes et des mises en œuvre métadonnées.
2. **Réalisation du contrat** : détermination des opérations CRUD pour le travail
   цикла pin (`ReplicationOrder`, `Precommit`, `Completion`, expulsion).
3. **Point de terminaison** : points de terminaison gRPC/REST, utilisation du registre et
   J'utilise Torii et le SDK, ainsi que la page de configuration et l'attestation.
4. **Outils et accessoires** : aides CLI, vecteurs de test et documentation pour
   Synchronisation des manifestes, des alias et des enveloppes de gouvernance.
5. **Mesures et opérations** : mesures, alertes et runbooks pour le registre de sauvegarde.

## Modèle donné

### Téléchargements rapides (Norito)

| Structure | Description | Pol |
|--------------|----------|------|
| `PinRecordV1` | Каноническая запись manifeste. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Сопоставляет alias -> Manifeste CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Les instructions pour les fournisseurs закрепить manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политики управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Ссылка на реализацию: см. `crates/sorafs_manifest/src/pin_registry.rs` pour le système Norito
Sur Rust et les aides fournies, ce qui se passe dans votre propre entreprise. Validation
outils de manifeste (registre de recherche de chunker, contrôle de la politique des broches), etc.
Le contrat, la page Torii et la CLI ont identifié les investisseurs.

Réponses :
- Remplacez les schémas Norito par `crates/sorafs_manifest/src/pin_registry.rs`.
- Générer le code (Rust + SDK) avec la macro Norito.
- Обновить документацию (`sorafs_architecture_rfc.md`) après le projet.

## Réalisation du contrat| Задача | Ответственные | Première |
|--------|---------------|---------------|
| Réalisez la gestion du registre (sled/sqlite/off-chain) ou un module de contrat intelligent. | Équipe Core Infra / Smart Contract | Pour utiliser le hachage déterminé, utilisez la virgule flottante. |
| Points d'entrée : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infrastructure de base | Utilisez `ManifestValidator` lors de la validation du plan. La liaison alias vous propose un produit provenant de `RegisterPinManifest` (DTO Torii), qui est également disponible dans l'avion pour `bind_alias`. последующих обновлений. |
| Переходы состояния : обеспечивать преемственность (manifeste A -> B), эпохи хранения, уникальность alias. | Conseil de gouvernance / Core Infra | Le nom d'alias unique, les limites et les preuves de l'arrivée/du produit précédent sont dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; Il y a de nombreuses opérations anticipées et de nombreuses répliques qui sont ouvertes. |
| Paramètres d'installation : ajoutez `ManifestPolicyV1` à la configuration/installation ; разрешить обновления через события управления. | Conseil de gouvernance | Préparer la CLI pour la politique de développement. |
| Émetteur-récepteur : sélectionnez l'émetteur-récepteur Norito pour les téléphones (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilité | Определить схему событий + journalisation. |

Test :
- Юнит-testы для каждой point d'entrée (позитивные + отказные сценарии).
- Propriété-tests для цепочки преемственности (sans циклов, монотонные эпохи).
- Fuzz valide la génération des manifestes (avec ограничениями).

## Façade de service (Torii/SDK)

| Composant | Задача | Ответственные |
|-----------|--------|---------------|
| Service Torii | Utilisez `/v1/sorafs/pin` (soumettre), `/v1/sorafs/pin/{cid}` (recherche), `/v1/sorafs/aliases` (liste/liaison), `/v1/sorafs/replication` (commandes/reçus). Обеспечить пагинацию + фильтрацию. | Mise en réseau TL / Core Infra |
| Attestation | Включать высоту/хэш registre в ответы; créez l'attestation de structure Norito en utilisant le SDK. | Infrastructure de base |
| CLI | Connectez `sorafs_manifest_stub` ou la nouvelle CLI `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Outillage |
| SDK | Créer des liaisons client (Rust/Go/TS) avec les schémas Norito ; faire des tests d'intégration. | Équipes SDK |

Fonctionnement :
- Ajouter le cache/ETag pour GET les points de terminaison.
- Précédez la limitation de débit/l'authentification dans les réponses aux politiques Torii.

## Calendriers et CI- Luminaires du catalogue : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` fournit des instantanés manifestes/alias/commande, en utilisant `cargo run -p iroha_core --example gen_pin_snapshot`.
- Sur CI : `ci/check_sorafs_fixtures.sh` permet de prendre un instantané et de passer par diff, en supprimant la synchronisation CI des appareils.
- Les tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) révèlent que Happy Path est également ouvert lors de la duplication d'alias, les gardes одобрения/хранения alias, les poignées manquantes du chunker, проверку числа реплик и отказы gardes преемственности (неизвестные/предодобренные/выведенные/самоссылки); см. Clé `register_manifest_rejects_*` pour l'achat de détails.
- Юнит-тесты теперь покрывают валидацию alias, guards хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; Il est possible de réaliser de nombreux travaux préliminaires, alors que la machine est installée.
- Golden JSON pour les utilisateurs, utilisé dans les sites Web.

## Télémétrie et activation

Mesures (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Le fournisseur de télévision fournisseur (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) est installé dans les ports de bout en bout.

Logiques :
- La structure du pot est Norito pour l'exploitation des auditeurs (en fonction ?).

Alertes :
- Vérifiez les réplications dans les délais de livraison, en prévoyant le SLA.
- Истечение срока alias ниже порога.
- Нарушения хранения (manifeste не продлен до истечения).

Dachbords :
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` complète les totaux des différents manifestes, l'alias de téléchargement, le backlog, le ratio SLA, les superpositions de latence par rapport au slack et les offres supplémentaires. заказов для sur appel ревью.

## Runbooks et documentation

- Ouvrez `docs/source/sorafs/migration_ledger.md` pour ouvrir le registre d'état d'avancement.
- Utilisateur de l'opérateur : `docs/source/sorafs/runbooks/pin_registry_ops.md` (disponible) avec mesures, alertes, sauvegardes, sauvegardes et sauvegardes.
- Fonctions de mise en œuvre : définir les paramètres politiques, l'évolution du flux de travail et les activités de travail.
- Description de l'API pour le point de terminaison de chaque client (documentation Docusaurus).

## Avis et suivi

1. Vérifiez le plan de validation (installation de ManifestValidator).
2. Finalisez le schéma Norito + paramètres politiques par défaut.
3. Réalisez le contrat + le service, puis connectez le téléphone.
4. Перегенерировать luminaires, запустить интеграционные suite.
5. Ouvrez les documents/runbooks et récupérez la feuille de route en cas de besoin.

Le SF-4 est désormais en mesure d'étudier ce plan en fonction du processus de programmation.
La phase REST permet d'afficher les points de terminaison d'attestation en détail :

- `GET /v1/sorafs/pin` et `GET /v1/sorafs/pin/{digest}` возвращают manifestes
  liaisons d'alias, réplications de commandes et attestations d'objets, livraisons
  хэша последнего блока.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` activés publiquement
  Alias de catalogue et backlog de réplication avec une configuration cohérente et
  statut des filtres.

CLI s'occupe de cela (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), les opérateurs peuvent automatiser les audits du registre
sans utiliser l'API.