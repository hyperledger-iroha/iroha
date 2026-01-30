---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Annonces de fournisseurs multi-source et planification

Cette page condense la specification canonique dans
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilisez ce document pour les schemas Norito verbatim et les changelogs; la copie du portail
conserve les consignes operateur, les notes SDK et les references de telemetrie a proximite du reste
des runbooks SoraFS.

## Ajouts au schema Norito

### Capacite de plage (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – plus grande plage contigue (octets) par requete, `>= 1`.
- `min_granularity` – resolution de seek, `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets` – permet des offsets non contigus dans une seule requete.
- `requires_alignment` – si true, les offsets doivent s'aligner sur `min_granularity`.
- `supports_merkle_proof` – indique la prise en charge des temoins PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` appliquent un encodage canonique
pour que les payloads de gossip restent deterministes.

### `StreamBudgetV1`
- Champs: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` optionnel.
- Regles de validation (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, lorsqu'il est present, doit etre `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Champs: `protocol: TransportProtocol`, `priority: u8` (fenetre 0-15 appliquee par
  `TransportHintV1::validate`).
- Protocoles connus: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Les entrees de protocole dupliquees par fournisseur sont rejetees.

### Ajouts a `ProviderAdvertBodyV1`
- `stream_budget` optionnel: `Option<StreamBudgetV1>`.
- `transport_hints` optionnel: `Option<Vec<TransportHintV1>>`.
- Les deux champs transitent desormais via `ProviderAdmissionProposalV1`, les enveloppes
  de gouvernance, les fixtures CLI et le JSON telemetrique.

## Validation et liaison a la gouvernance

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
rejettent les metadonnees mal formees:

- Les capacites de plage doivent se decoder et respecter les limites de plage/granularite.
- Les stream budgets / transport hints exigent un TLV `CapabilityType::ChunkRangeFetch`
  correspondant et une liste de hints non vide.
- Les protocoles de transport dupliques et les priorites invalides generent des erreurs
  de validation avant la diffusion des adverts.
- Les enveloppes d'admission comparent proposal/adverts pour les metadonnees de plage via
  `compare_core_fields` afin que les payloads de gossip non concordants soient rejetes tot.

La couverture de regression se trouve dans
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Outils et fixtures

- Les payloads d'annonces de fournisseurs doivent inclure les metadonnees `range_capability`,
  `stream_budget` et `transport_hints`. Validez via les reponses `/v1/sorafs/providers` et les
  fixtures d'admission; les resumes JSON doivent inclure la capacite parse, le stream budget
  et les tableaux de hints pour l'ingestion telemetrique.
- `cargo xtask sorafs-admission-fixtures` expose les stream budgets et transport hints dans
  ses artefacts JSON afin que les dashboards suivent l'adoption de la fonctionnalite.
- Les fixtures sous `fixtures/sorafs_manifest/provider_admission/` incluent desormais:
  - des adverts multi-source canoniques,
  - `multi_fetch_plan.json` pour que les suites SDK puissent rejouer un plan de fetch
    multi-peer deterministe.

## Integration avec l'orchestrateur et Torii

- Torii `/v1/sorafs/providers` renvoie les metadonnees de capacite de plage parsees avec
  `stream_budget` et `transport_hints`. Des avertissements de downgrade se declenchent quand
  les fournisseurs omettent la nouvelle metadonnee, et les endpoints de plage du gateway
  appliquent les memes contraintes pour les clients directs.
- L'orchestrateur multi-source (`sorafs_car::multi_fetch`) applique desormais les limites de
  plage, l'alignement des capacites et les stream budgets lors de l'affectation du travail.
  Les tests unitaires couvrent les scenarios de chunk trop grand, de seek disperses et de
  throttling.
- `sorafs_car::multi_fetch` diffuse des signaux de downgrade (echecs d'alignement,
  requetes throttled) afin que les operateurs puissent tracer pourquoi certains fournisseurs
  ont ete ignores pendant la planification.

## Reference de telemetrie

L'instrumentation de fetch de plage de Torii alimente le dashboard Grafana
**SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) et
les regles d'alerte associees (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Metrique | Type | Etiquettes | Description |
|----------|------|------------|-------------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Fournisseurs annonçant des fonctionnalites de capacite de plage. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentatives de fetch de plage bridees par politique. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | Streams actifs gardes consommant le budget de concurrence partage. |

Exemples de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilisez le compteur de throttling pour confirmer l'application des quotas avant d'activer
les valeurs par defaut de l'orchestrateur multi-source, et alertez quand la concurrence se
rapproche des maxima de budget de streams pour votre flotte.
