---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-index
titre : Guides du SDK de SoraFS
sidebar_label : Guides du SDK
description : Fragmentos específicos por lenguaje para integrar artefactos de SoraFS.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/sdk/index.md`. Mantén est une copie synchronisée.
:::

Utilisez ce hub pour suivre les assistants dans la langue utilisée avec la chaîne d'outils SoraFS.
Pour les extraits spécifiques de Rust et les [Extraits du SDK Rust] (./developer-sdk-rust.md).

## Aides en langue- **Python** — `sorafs_multi_fetch_local` (tests de l'humo del orquestador local) et
  `sorafs_gateway_fetch` (appareils E2E de la passerelle) maintenant j'accepte un `telemetry_region`
  en option, plus un remplacement de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), réfléchissez aux boutons de
  déploiement de la CLI. Lorsque vous utilisez un proxy QUIC local,
  `sorafs_gateway_fetch` développer le manifeste du navigateur en
  `local_proxy_manifest` pour que les tests entrent le bundle de confiance avec les adaptateurs
  du navigateur.
- **JavaScript** — `sorafsMultiFetchLocal` reflète l'aide de Python, en cours de développement
  octets de charge utile et résumés de réception, pendant l'exécution de `sorafsGatewayFetch`
  passerelles de Torii, l'encadena manifeste le proxy local et expose les mêmes remplacements
  de telemetría/transporte que el CLI.
- **Rust** — les services peuvent incruster le planificateur directement via
  `sorafs_car::multi_fetch` ; consulter la référence de
  [Rust SDK snippets](./developer-sdk-rust.md) pour les aides au proof-stream et à l'intégration
  de l'orchestre.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` réutilise l'éjecteur HTTP
  de Torii et respect `GatewayFetchOptions`. Combiné avec
  `ClientConfig.Builder#setSorafsGatewayUri` et l'indice de subvention PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quand les subidas deban ceñirse
  une route solo PQ.

## Tableau de bord et boutons politiques

Les assistants de Python (`sorafs_multi_fetch_local`) et JavaScript
(`sorafsMultiFetchLocal`) expose le tableau de bord du planificateur avec télémétrie utilisée
par la CLI :- Les binaires de production autorisent le tableau de bord par défaut ; établissement
  `use_scoreboard=True` (ou proporciona entradas de `telemetry`) à la reproduction
  luminaires pour que l'assistant dérive l'ordre pondéré des fournisseurs à partir de
  métadonnées de publicités et instantanés de télémétrie récents.
- Establece `return_scoreboard=True` pour recevoir les pesos calculés ensemble
  recevoir des morceaux et permettre aux journaux de diagnostic CI capturés.
- Utilisez les paramètres `deny_providers` ou `boost_providers` pour rechercher des pairs ou ajouter un
  `priority_delta` lorsque le planificateur sélectionne les fournisseurs.
- Mantén la postura predeterminada `"soranet-first"` salvo qui prépare un downgrade ;
  fournir `"direct-only"` seulement quand une région de conformité doit éviter les relais
  o pour tester le SNNet-5a de secours, et réserver `"soranet-strict"` pour les pilotes PQ uniquement
  avec l'approbation de la gouvernance.
- Les assistants de la passerelle exposent également `scoreboardOutPath` et `scoreboardNowUnixSecs`.
  Configurer `scoreboardOutPath` pour conserver le tableau de bord calculé (refléter le drapeau
  `--scoreboard-out` del CLI) pour que `cargo xtask sorafs-adoption-check` valide les artefacts
  du SDK, et utilisez `scoreboardNowUnixSecs` lorsque les appareils nécessitent une valeur stable de
  `assume_now` pour métadonnées reproductibles. L'assistant JavaScript peut également être utilisé
  établisseur `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ; quand il est omis
  l'étiquette dérive `region:<telemetryRegion>` (avec repli sur `sdk:js`). L'assistant dePython émet automatiquement `telemetry_source="sdk:python"` lorsque vous conservez un tableau de bord
  et maintenir les métadonnées implicites déshabilitées.

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