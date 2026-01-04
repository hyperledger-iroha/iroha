---
lang: fr
direction: ltr
source: docs/source/testing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7d9bce40727d178bcc7d780c608d82bcd14b0814a7b537cbe9c39a539a200c8
source_last_modified: "2025-12-19T22:31:17.718007+00:00"
translation_last_reviewed: 2026-01-01
---

# Guide de tests et depannage

Ce guide explique comment reproduire des scenarios d integration, quelle infrastructure doit etre en ligne, et comment collecter des logs exploitables. Reportez-vous au [rapport de statut](../../status.md) du projet avant de commencer afin de savoir quels composants sont actuellement au vert.

## Etapes de reproduction

### Tests d integration (`integration_tests` crate)

1. Assurez-vous que les dependances du workspace sont construites: `cargo build --workspace`.
2. Lancez la suite de tests d integration avec logs complets: `cargo test -p integration_tests -- --nocapture`.
3. Si vous devez relancer un scenario specifique, utilisez son chemin de module, par ex. `cargo test -p integration_tests settlement::happy_path -- --nocapture`.
4. Capturez des fixtures serialisees avec Norito afin de garantir des entrees coherentes entre noeuds:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "admin@test",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   Stockez le Norito JSON resultant a cote des artefacts de test afin que les peers puissent rejouer le meme etat.

### Tests du client Python (`pytests` directory)

1. Installez les dependances Python avec `pip install -r pytests/requirements.txt` dans un environnement virtuel.
2. Exportez les fixtures au format Norito generees ci-dessus via un chemin partage ou une variable d environnement.
3. Lancez la suite avec sortie verbeuse: `pytest -vv pytests`.
4. Pour un debogage cible, lancez `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`.

## Ports et services requis

Les services suivants doivent etre accessibles avant d executer l une ou l autre suite:

- **API HTTP Torii**: par defaut `127.0.0.1:1337`. Remplacez via `torii.address` dans votre config (voir `docs/source/references/peer.template.toml`).
- **Notifications WebSocket Torii**: par defaut `127.0.0.1:8080` pour les abonnements clients utilises par `pytests`.
- **Exportateur de telemetrie**: par defaut `127.0.0.1:8180`. Les tests d integration attendent que les metriques arrivent ici pour les verifications de sante.
- **PostgreSQL** (lorsqu il est active): par defaut `127.0.0.1:5432`. Assurez-vous que les identifiants sont alignes avec le profil compose dans [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml).

Consultez le [guide de depannage telemetrie](telemetry.md) si un endpoint est indisponible.

### Stabilite des peers embarques

`NetworkBuilder::start()` impose maintenant une fenetre de liveness de cinq secondes apres genesis pour chaque peer embarque. Si un processus se termine pendant cette periode de garde, le builder annule avec une erreur detaillee qui pointe vers les logs stdout/stderr en cache. Sur les machines a ressources limitees, vous pouvez etendre la fenetre (en millisecondes) en definissant `IROHA_TEST_POST_GENESIS_LIVENESS_MS`; la baisser a `0` desactive totalement le guard. Assurez-vous que votre environnement laisse assez de marge CPU pendant les premieres secondes de chaque suite d integration pour que les peers atteignent le bloc 1 sans declencher le watchdog.

## Collecte et analyse des logs

Demarrez depuis un repertoire d execution propre pour que les artefacts precedents ne masquent pas de nouveaux problemes. Les scripts ci-dessous collectent des logs dans des formats que les outils Norito en aval peuvent consommer.

- Utilisez [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) apres l execution des tests pour agreger les metriques des noeuds en snapshots Norito JSON horodates.
- Lors d une investigation reseau, lancez [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) pour diffuser des evenements Torii vers `monitor_output.norito.json`.
- Les logs des tests d integration sont stockes sous `integration_tests/target/`; compressez-les avec [`scripts/profile_build.sh`](../../scripts/profile_build.sh) pour les partager avec d autres equipes.
- Les logs du client Python sont ecrits dans `pytests/.pytest_cache`. Exportez-les avec la telemetrie capturee via:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

Constituez un bundle complet (integration, Python, telemetrie) avant d ouvrir un issue afin que les maintainers puissent rejouer les traces Norito.

## Prochaines etapes

Pour les checklists propres a une release, voir [pipeline](pipeline.md). Si vous decouvrez des regressions ou des echecs, documentez-les dans le [status tracker](../../status.md) partage et faites reference a toute entree pertinente de [depannage sumeragi](sumeragi.md).
