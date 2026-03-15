---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Annonces de fournisseurs multi-origines et planification

Cette page contient les spécifications canoniques en
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilisez ce document pour les modèles Norito textuellement et les journaux de modifications ; la copie du portail
gardez le guide des opérateurs, les notes du SDK et les références de télémétrie concernant le reste
des runbooks de SoraFS.

## Ajouts au modèle Norito

### Capacité de portée (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – maire tramo contiguo (octets) par sollicitude, `>= 1`.
- `min_granularity` – résolution de recherche, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` – permettez les compensations sans contact avec une sollicitude.
- `requires_alignment` – lorsque c'est vrai, les décalages doivent être alignés avec `min_granularity`.
- `supports_merkle_proof` – support indica de tests PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` Codification appliquée canonique
pour que les charges utiles de Gossip soient si déterministes.

### `StreamBudgetV1`
- Champs : `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` en option.
- Règlement de validation (`StreamBudgetV1::validate`) :
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, lorsque vous êtes présent, cela doit être `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Champs : `protocol: TransportProtocol`, `priority: u8` (ventana 0-15 appliqué par
  `TransportHintV1::validate`).
- Protocoles connus : `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Se rechazan entradas duplicadas de protocolo por provenor.

### Ajouts à `ProviderAdvertBodyV1`
- `stream_budget` en option : `Option<StreamBudgetV1>`.
- `transport_hints` en option : `Option<Vec<TransportHintV1>>`.
- Ambos campos ahora fluyen por `ProviderAdmissionProposalV1`, los enveloppes de gobernanza,
  les appareils de CLI et le télémétrie JSON.

## Validation et confirmation de la gouvernance

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
rechazan métadonnées malformées :

- Les capacités de gamme doivent être décodifiées et compléter les limites de portée/granularité.
- Les budgets de flux / conseils de transport nécessitent un TLV `CapabilityType::ChunkRangeFetch`
  Coïncidence et une liste d'indices ne va pas.
- Protocoles de transport dupliqués et priorités invalides générant des erreurs de validation
  avant que les publicités ne soient diffusées.
- Les enveloppes d'admission comparent les propositions/annonces pour les métadonnées de rango via
  `compare_core_fields` pour que les charges utiles de gossip desalineados soient rechacen temprano.

La couverture de régression vive en
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Outillage et montages- Les charges utiles des annonces du fournisseur doivent inclure les métadonnées `range_capability`,
  `stream_budget` et `transport_hints`. Validé via les réponses de `/v2/sorafs/providers` et
  calendriers d'admission ; les résultats JSON doivent inclure la capacité analysée,
  le budget du flux et les tableaux d'indications pour l'ingestion de télémétrie.
- `cargo xtask sorafs-admission-fixtures` expose les budgets de flux et les conseils de transport à l'intérieur de
  Vos artefacts JSON pour que les tableaux de bord signalent l'adoption de la fonctionnalité.
- Les luminaires sous `fixtures/sorafs_manifest/provider_admission/` incluent désormais :
  - annonces canónicos multi-origines,
  - une variante alternative sans rang pour les essais de rétrogradation, et
  - `multi_fetch_plan.json` pour que les suites du SDK reproduisent un plan de récupération
    déterministe multi-pairs.

## Intégration avec l'orchestre et Torii

- Torii `/v2/sorafs/providers` développe des métadonnées de capacité de rang analysées conjointement avec
  `stream_budget` et `transport_hints`. Il existe des annonces de rétrogradation différentes lorsque vous
  Les fournisseurs ont omis les nouvelles métadonnées et les points de terminaison de la passerelle appliquée
  autres restrictions pour les clients directs.
- L'orchestre multi-origine (`sorafs_car::multi_fetch`) doit maintenant remplir les limites de
  rango, alineación de capacidades et stream budgets al assignar trabajo. Les essais unitaires
  Il existe de nombreux scénarios de morceaux démasqués, dispersés et étranglés.
- `sorafs_car::multi_fetch` émet des signaux de rétrogradation (fallos de alineación,
  sollicitudes étranglées) pour que les opérateurs rastreen por qué se omitieron
  fournisseurs spécifiques lors de la planification.

## Référence de télémétrie

L'instrument de récupération de distance de Torii alimente le tableau de bord de Grafana
**SoraFS Récupérer l'observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`) y
las reglas de alerta asociadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrique | Type | Étiquettes | Description |
|---------|------|-----------|-------------|
| `torii_sorafs_provider_range_capability_total` | Jauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Fournissez des fonctionnalités annonçant la capacité de rang. |
| `torii_sorafs_range_fetch_throttle_events_total` | Compteur | `reason` (`quota`, `concurrency`, `byte_rate`) | Intentions de récupérer la portée avec des groupes de limitation politiques. |
| `torii_sorafs_range_fetch_concurrency_current` | Jauge | — | Streams actifs protégés qui consomment le présumé partage de concurrence. |

Exemples de PromQL :

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilisez le contrôleur de limitation pour confirmer l'application de cartes avant d'activer
les valeurs en cas de défaut de l'explorateur multi-origine et alertent lorsque la concurrence se produit
acerque a los maximos del presupuesto de streams de tu flota.