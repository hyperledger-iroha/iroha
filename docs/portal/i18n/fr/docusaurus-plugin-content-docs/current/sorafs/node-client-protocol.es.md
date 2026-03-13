---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocole de nœud ↔ client de SoraFS

Ce guide résume la définition canonique du protocole en
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilisez la spécification en amont pour les mises en page Norito au niveau d'octets et des
journaux des modifications ; la copie du portail maintient les points opérationnels à proximité du restaurant
des runbooks de SoraFS.

## Annonces du fournisseur et validation

Les fournisseurs de charges utiles difunden SoraFS `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) firmados por el operador gobernado.
Les annonces indiquent les métadonnées de découverte et les garde-fous que le
L'orchestre multifuente s'impose en temps d'exécution.- **Vigencia** — `issued_at < expires_at ≤ issued_at + 86,400 s`. Les fournisseurs
  deben refrescar cada 12 horas.
- **TLV de capacités** — la liste TLV annonce les fonctions de transport (Torii,
  QUIC+Noise, relais SoraNet, extensions du fournisseur). Les codes desconocidos
  Vous pouvez omettre lorsque `allow_unknown_capabilities = true`, en suivant le guide
  GRAISSE.
- **Pistas de QoS** — niveau de `availability` (Hot/Warm/Cold), latence maximale de
  récupération, limite de concurrence et présupposé de flux facultatif. La QoS
  doit être aligné avec la télémétrie observée et se auditer en admission.
- **Endpoints et sujets de rendez-vous** — URL de service concret avec
  métadonnées TLS/ALPN plus les sujets de découverte pour les clients
  deben abonnez-vous au constructeur d'ensembles de gardes.
- **Politique de diversité des routes** — `min_guard_weight`, topes de fan-out de
  AS/pool et `provider_failure_threshold` permettent de récupérer les déterministes possibles
  multi-pairs.
- **Identificadores de perfil** — les fournisseurs doivent exposer le handle
  canónico (p. ej., `sorafs.sf1@1.0.0`); `profile_aliases` options utiles pour
  migrer clients anciens.

Las reglas de validación rechazan mise zéro, listas vacías de capacités,
points finaux ou sujets, vigencias desordenadas ou objectifs de QoS faltantes. Los
conditions d'admission comparées aux corps de l'annonce et à la proposition
(`compare_core_fields`) avant de procéder à des mises à jour.

### Extensions de récupération par rangLes fournisseurs avec rang incluent les métadonnées suivantes :

| Champ | Proposé |
|-------|--------------|
| `CapabilityType::ChunkRangeFetch` | Déclarez `max_chunk_span`, `min_granularity` et les drapeaux d'alignement/prueba. |
| `StreamBudgetV1` | Enveloppe facultative de concurrence/débit (`max_in_flight`, `max_bytes_per_sec`, `burst` facultative). Nécessite une capacité de rang. |
| `TransportHintV1` | Préférences de transport ordonnées (p. ej., `torii_http_range`, `quic_stream`, `soranet_relay`). Les priorités de `0–15` et se rechazan duplicados. |

Support d'outillage :

- Les pipelines du fournisseur annoncent leur capacité de validation, flux
  conseils en matière de budget et de transport avant de déterminer les charges utiles des émetteurs pour les auditoires.
- `cargo xtask sorafs-admission-fixtures` empaqueta anuncios multifuentes
  canoniques avec les appareils de downgrade et
  `fixtures/sorafs_manifest/provider_admission/`.
- Les annonces avec rango qui omiten `stream_budget` ou `transport_hints` fils
  récupérés par les chargeurs CLI/SDK avant de programmer, en maintenant l'arnés
  Multifuente aligné avec les attentes d'admission de Torii.

## Endpoints de rang de la passerelle

Les passerelles acceptent de solliciter les déterministes HTTP qui réfléchissent aux métadonnées de
les annonces.

### `GET /v2/sorafs/storage/car/{manifest_id}`| Requis | Détails |
|-----------|----------|
| **En-têtes** | `Range` (ventana única alineada a offsets of chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` facultatif et `X-SoraFS-Stream-Token` base64 obligatoire. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range` qui décrivent la fenêtre de service, les métadonnées `X-Sora-Chunk-Range` et les en-têtes des chunker/token écoados. |
| **Modes de chute** | `416` pour les plages déchargées, `401` pour les jetons incorrects ou invalides, `429` lorsque les présupposés de flux/octets sont dépassés. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Récupérez un seul morceau avec les mêmes en-têtes plus le résumé déterminant du
morceau. Utiles pour réintenter ou télécharger des tranches lorsque vous n'en avez pas besoin
VOITURE.

## Flux de travail de l'orchestre multifonction

Lorsque vous activez la récupération multifonction SF-6 (CLI Rust via `sorafs_fetch`,
SDK via `sorafs_orchestrator`) :1. **Recopier les entrées** — décodifier le plan des morceaux du manifeste, suivre
   les annonces les plus récentes et, facultativement, passer un instantané de télémétrie
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construire un tableau de bord** — `Orchestrator::build_scoreboard` évaluer la
   éligibilité et inscription des raisons de rechazo ; `sorafs_fetch --scoreboard-out`
   conserver le JSON.
3. **Blocs de programmation** — `fetch_with_scoreboard` (ou `--plan`) impone
   restrictions de rango, présupposés de flux, topes de reintentos/pairs
   (`--retry-budget`, `--max-peers`) et émettez un jeton de flux en toute sécurité
   manifester por cada sollicitud.
4. **Vérifier les recettes** — les sorties incluent `chunk_receipts` et
   `provider_reports` ; les résumés de CLI persistent `provider_reports`,
   `chunk_receipts` et `ineligible_providers` pour les ensembles de preuves.

Erreurs communes que vous pouvez trouver sur les opérateurs/SDK :

| Erreur | Description |
|-------|-------------|
| `no providers were supplied` | Aucune entrée de foin éligible n'est laissée derrière le filtre. |
| `no compatible providers available for chunk {index}` | Désajustez la plage ou le présupposé pour un morceau spécifique. |
| `retry budget exhausted after {attempts}` | Incrementa `--retry-budget` ou expulsa les pairs faillis. |
| `no healthy providers remaining` | Tous les fournisseurs ont été déshabilités après des erreurs répétées. |
| `streaming observer failed` | El escritor CAR aval avorté. |
| `orchestrator invariant violated` | Capturez le manifeste, le tableau de bord, l'instantané de télémétrie et le JSON de la CLI pour le tri. |

## Télémétrie et preuves- Métriques émises par l'orchestre :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (étiquettes par manifeste/région/fournisseur). Configurer `telemetry_region` fr
  config ou via les drapeaux de CLI pour que les tableaux de bord soient séparés par flota.
- Les résultats de récupération dans CLI/SDK incluent le tableau de bord JSON persistant, reçus
  les morceaux et les informations des fournisseurs qui doivent être envoyés dans les bundles de déploiement
  pour les portes SF-6/SF-7.
- Les gestionnaires de l'exposant de la passerelle `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  pour que les tableaux de bord SRE correspondent aux décisions de l'explorateur avec le
  comportement du serveur.

## Aides de CLI et REST

- `iroha app sorafs pin list|show`, `alias list` et `replication list` envuelven les
  endpoints REST del pin-registry et impression Norito JSON brut avec blocs de
  attestation para evidencia de auditoría.
- `iroha app sorafs storage pin` et `torii /v2/sorafs/pin/register` manifestes d'acceptan
  Norito ou JSON plus de preuves d'alias optionnels et de successeurs ; preuves malformées
  elevan `400`, preuves obsolètes exponen `503` avec `Warning: 110`, et preuves
  expirados devuelven `412`.
- Les points de terminaison REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) comprend des structures d'attestation pour que les
  Les clients vérifient les données sur les en-têtes du dernier bloc avant l'actionnement.

## Références- Spécification canonique :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Types Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aides de CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caisse de l'orchestre : `crates/sorafs_orchestrator`
- Paquet de tableaux de bord : `dashboards/grafana/sorafs_fetch_observability.json`