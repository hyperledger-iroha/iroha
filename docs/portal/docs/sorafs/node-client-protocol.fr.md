<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03cf6fe78b8e0c8a68a15d2d7bdccb78b80a2f443ba9361f19ee8a114daab735
source_last_modified: "2025-11-21T19:51:16.332517+00:00"
translation_last_reviewed: 2025-12-29
---

# Protocole nœud ↔ client SoraFS

Ce guide résume la définition canonique du protocole dans
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilisez la spécification upstream pour les layouts Norito au niveau octet et
les changelogs ; la copie du portail garde les points opérationnels près du
reste des runbooks SoraFS.

## Adverts fournisseur et validation

Les fournisseurs SoraFS diffusent des payloads `ProviderAdvertV1` (voir
`crates/sorafs_manifest::provider_advert`) signés par l'opérateur gouverné. Les
adverts fixent les métadonnées de découverte et les garde-fous que l'orchestrateur
multi-source applique à l'exécution.

- **Durée de validité** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Les
  fournisseurs doivent rafraîchir toutes les 12 heures.
- **TLV de capacités** — la liste TLV annonce les fonctionnalités transport
  (Torii, QUIC+Noise, relais SoraNet, extensions vendor). Les codes inconnus
  peuvent être ignorés lorsque `allow_unknown_capabilities = true`, en suivant
  les recommandations GREASE.
- **Indices QoS** — tier de `availability` (Hot/Warm/Cold), latence maximale de
  récupération, limite de concurrence et budget de stream optionnel. La QoS doit
  s'aligner sur la télémétrie observée et est auditée lors de l'admission.
- **Endpoints et topics de rendezvous** — URLs de service concrètes avec
  métadonnées TLS/ALPN, plus les topics de découverte auxquels les clients
  doivent s'abonner lors de la construction des guard sets.
- **Politique de diversité de chemin** — `min_guard_weight`, caps de fan-out
  AS/pool et `provider_failure_threshold` rendent possibles les fetches
  déterministes multi-peer.
- **Identifiants de profil** — les fournisseurs doivent exposer le handle
  canonique (ex. `sorafs.sf1@1.0.0`) ; des `profile_aliases` optionnels aident
  les anciens clients à migrer.

Les règles de validation rejettent stake zéro, listes vides de capabilities,
endpoints ou topics, durées mal ordonnées, ou objectifs QoS manquants. Les
enveloppes d'admission comparent les corps d'advert et de proposition
(`compare_core_fields`) avant de diffuser des mises à jour.

### Extensions de fetch par plages

Les fournisseurs avec capacité range incluent les métadonnées suivantes :

| Champ | Objectif |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Déclare `max_chunk_span`, `min_granularity` et les flags d'alignement/preuve. |
| `StreamBudgetV1` | Envelope optionnelle de concurrence/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` optionnel). Requiert une capacité range. |
| `TransportHintV1` | Préférences de transport ordonnées (ex. `torii_http_range`, `quic_stream`, `soranet_relay`). Les priorités vont de `0–15` et les doublons sont rejetés. |

Support tooling :

- Les pipelines d'advert fournisseur doivent valider capacité range, stream
  budget et transport hints avant d'émettre des payloads déterministes pour les
  audits.
- `cargo xtask sorafs-admission-fixtures` regroupe des adverts multi-source
  canoniques avec des fixtures de downgrade dans
  `fixtures/sorafs_manifest/provider_admission/`.
- Les adverts range qui omettent `stream_budget` ou `transport_hints` sont
  rejetés par les loaders CLI/SDK avant planification, alignant le harness
  multi-source sur les attentes d'admission Torii.

## Endpoints range du gateway

Les gateways acceptent des requêtes HTTP déterministes qui reflètent les
métadonnées des adverts.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Exigence | Détails |
|----------|---------|
| **Headers** | `Range` (fenêtre unique alignée sur les offsets de chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` optionnel, et `X-SoraFS-Stream-Token` base64 obligatoire. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range` décrivant la fenêtre servie, métadonnées `X-Sora-Chunk-Range`, et headers chunker/token renvoyés. |
| **Modes d'échec** | `416` pour plages mal alignées, `401` pour tokens manquants/invalides, `429` lorsque les budgets stream/octet sont dépassés. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch d'un seul chunk avec les mêmes headers, plus le digest déterministe du
chunk. Utile pour les retries ou les téléchargements forensiques quand les
slices CAR sont inutiles.

## Workflow de l'orchestrateur multi-source

Quand le fetch multi-source SF-6 est activé (CLI Rust via `sorafs_fetch`,
SDKs via `sorafs_orchestrator`) :

1. **Collecter les entrées** — décoder le plan de chunks du manifest, récupérer
   les derniers adverts, et optionnellement passer un snapshot de télémétrie
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construire un scoreboard** — `Orchestrator::build_scoreboard` évalue
   l'éligibilité et enregistre les raisons de rejet ; `sorafs_fetch --scoreboard-out`
   persiste le JSON.
3. **Planifier les chunks** — `fetch_with_scoreboard` (ou `--plan`) impose les
   contraintes range, budgets stream, caps de retry/peer (`--retry-budget`,
   `--max-peers`) et émet un stream token scope sur le manifest pour chaque
   requête.
4. **Vérifier les reçus** — les sorties incluent `chunk_receipts` et
   `provider_reports`; les résumés CLI persistent `provider_reports`,
   `chunk_receipts` et `ineligible_providers` pour les bundles de preuves.

Erreurs courantes remontées aux opérateurs/SDKs :

| Erreur | Description |
|--------|-------------|
| `no providers were supplied` | Aucune entrée éligible après filtrage. |
| `no compatible providers available for chunk {index}` | Erreur de plage ou de budget pour un chunk spécifique. |
| `retry budget exhausted after {attempts}` | Augmentez `--retry-budget` ou évincez les peers en échec. |
| `no healthy providers remaining` | Tous les fournisseurs sont désactivés après des échecs répétés. |
| `streaming observer failed` | Le writer CAR downstream a avorté. |
| `orchestrator invariant violated` | Capturez manifest, scoreboard, snapshot de télémétrie et JSON CLI pour le triage. |

## Télémétrie et preuves

- Métriques émises par l'orchestrateur :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (taggées par manifest/région/fournisseur). Définissez `telemetry_region` en
  config ou via flags CLI pour partitionner les dashboards par flotte.
- Les résumés de fetch CLI/SDK incluent le scoreboard JSON persisté, les reçus
  de chunks et les rapports fournisseurs qui doivent voyager dans les bundles
  de rollout pour les gates SF-6/SF-7.
- Les handlers gateway exposent `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  pour que les dashboards SRE corrèlent les décisions orchestrateur avec le
  comportement serveur.

## Aides CLI et REST

- `iroha app sorafs pin list|show`, `alias list` et `replication list` emballent les
  endpoints REST du pin-registry et impriment du Norito JSON brut avec blocs
  d'attestation pour l'audit.
- `iroha app sorafs storage pin` et `torii /v1/sorafs/pin/register` acceptent des
  manifests Norito ou JSON, plus des proofs d'alias optionnels et des successors ;
  des proofs mal formés renvoient `400`, des proofs obsolètes exposent `503` avec
  `Warning: 110`, et des proofs expirés renvoient `412`.
- Les endpoints REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) incluent des structures d'attestation pour que les
  clients vérifient les données avec les derniers headers de bloc avant d'agir.

## Références

- Spécification canonique :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Types Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aides CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Crate orchestrateur : `crates/sorafs_orchestrator`
- Pack dashboards : `dashboards/grafana/sorafs_fetch_observability.json`
