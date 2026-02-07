---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# פרוטוקול nœud ↔ לקוח SoraFS

Ce guide resume la définition canonique du protocole dans
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
שימוש במפרט במעלה הזרם עבור פריסות Norito au niveau octet et
les changelogs ; la copie du portail garde les points opérationnels près du
reste des runbooks SoraFS.

## מפרסמת fournisseur et validation

Les fournisseurs SoraFS מפוזרים מטענים `ProviderAdvertV1` (voir
`crates/sorafs_manifest::provider_advert`) חתימות פר ל'אופרטור גוברנה. לס
פרסומות מתקנת les métadonnées de découverte et les garde-fous que l'orchestrateur
ריבוי מקורות אפליקציה לביצוע.

- **Durée de validité** — `issued_at < expires_at ≤ issued_at + 86 400 s`. לס
  fournisseurs doivent rafraîchir toutes les 12 heures.
- **TLV de capacités** - לה רשימה של TLV annonce les fonctionnalités transport
  (Torii, QUIC+Noise, relais SoraNet, ספק הרחבות). Les codes inconnus
  peuvent être ignorés lorsque `allow_unknown_capabilities = true`, en suivant
  les המלצות GREASE.
- **מדדי QoS** — tier de `availability` (חם/חם/קר), זמן אחזור מקסימלי דה
  récupération, limite de concurrence ו- budget de stream optionnel. La QoS doit
  s'aligner sur la télémétrie observée et est auditée lors de l'admission.
- **נקודות קצה et נושאי מפגש** - כתובות אתרים של שירות קונקרטיות avec
  שיטות TLS/ALPN, בתוספת נושאי עזר ללקוחות
  doivent s'abonner lors de la construction des guard sets.
- **Politique de diversité de chemin** — `min_guard_weight`, כיסויי מניפה
  AS/pool et `provider_failure_threshold` מעבירים תוצאות אפשריות
  déterministes multi-peer.
- **Identifiants de profil** - les fournisseurs doivent exposer le handle
  canonique (לדוגמה `sorafs.sf1@1.0.0`) ; des `profile_aliases` אפשרויות עזר
  les anciens clients à migrer.

Les règles de validation rejettent stake zero, רשימות סרטוני היכולות,
נקודות קצה או נושאים, durées mal ordonnées, או objectifs QoS manquants. לס
מעטפות ד'הקבלה comparent les corps d'advert et de proposition
(`compare_core_fields`) avant de diffuser des mises à jour.

### הרחבות ל-Fetch par plages

טווח הקיבולת של Les Fournisseurs כולל את השיטות הבאות:

| אלוף | Objectif |
|-------|--------|
| `CapabilityType::ChunkRangeFetch` | הצהר `max_chunk_span`, `min_granularity` et les flags d'alignement/preuve. |
| `StreamBudgetV1` | Envelope optionnelle de concurrence/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` optionnel). דורש טווח קיבולת אחד. |
| `TransportHintV1` | Preférences de transport ordonnées (לדוגמה `torii_http_range`, `quic_stream`, `soranet_relay`). Les priorités vont de `0–15` et les doublons sont rejetés. |

כלי תמיכה:- Les pipelines d'advert fournisseur doivent valider capacité טווח, זרם
  רמזים לתקציב ותחבורה Avant d'émettre des payloads déterministes pour les
  ביקורת.
- `cargo xtask sorafs-admission-fixtures` קבוצת פרסומות מרובת מקורות
  canoniques avec des fixtures de downgrade dans
  `fixtures/sorafs_manifest/provider_admission/`.
- מגוון הפרסומות של `stream_budget` או `transport_hints`
  דחויים עבור מעמיסים CLI/SDK אוונט תכנון, רתמה מיושרת
  multi-source sur les attentes d'admission Torii.

## טווח נקודות הקצה של השער

Les gateways acceptent des requêtes HTTP déterministes qui reflètent les
מדדי פרסומות.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| נחישות | פרטים |
|--------|--------|
| **כותרות** | `Range` (fenêtre unique alignée sur les offsets de chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` optionnel, et `X-SoraFS-Stream-Token` base64 obligatoire. |
| **תגובות** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range` מותאם לשירותי הגנה, מתודות `X-Sora-Chunk-Range`, ואסימונים של כותרות. |
| **Modes d'échec** | `416` pour plages mal alignées, `401` pour tokens manquants/invalides, `429` lorsque les budgets stream/octet sont dépassés. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch d'un seul chunk avec les mêmes כותרות, פלוס le digest déterministe du
נתח. Utile pour les retries ou les téléchargements forensiques quand les
פרוסות CAR sont inutiles.

## זרימת עבודה של תזמורת מרובת מקורות

Quand le fetch multi-source SF-6 est active (CLI Rust via `sorafs_fetch`,
ערכות SDK דרך `sorafs_orchestrator`):

1. **Collecter les entrées** - decoder le plan de chunks du manifest, récupérer
   les derniers adverts, et optionnellement passer un תמונת מצב של télémétrie
   (`--telemetry-json` או `TelemetrySnapshot`).
2. **בנה לוח תוצאות** — `Orchestrator::build_scoreboard` évalue
   l'éligibilité et enregistre les raisons de rejet; `sorafs_fetch --scoreboard-out`
   להתמיד ב-JSON.
3. **Planifier les chunks** - `fetch_with_scoreboard` (ou `--plan`) להטיל מס
   טווח גבולות, זרם תקציבים, מכסים של ניסיון חוזר/עמית (`--retry-budget`,
   `--max-peers`) et émet un stream token scope sur le manifest pour chaque
   לבקש.
4. **Vérifier les reçus** - les sorties incluent `chunk_receipts` et
   `provider_reports`; les résumés CLI persistent `provider_reports`,
   `chunk_receipts` et `ineligible_providers` pour les bundles de preuves.

Erreurs courantes remontées aux opérateurs/SDKs :

| ארור | תיאור |
|--------|----------------|
| `no providers were supplied` | סינון אפר'ה מתאים ל-Aucune entrée. |
| `no compatible providers available for chunk {index}` | Erreur de plage ou de budget pour un chunk spécifique. |
| `retry budget exhausted after {attempts}` | Augmentez `--retry-budget` או évincez les peers en échec. |
| `no healthy providers remaining` | Tous les fournisseurs sont désactivés après des échecs répétés. |
| `streaming observer failed` | Le writer CAR במורד הזרם אפורטה. |
| `orchestrator invariant violated` | מניפסט של Capturez, לוח תוצאות, תמונת מצב של טלמטריה ו-JSON CLI לטריאג'. |

## Télémétrie et preuves- Métriques émises par l'orchestrateur:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (taggées par manifest/region/fournissur). Définissez `telemetry_region` en
  להגדיר או באמצעות דגלים CLI לשפוך מחיצות ללוחות המחוונים של יופי.
- קורות החיים של הבאת CLI/SDK כולל לוח התוצאות JSON מתמיד, שוב ושוב
  de chunks et les rapports fournisseurs qui doivent voyager dans les bundles
  de rollout pour les gates SF-6/SF-7.
- שער המטפלים חשוף `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  pour que les לוחות מחוונים SRE corrèlent les décisions orchestrateur avec le
  משרת מחלקה.

## עוזרי CLI ו-REST

- `iroha app sorafs pin list|show`, `alias list` ו-`replication list` מסמלים
  נקודות קצה REST du pin-registry et impriment du Norito JSON brut avec blocks
  d'attestation pour l'audit.
- `iroha app sorafs storage pin` ו-`torii /v1/sorafs/pin/register` מקובלים
  מניפסט Norito או JSON, בתוספת des proofs d'alias optionnels et des successors;
  des proofs mal formés renvoient `400`, des proofs consolètes exposent `503` avec
  `Warning: 110`, et des proofs expirés renvoient `412`.
- Les נקודות קצה REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) incluent des structures d'atestation pour que les
  clients vérifient les données avec les derniers headers de bloc avant d'agir.

## רפרנסים

- מפרט קנוני:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- סוגי Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aides CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- מתזמר ארגז: `crates/sorafs_orchestrator`
- לוחות מחוונים מארז: `dashboards/grafana/sorafs_fetch_observability.json`