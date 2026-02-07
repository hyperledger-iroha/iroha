---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocole du no  client de SoraFS

Cette guide reprend la définition canonique du protocole
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilisez une spécification en amont pour les mises en page Norito au niveau de l'octet et des journaux de modifications ;
une copie du portail mantem os destaques operacionais perto do restante dos runbooks
SoraFS.

## Annonces de fournisseur et de validation

Provedores SoraFS dissémine les charges utiles `ProviderAdvertV1` (voir
`crates/sorafs_manifest::provider_advert`) assassinés par l'opérateur gouverné. Os
adverts fixam os metadados de descoberta e os guardrails que o orquestrador
Runtime d'application multi-source.- **Vigencia** - `issued_at < expires_at <= issued_at + 86,400 s`. Fournisseurs
  devem renovar a cada 12 horas.
- **TLV de capacité** - une liste d'annonces de ressources de transport TLV (Torii,
  QUIC+Noise, relais SoraNet, extensions de fornecedor). Codigos desconhecidos
  Ils peuvent être ignorés quand `allow_unknown_capabilities = true`, suite à
  orientacao GRAISSE.
- **Conseils de QoS** - niveau de `availability` (Hot/Warm/Cold), latence maximale de
  récupération, limite de concorrencia et budget de stream optionnel. Développement QoS
  alinhar com a telemetria observé et audité à l'admission.
- **Sujets de points de terminaison et de rendez-vous** - URL de service concret avec métadonnées
  TLS/ALPN plus de sujets de découverte chez les clients qui doivent être inscrits
  entre autres, construire des ensembles de gardes.
- **Politica de diversidade de caminho** - `min_guard_weight`, caps de fan-out de
  AS/pool e `provider_failure_threshold` tornam possiveis récupère les déterministes
  multi-pairs.
- **Identificadores de perfil** - les fournisseurs doivent exporter le handle canonique (ex.
  `sorafs.sf1@1.0.0`); `profile_aliases` opcionais ajudam clients antigos a migrar.

Regras de validacao rejeitam mise zéro, listes vazias de capacités/points finaux/sujets,
vigilances pour l'ordre ou les cibles de QoS ausentes. Comparatif des enveloppes d'admission
les corps de publicité et de proposition (`compare_core_fields`) avant de diffuser
actualisations.

### Extensions de plage à récupérer

Les fournisseurs de la gamme incluent les métadonnées suivantes :| Champ | Proposé |
|-------|---------------|
| `CapabilityType::ChunkRangeFetch` | Déclarer `max_chunk_span`, `min_granularity` et les drapeaux d'alignement/prova. |
| `StreamBudgetV1` | Enveloppe facultative de correspondance/débit (`max_in_flight`, `max_bytes_per_sec`, `burst` facultative). Demander une capacité de portée. |
| `TransportHintV1` | Préférences de transport ordonnées (ex. `torii_http_range`, `quic_stream`, `soranet_relay`). Priorités sao `0-15` et duplicados sao rejeitados. |

Support d'outillage :

- Pipelines de fournisseur annonçant le développement de la capacité de validation de la gamme, du budget de flux et
  conseils de transport avant d'émettre des charges utiles déterministes pour les auditoriums.
- `cargo xtask sorafs-admission-fixtures` agrupa annonce des canonicos multi-sources
  junto com rétrograder les appareils em `fixtures/sorafs_manifest/provider_admission/`.
- Annonces avec la gamme qui omitem `stream_budget` ou `transport_hints` sao rejeitados
  pelos loaders CLI/SDK avant l'agenda, maintenant ou l'exploitation multi-source
  alinhado com as expectativas de admissao do Torii.

## Points de terminaison de la plage de la passerelle

Les passerelles ont besoin de HTTP deterministas que espelham os métadados do
annonce.

### `GET /v1/sorafs/storage/car/{manifest_id}`| Requis | Détails |
|-----------|----------|
| **En-têtes** | `Range` (une seule combinaison de décalages de chunk), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` en option et `X-SoraFS-Stream-Token` base64 obligatoire. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car`, `Content-Range` décrivent le service principal, les métadonnées `X-Sora-Chunk-Range` et les en-têtes des chunker/token écoados. |
| **Falhas** | `416` pour les gammes desalinhados, `401` pour les jetons ausentes/invalidos, `429` lorsque les budgets de flux/octets sont dépassés. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Récupérez le chunk unique avec nos en-têtes mais le résumé détermine le chunk.
Utilisé pour les tentatives ou les téléchargements lorsque des tranches de CAR sont nécessaires.

## Workflow pour l'orquestrador multi-source

Lorsque vous récupérez SF-6 multi-sources est activé (CLI Rust via `sorafs_fetch`,
SDK via `sorafs_orchestrator`) :1. **Coletar entradas** - décodifier le plan des morceaux qui se manifestent, puxar os
   annonces les plus récentes et, facultativement, passer un instantané de télémétrie
   (`--telemetry-json` ou `TelemetrySnapshot`).
2. **Construire un tableau de bord** - `Orchestrator::build_scoreboard` avalia a
   éligibilité et inscription des motifs de refus ; `sorafs_fetch --scoreboard-out`
   persister ou JSON.
3. **Morceaux d'agenda** - `fetch_with_scoreboard` (ou `--plan`) renforcer les restrictions
   de plage, budgets de flux, plafonds de nouvelle tentative/peer (`--retry-budget`, `--max-peers`)
   Il émet un jeton de flux avec le manifeste pour chaque besoin.
4. **Vérifier les recettes** - comme indiqué, cela inclut `chunk_receipts` et `provider_reports` ;
   résumés de la CLI persistante `provider_reports`, `chunk_receipts` et
   `ineligible_providers` pour les lots de preuves.

Erreurs courantes présentées par les opérateurs/SDK :

| Erreur | Description |
|------|-----------|
| `no providers were supplied` | Nenhuma entrada elegivel apos o filtre. |
| `no compatible providers available for chunk {index}` | Inadéquation de la gamme ou du budget pour un morceau spécifique. |
| `retry budget exhausted after {attempts}` | Augmentez `--retry-budget` ou supprimez les pairs avec falha. |
| `no healthy providers remaining` | Tous les fournisseurs sont déstabilisés après des erreurs répétées. |
| `streaming observer failed` | O écrivain CAR aval abortou. |
| `orchestrator invariant violated` | Capturez le manifeste, le tableau de bord, l'instantané de télémétrie et CLI JSON pour le tri. |

## Télémétrie et preuves- Métriques émises par l'orchestre :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (tagueadas por manifest/region/provider). Définir `telemetry_region` dans la configuration
  ou via les drapeaux de CLI pour partitionner les tableaux de bord par frota.
- Les résumés de récupération sans CLI/SDK incluent le tableau de bord JSON persistant, les reçus de blocs
  Le fournisseur rapporte que nous avons développé nos bundles de déploiement pour les portes SF-6/SF-7.
- Exposition des gestionnaires de passerelle `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  pour que les tableaux de bord SRE correlacionem décident de l'explorateur avec le comportement
  faire serviteur.

## Helpers de CLI et REST

- `iroha app sorafs pin list|show`, `alias list` et `replication list` impliquent le système d'exploitation
  les points de terminaison REST du registre pin et l'impression Norito JSON brut avec les blocs de
  attestation para evidencias de auditoria.
- `iroha app sorafs storage pin` et `torii /v1/sorafs/pin/register` aceitam manifestes
  Norito ou JSON avec alias preuves et successeurs optionnels ; preuves malformées
  Geram `400`, épreuves périmées renvoyées `503` avec `Warning: 110`, et épreuves expirées
  retour du nom `412`.
- Points de terminaison REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  incluem estruturas de attestation para que clientses verifiquem dados contra os
  derniers en-têtes de bloc avant d'agir.

## Références- Spécification canonique :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Types Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Aides de CLI : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caisse de l'orchestre : `crates/sorafs_orchestrator`
- Pack de tableaux de bord : `dashboards/grafana/sorafs_fetch_observability.json`