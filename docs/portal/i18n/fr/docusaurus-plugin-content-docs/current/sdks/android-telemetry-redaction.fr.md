---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f22e82241a07052e62531058ef612cbd99bfe52a0bb0cf53ec6d2e28bd6bf389
source_last_modified: "2025-11-18T05:53:43.294424+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Plan de caviardage de la télémétrie Android
sidebar_label: Télémétrie Android
slug: /sdks/android-telemetry
---

:::note Source canonique
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Plan de caviardage de la télémétrie Android (AND7)

## Périmètre

Ce document décrit la politique proposée de caviardage de télémétrie et les artefacts d’activation pour le SDK Android, comme l’exige l’item de roadmap **AND7**. Il aligne l’instrumentation mobile sur la baseline des nœuds Rust tout en tenant compte des garanties de confidentialité propres aux appareils. Le résultat sert de pré‑lecture pour la revue de gouvernance SRE de février 2026.

Objectifs :

- Recenser chaque signal émis par Android qui atteint les backends d’observabilité partagés (traces OpenTelemetry, logs encodés Norito, exports de métriques).
- Classer les champs qui diffèrent de la baseline Rust et documenter les contrôles de caviardage ou de rétention.
- Décrire les actions d’activation et de test afin que les équipes support répondent de façon déterministe aux alertes liées au caviardage.

## Inventaire des signaux (brouillon)

Instrumentation planifiée regroupée par canal. Tous les noms de champ suivent le schéma de télémétrie du SDK Android (`org.hyperledger.iroha.android.telemetry.*`). Les champs optionnels sont marqués avec `?`.

| ID du signal | Canal | Champs clés | Classification PII/PHI | Caviardage / Rétention | Notes |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Span de trace | `authority_hash`, `route`, `status_code`, `latency_ms` | L’autorité est publique ; la route ne contient pas de secrets | Émettre l’autorité hashée (`blake2b_256`) avant export ; conserver 7 jours | Miroir de `torii.http.request` côté Rust ; le hachage protège l’alias mobile. |
| `android.torii.http.retry` | Événement | `route`, `retry_count`, `error_code`, `backoff_ms` | Aucun | Pas de caviardage ; conserver 30 jours | Utilisé pour des audits de retry déterministes ; champs identiques à Rust. |
| `android.pending_queue.depth` | Métrique gauge | `queue_type`, `depth` | Aucun | Pas de caviardage ; conserver 90 jours | Correspond à `pipeline.pending_queue_depth` côté Rust. |
| `android.keystore.attestation.result` | Événement | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (dérivé), métadonnées d’appareil | Remplacer l’alias par un libellé déterministe, caviarder la marque via un bucket enum | Requis pour la préparation AND2 ; les nœuds Rust n’émettent pas de métadonnées d’appareil. |
| `android.keystore.attestation.failure` | Compteur | `alias_label`, `failure_reason` | Aucun après caviardage de l’alias | Pas de caviardage ; conserver 90 jours | Soutient les drills de chaos ; `alias_label` dérivé de l’alias hashé. |
| `android.telemetry.redaction.override` | Événement | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Le rôle de l’acteur est une PII opérationnelle | Le champ exporte la catégorie de rôle masquée ; conserver 365 jours avec journal d’audit | Absent côté Rust ; les opérateurs doivent passer par le support. |
| `android.telemetry.export.status` | Compteur | `backend`, `status` | Aucun | Pas de caviardage ; conserver 30 jours | Parité avec les compteurs d’état d’exporter Rust. |
| `android.telemetry.redaction.failure` | Compteur | `signal_id`, `reason` | Aucun | Pas de caviardage ; conserver 30 jours | Requis pour refléter `streaming_privacy_redaction_fail_total` côté Rust. |
| `android.telemetry.device_profile` | Métrique gauge | `profile_id`, `sdk_level`, `hardware_tier` | Métadonnées d’appareil | Émettre des buckets grossiers (SDK majeur, hardware tier) ; conserver 30 jours | Active les dashboards de parité sans exposer les détails OEM. |
| `android.telemetry.network_context` | Événement | `network_type`, `roaming` | Le carrier peut être une PII | Supprimer `carrier_name` entièrement ; conserver les autres champs 7 jours | `ClientConfig.networkContextProvider` fournit l’instantané nettoyé pour que les apps émettent type de réseau + roaming sans divulguer l’abonné ; les dashboards de parité traitent ce signal comme l’analogue mobile de `peer_host` Rust. |
| `android.telemetry.config.reload` | Événement | `source`, `result`, `duration_ms` | Aucun | Pas de caviardage ; conserver 30 jours | Miroir des spans de reload de config Rust. |
| `android.telemetry.chaos.scenario` | Événement | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Profil d’appareil bucketisé | Identique à `device_profile` ; conserver 30 jours | Journalisé lors des rehearsals de chaos requis pour AND7. |
| `android.telemetry.redaction.salt_version` | Métrique gauge | `salt_epoch`, `rotation_id` | Aucun | Pas de caviardage ; conserver 365 jours | Suit la rotation du salt Blake2b ; alerte de parité quand l’epoch Android diverge de Rust. |
| `android.crash.report.capture` | Événement | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Empreinte de crash + métadonnées de processus | Hasher `crash_id` avec le salt partagé, bucketiser l’état du watchdog, supprimer les stack frames avant export ; conserver 30 jours | Activé automatiquement via `ClientConfig.Builder.enableCrashTelemetryHandler()` ; alimente les dashboards de parité sans exposer des traces identifiantes. |
| `android.crash.report.upload` | Compteur | `crash_id`, `backend`, `status`, `retry_count` | Empreinte de crash | Réutiliser le `crash_id` hashé, n’émettre que le statut ; conserver 30 jours | Émettre via `ClientConfig.crashTelemetryReporter()` ou `CrashTelemetryHandler.recordUpload` pour que les uploads partagent les garanties Sigstore/OLTP. |

### Points d’intégration

- `ClientConfig` propage désormais les données de télémétrie dérivées du manifest via `setTelemetryOptions(...)`/`setTelemetrySink(...)`, enregistrant automatiquement `TelemetryObserver` afin que les autorités hashées et les métriques de salt remontent sans observers ad hoc. Voir `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` et les classes associées sous `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Les applications peuvent appeler `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` pour enregistrer le `AndroidNetworkContextProvider` basé sur la réflexion, qui interroge `ConnectivityManager` à l’exécution et émet l’événement `android.telemetry.network_context` sans ajouter de dépendances Android à la compilation.
- Les tests unitaires `TelemetryOptionsTests` et `TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) protègent les helpers de hachage et le hook d’intégration ClientConfig afin que les régressions de manifest soient détectées immédiatement.
- Le kit/labs d’activation cite désormais des API concrètes plutôt que du pseudo‑code, gardant ce document et le runbook alignés sur le SDK livré.

> **Note opérations :** la feuille de responsables/état se trouve dans `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` et doit être mise à jour avec ce tableau à chaque checkpoint AND7.

## Listes d’autorisation de parité et workflow de diff de schéma

La gouvernance impose une double allowlist afin que les exports Android n’exposent jamais les identifiants que les services Rust publient volontairement. Cette section reflète l’entrée du runbook (`docs/source/android_runbook.md` §2.3) tout en gardant le plan AND7 autonome.

| Catégorie | Exportateurs Android | Services Rust | Point de validation |
|----------|-------------------|---------------|-----------------|
| Contexte autorité/route | Hasher `authority`/`alias` via Blake2b-256 et supprimer les hostnames Torii bruts avant export ; émettre `android.telemetry.redaction.salt_version` pour prouver la rotation du salt. | Émettre les hostnames Torii complets et les peer IDs pour corrélation. | Comparer `android.torii.http.request` vs `torii.http.request` dans le dernier diff sous `docs/source/sdk/android/readiness/schema_diffs/`, puis exécuter `scripts/telemetry/check_redaction_status.py` pour confirmer les epochs de salt. |
| Identité appareil/signataire | Bucketiser `hardware_tier`/`device_profile`, hasher les alias contrôleur et ne jamais exporter les numéros de série. | Émettre `peer_id` du validateur, `public_key` du contrôleur et hashes de file tels quels. | Aligner avec `docs/source/sdk/mobile_device_profile_alignment.md`, exécuter les tests de hachage d’alias via `java/iroha_android/run_tests.sh`, et archiver les sorties du queue inspector pendant les labs. |
| Métadonnées réseau | Exporter uniquement `network_type` + `roaming` ; supprimer `carrier_name`. | Conserver les métadonnées hostname/TLS des peers. | Stocker chaque diff dans `readiness/schema_diffs/` et alerter si le widget Grafana “Network Context” affiche des carriers. |
| Evidence override/chaos | Émettre `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` avec rôles masqués. | Émettre les approvals d’override non masqués ; pas de spans chaos spécifiques. | Recouper `docs/source/sdk/android/readiness/and7_operator_enablement.md` après les drills pour s’assurer que tokens d’override et artefacts de chaos cohabitent avec les événements Rust. |

Workflow :

1. Après chaque changement de manifest/exporter, exécuter `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` et déposer le JSON sous `docs/source/sdk/android/readiness/schema_diffs/`.
2. Relire le diff par rapport au tableau ci‑dessus. Si Android émet un champ Rust-only (ou inversement), ouvrir un bug AND7 et mettre à jour ce plan et le runbook.
3. Lors des revues ops hebdomadaires, exécuter `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` et consigner l’epoch du salt + le timestamp du diff dans la feuille de readiness.
4. Noter toute déviation dans `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` pour que les paquets de gouvernance capturent les décisions de parité.

> **Référence de schéma :** les identifiants de champ canoniques proviennent de `android_telemetry_redaction.proto` (matérialisé lors du build du SDK Android avec les descripteurs Norito). Le schéma expose `authority_hash`, `alias_label`, `attestation_digest`, `device_brand_bucket` et `actor_role_masked` utilisés dans le SDK et les exporteurs de télémétrie.

`authority_hash` est un digest fixe de 32 octets de la valeur d’autorité Torii. `attestation_digest` capture l’empreinte canonique de l’attestation, tandis que `device_brand_bucket` mappe la marque Android brute vers l’enum approuvé (`generic`, `oem`, `enterprise`). `actor_role_masked` transporte la catégorie d’acteur (`support`, `sre`, `audit`) plutôt que l’identifiant utilisateur brut.

### Alignement des exports de télémétrie de crash

La télémétrie de crash partage désormais les mêmes exporteurs OpenTelemetry et la même chaîne de provenance que les signaux réseau Torii, clôturant le follow‑up de gouvernance sur les exporteurs dupliqués. Le crash handler alimente l’événement `android.crash.report.capture` avec un `crash_id` hashé (Blake2b-256 utilisant le salt déjà suivi par `android.telemetry.redaction.salt_version`), des buckets d’état de processus et des métadonnées ANR watchdog assainies. Les stack traces restent sur l’appareil et ne sont résumées que dans `has_native_trace` et `anr_watchdog_bucket` avant export, afin qu’aucune PII ou chaîne OEM ne quitte l’appareil.

Le téléversement d’un crash crée l’entrée compteur `android.crash.report.upload`, permettant aux SRE d’auditer la fiabilité backend sans apprendre quoi que ce soit sur l’utilisateur ou la stack. Comme les deux signaux réutilisent l’exporter Torii, ils héritent des mêmes garanties Sigstore, politiques de rétention et hooks d’alerte déjà définis pour AND7. Les runbooks support peuvent ainsi corréler un identifiant de crash hashé entre bundles Android et Rust sans pipeline dédié.

Activez le handler via `ClientConfig.Builder.enableCrashTelemetryHandler()` une fois les options/sinks de télémétrie configurés ; les bridges d’upload peuvent réutiliser `ClientConfig.crashTelemetryReporter()` (ou `CrashTelemetryHandler.recordUpload`) pour émettre les résultats backend dans la même chaîne signée.

## Écarts de politique par rapport à la baseline Rust

Différences entre les politiques de télémétrie Android et Rust, avec mesures d’atténuation.

| Catégorie | Baseline Rust | Politique Android | Atténuation / Validation |
|----------|---------------|----------------|-------------------------|
| Identifiants autorité/peer | Chaînes d’autorité en clair | `authority_hash` (Blake2b-256, salt rotatif) | Salt partagé publié via `iroha_config.telemetry.redaction_salt` ; test de parité garantissant un mapping réversible pour le support. |
| Métadonnées host/réseau | Hostnames/IPs des nœuds exportés | Type de réseau + roaming uniquement | Dashboards de santé réseau mis à jour pour utiliser des catégories de disponibilité plutôt que des hostnames. |
| Caractéristiques appareil | N/A (côté serveur) | Profil bucketisé (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Rehearsals de chaos vérifient le mapping ; le runbook support documente l’escalade si davantage de détails sont requis. |
| Overrides de caviardage | Non supportés | Token d’override manuel stocké dans le ledger Norito (`actor_role_masked`, `reason`) | Overrides requièrent une demande signée ; log d’audit conservé 1 an. |
| Traces d’attestation | Attestation serveur via SRE uniquement | Le SDK émet un résumé d’attestation assaini | Recouper les hashes d’attestation avec le validateur Rust ; alias hashé évite les fuites. |

Checklist de validation :

- Tests unitaires de caviardage pour chaque signal vérifiant les champs hashés/masqués avant export.
- Outil de diff de schéma (partagé avec les nœuds Rust) exécuté chaque nuit pour confirmer la parité des champs.
- Script de rehearsal chaos qui exerce le workflow d’override et confirme le logging d’audit.

## Tâches d’implémentation (pré‑gouvernance SRE)

1. **Confirmation de l’inventaire** — Recouper le tableau ci‑dessus avec les hooks d’instrumentation Android et les définitions de schéma Norito. Responsables : Android Observability TL, LLM.
2. **Diff du schéma de télémétrie** — Exécuter l’outil de diff partagé contre les métriques Rust afin de produire les artefacts de parité pour la revue SRE. Responsable : SRE privacy lead.
3. **Brouillon du runbook (terminé 2026-02-03)** — `docs/source/android_runbook.md` documente désormais le workflow d’override end‑to‑end (Section 3) et la matrice d’escalade étendue plus les rôles (Section 3.1), reliant les helpers CLI, l’évidence d’incident et les scripts de chaos à la politique. Responsables : LLM avec édition Docs/Support.
4. **Contenu d’activation** — Préparer les slides de briefing, instructions de lab et questions de connaissance pour la session de février 2026. Responsables : Docs/Support Manager, équipe d’activation SRE.

## Workflow d’activation et hooks de runbook

### 1. Couverture smoke locale + CI

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` démarre le sandbox Torii, rejoue le fixture SoraFS multi‑source canonique (via `ci/check_sorafs_orchestrator_adoption.sh`) et injecte de la télémétrie Android synthétique.
  - La génération de trafic est gérée par `scripts/telemetry/generate_android_load.py`, qui enregistre un transcript request/response sous `artifacts/android/telemetry/load-generator.log` et respecte headers, overrides de route ou mode dry‑run.
  - Le helper copie le scoreboard et les résumés SoraFS dans `${WORKDIR}/sorafs/` pour que les rehearsals AND7 prouvent la parité multi‑source avant de toucher les clients mobiles.
- La CI réutilise la même toolchain : `ci/check_android_dashboard_parity.sh` exécute `scripts/telemetry/compare_dashboards.py` contre `dashboards/grafana/android_telemetry_overview.json`, le dashboard Rust de référence et le fichier d’allowlist `dashboards/data/android_rust_dashboard_allowances.json`, produisant le snapshot signé `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- Les rehearsals de chaos suivent `docs/source/sdk/android/telemetry_chaos_checklist.md` ; le script sample‑env plus le check de parité dashboards forment le bundle de preuve “ready” qui alimente l’audit burn‑in AND7.

### 2. Émission d’override et trace d’audit

- `scripts/android_override_tool.py` est la CLI canonique pour émettre et révoquer des overrides de caviardage. `apply` ingère une demande signée, émet le bundle de manifest (`telemetry_redaction_override.to` par défaut) et ajoute une ligne de token hashé dans `docs/source/sdk/android/telemetry_override_log.md`. `revoke` appose l’horodatage de révocation sur cette même ligne, et `digest` écrit le snapshot JSON assaini requis par la gouvernance.
- La CLI refuse de modifier le log d’audit si l’en‑tête de la table Markdown est absent, conformément à l’exigence de conformité dans `docs/source/android_support_playbook.md`. La couverture unitaire dans `scripts/tests/test_android_override_tool_cli.py` protège le parseur de table, les émetteurs de manifest et la gestion d’erreur.
- Les opérateurs joignent le manifest généré, l’extrait de log mis à jour **et** le digest JSON sous `docs/source/sdk/android/readiness/override_logs/` à chaque override ; le log conserve 365 jours d’historique selon la décision de gouvernance.

### 3. Capture d’évidence et rétention

- Chaque rehearsal ou incident produit un bundle structuré sous `artifacts/android/telemetry/` contenant :
  - Le transcript du générateur de charge et les compteurs agrégés de `generate_android_load.py`.
  - Le diff de dashboards (`android_vs_rust-<stamp>.json`) et le hash d’allowlist émis par `ci/check_android_dashboard_parity.sh`.
  - Le delta du log d’override (si un override a été accordé), le manifest correspondant et le digest JSON rafraîchi.
- Le rapport burn‑in SRE référence ces artefacts plus le scoreboard SoraFS copié par `android_sample_env.sh`, offrant à la revue AND7 une chaîne déterministe allant des hashes de télémétrie → dashboards → statut des overrides.

## Alignement des profils d’appareil entre SDK

Les dashboards traduisent le `hardware_tier` Android en `mobile_profile_class` canonique défini dans `docs/source/sdk/mobile_device_profile_alignment.md` afin que AND7 et IOS7 comparent les mêmes cohortes :

- `lab` — émis comme `hardware_tier = emulator`, aligné avec `device_profile_bucket = simulator` côté Swift.
- `consumer` — émis comme `hardware_tier = consumer` (avec suffixe SDK major) et regroupé avec `iphone_small`/`iphone_large`/`ipad` côté Swift.
- `enterprise` — émis comme `hardware_tier = enterprise`, aligné sur le bucket `mac_catalyst` côté Swift et les futurs runtimes iOS desktop gérés.

Tout nouveau tier doit être ajouté au document d’alignement et aux artefacts de diff avant consommation par les dashboards.

## Gouvernance et distribution

- **Paquet de pré‑read** — Ce document et les artefacts d’annexe (diff de schéma, diff du runbook, outline du deck readiness) seront distribués à la liste de gouvernance SRE au plus tard le **2026-02-05**.
- **Boucle de feedback** — Les retours collectés pendant la gouvernance alimenteront l’epic JIRA `AND7` ; les bloqueurs sont remontés dans `status.md` et dans les notes du stand‑up Android hebdomadaire.
- **Publication** — Après approbation, le résumé de politique sera lié depuis `docs/source/android_support_playbook.md` et référencé par la FAQ de télémétrie commune dans `docs/source/telemetry.md`.

## Notes d’audit et conformité

- La politique respecte GDPR/CCPA en supprimant les données d’abonné mobile avant export ; le salt d’autorité hashée tourne trimestriellement et est stocké dans le vault partagé.
- Les artefacts d’activation et les mises à jour du runbook sont consignés dans le registre de conformité.
- Les revues trimestrielles confirment que les overrides restent en boucle fermée (pas d’accès obsolètes).

## Résultat de gouvernance (2026-02-12)

La session de gouvernance SRE du **2026-02-12** a approuvé la politique de caviardage Android sans modification. Décisions clés (voir `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`) :

- **Acceptation de la politique.** L’autorité hashée, le bucketage du profil d’appareil et l’omission des noms de carrier ont été ratifiés. Le suivi de rotation de salt via `android.telemetry.redaction.salt_version` devient un item d’audit trimestriel.
- **Plan de validation.** Couverture unit/integration, diffs de schéma nocturnes et rehearsals de chaos trimestriels ont été validés. Action : publier un rapport de parité dashboards après chaque rehearsal.
- **Gouvernance des overrides.** Les tokens d’override enregistrés Norito sont approuvés avec une fenêtre de rétention de 365 jours. Le support engineering assure la revue du digest du log d’override lors des syncs mensuels.

## Statut des suivis

1. **Alignement des profils d’appareil (échéance 2026-03-01).** ✅ Terminé — le mapping partagé dans `docs/source/sdk/mobile_device_profile_alignment.md` définit comment `hardware_tier` Android mappe vers `mobile_profile_class` utilisé par les dashboards de parité et les outils de diff.

## Prochain brief de gouvernance SRE (T2 2026)

L’item de roadmap **AND7** exige que la prochaine session de gouvernance SRE reçoive un pré‑read concis sur le caviardage Android. Utilisez cette section comme brief vivant ; maintenez‑la à jour avant chaque réunion du conseil.

### Checklist de préparation

1. **Bundle d’évidence** — exportez le dernier diff de schéma, captures dashboards et digest du log d’override (voir matrice ci‑dessous) et placez‑les sous un dossier daté (par exemple `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) avant d’envoyer l’invitation.
2. **Résumé des drills** — joignez le log du rehearsal chaos le plus récent et le snapshot de la métrique `android.telemetry.redaction.failure` ; assurez‑vous que les annotations Alertmanager référencent le même timestamp.
3. **Audit d’override** — confirmez que tous les overrides actifs sont enregistrés dans le registre Norito et résumés dans le deck de réunion. Inclure dates d’expiration et IDs d’incident correspondants.
4. **Note d’agenda** — pingez le chair SRE 48 heures avant la réunion avec le lien du brief en mettant en évidence les décisions requises (nouveaux signaux, changements de rétention ou politique d’override).

### Matrice d’évidence

| Artefact | Emplacement | Owner | Notes |
|----------|----------|-------|-------|
| Diff de schéma vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | Doit être généré <72 h avant la réunion. |
| Captures du diff dashboards | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | Inclure `sorafs.fetch.*`, `android.telemetry.*` et snapshots Alertmanager. |
| Digest d’overrides | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | Exécuter `scripts/android_override_tool.sh digest` (voir README du dossier) sur le dernier `telemetry_override_log.md` ; tokens restent hashés avant partage. |
| Log de rehearsal chaos | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | Joindre un résumé KPI (stall count, retry ratio, override usage). |

### Questions ouvertes pour le conseil

- Faut‑il réduire la fenêtre de rétention des overrides de 365 jours maintenant que le digest est automatisé ?
- `android.telemetry.device_profile` doit‑il adopter les nouveaux labels partagés `mobile_profile_class` dès la prochaine release, ou attendre que les SDK Swift/JS déploient le même changement ?
- Des consignes supplémentaires sont‑elles nécessaires pour la résidence régionale des données une fois que les événements Torii Norito-RPC arrivent sur Android (follow‑up NRPC-3) ?

### Procédure de diff de schéma de télémétrie

Exécutez l’outil de diff de schéma au moins une fois par release candidate (et à chaque changement d’instrumentation Android) pour que le conseil SRE reçoive des artefacts de parité récents avec le diff dashboards :

1. Exportez les schémas de télémétrie Android et Rust à comparer. Pour la CI les configs sont sous `configs/android_telemetry.json` et `configs/rust_telemetry.json`.
2. Exécutez `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - Sinon, passez des commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) pour extraire les configs depuis git ; le script fige les hashes dans l’artefact.
3. Joignez le JSON au bundle de readiness et liez‑le depuis `status.md` + `docs/source/telemetry.md`. Le diff met en évidence les champs ajoutés/supprimés et les deltas de rétention pour que les auditeurs valident la parité sans relancer l’outil.
4. Quand le diff révèle des divergences permises (ex. signaux d’override Android‑only), mettez à jour le fichier d’allowlist référencé par `ci/check_android_dashboard_parity.sh` et notez la justification dans le README du dossier de diff.

> **Règles d’archivage :** conservez les cinq diffs les plus récents sous `docs/source/sdk/android/readiness/schema_diffs/` et déplacez les snapshots plus anciens vers `artifacts/android/telemetry/schema_diffs/` pour que les reviewers de gouvernance voient toujours les données les plus récentes.
