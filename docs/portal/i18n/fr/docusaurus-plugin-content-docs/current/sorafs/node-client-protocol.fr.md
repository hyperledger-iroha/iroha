---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocole noeud ↔ client SoraFS

Ce guide résume la définition canonique du protocole dans
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilisez la spécification en amont pour les layouts Norito au niveau octet et
les changelogs ; la copie du portail garde les points opérationnels près du
reste des runbooks SoraFS.

## Annonces fournisseur et validation

Les fournisseurs SoraFS diffusent des charges utiles `ProviderAdvertV1` (voir
`crates/sorafs_manifest::provider_advert`) signés par l'opérateur gouverné. Les
adverts fixent les métadonnées de découverte et les garde-fous que l'orchestrateur
applique à l'exécution multi-sources.- **Durée de validité** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Les
  Les fournisseurs doivent rafraîchir toutes les 12 heures.
- **TLV de capacités** — la liste TLV annonce les fonctionnalités de transport
  (Torii, QUIC+Noise, relais SoraNet, fournisseur d'extensions). Les codes inconnus
  peuvent être ignorés lorsque `allow_unknown_capabilities = true`, en suivant
  les recommandations GRAISSE.
- **Indices QoS** — niveau de `availability` (Hot/Warm/Cold), latence maximale de
  récupération, limite de concurrence et budget de flux optionnel. La QoS doit
  s'aligner sur la télémétrie enregistrée et est auditée lors de l'admission.
- **Endpoints et sujets de rendez-vous** — URLs de service concrets avec
  métadonnées TLS/ALPN, plus les sujets de découverte concernant les clients
  doivent s'abonner lors de la construction des postes de garde.
- **Politique de diversité de chemin** — `min_guard_weight`, caps de fan-out
  AS/pool et `provider_failure_threshold` rendent possibles les récupérations
  déterministes multi-pairs.
- **Identifiants de profil** — les fournisseurs doivent exposer le handle
  canonique (ex. `sorafs.sf1@1.0.0`) ; des `profile_aliases` optionnels disponibles
  les anciens clients à migrer.Les règles de validation rejettent l'enjeu zéro, listes vides de capacités,
points finaux ou sujets, durées mal ordonnées, ou objectifs QoS manquants. Les
enveloppes d'admission comparant les corps d'annonce et de proposition
(`compare_core_fields`) avant de diffuseur des mises à jour.

### Extensions de récupération par plages

Les fournisseurs avec capacité range incluent les métadonnées suivantes :

| Champion | Objectif |
|-------|--------------|
| `CapabilityType::ChunkRangeFetch` | Déclarez `max_chunk_span`, `min_granularity` et les drapeaux d'alignement/preuve. |
| `StreamBudgetV1` | Enveloppe optionnelle de concurrence/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` optionnel). Nécessite une plage de capacité. |
| `TransportHintV1` | Préférences de transports ordonnées (ex. `torii_http_range`, `quic_stream`, `soranet_relay`). Les priorités vont de `0–15` et les doublons sont rejetés. |

Outillage de support :

- Les pipelines d'annonce fournisseur doivent valider capacité range, stream
  budget et transport tips avant d'émettre des charges utiles déterministes pour les
  vérifications.
- `cargo xtask sorafs-admission-fixtures` regroupement des annonces multi-sources
  canoniques avec des luminaires de downgrade dans
  `fixtures/sorafs_manifest/provider_admission/`.
- Les annonces de la gamme qui omettent `stream_budget` ou `transport_hints` sont
  rejetés par les chargeurs CLI/SDK avant planification, alignant le harnais
  multi-source sur les attentes d'admission Torii.

## Plage de points de terminaison de la passerelleLes passerelles acceptent des requêtes HTTP déterministes qui renvoient les
métadonnées des publicités.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Exigence | Détails |
|----------|---------|
| **En-têtes** | `Range` (fenêtre unique alignée sur les offsets de chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` optionnel, et `X-SoraFS-Stream-Token` base64 obligatoire. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range` décrivant la fenêtre servie, métadonnées `X-Sora-Chunk-Range`, et headers chunker/token renvoyés. |
| **Modes d'échec** | `416` pour plages mal alignées, `401` pour tokens manquants/invalides, `429` lorsque les budgets stream/octets sont dépassés. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch d'un seul chunk avec les mêmes headers, plus le digest déterministe du
morceau. Utile pour les tentatives ou les téléchargements médico-légaux quand les
les tranches CAR sont inutiles.

## Workflow de l'orchestrateur multi-source

Quand le fetch multi-source SF-6 est activé (CLI Rust via `sorafs_fetch`,
SDK via `sorafs_orchestrator`) :1. **Collecter les entrées** — décoder le plan de morceaux du manifeste, récupérer
   les dernières publicités, et optionnellement passer un instantané de télémétrie
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construire un tableau de bord** — `Orchestrator::build_scoreboard` significative
   l'éligibilité et enregistrer les raisons de rejet ; `sorafs_fetch --scoreboard-out`
   persister le JSON.
3. **Planifier les chunks** — `fetch_with_scoreboard` (ou `--plan`) impose des fichiers
   Plage de contraintes, flux de budgets, plafonds de nouvelle tentative/peer (`--retry-budget`,
   `--max-peers`) et émettre un stream token scope sur le manifeste pour chaque
   requête.
4. **Vérifier les reçues** — les sorties incluent `chunk_receipts` et
   `provider_reports` ; les CV CLI persistants `provider_reports`,
   `chunk_receipts` et `ineligible_providers` pour les bundles de preuves.

Erreurs courantes remontées aux opérateurs/SDKs :

| Erreur | Descriptif |
|--------|-------------|
| `no providers were supplied` | Aucune entrée éligible après filtrage. |
| `no compatible providers available for chunk {index}` | Erreur de plage ou de budget pour un morceau spécifique. |
| `retry budget exhausted after {attempts}` | Augmentez `--retry-budget` ou évincez les pairs en échec. |
| `no healthy providers remaining` | Tous les fournisseurs sont désactivés après des échecs répétés. |
| `streaming observer failed` | L'écrivain CAR en aval a avorté. |
| `orchestrator invariant violated` | Capturez le manifeste, le tableau de bord, l'instantané de télémétrie et JSON CLI pour le triage. |

## Télémétrie et preuves- Métriques émises par l'orchestrateur :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (taggées par manifeste/région/fournisseur). Définissez `telemetry_region` fr
  config ou via flags CLI pour partitionner les tableaux de bord par flotte.
- Les résumés de fetch CLI/SDK incluent le scoreboard JSON persisté, les reçus
  de chunks et les rapports fournisseurs qui doivent voyager dans les bundles
  de rollout pour les portes SF-6/SF-7.
- Les handlers gateway exposant `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  pour que les tableaux de bord SRE corrèlent les décisions orchestrateur avec le
  comportement serveur.

## Aides CLI et REST

- `iroha app sorafs pin list|show`, `alias list` et `replication list` emballent les fichiers
  endpoints REST du pin-registry et impression du Norito JSON brut avec blocs
  d'attestation pour l'audit.
- `iroha app sorafs storage pin` et `torii /v1/sorafs/pin/register` acceptent les
  manifestes Norito ou JSON, plus des preuves d'alias optionnels et des successeurs ;
  des preuves mal formées renvoient `400`, des preuves obsolètes exposent `503` avec
  `Warning: 110`, et des épreuves expirées renvoient `412`.
- Les points de terminaison REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) inclut des structures d'attestation pour que les
  les clients vérifient les données avec les derniers en-têtes de bloc avant d'agir.

##Référence- Spécification canonique :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Type Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aide CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orchestrateur de caisse : `crates/sorafs_orchestrator`
- Pack tableaux de bord : `dashboards/grafana/sorafs_fetch_observability.json`