---
lang: fr
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2026-01-03T18:07:56.917770+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Voie de calcul (SSC-1)

La voie de calcul accepte les appels déterministes de style HTTP et les mappe sur Kotodama
points d'entrée et enregistre les relevés/reçus pour la facturation et l'examen de la gouvernance.
Cette RFC gèle le schéma du manifeste, les enveloppes d'appel/réception, les garde-corps du bac à sable,
et les paramètres de configuration par défaut pour la première version.

## Manifeste

- Schéma : `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` est épinglé à `1` ; les manifestes avec une version différente sont rejetés
  lors de la validation.
- Chaque itinéraire déclare :
  -`id` (`service`, `method`)
  - `entrypoint` (nom du point d'entrée Kotodama)
  - liste autorisée des codecs (`codecs`)
  - Capuchons TTL/gaz/demande/réponse (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - classe déterminisme/exécution (`determinism`, `execution_class`)
  - Descripteurs d'entrée/modèle SoraFS (`input_limits`, `model` en option)
  - famille de tarification (`price_family`) + profil de ressources (`resource_profile`)
  - politique d'authentification (`auth`)
- Les garde-corps du bac à sable se trouvent dans le bloc manifeste `sandbox` et sont partagés par tous
  routes (mode/caractère aléatoire/stockage et rejet d’appel système non déterministe).

Exemple : `fixtures/compute/manifest_compute_payments.json`.

## Appels, demandes et reçus

- Schéma : `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` dans
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` produit le hachage canonique de la requête (les en-têtes sont conservés
  dans un `BTreeMap` déterministe et la charge utile est transportée sous le nom `payload_hash`).
- `ComputeCall` capture l'espace de noms/route, le codec, le TTL/gas/cap de réponse,
  profil de ressource + famille de prix, authentification (`Public` ou lié à l'UAID
  `ComputeAuthn`), déterminisme (`Strict` vs `BestEffort`), classe d'exécution
  astuces (CPU/GPU/TEE), octets/morceaux d'entrée déclarés SoraFS, sponsor facultatif
  budget, et l’enveloppe canonique de la demande. Le hachage de la requête est utilisé pour
  protection contre la relecture et routage.
- Les itinéraires peuvent intégrer des références de modèle SoraFS et des limites d'entrée en option
  (capsules en ligne/morceaux) ; les règles du bac à sable manifestes donnent des conseils sur le GPU/TEE.
- `ComputePriceWeights::charge_units` convertit les données de comptage en calcul facturé
  unités via la division de plafond sur les cycles et les octets de sortie.
- `ComputeOutcome` rapporte `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted` ou `InternalError` et inclut éventuellement des hachages de réponse/
  tailles/codec pour l’audit.

Exemples :
- Appel : `fixtures/compute/call_compute_payments.json`
- Reçu : `fixtures/compute/receipt_compute_payments.json`

## Sandbox et profils de ressources- `ComputeSandboxRules` verrouille le mode d'exécution sur `IvmOnly` par défaut,
  génère le caractère aléatoire déterministe à partir du hachage de la demande, permet la lecture seule SoraFS
  accès et rejette les appels système non déterministes. Les indices GPU/TEE sont contrôlés par
  `allow_gpu_hints`/`allow_tee_hints` pour maintenir l'exécution déterministe.
- `ComputeResourceBudget` définit les plafonds par profil sur les cycles, la mémoire linéaire, la pile
  taille, budget IO et sortie, ainsi que des bascules pour les astuces GPU et les aides WASI-lite.
- Les valeurs par défaut expédient deux profils (`cpu-small`, `cpu-balanced`) sous
  `defaults::compute::resource_profiles` avec solutions de repli déterministes.

## Unités de tarification et de facturation

- Les familles de prix (`ComputePriceWeights`) mappent les cycles et les octets de sortie dans le calcul
  unités; les valeurs par défaut facturent `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` avec
  `unit_label = "cu"`. Les familles sont saisies par `price_family` dans les manifestes et
  appliqué à l’admission.
- Les enregistrements de comptage portent `charged_units` plus cycle brut/entrée/sortie/durée
  totaux pour le rapprochement. Les frais sont amplifiés par classe d'exécution et
  multiplicateurs de déterminisme (`ComputePriceAmplifiers`) et plafonnés par
  `compute.economics.max_cu_per_call` ; la sortie est bloquée par
  `compute.economics.max_amplification_ratio` pour l'amplification de réponse liée.
- Les budgets des sponsors (`ComputeCall::sponsor_budget_cu`) sont appliqués contre
  plafonds par appel/quotidiens ; les unités facturées ne doivent pas dépasser le budget déclaré du sponsor.
- Les mises à jour des prix de gouvernance utilisent les limites des classes de risque dans
  `compute.economics.price_bounds` et les familles de référence enregistrées dans
  `compute.economics.price_family_baseline` ; utiliser
  `ComputeEconomics::apply_price_update` pour valider les deltas avant la mise à jour
  la carte familiale active. Utilisation des mises à jour de configuration Torii
  `ConfigUpdate::ComputePricing`, et kiso l'applique avec les mêmes limites à
  garder les modifications de gouvernance déterministes.

##Configuration

La nouvelle configuration de calcul se trouve dans `crates/iroha_config/src/parameters` :

- Vue utilisateur : `Compute` (`user.rs`) avec remplacements d'environnement :
  - `COMPUTE_ENABLED` (par défaut `false`)
  -`COMPUTE_DEFAULT_TTL_SLOTS`/`COMPUTE_MAX_TTL_SLOTS`
  -`COMPUTE_MAX_REQUEST_BYTES`/`COMPUTE_MAX_RESPONSE_BYTES`
  -`COMPUTE_MAX_GAS_PER_CALL`
  -`COMPUTE_DEFAULT_RESOURCE_PROFILE`/`COMPUTE_DEFAULT_PRICE_FAMILY`
  -`COMPUTE_AUTH_POLICY`
- Tarification/économie : captures `compute.economics`
  `max_cu_per_call`/`max_amplification_ratio`, partage des frais, plafonds de sponsor
  (par appel et CU quotidiennes), lignes de base de la famille de prix + classes/limites de risque pour
  mises à jour de gouvernance et multiplicateurs de classe d’exécution (GPU/TEE/meilleur effort).
- Réel/par défaut : `actual.rs` / `defaults.rs::compute` exposé analysé
  Paramètres `Compute` (espaces de noms, profils, familles de prix, sandbox).
- Configurations invalides (espaces de noms vides, profil/famille par défaut manquant, plafond TTL
  inversions) apparaissent sous la forme `InvalidComputeConfig` lors de l'analyse.

## Tests et montages

- Les assistants déterministes (`request_hash`, tarification) et les allers-retours de luminaires vivent dans
  `crates/iroha_data_model/src/compute/mod.rs` (voir `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- Les appareils JSON résident dans `fixtures/compute/` et sont exercés par le modèle de données
  tests de couverture de régression.

## Exploitation et budgets SLO- La configuration `compute.slo.*` expose les boutons SLO de la passerelle (file d'attente en vol
  profondeur, plafond RPS et cibles de latence) dans
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Valeurs par défaut : 32
  en vol, 512 en file d'attente par itinéraire, 200 RPS, p50 25 ms, p95 75 ms, p99 120 ms.
- Exécutez le harnais de banc léger pour capturer les résumés SLO et une demande/sortie
  instantané : `cargo run -p xtask --bin computation_gateway -- bench [manifest_path]
  [itérations] [concurrency] [out_dir]` (defaults: `fixtures/compute/manifest_compute_payments.json`,
  128 itérations, concurrence 16, sorties sous
  `artifacts/compute_gateway/bench_summary.{json,md}`). Le banc utilise
  charges utiles déterministes (`fixtures/compute/payload_compute_payments.json`) et
  en-têtes par requête pour éviter les collisions de relecture pendant l'exercice
  Points d'entrée `echo`/`uppercase`/`sha3`.

## Appareils de parité SDK/CLI

- Les appareils canoniques sont sous `fixtures/compute/` : manifeste, appel, charge utile et
  la disposition de réponse/réception de style passerelle. Les hachages de charge utile doivent correspondre à l'appel
  `request.payload_hash` ; la charge utile d'assistance réside dans
  `fixtures/compute/payload_compute_payments.json`.
- La CLI est livrée avec `iroha compute simulate` et `iroha compute invoke` :

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS : `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` en direct
  `javascript/iroha_js/src/compute.js` avec tests de régression sous
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift : `ComputeSimulator` charge les mêmes appareils, valide les hachages de charge utile,
  et simule les points d'entrée avec des tests dans
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- Les assistants CLI/JS/Swift partagent tous les mêmes appareils Norito afin que les SDK puissent
  valider la construction de la requête et la gestion du hachage hors ligne sans toucher à un
  passerelle en cours d'exécution.