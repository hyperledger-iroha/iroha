---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Annonces de fournisseurs multi-origem et agendamento

Cette page reprend les spécifications canoniques du sujet
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilisez ce document pour les schémas Norito textuellement et les journaux des modifications ; un portail de copie
guide d'orientation pour les opérateurs, notes du SDK et références de télémétrie pour le reste
dos runbooks SoraFS.

## Adicoes à l'esquema Norito

### Capacité de portée (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - durée continue principale (octets) selon les exigences, `>= 1`.
- `min_granularity` - résolution de recherche, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - permet de compenser nao contiguos em uma requisicao.
- `requires_alignment` - quando true, compense devem alinhar com `min_granularity`.
- `supports_merkle_proof` - indica supporte un testemunhas PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` codage aplicam canonico
pour que les charges utiles de gossip soient déterministes en permanence.

### `StreamBudgetV1`
- Champs : `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` en option.
- Registres de validation (`StreamBudgetV1::validate`) :
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, lorsque vous êtes présent, vous devez être `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Champs : `protocol: TransportProtocol`, `priority: u8` (janela 0-15 appliqué par
  `TransportHintV1::validate`).
- Protocoles connus : `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Entrées dupliquées du protocole par le fournisseur sao rejetées.

### Adicoes a `ProviderAdvertBodyV1`
- `stream_budget` en option : `Option<StreamBudgetV1>`.
- `transport_hints` en option : `Option<Vec<TransportHintV1>>`.
- Ambos os campos agora passam por `ProviderAdmissionProposalV1`, enveloppes de gouvernance,
  appareils de CLI et JSON de télémétrie.

## Validacao e vinculacao com gouvernance

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
rejeitam métadonnées mal formées :

- Les capacités de portée doivent être décodées et cumulées avec des limites de portée/granularité.
- Les budgets de flux / conseils de transport exigent un TLV `CapabilityType::ChunkRangeFetch`
  correspondant et liste d'indices nao vazia.
- Protocoles de transport dupliqués et priorités invalides en cas d'erreurs de validation
  Avant les publicités, il y avait des rumeurs.
- Enveloppes d'admission comparam proposition/annonces para métadonnées de gamme via
  `compare_core_fields` pour que les charges utiles de Gossip divergentes soient rejetées.

La couverture de régression leur est vive
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Outillage et montages

- Les charges utiles des annonces du fournisseur incluent les métadonnées `range_capability`,
  `stream_budget` et `transport_hints`. Valide via les réponses de `/v1/sorafs/providers`
  e les conditions d'admission ; Les résumés JSON développent une capacité d'analyse, ou de flux
  budget et tableaux d'astuces pour l'acquisition de télémétrie.
- `cargo xtask sorafs-admission-fixtures` montre les budgets de flux et les conseils de transport à l'intérieur
  vos artefatos JSON pour que les tableaux de bord accompagnent l'ajout de la fonctionnalité.
- Les luminaires sob `fixtures/sorafs_manifest/provider_admission/` incluent :
  - annonces canonicos multi-origem,
  - `multi_fetch_plan.json` pour que les suites du SDK reproduisent un plan de récupération
    déterministe multi-pairs.

## Intégration de l'orchestrateur et Torii- Torii `/v1/sorafs/providers` renvoie les métadonnées de la plage analysée conjointement avec
  `stream_budget` et `transport_hints`. Les avis de rétrogradation disparam quando
  les fournisseurs ont omis les nouvelles métadonnées et les points de terminaison de la gamme d'applications de passerelle comme
  mesmas restrices para clientes directos.
- O orchestrateur multi-origem (`sorafs_car::multi_fetch`) il y a des limites d'application
  gamme, optimisation des capacités et budgets de flux pour contribuer au travail. Unité
  tests cobrem scénarios de chunk très grande, recherche clairsemée et limitation.
- `sorafs_car::multi_fetch` transmet sinais de downgrade (falhas de alinhamento,
  les exigences sont limitées) pour que les opérateurs rastreiem por que provideores spécifiques
  foram ignorantados durante o planejamento.

## Référence de télémétrie

Un instrument de gamme récupère de Torii alimente le tableau de bord Grafana
**SoraFS Récupérer l'observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`) e
comme regras de alerta associadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrique | Type | Étiquettes | Description |
|---------|------|--------|---------------|
| `torii_sorafs_provider_range_capability_total` | Jauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Provedores qui annoncent les fonctionnalités de la capacité de portée. |
| `torii_sorafs_range_fetch_throttle_events_total` | Compteur | `reason` (`quota`, `concurrency`, `byte_rate`) | Les tentatives de gamme récupèrent des groupes étranglés par la politique. |
| `torii_sorafs_range_fetch_concurrency_current` | Jauge | - | Les flux sont protégés en consommant ou en comparant le budget. |

Exemples de PromQL :

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilisez le contrôleur de limitation pour confirmer l'application des quotas avant de commencer
aos defaults fait orchestrator multi-origem et alerte quand la coïncidence se rapproche
dos maximos do stream budget da sua frota.