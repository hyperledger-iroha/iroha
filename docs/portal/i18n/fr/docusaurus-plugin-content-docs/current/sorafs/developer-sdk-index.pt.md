---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-index
titre : Guides du SDK pour SoraFS
sidebar_label : Guides du SDK
description : Trechos por linguagem para integrar artefatos da SoraFS.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas comme copies synchronisées.
:::

Utilisez ce hub pour accompagner les helpers dans le langage qui accompagne une chaîne d'outils de SoraFS.
Pour les extraits spécifiques de Rust, allez à [Rust SDK snippets](./developer-sdk-rust.md).

## Aides linguistiques- **Python** - `sorafs_multi_fetch_local` (les tests de fumée sont effectués par un orquestrador local) et
  `sorafs_gateway_fetch` (exercices E2E de la passerelle) maintenant pour `telemetry_region`
  optionnel mais un remplacement de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), utilisez les boutons de
  déploiement en CLI. Lorsqu'un proxy QUIC local est activé, `sorafs_gateway_fetch` renvoie le message
  manifeste du navigateur dans `local_proxy_manifest` pour que vos testicules passent ou trust bundle
  pour les adaptateurs de navigateur.
- **JavaScript** - `sorafsMultiFetchLocal` utilise l'assistant de Python, retourne
  octets de charge utile et résumés de réception, pendant l'exercice `sorafsGatewayFetch`
  passerelles Torii, encadeia manifeste le proxy local et expose les remplacements de mesmos
  de télémétrie/transport do CLI.
- **Rust** - les services peuvent embutir le planificateur directement via
  `sorafs_car::multi_fetch` ; je vois une référence de
  [Extraits du SDK Rust](./developer-sdk-rust.md) pour les aides de proof-stream et
  integracao do orquestrador.
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` réutilise l'exécuteur HTTP
  faire Torii et honorer `GatewayFetchOptions`. Combiner com
  `ClientConfig.Builder#setSorafsGatewayUri` et indice de téléchargement PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quando télécharge precisarem
  ficar em caminhos quelque peu PQ.

## Tableau de bord et boutons politiques

Outils helpers de Python (`sorafs_multi_fetch_local`) et JavaScript
(`sorafsMultiFetchLocal`) exposition du tableau de bord du planificateur avec télémétrie utilisée
sur la CLI :- Binarios de producao habilitam o scoreboard por padrao; définition `use_scoreboard=True`
  (ou forneca entradas `telemetry`) pour reproduire les appareils pour que l'aide puisse dériver
  l'ordre des fournisseurs réfléchit à partir des métadonnées de la publicité et des instantanés
  récentes de télémétrie.
- Définir `return_scoreboard=True` pour recevoir les pesos calculés avec les recettes
  le chunk, permettant aux journaux de CI de capturer des diagnostics.
- Utilisez les tableaux `deny_providers` ou `boost_providers` pour rejeter des pairs ou ajouter
  `priority_delta` lorsque le planificateur sélectionne les fournisseurs.
- Mantenha a postura padrao `"soranet-first"` pour moins que vous ne prépariez un déclassement ;
  Forneca `"direct-only"` indique quand une région de conformité précise éviter les relais
  ou pour utiliser le SNNet-5a de secours, et la réserve `"soranet-strict"` pour les pilotes PQ uniquement
  com aprovacao de gouvernance.
- Helpers de gateway également expoem `scoreboardOutPath` et `scoreboardNowUnixSecs`.
  Définir `scoreboardOutPath` pour persister le tableau de bord calculé (afficher le drapeau
  `--scoreboard-out` do CLI) pour que `cargo xtask sorafs-adoption-check` valide
  artefatos du SDK, et utilisez `scoreboardNowUnixSecs` lorsque les appareils sont précis
  valeur `assume_now` estavel para métadados reproduziveis. Pas d'aide de JavaScript,
  vous pouvez également définir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  Lorsque l'étiquette est omise, elle dérive `region:<telemetryRegion>` (repli pour `sdk:js`).L'assistant de Python émet automatiquement `telemetry_source="sdk:python"` quand
  persister dans un tableau de bord et des métadonnées implicites désactivées.

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