---
lang: es
direction: ltr
source: docs/references/configuration.md
status: complete
translator: manual
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-11-02T04:40:39.795595+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/references/configuration.md (Acceleration) -->

# Aceleración

La sección `[accel]` controla la aceleración opcional por hardware para IVM y
sus helpers. Todas las rutas aceleradas disponen de fallbacks deterministas en
CPU; si un backend falla una auto‑prueba golden en tiempo de ejecución, se
desactiva automáticamente y la ejecución continúa en CPU.

- `enable_cuda` (valor por defecto: `true`) – usar CUDA cuando esté compilado y
  disponible.
- `enable_metal` (valor por defecto: `true`) – usar Metal en macOS cuando esté
  disponible.
- `max_gpus` (valor por defecto: `0`) – número máximo de GPUs a inicializar;
  `0` significa auto/sin límite explícito.
- `merkle_min_leaves_gpu` (valor por defecto: `8192`) – número mínimo de hojas
  a partir del cual se delega el hashing de hojas Merkle a la GPU. Sólo
  reducir para GPUs inusualmente rápidas.
- Opciones avanzadas (habitualmente heredan valores razonables):
  - `merkle_min_leaves_metal` (por defecto: hereda `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (por defecto: hereda `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (por defecto: `32768`) – preferir
    SHA‑2 en CPU hasta este número de hojas en ARMv8 con soporte SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (por defecto: `32768`) – preferir SHA‑NI
    en CPU hasta este número de hojas en x86/x86_64.

Notas
- La prioridad es la determinismo: la aceleración nunca cambia los resultados
  observables; los backends ejecutan tests golden al inicializarse y hacen
  fallback a rutas escalares/SIMD cuando se detectan discrepancias.
- Configura siempre vía `iroha_config`; evita el uso de variables de entorno
  en producción.

