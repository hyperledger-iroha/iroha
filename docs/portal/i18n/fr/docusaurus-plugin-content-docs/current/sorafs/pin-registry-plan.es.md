---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : Plan de mise en œuvre du registre Pin de SoraFS
sidebar_label : Registre du Plan del Pin
description : Plan de mise en œuvre SF-4 qui comprend la machine des états du registre, la façade Torii, l'outillage et l'observabilité.
---

:::note Fuente canonica
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Manten ambas copias synchronisés mientras la documentacion heredada siga activa.
:::

# Plan de mise en œuvre du registre Pin de SoraFS (SF-4)

SF-4 entre le contrat du Pin Registry et les services de support qui stockent
compromis sur le manifeste, qui complètent la politique des broches et exposent les API à Torii, les passerelles
et les orchestres. Ce document étend le plan de validation avec les tâches de
mise en œuvre concrète, intégration de la logique en chaîne, des services de l'hôte, des
les installations et les éléments requis pour l'exploitation.

## Alcance

1. **Maquina de estados del Registry** : registros définis par Norito para manifests,
   pseudonymes, chaînes sucesoras, époques de rétention et métadonnées de gouvernance.
2. **Mise en œuvre du contrat** : opérations CRUD déterministes pour le cycle de vie
   de pins (`ReplicationOrder`, `Precommit`, `Completion`, expulsion).
3. **Fachada de servicio** : les points de terminaison gRPC/REST sont répondus au registre consommé
   Torii et les SDK, y compris la pagination et l'attestation.
4. **Outils et accessoires** : aides de CLI, vecteurs de test et documentation pour la maintenance
   manifestes, alias et enveloppes de gobernanza sincronizados.
5. **Télémétrie et opérations** : mesures, alertes et runbooks pour la santé du registre.

## Modèle de données

### Registres centraux (Norito)

| Structure | Description | Campos |
|------------|-------------|--------|
| `PinRecordV1` | Entrée canonique du manifeste. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​Mapea -> CID du manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction pour que les fournisseurs épinglent le manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Aperçu de la politique gouvernementale. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Référence de mise en œuvre : voir `crates/sorafs_manifest/src/pin_registry.rs` pour les
esquemas Norito en Rust et les aides de validation qui répondent à ces enregistrements. La
validation reflète l'outillage du manifeste (recherche du registre chunker, contrôle de la politique des broches)
pour que le contrat, les fournisseurs Torii et la CLI partagent des invariants identiques.Zones :
- Finaliser les schémas Norito et `crates/sorafs_manifest/src/pin_registry.rs`.
- Générer un code (Rust + autres SDK) à l'aide des macros Norito.
- Actualiser la documentation (`sorafs_architecture_rfc.md`) une fois que les images sont dans la liste.

## Implémentation du contrat

| Tarée | Responsable(s) | Notes |
|------|------|------|
| Implémenter l'hébergement du registre (sled/sqlite/off-chain) ou le module de contrat intelligent. | Équipe Core Infra / Smart Contract | Proveer hachage déterministe, éviter le point flottant. |
| Points d'entrée : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infrastructure de base | Aprovechar `ManifestValidator` du plan de validation. La liaison de l'alias maintenant se fait via `RegisterPinManifest` (DTO de Torii) tandis que `bind_alias` est dédiée à la planification des mises à jour successives. |
| Transitions de l'état : succession d'imposant (manifeste A -> B), époques de rétention, unicité d'alias. | Conseil de gouvernance / Core Infra | Unicité de pseudonyme, limites de rétention et vérifications d'approbation/retrait des prédécesseurs vivant en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-sauts et la comptabilité de réplication sont toujours ouvertes. |
| Paramètres réglés : charger `ManifestPolicyV1` depuis la configuration/état de fonctionnement ; permettre les actualisations via eventos de gobernanza. | Conseil de gouvernance | Proveer CLI pour l’actualisation de la politique. |
| Émission d'événements : émettre des événements Norito pour la télémétrie (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilité | Définir un schéma d'événements + journalisation. |

Pruebas :
- Tests unitarios para cada point d'entrée (positivo + rechazo).
- Tests de propiedades para la cadena de succession (sin ciclos, epocas monotonicamente crecientes).
- Fuzz de validacion generando manifeste des aleatorios (acotados).

## Façade de service (Intégration Torii/SDK)

| Composants | Tarée | Responsable(s) |
|---------------|-------|----------------|
| Service Torii | Exponer `/v2/sorafs/pin` (soumettre), `/v2/sorafs/pin/{cid}` (recherche), `/v2/sorafs/aliases` (liste/liaison), `/v2/sorafs/replication` (commandes/reçus). Afficher la page + filtré. | Mise en réseau TL / Core Infra |
| Attestation | Incluez la hauteur/le hachage du registre dans les réponses ; ajouter la structure de certification Norito utilisée pour les SDK. | Infrastructure de base |
| CLI | Extender `sorafs_manifest_stub` ou une nouvelle CLI `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Outillage |
| SDK | Générer des liaisons client (Rust/Go/TS) à partir du modèle Norito ; agréger les tests d'intégration. | Équipes SDK |

Opérations :
- Ajouter une capacité de cache/ETag pour les points de terminaison GET.
- Proveer rate limitation/auth cohérent avec la politique de Torii.

## Calendriers et CI- Directeur des appareils : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` garde les instantanés fermes du manifeste/alias/ordre régénérés par `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI : `ci/check_sorafs_fixtures.sh` régénérera l'instantané et supprimera les différences, en maintenant les appareils de CI alignés.
- Tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el flujo feliz mas el rechazo de alias duplicado, gardes d'approbation/rétention d'alias, poignées de chunker desalineados, validation de conteo de répliques et chutes de gardes de succession (points desconocidos/preaprobados/retirados/autorreferencias); voir les cas `register_manifest_rejects_*` pour les détails de la couverture.
- Tests unitarios ahora cubren validacion de alias, guards de retencion y checks de successeur en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-sauts lorsque la maquina de estados est aterrice.
- JSON Golden pour les événements utilisés par les pipelines d'observabilité.

## Télémétrie et observabilité

Métriques (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La télémétrie existante des fournisseurs (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) est disponible pour les tableaux de bord de bout en bout.

Journaux :
- Stream de eventos Norito structuré pour les auditoriums de gouvernement (firmados ?).

Alertes :
- Ordenes de réplication pendantes dépassant le SLA.
- Expiration de l'alias par debajo del umbral.
- Violaciones de retencion (manifeste non rénové avant l'expiration).

Tableaux de bord :
- Le JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea total du cycle de vie des manifestes, couverture d'alias, saturation du backlog, ratio de SLA, superpositions de latence vs slack et charges de commandes perdues pour la révision d'astreinte.

## Runbooks et documentation

- Actualiser `docs/source/sorafs/migration_ledger.md` pour inclure les actualisations de l'état du registre.
- Guide des opérateurs : `docs/source/sorafs/runbooks/pin_registry_ops.md` (vous avez publié) cubriendo metricas, alertas, despliegue, backup and flujos de recuperacion.
- Guide d'administration : décrit les paramètres politiques, le flux de travail d'approbation, la gestion des litiges.
- Pages de référence de l'API pour chaque point de terminaison (documents Docusaurus).

## Dépendances et sécurité

1. Compléter les zones du plan de validation (intégration de ManifestValidator).
2. Finaliser le schéma Norito + valeurs politiques par défaut.
3. Mettre en œuvre un contrat + service, connecter la télémétrie.
4. Régénérer les luminaires, corriger les suites d'intégration.
5. Actualiser les documents/runbooks et marquer les éléments de la feuille de route comme complets.

Chaque liste de contrôle du SF-4 doit être référencée à ce plan lorsqu'elle est marquée en cours.
La page REST contient maintenant les points finaux de la liste avec attestation :

- `GET /v2/sorafs/pin` et `GET /v2/sorafs/pin/{digest}` manifestes de développement avec
  liaisons d'alias, ordonnances de réplication et un objet d'attestation dérivé du
  hash du dernier bloc.
- `GET /v2/sorafs/aliases` et `GET /v2/sorafs/replication` exposent le catalogue de
  alias actif et le backlog des commandes de réplication avec une page cohérente et
  filtres d'état.La CLI envoie ces appels (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour que les opérateurs puissent automatiser les salles du
le registre ne couvre pas les API de bas niveau.