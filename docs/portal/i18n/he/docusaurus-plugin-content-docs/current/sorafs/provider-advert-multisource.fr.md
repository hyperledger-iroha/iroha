---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Annonces de fournisseurs multi-source et planification

Cette page condense la specification canonique dans
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
לנצל את המסמך לשפוך סכימות Norito מילולית ורישומי שינוי; la copie du portail
לשמר את המבצעים הנשלחים, את הערות SDK ואת ההתייחסויות לטלמטריה קרובה
des runbooks SoraFS.

## Ajouts au schema Norito

### Capacite de plage (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - בתוספת grande plage contigue (אוקטטים) לפי בקשה, `>= 1`.
- `min_granularity` - רזולוציית חיפוש, `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets` - Permet des offsets non contigus dans une seule requete.
- `requires_alignment` – זה נכון, פחות קיזוזים מתאימים ל-`min_granularity`.
- `supports_merkle_proof` – indique la prize en charge des temoins PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` appliquent un encodage canonique
pour que les payloads de restent deterministes של רכילות.

### `StreamBudgetV1`
- Champs: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` אופציונלי.
- חוקי אימות (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, lorsqu'il est הווה, doit etre `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Champs: `protocol: TransportProtocol`, `priority: u8` (חוד 0-15 אפליקציות ערך
  `TransportHintV1::validate`).
- פרוטוקולים קונוסים: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Les entrees de protocole dupliquees par fournisseur sont rejetees.

### יוצא ל-`ProviderAdvertBodyV1`
- `stream_budget` אופציונלי: `Option<StreamBudgetV1>`.
- `transport_hints` אופציונלי: `Option<Vec<TransportHintV1>>`.
- Les deux champs transitent desormais via `ProviderAdmissionProposalV1`, les enveloppes
  de gouvernance, les fixtures CLI et le JSON telemetrique.

## אימות וקשר א-לה-ממשל

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
rejettent les metadonnees mal formees:

- Les capacites de plage doivent se decoder et respecter les limites de plage/granularite.
- תקציבי זרם / רמזים לתחבורה נחוצים ל-TLV `CapabilityType::ChunkRangeFetch`
  correspondant et une list de hints non vide.
- Les protocoles de transport dupliques et les priorites invalides generent des erreurs
  de validation avant la diffusion des adverts.
- הצעה נלווית/פרסומות של Les enveloppes d'admission pour les metadonnees de plage via
  `compare_core_fields` afin que les payloads de gossip non concordants soient rejetes tot.

La couverture de regression se trouve dans
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Outils et fixtures- Les payloads d'annonces de fournisseurs doivent כוללים les metadonnees `range_capability`,
  `stream_budget` et `transport_hints`. Validez via les reponses `/v1/sorafs/providers` et les
  מתקני כניסה; קורות החיים של JSON אינם כוללים ניתוח קיבול, תקציב זרם
  et les tableaux de hints pour l'ingestion telemetrique.
- `cargo xtask sorafs-admission-fixtures` לחשוף את תקציבי הזרימה ורמזים לתחבורה
  ses artefacts JSON אפיינה את לוחות המחוונים המתאימים לאימוץ של פונקציונליות.
- אביזרי ה-Sous `fixtures/sorafs_manifest/provider_admission/` כוללים desormais:
  - פרסומות מרובות מקורות canoniques,
  - `multi_fetch_plan.json` pour que les suites SDK puissent rejouer un plan de fetch
    דטרמיניסטי מרובה עמיתים.

## אינטגרציה עם תזמורת ו-Torii

- Torii `/v1/sorafs/providers` renvoie les metadonnees de capacite de plage parsees avec
  `stream_budget` et `transport_hints`. Des adertissements de downgrade se declenchent quand
  les fournisseurs omettent la nouvelle metadonnee, et les endpoints de plage du gateway
  appliquent les memes contraintes pour les לקוחות מכוון.
- L'orchestrator multi-source (`sorafs_car::multi_fetch`) applique desormais les limites de
  plage, l'alignement des capacites et les stream budgets lors de l'affection du travail.
  Les tests unitaires couvrent les scenarios de chunk trop grand, de seek disperses et de
  מצערת.
- `sorafs_car::multi_fetch` דירוג לאחור מפוזר (echecs d'alignement,
  מבקש מצר) afin que les operateurs puissent tracer pourquoi certains fournisseurs
  ont ete מתעלם מתליון la planification.

## Reference de telemetrie

L'instrumentation de fetch de plage de Torii alimente le לוח המחוונים Grafana
**SoraFS תצפית אחזור** (`dashboards/grafana/sorafs_fetch_observability.json`) et
les regles d'alerte associees (`dashboards/alerts/sorafs_fetch_rules.yml`).

| מטריק | הקלד | נימוסים | תיאור |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | מד | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Fournisseurs annonçant des fonctionnalites de capacite de plage. |
| `torii_sorafs_range_fetch_throttle_events_total` | מונה | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentatives de fetch de plage bridees par politique. |
| `torii_sorafs_range_fetch_concurrency_current` | מד | — | Streams actifs gardes consommant le budget de concurrence partage. |

דוגמאות של PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilisez le compteur de throttling pour confirmer l'application des quotas avant d'activer
les valeurs par defaut de l'orchestrator multi-source, et alertez quand la concurrence se
rapproche des maxima de budget de streams pour votre flotte.