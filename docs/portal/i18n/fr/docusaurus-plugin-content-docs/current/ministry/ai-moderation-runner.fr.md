---
lang: fr
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: fr
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-11-10T16:27:31.384538+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Spécification du runner de modération IA
summary: Conception déterministe du comité de modération pour le livrable du Ministère de l’Information (MINFO-1).
---

# Spécification du runner de modération IA

Cette spécification remplit la partie documentation de **MINFO-1 — Établir la base de modération IA**. Elle définit le contrat d’exécution déterministe du service de modération du Ministère de l’Information afin que chaque gateway exécute des pipelines identiques avant les flux d’appel et de transparence (SFM-4/SFM-4b). Tout le comportement décrit ici est normatif sauf mention contraire.

## 1. Objectifs et périmètre
- Fournir un comité de modération reproductible qui évalue le contenu des gateways (objets, manifests, métadonnées, audio) à l’aide de modèles hétérogènes.
- Garantir une exécution déterministe entre opérateurs : opset fixe, tokenisation seedée, précision bornée et artefacts versionnés.
- Produire des artefacts prêts pour l’audit : manifests, scorecards, preuves de calibration et digests de transparence publiables dans le DAG de gouvernance.
- Exposer de la télémétrie pour permettre aux SRE de détecter la dérive, les faux positifs et les indisponibilités sans collecter de données brutes utilisateurs.

## 2. Contrat d’exécution déterministe
- **Runtime :** ONNX Runtime 1.19.x (backend CPU) compilé avec AVX2 désactivé et `--enable-extended-minimal-build` pour figer l’ensemble des opcodes. Les runtimes CUDA/Metal sont explicitement interdits en production.
- **Opset :** `opset=17`. Les modèles ciblant des opsets plus récents doivent être rétroportés et validés avant admission.
- **Dérivation de la graine :** chaque évaluation dérive une graine RNG depuis `BLAKE3(content_digest || manifest_id || run_nonce)` où `run_nonce` provient du manifeste approuvé par la gouvernance. Les graines alimentent tous les composants stochastiques (beam search, bascules de dropout) afin que les résultats soient reproductibles bit à bit.
- **Threading :** un worker par modèle. La concurrence est coordonnée par l’orchestrateur du runner pour éviter les conditions de course sur l’état partagé. Les bibliothèques BLAS fonctionnent en mono-thread.
- **Numérique :** l’accumulation FP16 est interdite. Utiliser des intermédiaires FP32 et borner les sorties à quatre décimales avant agrégation.

## 3. Composition du comité
Le comité de base contient trois familles de modèles. La gouvernance peut en ajouter, mais le quorum minimal doit rester satisfait.

| Famille | Modèle de base | Finalité |
|--------|----------------|---------|
| Vision | OpenCLIP ViT-H/14 (fine-tuné sécurité) | Détecte la contrebande visuelle, la violence, les indicateurs CSAM. |
| Multimodal | LLaVA-1.6 34B Safety | Capte les interactions texte + image, les indices contextuels, le harcèlement. |
| Perceptuel | Ensemble pHash + aHash + NeuralHash-lite | Détection rapide des quasi-doublons et rappel de contenu malveillant connu. |

Chaque entrée de modèle précise :
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 de l’image OCI)
- `weights_digest` (BLAKE3-256 de l’ONNX ou du blob safetensors fusionné)
- `opset` (doit être `17`)
- `weight` (poids du comité, défaut `1.0`)
- `critical_labels` (ensemble d’étiquettes déclenchant immédiatement `Escalate`)
- `max_eval_ms` (garde-fou pour les watchdogs déterministes)

## 4. Manifests et résultats Norito

### 4.1 Manifeste du comité
```norito
struct AiModerationManifestV1 {
    manifest_id: Uuid,
    issued_at: Timestamp,
    runner_hash: Digest32,
    runtime_version: String,
    models: Vec<AiModerationModelV1>,
    calibration_dataset: DatasetReferenceV1,
    calibration_hash: Digest32,
    thresholds: AiModerationThresholdsV1,
    run_nonce: Digest32,
    governance_signature: Signature,
}

struct AiModerationModelV1 {
    model_id: Uuid,
    family: AiModerationFamilyV1, // vision | multimodal | perceptual | audio
    artifact_digest: Digest32,
    weights_digest: Digest32,
    opset: u8,
    weight: f32,
    critical_labels: Vec<String>,
    max_eval_ms: u32,
}
```

### 4.2 Résultat d’évaluation
```norito
struct AiModerationResultV1 {
    manifest_id: Uuid,
    request_id: Uuid,
    content_digest: Digest32,
    content_uri: String,
    content_class: ModerationContentClassV1, // manifest | chunk | metadata | audio
    model_scores: Vec<AiModerationModelScoreV1>,
    combined_score: f32,
    verdict: ModerationVerdictV1, // pass | quarantine | escalate
    executed_at: Timestamp,
    execution_ms: u32,
    runner_hash: Digest32,
    annotations: Option<Vec<String>>,
}

struct AiModerationModelScoreV1 {
    model_id: Uuid,
    score: f32,
    threshold: f32,
    confidence: f32,
    label: Option<String>,
}
```

Le runner DOIT émettre un `AiModerationDigestV1` déterministe (BLAKE3 sur le résultat sérialisé) pour les journaux de transparence et ajouter les résultats au ledger de modération lorsque le verdict n’est pas `pass`.

### 4.3 Manifeste du corpus adversarial

Les opérateurs de gateway ingèrent désormais un manifeste compagnon qui énumère les “familles” de hash/embeddings perceptuels dérivées des exécutions de calibration :

```norito
struct AdversarialCorpusManifestV1 {
    schema_version: u16,                // must equal 1
    issued_at_unix: u64,
    cohort_label: Option<String>,       // e.g. "2026-Q1"
    families: Vec<AdversarialPerceptualFamilyV1>,
}

struct AdversarialPerceptualFamilyV1 {
    family_id: Uuid,
    description: String,
    variants: Vec<AdversarialPerceptualVariantV1>,
}

struct AdversarialPerceptualVariantV1 {
    variant_id: Uuid,
    attack_vector: String,
    reference_cid_b64: Option<String>,
    perceptual_hash: Option<Digest32>,   // Goldilocks hash, BLAKE3 domain separated
    hamming_radius: u8,                  // ≤ 32
    embedding_digest: Option<Digest32>,  // BLAKE3 of quantised embedding vector
    notes: Option<String>,
}
```

Le schéma vit dans `crates/iroha_data_model/src/sorafs/moderation.rs` et est validé via `AdversarialCorpusManifestV1::validate()`. Le manifeste permet au chargeur de denylist de renseigner des entrées `perceptual_family` qui bloquent des clusters entiers de quasi-doublons au lieu d’octets individuels. Un fixture exécutable (`docs/examples/ai_moderation_perceptual_registry_202602.json`) démontre le layout attendu et alimente directement la denylist de gateway de démonstration.

## 5. Pipeline d’exécution
1. Charger `AiModerationManifestV1` depuis le DAG de gouvernance. Rejeter si `runner_hash` ou `runtime_version` ne correspondent pas au binaire déployé.
2. Récupérer les artefacts modèles via digest OCI, en vérifiant les digests avant chargement.
3. Construire des lots d’évaluation par type de contenu ; l’ordre doit trier par `(content_digest, manifest_id)` pour assurer une agrégation déterministe.
4. Exécuter chaque modèle avec la graine dérivée. Pour les hash perceptuels, combiner l’ensemble par vote majoritaire → score dans `[0,1]`.
5. Agréger les scores dans `combined_score` via le ratio tronqué pondéré :
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. Produire `ModerationVerdictV1` :
   - `escalate` si une `critical_labels` se déclenche ou si `combined ≥ thresholds.escalate`.
   - `quarantine` si supérieur à `thresholds.quarantine` mais inférieur à `escalate`.
   - `pass` sinon.
7. Persister `AiModerationResultV1` et mettre en file les processus en aval :
   - Service de quarantaine (si le verdict escalade/quarantaine)
   - Écrivain du journal de transparence (`ModerationLedgerV1`)
   - Exporteur de télémétrie

## 6. Calibration et évaluation
- **Datasets :** la calibration de base utilise le corpus mixte validé par l’équipe policy. Référence enregistrée dans `calibration_dataset`.
- **Métriques :** calculer Brier, Expected Calibration Error (ECE) et AUROC par modèle et verdict combiné. La recalibration mensuelle DOIT maintenir `Brier ≤ 0.18` et `ECE ≤ 0.05`. Les résultats sont stockés dans l’arborescence des rapports SoraFS (ex. [calibration février 2026](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Planning :** recalibration mensuelle (premier lundi). Recalibration d’urgence autorisée si des alertes de dérive se déclenchent.
- **Processus :** exécuter le pipeline d’évaluation déterministe sur le jeu de calibration, régénérer `thresholds`, mettre à jour le manifeste et préparer les changements pour vote de gouvernance.

## 7. Packaging et déploiement
- Construire les images OCI via `docker buildx bake -f docker/ai_moderation.hcl`.
- Les images incluent :
  - Environnement Python verrouillé (`poetry.lock`) ou binaire Rust `Cargo.lock`.
  - Répertoire `models/` avec des poids ONNX hashés.
  - Point d’entrée `run_moderation.py` (ou équivalent Rust) exposant une API HTTP/gRPC.
- Publier les artefacts dans `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- Le binaire du runner est livré dans le crate `sorafs_ai_runner`. La pipeline de build embarque le hash du manifeste dans le binaire (exposé via `/v2/info`).

## 8. Télémétrie et observabilité
- Métriques Prometheus :
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Logs : lignes JSON avec `request_id`, `manifest_id`, `verdict` et le digest du résultat stocké. Les scores bruts sont redactionnés à deux décimales dans les logs.
- Dashboards stockés dans `dashboards/grafana/ministry_moderation_overview.json` (publiés avec le premier rapport de calibration).
- Seuils d’alerte :
  - Ingestion manquante (`moderation_requests_total` à l’arrêt pendant 10 minutes).
  - Détection de dérive (delta moyen des scores modèle >20% vs moyenne mobile 7 jours).
  - Backlog de faux positifs (file de quarantaine > 50 éléments pendant >30 minutes).

## 9. Gouvernance et contrôle des changements
- Les manifests requièrent une double signature : membre du conseil du Ministère + lead SRE modération. Les signatures sont enregistrées dans `AiModerationManifestV1.governance_signature`.
- Les changements suivent `ModerationManifestChangeProposalV1` via Torii. Les hashes sont inscrits dans le DAG de gouvernance ; le déploiement est bloqué tant que la proposition n’est pas promulguée.
- Les binaires du runner embarquent `runner_hash` ; la CI refuse le déploiement si les hashes divergent.
- Transparence : `ModerationScorecardV1` hebdomadaire résumant volume, mix de verdicts et résultats d’appel. Publié sur le portail du Parlement Sora.

## 10. Sécurité et confidentialité
- Les digests de contenu utilisent BLAKE3. Les payloads bruts ne persistent jamais en dehors de la quarantaine.
- L’accès à la quarantaine requiert des approbations Just-In-Time ; tous les accès sont journalisés.
- Le runner sandboxe le contenu non fiable, impose des limites mémoire de 512 MiB et des garde-fous de 120 s de wall-clock.
- La confidentialité différentielle n’est PAS appliquée ici ; les gateways s’appuient sur la quarantaine + les workflows d’audit. Les politiques de redaction suivent le plan de conformité gateway (`docs/source/sorafs_gateway_compliance_plan.md` ; copie portail en attente).

## 11. Publication de la calibration (2026-02)
- **Manifeste :** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  enregistre le `AiModerationManifestV1` signé par la gouvernance (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), la référence dataset
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, le hash runner
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`, et les
  seuils de calibration 2026-02 (`quarantine = 0.42`, `escalate = 0.78`).
- **Scoreboard :** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  plus le rapport lisible dans
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  capturent Brier, ECE, AUROC et le mix de verdicts pour chaque modèle. Les métriques combinées atteignent les objectifs (`Brier = 0.126`, `ECE = 0.034`).
- **Dashboards et alertes :** `dashboards/grafana/ministry_moderation_overview.json`
  et `dashboards/alerts/ministry_moderation_rules.yml` (avec tests de régression dans
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) fournissent le suivi ingestion/latence/dérive requis pour le rollout.

## 12. Schéma de reproductibilité et validateur (MINFO-1b)
- Les types Norito canoniques vivent désormais avec le reste du schéma SoraFS dans
  `crates/iroha_data_model/src/sorafs/moderation.rs`. Les structures
  `ModerationReproManifestV1`/`ModerationReproBodyV1` capturent l’UUID du manifeste, le hash du runner, les digests des modèles, le jeu de seuils et la matière de graine.
  `ModerationReproManifestV1::validate` impose la version de schéma
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), garantit que chaque manifeste porte au moins un modèle et un signataire, et vérifie chaque `SignatureOf<ModerationReproBodyV1>` avant de retourner un résumé lisible par machine.
- Les opérateurs peuvent invoquer le validateur partagé via
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (implémenté dans `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). Le CLI
  accepte les artefacts JSON publiés sous
  `docs/examples/ai_moderation_calibration_manifest_202602.json` ou l’encodage Norito brut et imprime les comptes modèles/signatures ainsi que l’horodatage du manifeste après validation.
- Les gateways et l’automatisation s’appuient sur le même helper pour rejeter de manière déterministe les manifests de reproductibilité lorsque les schémas dérivent, que des digests manquent ou que les signatures échouent.
- Les bundles de corpus adversarial suivent le même modèle :
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  parse `AdversarialCorpusManifestV1`, impose la version de schéma et refuse les manifests qui omettent des familles, variantes ou métadonnées d’empreinte. Les exécutions réussies émettent l’horodatage d’émission, l’étiquette de cohorte et les comptes familles/variantes afin que les opérateurs puissent épingler la preuve avant de mettre à jour les entrées de denylist gateway décrites en Section 4.3.

## 13. Suivis ouverts
- Les fenêtres de recalibration mensuelle après le 2026-03-02 continuent de suivre la procédure de la Section 6 ; publier `ai-moderation-calibration-<YYYYMM>.md` avec les bundles manifeste/scorecard mis à jour sous l’arborescence des rapports SoraFS.
- MINFO-1b et MINFO-1c (validateurs de manifests de reproductibilité plus registre du corpus adversarial) restent suivis séparément dans le roadmap.
