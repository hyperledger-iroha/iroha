---
lang: fr
direction: ltr
source: docs/references/configuration.md
status: complete
translator: manual
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-11-02T04:40:39.795595+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/references/configuration.md (Acceleration) -->

# Accélération

La section `[accel]` contrôle l’activation optionnelle de l’accélération
matérielle pour IVM et ses helpers. Tous les chemins accélérés disposent de
fallbacks déterministes sur CPU ; si un backend échoue à un golden self‑test
au runtime, il est automatiquement désactivé et l’exécution se poursuit sur
CPU.

- `enable_cuda` (par défaut : `true`) – utiliser CUDA lorsqu’il est compilé et
  disponible.
- `enable_metal` (par défaut : `true`) – utiliser Metal sur macOS lorsqu’il est
  disponible.
- `max_gpus` (par défaut : `0`) – nombre maximal de GPU à initialiser ; `0`
  signifie auto/sans limite explicite.
- `merkle_min_leaves_gpu` (par défaut : `8192`) – nombre minimal de feuilles à
  partir duquel le hachage des feuilles Merkle est délégué au GPU. Ne réduire
  que pour des GPU exceptionnellement rapides.
- Paramètres avancés (optionnels ; les valeurs par défaut sont généralement
  suffisantes) :
  - `merkle_min_leaves_metal` (par défaut : hérite de `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (par défaut : hérite de `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (par défaut : `32768`) – préférer
    SHA‑2 sur CPU jusqu’à ce nombre de feuilles sur ARMv8 avec support SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (par défaut : `32768`) – préférer SHA‑NI
    sur CPU jusqu’à ce nombre de feuilles sur x86/x86_64.

Notes
- Le déterminisme avant tout : l’accélération ne modifie jamais les résultats
  observables ; les backends exécutent des tests golden à l’initialisation et
  basculent vers des chemins scalaire/SIMD dès qu’une divergence est détectée.
- Configurez via `iroha_config` ; évitez les variables d’environnement en
  production.

