---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS نوڈ ↔ کلائنٹ پروٹوکول

یہ گائیڈ پروٹوکول کی canonical تعریف کو
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
میں résumer کرتی ہے۔ Dispositions Norito au niveau de l'octet et journaux des modifications en amont
spec استعمال کریں؛ copie du portail Runbooks SoraFS et points forts opérationnels
قریب رکھتی ہے۔

## Annonces du fournisseur et validation

Fournisseurs SoraFS Charges utiles `ProviderAdvertV1` (دیکھیں
`crates/sorafs_manifest::provider_advert`) potins کرتے ہیں جو opérateur gouverné
نے signe کیے ہوتے ہیں۔ métadonnées de découverte des publicités et garde-corps et broches
Le runtime d'orchestrateur multi-sources permet d'appliquer les règles d'exécution- **Durée de vie** — `issued_at < expires_at ≤ issued_at + 86,400 s`. fournisseurs کو
  12 jours de rafraîchissement des fichiers
- **Capability TLVs** — Les fonctionnalités de transport de liste TLV annoncent کرتی ہے (Torii,
  QUIC+Noise, relais SoraNet, extensions fournisseurs). codes inconnus
  `allow_unknown_capabilities = true` pour sauter le guide de GRAISSE
- **Astuces QoS** — Niveau `availability` (Chaud/Chaud/Froid), latence de récupération maximale,
  limite de concurrence et budget de flux facultatif. QoS et télémétrie observée
  مطابق ہونا چاہیے اور admission میں audit کیا جاتا ہے۔
- **Points de terminaison et sujets de rendez-vous** — Métadonnées TLS/ALPN en béton
  URL de service et sujets de découverte, clients et ensembles de gardes et abonnez-vous
  کرنا چاہیے۔
- **Politique de diversité des chemins** — `min_guard_weight`, capuchons de distribution AS/pool, ici
  `provider_failure_threshold` récupérations multi-pairs déterministes
- **Identifiants de profil** — les fournisseurs et les identifiants canoniques exposent les utilisateurs.
  (مثلاً `sorafs.sf1@1.0.0`); clients `profile_aliases` en option
  migration میں مدد دیتے ہیں۔

Règles de validation à enjeu nul, capacités/points de terminaison/listes de sujets vides, mal ordonnées
durées de vie, cibles QoS manquantes et rejet annonce d'enveloppes d'admission
Les organismes de proposition (`compare_core_fields`) et comparer les propositions et les mises à jour des potins
کرتے ہیں۔

### Extensions de récupération de plage

Les fournisseurs capables de fournir des métadonnées sont disponibles :| Champ | Objectif |
|-------|--------------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` et les drapeaux d'alignement/preuve déclarent کرتا ہے۔ |
| `StreamBudgetV1` | Enveloppe de simultanéité/débit en option (`max_in_flight`, `max_bytes_per_sec`, `burst` en option). capacité de portée requise ہے۔ |
| `TransportHintV1` | préférences de transport ordonnées (مثلاً `torii_http_range`, `quic_stream`, `soranet_relay`). priorités `0–15` pour le rejet des doublons |

Prise en charge de l'outillage :

- Pipelines publicitaires des fournisseurs, capacité de portée, budget de flux, conseils de transport
  valider les audits et les charges utiles déterministes émettent des
- Annonces canoniques multi-sources `cargo xtask sorafs-admission-fixtures`
  downgrade luminaires par `fixtures/sorafs_manifest/provider_admission/` bundle pour les utilisateurs
- Les publicités compatibles avec `stream_budget` et `transport_hints` omettent le CLI/SDK
  chargeurs de planification et de rejet de faisceaux multi-sources
  Torii attentes d'admission en ligne alignées sur

## Points de terminaison de la plage de passerelle

Les requêtes HTTP déterministes des passerelles acceptent les métadonnées publicitaires et les miroirs
کرتی ہیں۔

### `GET /v1/sorafs/storage/car/{manifest_id}`| Exigence | Détails |
|-------------|---------|
| **En-têtes** | `Range` (fenêtre unique alignée sur les décalages de blocs), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` en option, et base64 `X-SoraFS-Stream-Token`. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range` et fenêtre servie pour les métadonnées `X-Sora-Chunk-Range` et les en-têtes de chunker/token et d'écho |
| **Modes de défaillance** | plages mal alignées pour `416`, jetons manquants/invalides pour `401`, et les budgets de flux/octets dépassent ہونے pour `429`۔ |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Récupération d'un seul morceau d'en-têtes et d'un résumé de fragments déterministes nouvelles tentatives
Téléchargements médico-légaux pour les tranches de CAR

## Workflow d'orchestrateur multi-source

La récupération multi-source SF-6 est activée (Rust CLI via `sorafs_fetch`, SDK via
`sorafs_orchestrator`) :1. **Collecter les entrées** — le plan de fragments du manifeste décode les dernières publicités et extrait les informations
   Un instantané de télémétrie en option (`--telemetry-json` ou `TelemetrySnapshot`) est disponible
2. **Créer un tableau de bord** — Évaluation de l'éligibilité `Orchestrator::build_scoreboard`
   کرتا ہے اور enregistrement des raisons de rejet کرتا ہے؛ `sorafs_fetch --scoreboard-out`
   JSON persiste ici
3. **Blocs de planification** – Contraintes de plage `fetch_with_scoreboard` (یا `--plan`),
   les budgets de flux, les plafonds de tentatives/peer (`--retry-budget`, `--max-peers`) appliquent les limites
   La demande de jeton de flux à portée de manifeste est émise.
4. **Vérifier les reçus** — sorties میں `chunk_receipts` اور `provider_reports` شامل
   ہوتے ہیں؛ Résumés CLI `provider_reports`, `chunk_receipts`, اور
   `ineligible_providers` et les faisceaux de preuves persistent

Opérateurs/SDK et erreurs suivantes :

| Erreur | Descriptif |
|-------|-------------|
| `no providers were supplied` | filtrage des entrées éligibles |
| `no compatible providers available for chunk {index}` | Un gros morceau de gamme et une inadéquation budgétaire |
| `retry budget exhausted after {attempts}` | `--retry-budget` pour les pairs défaillants et l'expulsion des pairs |
| `no healthy providers remaining` | échecs répétés lorsque les fournisseurs sont désactivés |
| `streaming observer failed` | l'écrivain CAR en aval abandonne ہو گیا۔ |
| `orchestrator invariant violated` | triage, manifeste, tableau de bord, instantané de télémétrie, capture CLI JSON |

## Télémétrie et preuves- Métriques d'Orchestrator :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifeste/région/fournisseur et balises et étiquettes). tableaux de bord et flotte
  la partition est configurée pour la configuration et les indicateurs CLI sont `telemetry_region` pour la configuration
- CLI/SDK récupère les résumés et les reçus de fragments JSON du tableau de bord persistant et
  Le fournisseur rapporte des informations sur les portes SF-6/SF-7 et les bundles de déploiement pour les portes SF-6/SF-7.
- Gestionnaires de passerelle `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  exposer les décisions de l'orchestrateur des tableaux de bord SRE et le comportement du serveur
  corréler کر سکیں۔

## CLI et aides REST

- `iroha app sorafs pin list|show`, `alias list`, et `replication list` registre de broches
  Les points de terminaison REST enveloppent les éléments de preuve d'audit et les blocs d'attestation.
  raw Norito Impression JSON en anglais
- `iroha app sorafs storage pin` et `torii /v1/sorafs/pin/register` Norito et JSON
  manifeste des preuves d'alias facultatives et les successeurs acceptent des preuves d'alias facultatives mal formé
  épreuves پر `400`, épreuves périmées پر `503` مع `Warning: 110`, اور épreuves périmées
  par `412`۔
-Points de terminaison REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  structures d'attestation شامل کرتے ہیں تاکہ clients derniers en-têtes de bloc کے
  خلاف données vérifier کر سکیں۔

## Références- Spécification canonique :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
-Type Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aides CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caisse d'orchestrateur : `crates/sorafs_orchestrator`
- Pack tableau de bord : `dashboards/grafana/sorafs_fetch_observability.json`