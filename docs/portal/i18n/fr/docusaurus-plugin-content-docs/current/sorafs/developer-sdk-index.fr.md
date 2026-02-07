---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-index
titre : Guides SDK SoraFS
sidebar_label : SDK Guides
description : Extraits par langage pour intégrer les artefacts SoraFS.
---

:::note Source canonique
:::

Utilisez ce hub pour suivre les helpers par langage livrés avec la toolchain SoraFS.
Pour les snippets Rust, allez à [Rust SDK snippets](./developer-sdk-rust.md).

## Helpers par langage- **Python** — `sorafs_multi_fetch_local` (tests smoke de l'orchestrateur local) et
  `sorafs_gateway_fetch` (exercices E2E de gateway) accepte désormais un
  `telemetry_region` optionnel plus un override de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), en miroir des boutons de
  déploiement de la CLI. Lorsqu'un proxy QUIC démarre localement,
  `sorafs_gateway_fetch` renvoyer le navigateur manifest via
  `local_proxy_manifest` afin que les tests transmettent le trust bundle aux
  adaptateurs navigateurs.
- **JavaScript** — `sorafsMultiFetchLocal` reflète le helper Python, renvoyant les
  bytes de payload et les résumés de reçus, tandis que `sorafsGatewayFetch` exerce
  les gateways Torii, passe les manifestes du proxy local et expose les mêmes
  remplace la télémétrie/transport que le CLI.
- **Rust** — les services peuvent embarquer le planificateur directement via
  `sorafs_car::multi_fetch` ; consulter la référence
  [Rust SDK snippets](./developer-sdk-rust.md) pour les helpers proof-stream et
  l'intégration de l'orchestrateur.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` réutilise l'exécuteur HTTP
  Torii et respecte `GatewayFetchOptions`. Combinez-le avec
  `ClientConfig.Builder#setSorafsGatewayUri` et l'indice de téléchargement PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) lorsque les uploads doivent
  rester sur des chemins PQ seulement.

## Scoreboard et boutons de politiqueLes helpers Python (`sorafs_multi_fetch_local`) et JavaScript
(`sorafsMultiFetchLocal`) expose le tableau de bord du planificateur basé télémétrie utilisé
par CLI :- Les binaires de production activent le scoreboard par défaut ; défini
  `use_scoreboard=True` (ou fournissez des entrées `telemetry`) lors de la relecture des
  luminaires afin que le helper dérive l'ordre pondéré des prestataires à partir des
  métadonnées d'annonce et des instantanés de télémétrie récents.
- Définissez `return_scoreboard=True` pour recevoir les poids calculés avec les
  reçus de chunk afin que les logs CI capturent les diagnostics.
- Utilisez les tableaux `deny_providers` ou `boost_providers` pour rejeter des pairs
  ou ajoutez un `priority_delta` lorsque le planificateur sélectionne des fournisseurs.
- Conservez la posture par défaut `"soranet-first"` sauf en cas de downgrade ; fournissez
  `"direct-only"` seulement lorsqu'une région de conformité doit éviter les relais ou
  lors d'une répétition du repli SNNet-5a, et réservez `"soranet-strict"` aux pilotes
  PQ seulement avec approbation de gouvernance.
- Les helpers gateway exposent également `scoreboardOutPath` et `scoreboardNowUnixSecs`.
  Définissez `scoreboardOutPath` pour persister le scoreboard calculé (miroir du flag
  CLI `--scoreboard-out`) afin que `cargo xtask sorafs-adoption-check` valide les artefacts
  SDK, et utilisez `scoreboardNowUnixSecs` lorsque les appareils ont besoin d'une valeur
  `assume_now` stable pour des métadonnées reproductibles. Dans l'assistant JavaScript,
  vous pouvez également définir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;si le label est omis, il dérive `region:<telemetryRegion>` (avec repli vers `sdk:js`).
  Le helper Python génère automatiquement `telemetry_source="sdk:python"` chaque fois qu'il
  persister un scoreboard et garder les métadonnées implicites désactivées.

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