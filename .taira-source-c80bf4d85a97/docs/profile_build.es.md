---
lang: es
direction: ltr
source: docs/profile_build.md
status: complete
translator: manual
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-11-02T04:40:28.811778+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/profile_build.md (Profiling iroha_data_model Build) -->

# Perfilado del build de `iroha_data_model`

Para localizar pasos lentos en la compilación de `iroha_data_model`, ejecuta el script de
ayuda:

```sh
./scripts/profile_build.sh
```

Este comando lanza `cargo build -p iroha_data_model --timings` y escribe los informes de
tiempos en `target/cargo-timings/`.
Abre `cargo-timing.html` en un navegador y ordena las tareas por duración para ver qué
crates o pasos de build consumen más tiempo.

Utiliza estos tiempos para centrar los esfuerzos de optimización en las tareas más lentas.

