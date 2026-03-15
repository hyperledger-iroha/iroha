---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocole SoraFS utilisé ↔ client

Ceci permet de reprendre l'application canonique du protocole
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilisez les spécifications en amont pour la mise en page Norito et le journal des modifications ;
La copie du portail fournit des éléments d'exploitation directement avec le runbook SoraFS.

## Fournisseur et validation

Le fournisseur SoraFS transmet la charge utile `ProviderAdvertV1` (avec.
`crates/sorafs_manifest::provider_advert`), подписанные управляемым оператором.
Fixation des métadonnées et des garde-corps,
Un opérateur multi-industriel joue un rôle important dans la sélection des produits.- **Срок действия** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Fournisseurs
  должны обновлять каждые 12 часов.
- **Capacité TLV** — TLV-список рекламирует транспортные возможности (Torii,
  QUIC+Noise, relais SoraNet, extensions fournisseurs). Les codes les plus récents peuvent être utilisés
  acheter pour `allow_unknown_capabilities = true`, selon les recommandations
  GRAISSE.
- **Conseils QoS** — niveau `availability` (Chaud/Chaud/Froid), maximum
  la gestion, la limitation des coûts et le budget de flux opérationnel. QoS avancé
  совпадать с наблюдаемой телеметрией и проверяется при admission.
- **Points de terminaison et sujets de rendez-vous** — URL de service concrète avec TLS/ALPN
  des métadonnées et des sujets de découverte, pour tous vos clients
  построении garde ensembles.
- **Политика разнообразия путей** — `min_guard_weight`, limite la diffusion pour
  AS/пулов и `provider_failure_threshold` обеспечивают детерминированные
  récupération multi-pairs.
- **Profils de professionnels** — Les fournisseurs s'occupent de la publication canonique
  poignée (par exemple, `sorafs.sf1@1.0.0`); optionnel `profile_aliases`
  миграции старых clients.

Il y a des validations sur n'importe quel enjeu, des capacités/points finaux/sujets spécifiques,
Il s'agit de tâches qui permettent d'obtenir des cibles QoS. Enveloppes d'admission
сравнивают тела объявления и предложения (`compare_core_fields`) avant
распространением обновлений.

### Récupération de la plage de résolution

Le fournisseur de gamme comprend les métadonnées suivantes :| Pôle | Назначение |
|------|------------|
| `CapabilityType::ChunkRangeFetch` | Объявляет `max_chunk_span`, `min_granularity` and flags выравнивания/доказательств. |
| `StreamBudgetV1` | Опциональный enveloppe конкурентности/throughput (`max_in_flight`, `max_bytes_per_sec`, опциональный `burst`). Capacité de la gamme Trebute. |
| `TransportHintV1` | Transport préalable en cours (par exemple, `torii_http_range`, `quic_stream`, `soranet_relay`). Les priorités `0–15` sont doublement ouvertes. |

Outillage complémentaire :

- L'annonce du fournisseur sur le site permet de valider la capacité de gamme et le budget de flux
  et des conseils de transport pour déterminer la charge utile pour les auditeurs.
- `cargo xtask sorafs-admission-fixtures` paquet canonique multi-source
  annonces вместе с downgrade luminaires в
  `fixtures/sorafs_manifest/provider_admission/`.
- Les annonces de gamme hors `stream_budget` ou `transport_hints` s'affichent.
  CLI/SDK pour la planification, permettant d'exploiter un faisceau multi-source dans les solutions
  ожиданиями admission Torii.

## Passerelle des points de terminaison de la plage

Les passerelles permettent de détecter les connexions HTTP et d'obtenir des métadonnées.

### `GET /v1/sorafs/storage/car/{manifest_id}`| Trebovanie | Détails |
|------------|--------|
| **En-têtes** | `Range` (intervalle, disponible pour les décalages de blocs), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` et base64 disponibles `X-SoraFS-Stream-Token`. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range`, nous vous proposons une description de l'intervalle, des métadonnées `X-Sora-Chunk-Range` et de ce chunker/token d'en-têtes. |
| **Modes de défaillance** | `416` pour les diapasons jamais utilisés, `401` pour les serviettes jetables/non valides, `429` pour превышении budget flux/octet. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Récupère le morceau de fragments correspondant aux en-têtes ainsi que le morceau de résumé digest.
Plutôt pour les récupérations ou les téléchargements médico-légaux, il n'y a pas de tranches CAR.

## Workflow pour un opérateur multi-industriel

Vous pouvez utiliser la récupération multi-source SF-6 (Rust CLI depuis `sorafs_fetch`,
SDK ici `sorafs_orchestrator`) :1. **Собрать входные данные** — декодировать план chunk'ов manifest, получить
   après les publicités et, éventuellement, avant l'instantané de télémétrie
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Publier le tableau de bord** — `Orchestrator::build_scoreboard` оценивает
   пригодность и записывает причины отказа; `sorafs_fetch --scoreboard-out`
   utilise JSON.
3. **Планировать chunk'и** — `fetch_with_scoreboard` (ou `--plan`) применяет
   plage-organisation, budget de flux, limites de tentative/peer (`--retry-budget`,
   `--max-peers`) et ajoutez un jeton de flux dans le manifeste de portée pour votre projet.
4. **Provérifier les reçus** — le résultat est `chunk_receipts` et
   `provider_reports` ; Résumé CLI concernant `provider_reports`, `chunk_receipts`
   et `ineligible_providers` pour les lots de preuves.

Logiciels compatibles, opérateurs/SDK disponibles :

| Ошибка | Description |
|--------|----------|
| `no providers were supplied` | Il n'y a aucun moyen de le filtrer après le filtrage. |
| `no compatible providers available for chunk {index}` | Il est nécessaire de mettre la diapason ou le boudin pour le morceau de béton. |
| `retry budget exhausted after {attempts}` | Utilisez `--retry-budget` ou excluez vos pairs. |
| `no healthy providers remaining` | Votre fournisseur s'ouvrira après les appels d'offres. |
| `streaming observer failed` | L'écrivain CAR en aval завершился с ошибкой. |
| `orchestrator invariant violated` | Enregistrez le manifeste, le tableau de bord, l'instantané de télémétrie et CLI JSON pour le tri. |

## Télémétrie et documentation- Mesures de l'opérateur commercial :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (avec plusieurs manifeste/région/fournisseur). Sélectionnez `telemetry_region` dans la configuration ou
  Ici, les drapeaux CLI sont placés sur le bord du bateau.
- CLI/SDK récupère les résumés du tableau de bord JSON, des reçus de morceaux et
  rapports du fournisseur, qui devraient être fournis dans les bundles de déploiement pour gate'ов SF-6/SF-7.
-Les gestionnaires de passerelle publient `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`,
  Les tableaux de bord SRE correspondent à l'opérateur de résolution avec le serveur.

## CLI et REST helpers

- `iroha app sorafs pin list|show`, `alias list` et `replication list` fonctionnent
  points de terminaison REST pin-registry et certification Norito JSON avec blocs
  для аудиторских доказательств.
- `iroha app sorafs storage pin` et `torii /v1/sorafs/pin/register` pour Norito
  ou JSON manifeste plus de preuves d'alias et de successeurs ; épreuves mal formées
  возвращают `400`, épreuves périmées de `503` à `Warning: 110`, épreuves périmées
  возвращают `412`.
-Points de terminaison REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) pour l'attestation de structure, pour tous les clients
  проверить данные относительно последних bloc-têtes avant la conception.

## Ссылки- Spécifications canoniques :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito types : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aide CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caisse оркестратора: `crates/sorafs_orchestrator`
- Paquet de bord : `dashboards/grafana/sorafs_fetch_observability.json`