---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-11-17T18:34:13.669847+00:00"
translation_last_reviewed: 2026-01-01
---

# Calendrier d'adoption Norito-RPC

> Les notes de planification canoniques se trouvent dans `docs/source/torii/norito_rpc_adoption_schedule.md`.  
> Cette copie du portail resume les attentes de deploiement pour les auteurs SDK, les operateurs et les relecteurs.

## Objectifs

- Aligner chaque SDK (Rust CLI, Python, JavaScript, Swift, Android) sur le transport binaire Norito-RPC avant le basculement de production AND4.
- Garder deterministes les portes de phase, les bundles de preuve et les hooks de telemetrie afin que la gouvernance puisse auditer le deploiement.
- Rendre triviale la capture de preuves de fixtures et de canary avec les helpers partages mentionnes par la feuille de route NRPC-4.

## Chronologie des phases

| Phase | Fenetre | Portee | Criteres de sortie |
|-------|---------|--------|--------------------|
| **P0 - Parite labo** | Q2 2025 | Les suites smoke Rust CLI + Python executent `/v1/norito-rpc` en CI, le helper JS passe les tests unitaires, le harness mock Android exerce les deux transports. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` et `javascript/iroha_js/test/noritoRpcClient.test.js` verts en CI; le harness Android branche sur `./gradlew test`. |
| **P1 - Apercu SDK** | Q3 2025 | Le bundle de fixtures partage est commite, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` enregistre logs + JSON dans `artifacts/norito_rpc/`, et des flags de transport Norito optionnels sont exposes dans les samples SDK. | Manifeste de fixtures signe, mises a jour README montrent l'usage opt-in, l'API d'apercu Swift disponible derriere le flag IOS2. |
| **P2 - Staging / apercu AND4** | Q1 2026 | Les pools Torii de staging preferent Norito, les clients AND4 preview Android et les suites de parite IOS2 Swift utilisent par defaut le transport binaire, le dashboard de telemetrie `dashboards/grafana/torii_norito_rpc_observability.json` renseigne. | `docs/source/torii/norito_rpc_stage_reports.md` capture le canary, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` passe, le replay du harness mock Android capture les cas de succes/erreur. |
| **P3 - GA production** | Q4 2026 | Norito devient le transport par defaut pour tous les SDK; JSON reste un fallback brownout. Les jobs de release archivent les artefacts de parite avec chaque tag. | La checklist de release regroupe les sorties smoke Norito pour Rust/JS/Python/Swift/Android; les seuils d'alerte pour les SLOs de taux d'erreur Norito vs JSON sont appliques; `status.md` et les notes de release citent l'evidence GA. |

## Livrables SDK et hooks CI

- **Rust CLI et harness d'integration** - etendre les tests smoke `iroha_cli pipeline` pour forcer le transport Norito une fois que `cargo xtask norito-rpc-verify` est disponible. Proteger avec `cargo test -p integration_tests -- norito_streaming` (lab) et `cargo xtask norito-rpc-verify` (staging/GA), en stockant les artefacts sous `artifacts/norito_rpc/`.
- **SDK Python** - mettre le smoke de release (`python/iroha_python/scripts/release_smoke.sh`) par defaut sur Norito RPC, garder `run_norito_rpc_smoke.sh` comme entree CI, et documenter la parite dans `python/iroha_python/README.md`. Cible CI: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **SDK JavaScript** - stabiliser `NoritoRpcClient`, laisser les helpers de gouvernance/query par defaut sur Norito lorsque `toriiClientConfig.transport.preferred === "norito_rpc"`, et capturer des samples end-to-end dans `javascript/iroha_js/recipes/`. CI doit lancer `npm test` plus le job dockerise `npm run test:norito-rpc` avant publication; provenance charge les logs smoke Norito dans `javascript/iroha_js/artifacts/`.
- **SDK Swift** - brancher le transport Norito bridge derriere le flag IOS2, caler la cadence des fixtures, et garantir que la suite de parite Connect/Norito tourne dans les lanes Buildkite references dans `docs/source/sdk/swift/index.md`.
- **SDK Android** - les clients AND4 preview et le harness mock Torii adoptent Norito, avec la telemetrie retry/backoff documentee dans `docs/source/sdk/android/networking.md`. Le harness partage les fixtures avec les autres SDK via `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## Preuves et automatisation

- `scripts/run_norito_rpc_fixtures.sh` encapsule `cargo xtask norito-rpc-verify`, capture stdout/stderr, et emet `fixtures.<sdk>.summary.json` pour que les owners SDK aient un artefact deterministe a attacher a `status.md`. Utilisez `--sdk <label>` et `--out artifacts/norito_rpc/<stamp>/` pour garder les bundles CI propres.
- `cargo xtask norito-rpc-verify` impose la parite de hash de schema (`fixtures/norito_rpc/schema_hashes.json`) et echoue si Torii renvoie `X-Iroha-Error-Code: schema_mismatch`. Associez chaque echec a une capture de fallback JSON pour le debug.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` et `dashboards/grafana/torii_norito_rpc_observability.json` definissent les contrats d'alerte pour NRPC-2. Lancez le script apres chaque edition du dashboard et stockez la sortie `promtool` dans le bundle canary.
- `docs/source/runbooks/torii_norito_rpc_canary.md` decrit les drills staging et production; mettez-le a jour lorsque les hashes de fixtures ou les gates d'alerte changent.

## Checklist pour relecteurs

Avant de cocher un jalon NRPC-4, confirmez:

1. Les hashes du bundle de fixtures le plus recent correspondent a `fixtures/norito_rpc/schema_hashes.json` et l'artefact CI correspondant est consigne sous `artifacts/norito_rpc/<stamp>/`.
2. Les README SDK et les docs du portail expliquent comment forcer le fallback JSON et citent le transport Norito par defaut.
3. Les dashboards de telemetrie montrent des panneaux de taux d'erreur dual-stack avec liens d'alerte, et le dry run Alertmanager (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) est joint au tracker.
4. Le calendrier d'adoption ici correspond a l'entree du tracker (`docs/source/torii/norito_rpc_tracker.md`) et la feuille de route (NRPC-4) reference le meme bundle de preuve.

Rester discipline sur le calendrier garde le comportement cross-SDK previsible et permet a la gouvernance d'auditer l'adoption Norito-RPC sans demandes sur mesure.
