---
lang: fr
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

# Plan de repetition chaos et pannes Connect (IOS3 / IOS7)

Ce playbook definit les drills de chaos repetables qui satisfont l'action roadmap _"plan joint chaos rehearsal"_ (`roadmap.md:1527`). Associez-le au runbook de preview Connect (`docs/runbooks/connect_session_preview_runbook.md`) lors des demos cross-SDK.

## Objectifs et criteres de succes
- Exercer la politique partagee de retry/back-off Connect, les limites de file offline et les exporters de telemetrie sous des pannes controlees sans modifier le code de production.
- Capturer des artefacts deterministes (sortie `iroha connect queue inspect`, snapshots de metriques `connect.*`, logs des SDK Swift/Android/JS) afin que la gouvernance puisse auditer chaque drill.
- Prouver que wallets et dApps respectent les changements de config (manifest drift, rotation de sel, echecs d'attestation) en exposant la categorie canonique `ConnectError` et des evenements de telemetrie conformes a la redaction.

## Prerequis
1. **Bootstrap d'environnement**
   - Demarrez la stack demo Torii: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Lancez au moins un sample SDK (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Instrumentation**
   - Activez les diagnostics SDK (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` en Swift; equivalents `ConnectQueueJournal` + `ConnectQueueJournalTests`
     en Android/JS).
   - Verifiez que le CLI `iroha connect queue inspect --sid <sid> --metrics` resout
     le chemin de file produit par le SDK (`~/.iroha/connect/<sid>/state.json` et
     `metrics.ndjson`).
   - Branchez les exporters de telemetrie pour que les series suivantes soient visibles dans
     Grafana et via `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Dossiers de preuves** - creez `artifacts/connect-chaos/<date>/` et stockez:
   - logs bruts (`*.log`), snapshots de metriques (`*.json`), exports de dashboard
     (`*.png`), sorties CLI et IDs PagerDuty.

## Matrice de scenarios

| ID | Panne | Etapes d'injection | Signaux attendus | Preuves |
|----|-------|--------------------|------------------|----------|
| C1 | Coupure WebSocket et reconnexion | Encapsulez `/v1/connect/ws` derriere un proxy (ex: `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) ou bloquez temporairement le service (`kubectl scale deploy/torii --replicas=0` pendant <=60 s). Forcez le wallet a continuer d'envoyer des frames pour remplir les files offline. | `connect.reconnects_total` augmente, `connect.resume_latency_ms` pique mais reste <1 s p95, les files passent en `state=Draining` via `ConnectQueueStateTracker`. Les SDKs emettent `ConnectError.Transport.reconnecting` une fois puis reprennent. | - Sortie `iroha connect queue inspect --sid <sid>` montrant `resume_attempts_total` non nul.<br>- Annotation de dashboard pour la fenetre d'outage.<br>- Extrait de logs avec messages reconnect + drain. |
| C2 | Overflow de file offline / expiration TTL | Patcher le sample pour reduire les limites de file (Swift: instancier `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` dans `ConnectSessionDiagnostics`; Android/JS utilisent des constructeurs equivalents). Suspendre le wallet pendant >=2x `retentionInterval` pendant que la dApp continue d'enqueue des requetes. | `connect.queue_dropped_total{reason="overflow"}` et `{reason="ttl"}` augmentent, `connect.queue_depth` se stabilise au nouveau seuil, les SDKs affichent `ConnectError.QueueOverflow(limit: 4)` (ou `.QueueExpired`). `iroha connect queue inspect` montre `state=Overflow` avec marques `warn/drop` a 100%. | - Capture des compteurs de metriques.<br>- JSON CLI capturant l'overflow.<br>- Log Swift/Android contenant la ligne `ConnectError`. |
| C3 | Manifest drift / rejet d'admission | Modifiez le manifest Connect servi aux wallets (ex: modifiez le sample manifest `docs/connect_swift_ios.md`, ou demarrez Torii avec `--connect-manifest-path` pointant vers une copie ou `chain_id` ou `permissions` differents). Demandez une approbation et assurez-vous que le wallet rejette selon la politique. | Torii renvoie `HTTP 409` pour `/v1/connect/session` avec `manifest_mismatch`, les SDKs emettent `ConnectError.Authorization.manifestMismatch(manifestVersion)`, la telemetrie augmente `connect.manifest_mismatch_total`, et les files restent vides (`state=Idle`). | - Extrait des logs Torii montrant la detection de mismatch.<br>- Capture SDK de l'erreur exposee.<br>- Snapshot de metriques prouvant qu'aucun frame n'a ete en file pendant le test. |
| C4 | Rotation de cle / saut de version de sel | Faites tourner le sel ou la cle AEAD Connect en milieu de session. En stack dev, redemarrez Torii avec `CONNECT_SALT_VERSION=$((old+1))` (reflete le test de sel Android dans `docs/source/sdk/android/telemetry_schema_diff.md`). Gardez le wallet offline jusqu'a la fin de la rotation, puis reprenez. | La premiere reprise echoue avec `ConnectError.Authorization.invalidSalt`, les files se vident (la dApp jette les frames caches avec raison `salt_version_mismatch`), la telemetrie emet `android.telemetry.redaction.salt_version` (Android) et `swift.connect.session_event{event="salt_rotation"}`. La seconde session apres refresh du SID reussit. | - Annotation de dashboard avec l'epoque de sel avant/apres.<br>- Logs avec l'erreur invalid-salt puis le succes.
- Sortie `iroha connect queue inspect` montrant `state=Stalled` puis `state=Active`. |
| C5 | Echec d'attestation / StrongBox | Sur wallets Android, configurez `ConnectApproval` pour inclure `attachments[]` + attestation StrongBox. Utilisez le harness d'attestation (`scripts/android_keystore_attestation.sh` avec `--inject-failure strongbox-simulated`) ou modifiez le JSON d'attestation avant de le fournir a la dApp. | La dApp rejette l'approbation avec `ConnectError.Authorization.invalidAttestation`, Torii journalise la raison, les exporters incrementent `connect.attestation_failed_total`, et la file purge l'entree fautive. Les dApps Swift/JS loggent l'erreur en gardant la session active. | - Log du harness avec ID de panne injectee.<br>- Log d'erreur SDK + capture du compteur de telemetrie.<br>- Preuve que la file a supprime le mauvais frame (`recordsRemoved > 0`). |

## Details des scenarios

### C1 - Coupure WebSocket et reconnexion
1. Encapsulez Torii derriere un proxy (toxiproxy, Envoy, ou un `kubectl port-forward`) afin de
   pouvoir basculer la disponibilite sans tuer tout le noeud.
2. Declenchez une coupure de 45 s:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Observez les dashboards de telemetrie et `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. Dumpez l'etat de la file juste apres la coupure:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Succes = une seule tentative de reconnexion, croissance de file bornee et drain
   automatique apres le retour du proxy.

### C2 - Overflow de file offline / expiration TTL
1. Reduisez les seuils de file dans les builds locales:
   - Swift: mettez a jour l'init de `ConnectQueueJournal` dans votre sample
     (ex: `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     pour passer `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: passez la config equivalente lors de la construction de `ConnectQueueJournal`.
2. Suspendez le wallet (background simulateur ou mode avion) pendant >=60 s
   pendant que la dApp emet des appels `ConnectClient.requestSignature(...)`.
3. Utilisez `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) ou l'outil
   de diagnostics JS pour exporter le bundle de preuves (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Succes = les compteurs d'overflow augmentent, le SDK affiche `ConnectError.QueueOverflow`
   une fois, et la file se retablit apres la reprise du wallet.

### C3 - Manifest drift / rejet d'admission
1. Faites une copie du manifest d'admission, par exemple:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Demarrez Torii avec `--connect-manifest-path /tmp/manifest_drift.json` (ou
   mettez a jour la config docker compose/k8s pour le drill).
3. Tentez de demarrer une session depuis le wallet; attendez HTTP 409.
4. Capturez les logs Torii + SDK et `connect.manifest_mismatch_total` depuis
   le dashboard de telemetrie.
5. Succes = rejet sans croissance de file, et le wallet affiche l'erreur de
   taxonomie partagee (`ConnectError.Authorization.manifestMismatch`).

### C4 - Rotation de cle / saut de sel
1. Enregistrez la version de sel actuelle via la telemetrie:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Redemarrez Torii avec un nouveau sel (`CONNECT_SALT_VERSION=$((OLD+1))` ou mettez a jour le
   config map). Gardez le wallet offline jusqu'a la fin du redemarrage.
3. Reprenez le wallet; la premiere tentative doit echouer avec une erreur invalid-salt
   et `connect.queue_dropped_total{reason="salt_version_mismatch"}` augmente.
4. Forcez l'app a jeter les frames caches en supprimant le repertoire de session
   (`rm -rf ~/.iroha/connect/<sid>` ou nettoyage cache par plateforme), puis
   redemarrez la session avec des tokens frais.
5. Succes = la telemetrie montre le saut de sel, l'evenement invalid-resume est logge
   une fois, et la session suivante reussit sans intervention manuelle.

### C5 - Echec d'attestation / StrongBox
1. Generez un bundle d'attestation avec `scripts/android_keystore_attestation.sh`
   (utilisez `--inject-failure strongbox-simulated` pour inverser la signature).
2. Demandez au wallet d'attacher ce bundle via son API `ConnectApproval`; la dApp
   doit valider et rejeter le payload.
3. Verifiez la telemetrie (`connect.attestation_failed_total`, metriques d'incident
   Swift/Android) et assurez-vous que la file a retire l'entree empoisonnee.
4. Succes = le rejet est isole a la mauvaise approval, les files restent saines
   et le log d'attestation est stocke avec les preuves du drill.

## Checklist des preuves
- exports `artifacts/connect-chaos/<date>/c*_metrics.json` depuis
  `scripts/swift_status_export.py telemetry`.
- sorties CLI (`c*_queue.txt`) de `iroha connect queue inspect`.
- logs SDK + Torii avec timestamps et hashes de SID.
- captures de dashboard avec annotations pour chaque scenario.
- PagerDuty / incident IDs si des alertes Sev 1/2 ont ete declenchees.

Completer la matrice une fois par trimestre satisfait le gate de roadmap et
montre que les implementations Connect Swift/Android/JS repondent de maniere
deterministe aux modes de panne les plus risques.
