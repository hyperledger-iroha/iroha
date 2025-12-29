---
lang: es
direction: ltr
source: docs/source/README.md
status: complete
translator: manual
source_hash: 4a6e55a3232ff38c5c2f45b0a8a3d97471a14603bea75dc2034d7c9c4fb3f862
source_last_modified: "2025-11-10T19:43:50.185052+00:00"
translation_last_reviewed: 2025-11-14
---

# Índice de documentación de Iroha VM y Kotodama

Este índice enlaza los principales documentos de diseño y referencia para IVM,
Kotodama y el pipeline centrado en la IVM. Para una versión en japonés, consulta
[`README.ja.md`](./README.ja.md).

- Arquitectura de IVM y correspondencia con el lenguaje: `../../ivm.md`
- ABI de syscalls de IVM: `ivm_syscalls.md`
- Constantes de syscalls generadas: `ivm_syscalls_generated.md` (ejecuta `make docs-syscalls` para regenerar)
- Cabecera de bytecode de IVM: `ivm_header.md`
- Gramática y semántica de Kotodama: `kotodama_grammar.md`
- Ejemplos de Kotodama y mapeo de syscalls: `kotodama_examples.md`
- Pipeline de transacciones (IVM‑first): `../../new_pipeline.md`
- API de contratos de Torii (manifiestos): `torii_contracts_api.md`
- Sobre JSON de consultas (CLI / tooling): `query_json.md`
- Referencia del módulo de streaming Norito: `norito_streaming.md`
- Muestras de ABI en tiempo de ejecución: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- API de aplicaciones ZK (adjuntos, probador, recuento de votos): `zk_app_api.md`
- Runbook de adjuntos/probador ZK en Torii: `zk/prover_runbook.md`
- Guía operativa de la API ZK App de Torii (adjuntos/probador; doc del crate): `../../crates/iroha_torii/docs/zk_app_api.md`
- Ciclo de vida de VK/proofs (registro, verificación, telemetría): `zk/lifecycle.md`
- Ayudas operativas de Torii (endpoints de visibilidad): `references/operator_aids.md`
- Guía rápida del carril por defecto de Nexus: `quickstart/default_lane.md`
- Guía y arquitectura del supervisor MOCHI: `mochi/index.md`
- Guías del SDK de JavaScript (inicio rápido, configuración, publicación): `sdk/js/index.md`
- Paneles de paridad/CI del SDK de Swift: `references/ios_metrics.md`
- Gobernanza: `../../gov.md`
- Prompts de coordinación y aclaración: `coordination_llm_prompts.md`
- Hoja de ruta: `../../roadmap.md`
- Uso de la imagen de compilación para Docker: `docker_build.md`

Consejos de uso

- Compila y ejecuta ejemplos en `examples/` usando las herramientas externas
  (`koto_compile`, `ivm_run`):
  - `make examples-run` (y `make examples-inspect` si `ivm_tool` está disponible).
- Las pruebas de integración opcionales (ignoradas por defecto) para ejemplos y
  comprobaciones de cabecera viven en `integration_tests/tests/`.

Configuración del pipeline

- Todo el comportamiento en tiempo de ejecución se configura mediante archivos
  `iroha_config`. Los operadores no usan variables de entorno.
- Se proporcionan valores por defecto razonables; la mayoría de los despliegues
  no necesitarán cambios.
- Claves relevantes bajo `[pipeline]`:
  - `dynamic_prepass`: habilita el pre‑análisis de solo lectura en IVM para
    derivar conjuntos de acceso (valor por defecto: true).
  - `access_set_cache_enabled`: guarda en caché los conjuntos de acceso
    derivados por `(code_hash, entrypoint)`; desactívalo para depurar los hints
    (valor por defecto: true).
  - `parallel_overlay`: construye overlays en paralelo; el commit sigue siendo
    determinista (por defecto: true).
  - `gpu_key_bucket`: agrupación opcional de claves para el pre‑análisis del
    planificador mediante radix estable sobre `(key, tx_idx, rw_flag)`; la ruta
    determinista en CPU siempre está activa (por defecto: false).
  - `cache_size`: capacidad de la caché global de pre‑decodificación IVM
    (streams decodificados). Valor por defecto: 128. Aumentarlo puede reducir
    el tiempo de decodificación para ejecuciones repetidas.

Comprobaciones de sincronización de docs

- Constantes de syscalls (`docs/source/ivm_syscalls_generated.md`)
  - Regenerar: `make docs-syscalls`
  - Sólo comprobar: `bash scripts/check_syscalls_doc.sh`
- Tabla de la ABI de syscalls (`crates/ivm/docs/syscalls.md`)
  - Sólo comprobar: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Actualizar la sección generada (y la tabla en los docs de código):
    `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Tablas de pointer‑ABI (`crates/ivm/docs/pointer_abi.md` e `ivm.md`)
  - Sólo comprobar: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Actualizar secciones: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- Política de cabecera de IVM y hashes de ABI (`docs/source/ivm_header.md`)
  - Sólo comprobar: `cargo run -p ivm --bin gen_header_doc -- --check` y
    `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Actualizar secciones: `cargo run -p ivm --bin gen_header_doc -- --write` y
    `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI

- El workflow de GitHub Actions `.github/workflows/check-docs.yml` ejecuta estas
  comprobaciones en cada push/PR y fallará si los documentos generados se
  desincronizan de la implementación.
- [Guía de gobernanza](governance_playbook.md)
