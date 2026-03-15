---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس پرووائیڈر annonces et اور شیڈولنگ

Il y a plus de spécifications canoniques que de spécifications canoniques :
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Schémas Norito et journaux des modifications sont également disponibles. portail کی کاپی
Le SDK est disponible pour les runbooks et les runbooks رکھتی ہے۔

## Norito schéma en cours

### Capacité de portée (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – La valeur de la plage est la suivante (octets) et `>= 1`.
- `min_granularity` – recherchez ریزولوشن، `1 <= قدر <= max_chunk_span`.
- `supports_sparse_offsets` – Les compensations pour les compensations et les compensations
- `requires_alignment` – vrai et décalages et `min_granularity` pour aligner les valeurs.
- `supports_merkle_proof` – PoR témoin کی سپورٹ ظاہر کرتا ہے۔

Codage canonique `ProviderCapabilityRangeV1::to_bytes` / `from_bytes`
تاکہ charges utiles de potins déterministes رہیں۔

### `StreamBudgetV1`
- Nom : `max_in_flight`, `max_bytes_per_sec`, `burst_bytes`.
- Règles de validation (`StreamBudgetV1::validate`) :
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` et `> 0` et `<= max_bytes_per_sec` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Type : `protocol: TransportProtocol`, `priority: u8` (0-15 °C
  `TransportHintV1::validate` نافذ کرتا ہے).
- Protocoles suivants : `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Les entrées de protocole du fournisseur et les entrées de protocole

### `ProviderAdvertBodyV1` میں اضافے
- اختیاری `stream_budget: Option<StreamBudgetV1>`.
- اختیاری `transport_hints: Option<Vec<TransportHintV1>>`.
- Il s'agit d'une enveloppe de gouvernance `ProviderAdmissionProposalV1`, d'appareils CLI et d'un JSON télémétrique pour une utilisation en ligne.

## Validation et liaison de gouvernance

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
Les métadonnées associées sont les suivantes :

- Capacités de portée et de décodage et limites de portée/granularité pour les utilisateurs
- Budgets de flux / conseils de transport par `CapabilityType::ChunkRangeFetch` TLV et liste d'indices non vides par ici
- Les protocoles de transport et les priorités des potins ainsi que les erreurs de validation et les erreurs de validation.
- Enveloppes d'admission `compare_core_fields` pour les propositions/annonces et les métadonnées de plage et pour comparer les charges utiles des potins qui ne correspondent pas aux charges utiles.

Couverture de régression یہاں موجود ہے :
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Outillage et montages

- Charges utiles des annonces du fournisseur telles que `range_capability`, `stream_budget`, et `transport_hints` pour votre recherche.
  Réponses `/v2/sorafs/providers` pour les rendez-vous d'admission et validation Résumés JSON, capacité analysée, budget de flux, tableaux d'indices et informations sur l'acquisition de télémétrie et l'acquisition de télémétrie.
- `cargo xtask sorafs-admission-fixtures` Les artefacts JSON, les budgets de flux et les conseils de transport ainsi que les tableaux de bord comportent un suivi d'adoption.
- `fixtures/sorafs_manifest/provider_admission/` pour les luminaires de votre choix :
  - Annonces canoniques multi-sources،
  - `multi_fetch_plan.json` SDK suites replay déterministe du plan de récupération multi-peer

## Orchestrator اور Torii انضمام- Torii `/v2/sorafs/providers` métadonnées de capacité de plage analysée ici `stream_budget` et `transport_hints` et `transport_hints`.
  les fournisseurs ont des métadonnées et des avertissements de rétrogradation ainsi que les points de terminaison de la gamme de passerelles pour les clients et les contraintes de sécurité.
- Orchestrateur multi-source (`sorafs_car::multi_fetch`) pour les limites de plage, l'alignement des capacités, les budgets de flux et l'affectation des tâches pour appliquer les règles. Tests unitaires tels que des morceaux trop volumineux, une recherche clairsemée et des scénarios de limitation
- Les signaux de rétrogradation `sorafs_car::multi_fetch` (échecs d'alignement, requêtes limitées) diffusent des flux vers les opérateurs et les fournisseurs de services et sautent ہوئے۔

## Référence de télémétrie

Torii et instrumentation de récupération de portée **SoraFS Récupération d'observabilité** Grafana Tableau de bord
(`dashboards/grafana/sorafs_fetch_observability.json`) Pour plus de règles d'alerte
(`dashboards/alerts/sorafs_fetch_rules.yml`) et alimentation en ligne

| Métrique | Tapez | Étiquettes | Descriptif |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Jauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Les fonctionnalités de capacité de portée annoncent les fournisseurs et les fournisseurs |
| `torii_sorafs_range_fetch_throttle_events_total` | Compteur | `reason` (`quota`, `concurrency`, `byte_rate`) | Politique concernant les tentatives de récupération de plage limitée |
| `torii_sorafs_range_fetch_concurrency_current` | Jauge | — | Budget de simultanéité partagé pour les flux gardés actifs |

Exemples d'extraits PromQL :

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Application des quotas avec compteur de limitation et compteur de limitation pour les valeurs par défaut de l'orchestrateur multi-source et avec la concurrence simultanée Flotte et budget de flux maxima pour les alertes de flotte