---
lang: fr
direction: ltr
source: docs/source/nexus_elastic_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0c93bb174622874e22cbc7962759a842095aec14389d601805c2a20632c86958
source_last_modified: "2025-11-21T18:07:10.137018+00:00"
translation_last_reviewed: 2026-01-01
---

# Toolkit de provisioning de lane elastique (NX-7)

> **Element roadmap :** NX-7 - tooling de provisioning de lane elastique  
> **Statut :** Tooling complet - genere des manifests, des catalog snippets, des payloads Norito,
> des smoke tests, et le helper de bundle de load-test assemble maintenant le gating de latence de
> slot + les manifests de preuve pour publier les runs de charge validateur sans scripting ad hoc.

Ce guide accompagne les operateurs dans le helper `scripts/nexus_lane_bootstrap.sh` qui automatise
la generation des manifests de lane, des snippets de catalog lane/dataspace, et des preuves de
rollout. Le but est de rendre simple la creation de nouvelles lanes Nexus (publiques ou privees)
sans editer plusieurs fichiers ni re-deriver la geometrie du catalog a la main.

## 1. Prerequis

1. Approbation de gouvernance pour l'alias de lane, le dataspace, le set de validateurs, la
   tolerance aux fautes (`f`) et la policy de settlement.
2. Une liste finale de validateurs (account IDs) et une liste de namespaces proteges.
3. Acces au repo de configuration des noeuds pour ajouter les snippets generes.
4. Chemins pour le registry des manifests de lane (voir `nexus.registry.manifest_directory` et
   `cache_directory`).
5. Contacts telemetrie/PagerDuty pour la lane afin que les alertes soient cablees au demarrage.

## 2. Generer les artefacts de lane

Lancez le helper depuis la racine du repo :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Flags clefs :

- `--lane-id` doit correspondre a l'index de la nouvelle entree dans `nexus.lane_catalog`.
- `--dataspace-alias` et `--dataspace-id/hash` controlent l'entree du catalog dataspace (par defaut
  le lane id si omis).
- `--validator` peut etre repete ou lu via `--validators-file`.
- `--route-instruction` / `--route-account` emettent des regles de routage pretes a coller.
- `--metadata key=value` (ou `--telemetry-contact/channel/runbook`) capture les contacts runbook
  pour que les dashboards listent les bons owners.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ajoutent le hook runtime-upgrade au manifest
  quand la lane requiert des controles operateurs etendus.
- `--encode-space-directory` invoque `cargo xtask space-directory encode` automatiquement. Associez
  `--space-directory-out` si vous voulez le fichier `.to` ailleurs que la valeur par defaut.

Le script produit trois artefacts dans `--output-dir` (par defaut le repertoire courant), plus un
quatrieme optionnel lorsque l'encoding est active :

1. `<slug>.manifest.json` - manifest de lane contenant quorum validateurs, namespaces proteges et
   metadata optionnelle du hook runtime-upgrade.
2. `<slug>.catalog.toml` - snippet TOML avec `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`,
   et les regles de routage demandees. Assurez que `fault_tolerance` est defini sur l'entree
   dataspace pour dimensionner le comite lane-relay (`3f+1`).
3. `<slug>.summary.json` - resume d'audit decrivant la geometrie (slug, segments, metadata) plus les
   etapes de rollout et la commande exacte `cargo xtask space-directory encode` (dans
   `space_directory_encode.command`). Attachez ce JSON au ticket d'onboarding pour preuve.
4. `<slug>.manifest.to` - emis quand `--encode-space-directory` est active; pret pour
   `iroha app space-directory manifest publish` via Torii.

Utilisez `--dry-run` pour previsualiser les JSON/snippets sans ecrire de fichiers, et `--force`
pour ecraser des artefacts existants.

## 3. Appliquer les changements

1. Copiez le manifest JSON dans le `nexus.registry.manifest_directory` configure (et dans le
   repertoire cache si le registry miroite des bundles distants). Committez le fichier si les
   manifests sont versionnes dans votre repo de config.
2. Ajoutez le snippet de catalog a `config/config.toml` (ou au `config.d/*.toml`). Assurez que
   `nexus.lane_count` est au moins `lane_id + 1`, et mettez a jour les
   `nexus.routing_policy.rules` qui doivent pointer vers la nouvelle lane.
3. Encodez (si vous avez saute `--encode-space-directory`) et publiez le manifest dans Space
   Directory avec la commande capturee dans le summary (`space_directory_encode.command`). Cela
   produit le payload `.manifest.to` attendu par Torii et enregistre la preuve pour les auditeurs;
   soumettez via `iroha app space-directory manifest publish`.
4. Lancez `irohad --sora --config path/to/config.toml --trace-config` et archivez la sortie de trace
   dans le ticket de rollout. Cela prouve que la nouvelle geometrie correspond au slug/segments
   genere.
5. Redemarrez les validateurs assignes a la lane une fois les changements manifest/catalog deployes.
   Conservez le summary JSON dans le ticket pour les audits futurs.

## 4. Construire un bundle de distribution du registry

Une fois le manifest, le snippet catalog et le summary prets, empaquetez-les pour distribution aux
validateurs. Le nouveau bundler copie les manifests dans le layout attendu par
`nexus.registry.manifest_directory` / `cache_directory`, emet un overlay de catalog de gouvernance
pour pouvoir permuter les modules sans editer le config principal, et archive optionnellement le
bundle :

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Resultats :

1. `manifests/<slug>.manifest.json` - copiez dans `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - placez dans `nexus.registry.cache_directory` pour override ou
   remplacer des modules de gouvernance (`--module ...` override le catalog cache). C'est le chemin
   module plug-in pour NX-2 : remplacez une definition de module, relancez le bundler et distribuez
   l'overlay cache sans toucher `config.toml`.
3. `summary.json` - inclut des digests SHA-256 / Blake2b pour chaque manifest plus la metadata
   overlay.
4. `registry_bundle.tar.*` optionnel - pret pour Secure Copy / stockage d'artefacts.

Si votre deploiement miroite des bundles vers des hosts air-gapped, synchronisez tout le dossier de
sortie (ou le tarball genere). Les noeuds en ligne peuvent monter le dossier de manifests
 directement tandis que les noeuds offline consomment le tarball, l'extraient et copient manifests
+ overlay cache dans leurs chemins configures.

## 5. Smoke tests validateurs

Apres les redemarrages Torii, lancez le helper smoke pour verifier que la lane rapporte
`manifest_ready=true`, que les metriques exposent le nombre de lanes attendu, et que le gauge sealed
est propre. Les lanes qui exigent un manifest doivent maintenant exposer un `manifest_path` non vide
- le helper echoue rapidement si la preuve manque afin que les change controls NX-7 incluent les
references bundle signees automatiquement :

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Ajoutez `--insecure` pour tester des environnements self-signed. Le script sort non-zero si la lane
manque, est sealed, ou si les metriques/telemetries divergent des valeurs attendues. Utilisez les
nouveaux knobs `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, et
`--max-headroom-events` pour maintenir la telemetrie de hauteur/finalite/backlog/headroom dans vos
seuils operationnels. Combinez avec `--max-slot-p95/--max-slot-p99` (plus `--min-slot-samples`) pour
faire respecter le SLO de duree des slots NX-18 directement dans le helper, et passez
`--allow-missing-lane-metrics` uniquement si les clusters staging n'exposent pas encore ces gauges
(en production, gardez les defaults actives).

Le meme helper enforce maintenant la telemetrie de load-test du scheduler. Utilisez
`--min-teu-capacity` pour prouver que chaque lane rapporte un `nexus_scheduler_lane_teu_capacity`,
verrouillez l'utilisation des slots avec `--max-teu-slot-commit-ratio` (compare
`nexus_scheduler_lane_teu_slot_committed` a la capacite), et gardez les compteurs de deferral/
truncation a zero via `--max-teu-deferrals` et `--max-must-serve-truncations`. Ces knobs transforment
l'exigence NX-7 "deeper validator load tests" en un check CLI repetable : le helper echoue quand une
lane differe du travail PQ/TEU ou quand le TEU commit par slot depasse le headroom configure, et le
CLI imprime le resume par lane pour que les paquets de preuve capturent les memes chiffres que CI.

Pour des validations air-gapped (ou CI) vous pouvez rejouer une reponse Torii capturee au lieu de
contacter un noeud live :

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Les fixtures sous `fixtures/nexus/lanes/` refletent les artefacts produits par le helper bootstrap
pour que de nouveaux manifests puissent etre lint sans scripting ad hoc. CI exerce le meme flux via
`ci/check_nexus_lane_smoke.sh` et lance aussi `ci/check_nexus_lane_registry_bundle.sh`
(alias: `make check-nexus-lanes`) pour prouver que le helper smoke NX-7 reste conforme au
format de payload publie et pour garantir que les digests/overlays de bundle restent reproductibles.

Quand une lane est renommee, capturez les evenements telemetrie `nexus.lane.topology` (par exemple
avec `journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) et injectez-les dans
le helper smoke. Le nouveau flag `--telemetry-file/--from-telemetry` accepte le log newline-delimited
et `--require-alias-migration old:new` affirme qu'un evenement `alias_migrated` a enregistre le
rename :

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --min-teu-capacity 64 \
  --max-teu-slot-commit-ratio 0.85 \
  --max-teu-deferrals 0 \
  --max-must-serve-truncations 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

Le fixture `telemetry_alias_migrated.ndjson` bundle l'exemple canonique de rename pour que CI puisse
verifier le parsing telemetrie sans contacter un noeud live.

## 6. Validator load tests (preuve NX-7)

Le roadmap **NX-7** exige que les operateurs de lane capturent un run de charge reproductible avant
qu'une lane soit marquee production ready. L'objectif est de stresser la lane suffisamment pour
exercer la duree des slots, le backlog settlement, le quorum DA, les oracles, le scheduler headroom
et les metriques TEU, puis d'archiver le resultat de facon a ce que les auditeurs puissent rejouer
sans tooling ad hoc. Le helper `scripts/nexus_lane_load_test.py` assemble les checks smoke, le
slot-duration gating et le slot bundle manifest en un set d'artefacts afin que les runs de charge
soient publies directement aux tickets de gouvernance.

### 6.1 Preparation du workload

1. Creez un dossier de run et capturez les fixtures canoniques pour la lane sous test :

   ```bash
   mkdir -p artifacts/nexus/load/payments-2026q2
   cargo xtask nexus-fixtures --output artifacts/nexus/load/payments-2026q2/fixtures
   ```

   Les fixtures refletent `fixtures/nexus/lane_commitments/*.json` et donnent au generateur de
   charge une seed deterministe (notez la seed dans `artifacts/.../README.md`).
2. Baseline de la lane avant le run :

   ```bash
   scripts/nexus_lane_smoke.py \
     --status-url https://torii.example.com/v1/sumeragi/status \
     --metrics-url https://torii.example.com/metrics \
     --lane-alias payments \
     --expected-lane-count 3 \
     --min-block-height 50000 \
     --max-finality-lag 4 \
     --max-settlement-backlog 0.5 \
     --min-settlement-buffer 0.25 \
     --max-slot-p95 1000 \
     --max-slot-p99 1100 \
     --min-slot-samples 50 \
     --insecure \
     > artifacts/nexus/load/payments-2026q2/smoke_before.log
   ```

   Gardez stdout/stderr dans le dossier run pour que les thresholds de smoke soient auditables.
3. Capturez le log telemetrie qui alimentera `--telemetry-file` (evidence de alias migration) et
   `validate_nexus_telemetry_pack.py` :

   ```bash
   journalctl -u irohad -o json \
     --since "2026-05-10T09:00:00Z" \
     --until "2026-05-10T11:00:00Z" \
     > artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson
   ```

4. Lancez le workload de la lane (profil k6, replay harness, ou tests d'ingestion federes) et
   gardez la seed de workload + la plage de slots a portee; la metadata est consommee par le
   validateur du telemetry manifest en section 6.3.

5. Empaquetez les preuves de run avec le nouveau helper. Fournissez les payloads captures
   status/metrics/telemetry, les aliases de lane, et les evenements de alias migration attendus.
   Le helper ecrit `smoke.log`, `slot_summary.json`, un slot bundle manifest, et
   `load_test_manifest.json` qui relie l'ensemble pour revue gouvernance :

   ```bash
   scripts/nexus_lane_load_test.py \
     --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
     --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
     --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
     --lane-alias payments \
     --lane-alias core \
     --expected-lane-count 3 \
     --slot-range 81200-81600 \
     --workload-seed NX7-PAYMENTS-2026Q2 \
     --require-alias-migration core:payments \
     --out-dir artifacts/nexus/load/payments-2026q2
   ```

   La commande enforce les memes gates de quorum DA, oracle, settlement buffer, TEU et
   slot-duration que dans ce guide et produit un manifest pret a attacher sans scripting ad hoc.

### 6.2 Run instrumente

Pendant que le workload sature la lane :

1. Snapshot Torii status + metrics :

   ```bash
   curl -sS https://torii.example.com/v1/sumeragi/status \
     > artifacts/nexus/load/payments-2026q2/torii_status.json
   curl -sS https://torii.example.com/metrics \
     > artifacts/nexus/load/payments-2026q2/metrics.prom
   ```

2. Calculez les quantiles slot-duration et archivez le summary :

   ```bash
   scripts/telemetry/check_slot_duration.py \
     artifacts/nexus/load/payments-2026q2/metrics.prom \
     --max-p95-ms 1000 \
     --max-p99-ms 1100 \
     --min-samples 200 \
     --json-out artifacts/nexus/load/payments-2026q2/slot_summary.json
   scripts/telemetry/bundle_slot_artifacts.py \
     --metrics artifacts/nexus/load/payments-2026q2/metrics.prom \
     --summary artifacts/nexus/load/payments-2026q2/slot_summary.json \
     --out-dir artifacts/nexus/load/payments-2026q2/slot_bundle \
     --metadata lane=payments \
     --metadata workload_seed=NX7-PAYMENTS-2026Q2
   ```

3. Exportez le snapshot lane-governance en JSON + Parquet pour audits long terme :

   ```bash
   cargo xtask nexus-lane-audit \
     --status artifacts/nexus/load/payments-2026q2/torii_status.json \
     --json-out artifacts/nexus/load/payments-2026q2/lane_audit.json \
     --parquet-out artifacts/nexus/load/payments-2026q2/lane_audit.parquet \
     --captured-at 2026-05-10T10:15:00Z
   ```

   Le snapshot JSON/Parquet enregistre maintenant l'utilisation TEU, les niveaux de trigger du
   scheduler, les compteurs RBC chunk/byte et les stats de graph transactions par lane pour que la
   preuve de rollout montre backlog et pression d'execution.

4. Relancez le helper smoke au pic de charge pour evaluer les thresholds en stress (ecrire dans
   `smoke_during.log`) puis re-lancez apres la fin du workload (`smoke_after.log`).

### 6.3 Telemetry pack & manifest gouvernance

Le dossier de run doit inclure un telemetry pack (`prometheus.tgz`, flux OTLP, logs structures, et
les sorties du harness). Validez-le et stamp la metadata attendue par la gouvernance :

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus/load/payments-2026q2 \
  --manifest-out artifacts/nexus/load/payments-2026q2/telemetry_manifest.json \
  --expected prometheus.tgz --expected otlp.ndjson \
  --expected torii_structured_logs.jsonl --expected B4-RB-2026Q1.log \
  --slot-range 81200-81600 --require-slot-range \
  --workload-seed NX7-PAYMENTS-2026Q2 --require-workload-seed \
  --metadata lane=payments --metadata run=2026q2-rollout
```

Enfin, attachez le log telemetrie capture et exigez la preuve alias migration quand une lane est
renommee durant le test :

```bash
scripts/nexus_lane_smoke.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --require-alias-migration core:payments \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-block-height 50000 \
  --max-finality-lag 4 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 200
```

Archivez les artefacts suivants pour le ticket gouvernance :

- `smoke_before.log`, `smoke_during.log`, `smoke_after.log`
- `metrics.prom`, `slot_summary.json`, `slot_bundle_manifest.json`
- `lane_audit.{json,parquet}`
- `telemetry_manifest.json` + contenu du pack (`prometheus.tgz`, `otlp.ndjson`, etc.)
- `nexus.lane.topology.ndjson` (ou le slice telemetrie pertinent)

Le run peut maintenant etre reference dans Space Directory manifests et trackers de gouvernance
comme le load test NX-7 canonique pour la lane.

## 7. Telemetry & gouvernance - follow-ups

- Mettez a jour les dashboards de lane (`dashboards/grafana/nexus_lanes.json` et overlays associes)
  avec le nouveau lane id et la metadata. Les keys generees (`contact`, `channel`, `runbook`, etc.)
  facilitent le pre-remplissage des labels.
- Cablez les regles PagerDuty/Alertmanager pour la nouvelle lane avant d'activer admission. Le
  `summary.json` refletera la checklist dans `docs/source/nexus_operations.md`.
- Enregistrez le manifest bundle dans Space Directory une fois le set de validateurs live. Utilisez
  le meme manifest JSON genere par le helper, signe selon le runbook gouvernance.
- Suivez `docs/source/sora_nexus_operator_onboarding.md` pour les smoke tests (FindNetworkStatus,
  reachability Torii) et capturez la preuve avec le set d'artefacts produit ci-dessus.

## 8. Exemple dry-run

Pour previsualiser les artefacts sans ecrire de fichiers :

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --dry-run
```

La commande imprime le JSON summary et le snippet TOML sur stdout, permettant une iteration rapide
lors de la planification.

---

Pour plus de contexte, voir :

- `docs/source/nexus_operations.md` - checklist operationnelle et exigences telemetrie.
- `docs/source/sora_nexus_operator_onboarding.md` - flux d'onboarding detaille qui reference le
  nouveau helper.
- `docs/source/nexus_lanes.md` - geometrie des lanes, slugs, et layout de storage utilise par l'outil.
