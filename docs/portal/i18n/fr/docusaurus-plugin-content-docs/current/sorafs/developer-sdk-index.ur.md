---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-index
titre : Guides du SDK SoraFS
sidebar_label : guides du SDK
description : les artefacts SoraFS intègrent des extraits de code différents
---

:::note مستند ماخذ
:::

Le hub comprend la chaîne d'outils SoraFS et la piste des assistants linguistiques.
Rust مخصوص snippets کے لیے [Rust SDK snippets](./developer-sdk-rust.md) ici

## Aides linguistiques- **Python** — `sorafs_multi_fetch_local` (tests de fumée de l'orchestrateur local)
  `sorafs_gateway_fetch` (exercices de passerelle E2E) en option `telemetry_region` en option
  `transport_policy` override قبول کرتے ہیں
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), avec les boutons de déploiement CLI et les boutons de déploiement CLI.
  Il s'agit d'un proxy QUIC local et d'un manifeste de navigateur `sorafs_gateway_fetch`.
  `local_proxy_manifest` teste le bundle de confiance et les adaptateurs de navigateur
  پہنچا سکیں۔
- **JavaScript** — `sorafsMultiFetchLocal` Assistant Python et miroir avec octets de charge utile
  Pour les résumés de reçus, pour les passerelles `sorafsGatewayFetch` Torii et pour les exercices
  les manifestes de proxy local et les threads de discussion et les remplacements de télémétrie/transport de la CLI exposent les problèmes
- **Rust** — le planificateur de services est utilisé pour `sorafs_car::multi_fetch` et est intégré dans la version `sorafs_car::multi_fetch`.
  aides de flux de preuve et intégration d'orchestrateur ici [Extraits de SDK Rust] (./developer-sdk-rust.md) ici
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii Réutilisation de l'exécuteur HTTP
  `GatewayFetchOptions` et honneur ہے۔ Voir `ClientConfig.Builder#setSorafsGatewayUri` ici
  Indice de téléchargement PQ (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) pour combiner les téléchargements
  Les chemins réservés au PQ uniquement

## Tableau de bord et boutons de politique

Aides Python (`sorafs_multi_fetch_local`) et JavaScript (`sorafsMultiFetchLocal`) CLI
Le tableau de bord du planificateur prenant en charge la télémétrie expose les éléments suivants :- Le tableau de bord des binaires de production est activé par défaut et activé. replay des rencontres کرتے وقت
  `use_scoreboard=True` (entrées `telemetry`) avec métadonnées d'annonce d'assistance et télémétrie récente
  instantanés et ordre pondéré des fournisseurs dériver
- `return_scoreboard=True` définit les reçus de blocs de poids calculés et les journaux CI
  capture de diagnostics
- Les tableaux `deny_providers` et `boost_providers` rejettent les pairs rejetés et `priority_delta`
  ajouter ہو جب fournisseurs de planificateurs sélectionnez کرے۔
- Posture `"soranet-first"` par défaut pour l'étape de déclassement maintenant `"direct-only"` صرف تبدیں
  Dans la région de conformité et les relais, il y a la répétition de secours SNNet-5a et `"soranet-strict"`
  Pour les projets pilotes PQ uniquement pour l'approbation de la gouvernance et pour la réserve
- Les assistants de passerelle `scoreboardOutPath` et `scoreboardNowUnixSecs` exposent les utilisateurs. `scoreboardOutPath`
  définir la persistance du tableau de bord calculé (indicateur CLI `--scoreboard-out`)
  Les artefacts du SDK `cargo xtask sorafs-adoption-check` valident la fonction `scoreboardNowUnixSecs`.
  luminaires et métadonnées reproductibles et valeur `assume_now` stable. Assistant JavaScript ici
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` pour définir un ensemble de fonctions اگر étiquette omettre ہو تو
  Et `region:<telemetryRegion>` dérivent کرتا ہے (repli `sdk:js`). Python helper pour le tableau de bord persiste
  `telemetry_source="sdk:python"` est en train d'émettre des métadonnées implicites et désactivées.```python
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