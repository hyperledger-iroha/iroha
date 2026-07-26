---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-index
כותרת: Guides SDK SoraFS
sidebar_label: Guides SDK
תיאור: Extraits par langage pour intégrer les artefacts SoraFS.
---

:::הערה מקור קנוניק
:::

Utilisez ce hub pour suivre les helpers par langage livrés avec la toolchain SoraFS.
Pour les snippets Rust, allez à [Rust SDK snippets](./developer-sdk-rust.md).

## עוזרים לפי שפה

- **Python** — `sorafs_multi_fetch_local` (מבחני עשן de l'orchestrator local) et
  `sorafs_gateway_fetch` (תרגילים E2E de gateway) acceptent désormais un
  `telemetry_region` אופציונלי פלוס un override de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` או `"direct-only"`), en miroir des knobs de
  השקת du CLI. Lorsqu'un proxy QUIC local démarre,
  `sorafs_gateway_fetch` renvoie le manifest navigateur via
  `local_proxy_manifest` afin que les tests transmettent le trust bundle aux
  מתאמים לנווט.
- **JavaScript** — `sorafsMultiFetchLocal` רפלטה לעוזר פייתון, בעל מקצוע
  bytes de payload et les résumés de reçus, tandis que `sorafsGatewayFetch` תרגיל
  les gateways Torii, מעבר למניפסטים du proxy local et expose les mêmes
  עוקף את télémétrie/transport que le CLI.
- **חלודה** — les services peuvent embarquer le scheduler direction via
  `sorafs_car::multi_fetch` ; consultez la référence
  [Rust SDK snippets](./developer-sdk-rust.md) pour les helpers proof-stream et
  l'integration de l'orchestrateur.
- **אנדרואיד** — `HttpClientTransport.sorafsGatewayFetch(…)` réutilise l'exécuteur HTTP
  Torii וכבוד `GatewayFetchOptions`. Combinez-le avec
  `ClientConfig.Builder#setSorafsGatewayUri` et l'indice d'upload PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) lorsque les uploads doivent
  rester sur des chemins PQ בלבד.

## לוח תוצאות וידיות פוליטיות

Les helpers Python (`sorafs_multi_fetch_local`) et JavaScript
(`sorafsMultiFetchLocal`) חשוף ללוח התוצאות של לוח הזמנים הבסיסי של מכשיר השימוש
par le CLI:- Les binaires de production activent le לוח התוצאות par défaut; définissez
  `use_scoreboard=True` (ou fournissez des entrées `telemetry`) lors du replay des
  מתקנים afin que le helper dérive l'ordre pondéré des providers à partir des
  מדדי פרסום ותמונות בזק של טלמטרים אחרונים.
- Définissez `return_scoreboard=True` pour recevoir les poids calculés avec les
  reçus de chunk efin que les logs CI capturent les diagnostics.
- Utilisez les tableaux `deny_providers` או `boost_providers` pour rejeter des peers
  ou ajouter un `priority_delta` quand le scheduler sélectionne des providers.
- Conservez la posture par défaut `"soranet-first"` sauf en cas de downgrade ; פורניסז
  `"direct-only"` seulement lorsqu'une région de conformité doit éviter les relays ou
  lors d'une répétition du fallback SNNet-5a, et réservez `"soranet-strict"` aux pilotes
  PQ-only עם אישור ממשלתי.
- Les helpers gateway exposent aussi `scoreboardOutPath` et `scoreboardNowUnixSecs`.
  Définissez `scoreboardOutPath` pour persister le scoreboard calculé (miroir du flag
  CLI `--scoreboard-out`) afin que `cargo xtask sorafs-adoption-check` valide les artefacts
  SDK, et utilisez `scoreboardNowUnixSecs` quand les fixtures on besoin d'une valeur
  `assume_now` יציב pour des métadonnées reproductibles. Dans le helper JavaScript,
  vous pouvez aussi définir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  si le label est omis, il dérive `region:<telemetryRegion>` (avec fallback vers `sdk:js`).
  Le helper Python émet automatiquement `telemetry_source="sdk:python"` chaque fois qu'il
  ממשיך ולוח תוצאות ו-garde les métadonnées implicites désactivées.

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