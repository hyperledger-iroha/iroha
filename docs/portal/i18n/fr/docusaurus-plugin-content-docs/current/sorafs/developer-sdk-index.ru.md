---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-index
titre : Recherche sur le SDK SoraFS
sidebar_label : Rechercher dans le SDK
description : Extraits de extraits d'art d'intégration SoraFS.
---

:::note Канонический источник
:::

Utilisez-le pour utiliser les aides linguistiques, en postant avec la chaîne d'outils SoraFS.
Pour les extraits de code spécifiques à Rust, passez à [Extraits de SDK Rust] (./developer-sdk-rust.md).

## Aides Языковые- **Python** — `sorafs_multi_fetch_local` (tests de fumée pour un opérateur local) et
  `sorafs_gateway_fetch` (gestion de la passerelle E2E) est disponible en option
  `telemetry_region` et remplacement pour `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), autres boutons de déploiement
  CLI. Lorsque le proxy QUIC local est disponible, `sorafs_gateway_fetch` est utilisé
  Manifeste du navigateur dans `local_proxy_manifest`, les tests peuvent être effectués sur le bundle de confiance
  браузерным адаптерам.
- **JavaScript** — `sorafsMultiFetchLocal` fournit l'assistant Python, augmente les octets de charge utile
  et résumés квитанций, тогда как `sorafsGatewayFetch` упражняет Torii passerelles,
  Procéder au proxy local et activer les remplacements de télémétrie/transport,
  c'est et CLI.
- **Rust** — Les services peuvent configurer le planificateur à partir de `sorafs_car::multi_fetch` ;
  см. [Extraits du SDK Rust](./developer-sdk-rust.md) pour les aides et les intégrations de flux de preuve
  orchestre.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` utilise l'exécuteur HTTP Torii
  et учитывает `GatewayFetchOptions`. Combinez avec
  `ClientConfig.Builder#setSorafsGatewayUri` et indice de téléchargement PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) Les fichiers correspondants sont uniquement destinés au PQ.

## Tableau de bord et boutons de politique

Aides Python (`sorafs_multi_fetch_local`) et JavaScript (`sorafsMultiFetchLocal`)
Vous pouvez créer un tableau de bord du planificateur prenant en charge la télémétrie, en utilisant la CLI :- Production бинари включают tableau de bord по умолчанию; utiliser `use_scoreboard=True`
  (ou avant les entrées `telemetry`) lors de la programmation des appareils, cette aide vous a été envoyée
  Plusieurs fournisseurs proposent des publicités de métadonnées et de nombreux instantanés de télémétrie.
- Installez `return_scoreboard=True` pour pouvoir partager vos reçus en morceaux,
  позволяя CI logam фиксировать диагностику.
- Utilisez massivement `deny_providers` ou `boost_providers` pour l'ouverture des pairs ou la connexion
  `priority_delta`, votre planificateur sélectionne les fournisseurs.
- Vérifiez la position `"soranet-first"` en cas de déclassement, si vous ne souhaitez pas rétrograder ;
  Achetez `"direct-only"` pour la région de conformité de votre région en installant des relais ou par
  Répétez le SNNet-5a de secours et réinitialisez `"soranet-strict"` pour les pilotes PQ uniquement.
  одобрением gouvernance.
- Les assistants de passerelle incluent `scoreboardOutPath` et `scoreboardNowUnixSecs`.
  Choisissez `scoreboardOutPath` pour la sécurité de votre tableau de bord (avec le drapeau
  CLI `--scoreboard-out`), que `cargo xtask sorafs-adoption-check` peut valider
  Les éléments du SDK et l'utilisation du `scoreboardNowUnixSecs`, ainsi que les appareils utilisés
  Mise à jour stable `assume_now` pour vos métadonnées. L'assistant JavaScript peut être utilisé
  дополнительно установить `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  Si l'étiquette est ouverte, vous devez utiliser `region:<telemetryRegion>` (repli sur `sdk:js`). Assistant PythonMachine automatique `telemetry_source="sdk:python"` pour le tableau de bord et le jeu
  métadonnées implicites выключенными.

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