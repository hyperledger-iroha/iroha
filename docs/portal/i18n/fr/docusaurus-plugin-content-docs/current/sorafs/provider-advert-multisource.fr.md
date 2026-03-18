---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Annonces de fournisseurs multi-source et planification

Cette page condense la spécification canonique dans
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilisez ce document pour les schémas Norito textuellement et les changelogs ; la copie du portail
conserver les consignes opérateur, les notes SDK et les références de télémétrie à proximité du reste
des runbooks SoraFS.

## Ajouts au schéma Norito

### Capacité de plage (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – plus grande plage contigue (octets) par requête, `>= 1`.
- `min_granularity` – résolution de recherche, `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets` – permet des offsets non contigus dans une seule demande.
- `requires_alignment` – si vrai, les offsets doivent s'aligner sur `min_granularity`.
- `supports_merkle_proof` – indique la prise en charge des témoins PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` appliquer un encodage canonique
pour que les charges utiles de gossip restent déterministes.

### `StreamBudgetV1`
- Champs : `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` en option.
- Règlement de validation (`StreamBudgetV1::validate`) :
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, lorsqu'il est présent, doit être `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Champs : `protocol: TransportProtocol`, `priority: u8` (fenêtre 0-15 appliquée par
  `TransportHintV1::validate`).
- Protocoles connus : `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Les entrées de protocole dupliquées par le fournisseur sont rejetées.

### Ajouts à `ProviderAdvertBodyV1`
- `stream_budget` en option : `Option<StreamBudgetV1>`.
- `transport_hints` en option : `Option<Vec<TransportHintV1>>`.
- Les deux champs transitent désormais via `ProviderAdmissionProposalV1`, les enveloppes
  de gouvernance, les luminaires CLI et le JSON télémétrique.

## Validation et liaison à la gouvernance

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
rejettent les métadonnées mal formées:

- Les capacités de plage doivent être décodées et respecter les limites de plage/granularite.
- Les budgets de flux / conseils de transport exigent un TLV `CapabilityType::ChunkRangeFetch`
  correspondant et une liste de conseils non vide.
- Les protocoles de transport en double et les priorités invalides génèrent des erreurs
  de validation avant la diffusion des annonces.
- Les enveloppes d'admission comparent propositions/annonces pour les métadonnées de plage via
  `compare_core_fields` afin que les payloads de gossip non concordants soient rejetés tot.

La couverture de régression se trouve dans
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Outils et agencements- Les payloads d'annonces de fournisseurs doivent inclure les métadonnées `range_capability`,
  `stream_budget` et `transport_hints`. Validez via les réponses `/v1/sorafs/providers` et les
  installations d'admission; les CV JSON doivent inclure la capacité d'analyse, le budget du flux
  et les tableaux de conseils pour l'ingestion télémétrique.
- `cargo xtask sorafs-admission-fixtures` expose les budgets de flux et les astuces de transport dans
  ses artefacts JSON afin que les tableaux de bord suivent l'adoption de la fonctionnalité.
- Les luminaires sous `fixtures/sorafs_manifest/provider_admission/` incluent désormais :
  - des annonces canoniques multi-sources,
  - `multi_fetch_plan.json` pour que les suites SDK puissent rejouer un plan de fetch
    déterministe multi-pairs.

## Intégration avec l'orchestrateur et Torii

- Torii `/v1/sorafs/providers` renvoyer les métadonnées de capacité de plage analysées avec
  `stream_budget` et `transport_hints`. Des avertissements de downgrade se déclenchent quand
  les fournisseurs omettent la nouvelle métadonnée, et les endpoints de plage du gateway
  appliquer les memes contraintes pour les clients directs.
- L'orchestrateur multi-source (`sorafs_car::multi_fetch`) applique désormais les limites de
  plage, l'alignement des capacités et les budgets lors de l'affectation du travail.
  Les tests unitaires couvrent les scénarios de chunk trop grand, de seek disperses et de
  limitation.
- `sorafs_car::multi_fetch` diffuse des signaux de downgrade (echecs d'alignement,
  requetes throttled) afin que les opérateurs puissent tracer pourquoi certains fournisseurs
  ont été ignorés pendant la planification.

## Référence de télémétrie

L'instrumentation de récupération de plage de Torii alimente le tableau de bord Grafana
**SoraFS Récupérer l'observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`) et
les règles d'alerte associées (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrique | Tapez | Étiquettes | Descriptif |
|--------------|------|------------|-------------|
| `torii_sorafs_provider_range_capability_total` | Jauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Fournisseurs annonçant des fonctionnalités de capacité de plage. |
| `torii_sorafs_range_fetch_throttle_events_total` | Compteur | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentatives de chercher de plage mariées par politique. |
| `torii_sorafs_range_fetch_concurrency_current` | Jauge | — | Streams actifs gardes consomment le budget de concurrence partage. |

Exemples de PromQL :

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilisez le compteur de limitation pour confirmer l'application des quotas avant d'activer
les valeurs par défaut de l'orchestrateur multi-source, et alertez quand la concurrence se
rapprocher les maxima de budget de streams pour votre flotte.