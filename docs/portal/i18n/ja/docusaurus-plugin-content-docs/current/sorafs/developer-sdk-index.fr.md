---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: developer-sdk-index
title: Guides SDK SoraFS
sidebar_label: Guides SDK
description: Extraits par langage pour intégrer les artefacts SoraFS.
---

:::note Source canonique
:::

Utilisez ce hub pour suivre les helpers par langage livrés avec la toolchain SoraFS.
Pour les snippets Rust, allez à [Rust SDK snippets](./developer-sdk-rust.md).

## Helpers par langage

- **Python** — `sorafs_multi_fetch_local` (tests smoke de l'orchestrateur local) et
  `sorafs_gateway_fetch` (exercices E2E de gateway) acceptent désormais un
  `telemetry_region` optionnel plus un override de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), en miroir des knobs de
  rollout du CLI. Lorsqu'un proxy QUIC local démarre,
  `sorafs_gateway_fetch` renvoie le manifest navigateur via
  `local_proxy_manifest` afin que les tests transmettent le trust bundle aux
  adaptateurs navigateur.
- **JavaScript** — `sorafsMultiFetchLocal` reflète le helper Python, renvoyant les
  bytes de payload et les résumés de reçus, tandis que `sorafsGatewayFetch` exerce
  les gateways Torii, passe les manifests du proxy local et expose les mêmes
  overrides télémétrie/transport que le CLI.
- **Rust** — les services peuvent embarquer le scheduler directement via
  `sorafs_car::multi_fetch` ; consultez la référence
  [Rust SDK snippets](./developer-sdk-rust.md) pour les helpers proof-stream et
  l'intégration de l'orchestrateur.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` réutilise l'exécuteur HTTP
  Torii et respecte `GatewayFetchOptions`. Combinez-le avec
  `ClientConfig.Builder#setSorafsGatewayUri` et l'indice d'upload PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) lorsque les uploads doivent
  rester sur des chemins PQ-only.

## Scoreboard et knobs de politique

Les helpers Python (`sorafs_multi_fetch_local`) et JavaScript
(`sorafsMultiFetchLocal`) exposent le scoreboard du scheduler basé télémétrie utilisé
par le CLI :

- Les binaires de production activent le scoreboard par défaut ; définissez
  `use_scoreboard=True` (ou fournissez des entrées `telemetry`) lors du replay des
  fixtures afin que le helper dérive l'ordre pondéré des providers à partir des
  métadonnées d'advert et des snapshots de télémétrie récents.
- Définissez `return_scoreboard=True` pour recevoir les poids calculés avec les
  reçus de chunk afin que les logs CI capturent les diagnostics.
- Utilisez les tableaux `deny_providers` ou `boost_providers` pour rejeter des peers
  ou ajouter un `priority_delta` quand le scheduler sélectionne des providers.
- Conservez la posture par défaut `"soranet-first"` sauf en cas de downgrade ; fournissez
  `"direct-only"` seulement lorsqu'une région de conformité doit éviter les relays ou
  lors d'une répétition du fallback SNNet-5a, et réservez `"soranet-strict"` aux pilotes
  PQ-only avec approbation de gouvernance.
- Les helpers gateway exposent aussi `scoreboardOutPath` et `scoreboardNowUnixSecs`.
  Définissez `scoreboardOutPath` pour persister le scoreboard calculé (miroir du flag
  CLI `--scoreboard-out`) afin que `cargo xtask sorafs-adoption-check` valide les artefacts
  SDK, et utilisez `scoreboardNowUnixSecs` quand les fixtures ont besoin d'une valeur
  `assume_now` stable pour des métadonnées reproductibles. Dans le helper JavaScript,
  vous pouvez aussi définir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  si le label est omis, il dérive `region:<telemetryRegion>` (avec fallback vers `sdk:js`).
  Le helper Python émet automatiquement `telemetry_source="sdk:python"` chaque fois qu'il
  persiste un scoreboard et garde les métadonnées implicites désactivées.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```
