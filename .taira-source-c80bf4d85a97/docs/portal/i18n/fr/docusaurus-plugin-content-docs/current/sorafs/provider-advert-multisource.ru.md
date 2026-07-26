---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fournisseurs et planifications multi-histoires

Il s'agit d'une section spécifique du canal canonique.
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilisez ce document pour le système Norito et le journal des modifications ; version du portail
Vous trouverez ici des instructions d'utilisation, des composants SDK et des outils de télémétrie pour l'installation.
pour les runbooks SoraFS.

## Ajout du schéma Norito

### Diapason de la fenêtre (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – intervalle maximal (banque) pour l'installation, `>= 1`.
- `min_granularity` – recherche de recherche, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets` – permet d'effectuer des compensations inutiles à chaque fois.
- `requires_alignment` – si vrai, les décalages doivent être appliqués à `min_granularity`.
- `supports_merkle_proof` – указывает подддержку свидетельств PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` est compatible avec les canons
кодирование, чтобы payloads gossip оставались детерминированными.

### `StreamBudgetV1`
- Nom : `max_in_flight`, `max_bytes_per_sec`, optionnel `burst_bytes`.
- Validation valide (`StreamBudgetV1::validate`) :
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, si vous le souhaitez, vous devez utiliser `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Nom : `protocol: TransportProtocol`, `priority: u8` (jusqu'à 0-15 contrôleurs
  `TransportHintV1::validate`).
- Protocoles disponibles : `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Doublez les protocoles de vérification auprès du fournisseur.

### Ajout à `ProviderAdvertBodyV1`
- Опциональный `stream_budget: Option<StreamBudgetV1>`.
- Опциональный `transport_hints: Option<Vec<TransportHintV1>>`.
- Оба поля проходят через `ProviderAdmissionProposalV1`, enveloppes de gouvernance,
  Appareils CLI et JSON télémétrique.

## Validation et confidentialité en matière de gouvernance

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
отклоняют поврежденные метаданные:

- Les diapasons sont suffisamment longs pour décoder correctement et supprimer les limites
  диапазона/гранулярности.
- Budgets de flux / conseils de transport требуют TLV `CapabilityType::ChunkRangeFetch`
  и непустого списка conseils.
- Doubler les protocoles de transport et les priorités négatives pour les utilisateurs
  валидации до gossip рассылки annonces.
- Enveloppes d'admission сравнивают proposition/annonces по диапазонным метаданным через
  `compare_core_fields`, les charges utiles Gossip ne sont pas disponibles.

La région est actuellement en contact avec
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Instruments et luminaires

- Les charges utiles fournies par les fournisseurs incluent `range_capability`, `stream_budget`
  et `transport_hints`. Vérifiez les informations `/v1/sorafs/providers` et les conditions d'admission ;
  JSON-резюме должны включать разобранную capacité, flux de budget et de nombreux conseils
  для телеметрического ingérer.
- `cargo xtask sorafs-admission-fixtures` vous permet de consulter les budgets de flux et les conseils de transport
  Dans le cadre des artefacts JSON, les tableaux de bord offrent des fonctionnalités avancées.
- Les luminaires du `fixtures/sorafs_manifest/provider_admission/` sont disponibles :
  - annonces canoniques мульти-источниковые,
  - `multi_fetch_plan.json`, le SDK peut vous aider à effectuer des recherches
    plan de récupération multi-pairs.

## Intégration de l'opérateur et Torii- Torii `/v1/sorafs/providers` возвращает метаданные диапазонных возможностей
  Il s'agit de `stream_budget` et `transport_hints`. Le déclassement préalable est prévu, alors
  Les fournisseurs proposent de nouvelles métadonnées, des points de terminaison de plage indiquant votre organisation
  pour les clients.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяет лимиты диапазона,
  Выравнивание возможностей и stream budgets при распределении работы. Unité-tests покрывают
  En utilisant un gros morceau, vous pouvez rechercher et étrangler.
- `sorafs_car::multi_fetch` avant la mise à niveau des signaux (ошибки выравнивания,
  запросы étranglés), чтобы операторы могли понимать, почему конкретные провайдеры
  были пропущены при планировании.

## Справочник телеметрии

Récupération de plage d'instruments dans Torii sur le tableau de bord Grafana **SoraFS Récupération d'observabilité**
(`dashboards/grafana/sorafs_fetch_observability.json`) et réponses aux alertes
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrique | Astuce | Metki | Description |
|---------|-----|-------|--------------|
| `torii_sorafs_provider_range_capability_total` | Jauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Le fournisseur a vérifié les fonctions de dialogue. |
| `torii_sorafs_range_fetch_throttle_events_total` | Compteur | `reason` (`quota`, `concurrency`, `byte_rate`) | Les gammes populaires sont récupérées avec la limitation, associées à la politique. |
| `torii_sorafs_range_fetch_concurrency_current` | Jauge | — | Les prises de courant actives peuvent être utilisées pour obtenir une conversation. |

Exemples de PromQL :

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilisez la limitation progressive pour pouvoir activer la synchronisation avant l'activation.
Des fournisseurs d'usines multi-industriels et des alertes de convocation
Le budget de flux est fixé au maximum pour vos flottilles.