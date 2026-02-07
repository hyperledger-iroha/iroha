---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : Plan de mise en œuvre du registre des broches par SoraFS
sidebar_label : Registre Plano do Pin
description : Plan de mise en œuvre SF-4 cobrindo a maquina de estados do Registry, a fachada Torii, outillage et observabilité.
---

:::note Fonte canonica
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas quanto a documentacao herdada permanecer ativa.
:::

# Plan de mise en œuvre du registre Pin SoraFS (SF-4)

Le SF-4 entre le contrat du registre Pin et les services d'activation
compromissos de manifeste, impoem politicas de pin et expoem APIs para Torii,
passerelles et orchestres. Ce document est étendu ou plan de validation avec
tâches de mise en œuvre concrètes, cobrindo a logica on-chain, les services du
hôte, les installations et les exigences opérationnelles.

## Escopo

1. **Maquina de estados do Registry** : registros définisdos por Norito para manifests,
   pseudonymes, cadeias successeurs, époques de rétention et métadonnées de gouvernance.
2. **Mise en œuvre du contrat** : opérations déterministes CRUD pour le cycle de vie
   dos pins (`ReplicationOrder`, `Precommit`, `Completion`, expulsion).
3. **Fachada de servico**: endpoints gRPC/REST soutenus par le registre Torii
   Il existe un ensemble de SDK, y compris la page et le certificat.
4. **Outils et accessoires** : aides de CLI, anciens tests et documentation pour le reste
   manifestes, alias et enveloppes de gouvernance synchronisées.
5. **Télémétrie et opérations** : mesures, alertes et runbooks pour le registre légitime.

## Modèle de données

### Registres centraux (Norito)

| Structure | Description | Campos |
|--------|-----------|--------|
| `PinRecordV1` | Entrée canonique du manifeste. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​Mapeia -> CID du manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instructions pour les fournisseurs de réparation du manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Confirmation du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Aperçu de la politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Référence de mise en œuvre : voir `crates/sorafs_manifest/src/pin_registry.rs` pour l'utilisateur
esquemas Norito em Rust et les aides de validation qui soutiennent ces registres. Un
validation de l'outillage du manifeste (recherche du registre chunker, contrôle de la politique des broches)
pour le contrat, comme les modèles Torii et la CLI répartissent les invariants identiques.

Tarifs :
- Finaliser les schémas Norito et `crates/sorafs_manifest/src/pin_registry.rs`.
- Créer un code (Rust + autres SDK) en utilisant les macros Norito.
- Actualiser la documentation (`sorafs_architecture_rfc.md`) lorsque les images sont terminées rapidement.## Implémentation du contrat

| Taréfa | Responsavel(est) | Notes |
|--------|-----------------|-------|
| Implémenter l'armement du registre (sled/sqlite/off-chain) ou du module de contrat intelligent. | Équipe Core Infra / Smart Contract | Fornecer hachage déterministe, éviter les fluctuations. |
| Points d'entrée : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infrastructure de base | Approuver `ManifestValidator` sur le plan de validation. La liaison de l'alias il y a peu via `RegisterPinManifest` (DTO vers Torii) quant à `bind_alias` est dédiée à la transition vers les actualisations successives. |
| Transicoes de estado: impor successao (manifeste A -> B), époques de rétention, unicité d'alias. | Conseil de gouvernance / Core Infra | Unicité d'alias, limites de rétention et chèques d'approbation/retraite des prédécesseurs vivem em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection du succès multi-sauts et la comptabilité de réplication permanente à l'extérieur. |
| Paramètres gérés : charger `ManifestPolicyV1` de config/estado de gouvernance ; permettre des mises à jour via des événements de gouvernance. | Conseil de gouvernance | Fornecer CLI para actualizacoes de politica. |
| Émission d'événements : émettre des événements Norito pour la télémétrie (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilité | Définir un schéma d'événements + journalisation. |

Testicules :
- Testes unitarios para cada point d'entrée (positivo + rejeicao).
- Testes de propriedades para acadeia de successao (sem cycles, epocas monotonicamente crescentes).
- Fuzz de validacao gerando manifeste des aleatorios (limitados).

## Façade de service (Integraçao Torii/SDK)

| Composants | Taréfa | Responsavel(est) |
|------------|--------|-----------------|
| Servico Torii | Expor `/v1/sorafs/pin` (soumettre), `/v1/sorafs/pin/{cid}` (recherche), `/v1/sorafs/aliases` (liste/liaison), `/v1/sorafs/replication` (commandes/reçus). Fornecer paginacao + filtragem. | Mise en réseau TL / Core Infra |
| Atestacao | Inclut la hauteur/le hachage du registre dans les réponses ; ajouter une structure de certification Norito utilisée pour les SDK. | Infrastructure de base |
| CLI | Estender `sorafs_manifest_stub` ou une nouvelle CLI `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Outillage |
| SDK | Créer des liaisons client (Rust/Go/TS) à partir du schéma Norito ; ajouter des testicules d'intégration. | Équipes SDK |

Opéras :
- Ajout d'une caméra de cache/ETag pour les points de terminaison GET.
- Fornecer rate limiting / auth cohérentes com as politicas do Torii.

## Calendriers et CI- Répertoire des appareils : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assassinés de manifeste/alias/ordre régénérés par `cargo run -p iroha_core --example gen_pin_snapshot`.
- Étape de CI : `ci/check_sorafs_fixtures.sh` régénérera l'instantané et falha se maintiendra en diffs, en gardant les appareils de CI alignés.
- Testes d'intégration (`crates/iroha_core/tests/pin_registry.rs`) exercent le flux agréable mais avec le rejet d'alias dupliqué, gardes d'approbation/retenue d'alias, poignées de chunker incompatibles, validation de contagem de répliques et falhas de guardas de successao (ponteiros desconhecidos/preaprovados/retirados/autorreferencias); Voir les cas `register_manifest_rejects_*` pour les détails de la couverture.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks of successeur em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; une détection de succès multi-saut quando a maquina de estados chegar.
- JSON Golden pour les événements utilisés par les pipelines d'observation.

## Télémétrie et observation

Métriques (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Une télémétrie existante des fournisseurs (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) est permanente sous surveillance pour les tableaux de bord de bout en bout.

Journaux :
- Stream de eventos Norito structuré pour les auditoires de gouvernance (assinados ?).

Alertes :
- Ordres de réplication pendants dépassant le SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (manifeste nao renovado antes de expirar).

Tableaux de bord :
- Le JSON du Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totalise le cycle de vie des manifestes, la couverture des alias, la saturation du backlog, le calcul du SLA, les superpositions de latence par rapport au slack et les taxes de commandes perdues pour la révision sur appel.

## Runbooks et documentation

- Actualiser `docs/source/sorafs/migration_ledger.md` pour inclure l'actualisation de l'état du registre.
- Guide de l'opérateur : `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publié) cobrindo métriques, alertes, déploiement, sauvegarde et flux de récupération.
- Guide de gouvernance : découvrir les paramètres politiques, le flux de travail d'approbation, le traitement des litiges.
- Pages de référence de l'API pour chaque point de terminaison (docs Docusaurus).

## Dépendances et séquencement

1. Compléter les tarifications du plan de validation (intégration du ManifestValidator).
2. Finaliser le schéma Norito + valeurs politiques par défaut.
3. Mettre en œuvre un contrat + service, connecter la télémétrie.
4. Régénérer les luminaires, les suites rodar de integracao.
5. Actualiser les documents/runbooks et marquer les éléments de la feuille de route comme des éléments complets.

Chaque liste de contrôle du SF-4 doit être référencée à ce plan lorsque vous continuez.
La page REST vient d'entrer les points de terminaison de la liste avec attestation :

- `GET /v1/sorafs/pin` et `GET /v1/sorafs/pin/{digest}` retornam manifestes com
  liaisons d'alias, ordres de réplication et un objet d'attestation dérivé du
  hash fait le dernier bloc.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` exposition ou catalogue de
  Alias ativo et le backlog des commandes de réplication avec une page cohérente et
  filtres de statut.Une CLI encapsule ces chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour que les opérateurs puissent automatiser les auditoires
registre avec les API de bas niveau.