---
lang: es
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2026-01-03T18:08:01.361022+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guía de ejecución del agente de automatización

Esta página resume las barreras operativas para cualquier agente de automatización.
trabajando dentro del espacio de trabajo Hyperledger Iroha. Refleja lo canónico.
Guía `AGENTS.md` y las referencias de la hoja de ruta para compilar, documentar y
Todos los cambios de telemetría tienen el mismo aspecto, ya sea que hayan sido producidos por un humano o por un humano.
un colaborador automatizado.

Se espera que cada tarea genere código determinista además de documentos, pruebas y
y evidencia operativa. Trate las secciones siguientes como referencia inmediata antes
tocar elementos `roadmap.md` o responder preguntas de comportamiento.

## Comandos de inicio rápido

| Acción | Comando |
|--------|---------|
| Construya el espacio de trabajo | `cargo build --workspace` |
| Ejecute el conjunto de pruebas completo | `cargo test --workspace` *(normalmente tarda varias horas)* |
| Ejecute clippy con advertencias de denegación predeterminadas | `cargo clippy --workspace --all-targets -- -D warnings` |
| Formatear código Rust | `cargo fmt --all` *(edición 2024)* |
| Pruebe una sola caja | `cargo test -p <crate>` |
| Ejecute una prueba | `cargo test -p <crate> <test_name> -- --nocapture` |
| Pruebas rápidas del SDK | Desde `IrohaSwift/`, ejecute `swift test` |

## Fundamentos del flujo de trabajo

- Lea las rutas de código relevantes antes de responder preguntas o cambiar la lógica.
- Dividir elementos grandes de la hoja de ruta en confirmaciones manejables; Nunca rechaces el trabajo de plano.
- Permanezca dentro de la membresía del espacio de trabajo existente, reutilice las cajas internas y no
  **no** alterar `Cargo.lock` a menos que se indique explícitamente.
- Utilice indicadores de funciones y alternancias de capacidad solo cuando lo exija el hardware
  aceleradores; Mantenga respaldos deterministas disponibles en todas las plataformas.
- Actualizar la documentación y las referencias de Markdown junto con cualquier cambio funcional.
  entonces los documentos siempre describen el comportamiento actual.
- Agregue al menos una prueba unitaria para cada función nueva o modificada. Prefiero en línea
  Módulos `#[cfg(test)]` o la carpeta `tests/` de la caja, según el alcance.
- Después de terminar el trabajo, actualice `status.md` con un breve resumen y referencia.
  archivos relevantes; mantenga `roadmap.md` enfocado en elementos que aún necesitan trabajo.

## Barandillas de implementación

### Serialización y modelos de datos
- Utilice el códec Norito en todas partes (binario a través de `norito::{Encode, Decode}`,
  JSON a través de `norito::json::*`). No agregue el uso directo de serde/`serde_json`.
- Las cargas útiles Norito deben anunciar su diseño (byte de versión o indicadores de encabezado),
  y los nuevos formatos requieren las correspondientes actualizaciones de la documentación (p. ej.,
  `norito.md`, `docs/source/da/*.md`).
- Los datos de Génesis, los manifiestos y las cargas útiles de redes deben seguir siendo deterministas.
  entonces dos pares con las mismas entradas producen hashes idénticos.

### Configuración y comportamiento en tiempo de ejecución
- Prefiera las perillas que viven en `crates/iroha_config` a las nuevas variables de entorno.
  Enhebre valores explícitamente a través de constructores o inyección de dependencia.
- Nunca controle las llamadas al sistema IVM ni el comportamiento del código de operación: ABI v1 se envía a todas partes.
- Cuando se agregan nuevas opciones de configuración, actualice los valores predeterminados, los documentos y cualquier tema relacionado.
  plantillas (`peer.template.toml`, `docs/source/configuration*.md`, etc.).### ABI, llamadas al sistema y tipos de puntero
- Tratar la política ABI como incondicional. Agregar/eliminar llamadas al sistema o tipos de puntero
  requiere actualización:
  - `ivm::syscalls::abi_syscall_list` y `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` más las pruebas doradas
  - `crates/ivm/tests/abi_hash_versions.rs` cada vez que cambia el hash ABI
- Las llamadas al sistema desconocidas deben asignarse a `VMError::UnknownSyscall` y los manifiestos deben
  conservar los controles de igualdad firmados `abi_hash` en las pruebas de admisión.

### Aceleración y determinismo de hardware
- Las nuevas primitivas criptográficas o matemáticas pesadas deben enviarse aceleradas por hardware.
  rutas (METAL/NEON/SIMD/CUDA) manteniendo retrocesos deterministas.
- evitar reducciones paralelas no deterministas; La prioridad son salidas idénticas en
  todos los pares incluso cuando el hardware difiere.
- Mantenga reproducibles los accesorios Norito y FASTPQ para que SRE pueda realizar auditorías en toda la flota
  telemetría.

### Documentación y evidencia
- Reflejar cualquier cambio de documento público en el portal (`docs/portal/...`) cuando
  aplicable para que el sitio de documentos se mantenga actualizado con las fuentes de Markdown.
- Cuando se introducen nuevos flujos de trabajo, agregue runbooks, notas de gobernanza o
  listas de verificación que explican cómo ensayar, revertir y capturar evidencia.
- Al traducir contenido al acadio, proporcione representaciones semánticas escritas
  en transliteraciones cuneiformes en lugar de fonéticas.

### Expectativas de pruebas y herramientas
- Ejecute los conjuntos de pruebas relevantes localmente (`cargo test`, `swift test`,
  arneses de integración) y documente los comandos en la sección de pruebas de relaciones públicas.
- Mantenga los scripts de protección de CI (`ci/*.sh`) y los paneles sincronizados con la nueva telemetría.
- Para proc-macros, empareje las pruebas unitarias con las pruebas de UI `trybuild` para bloquear los diagnósticos.

## Lista de verificación lista para enviar

1. El código se compila y `cargo fmt` no produjo diferencias.
2. Los documentos actualizados (espacio de trabajo Markdown más espejos del portal) describen el nuevo
   comportamiento, nuevos indicadores CLI o botones de configuración.
3. Las pruebas cubren cada ruta de código nueva y fallan de manera determinista cuando se realizan regresiones.
   aparecer.
4. La telemetría, los paneles y las definiciones de alerta hacen referencia a cualquier métrica nueva o
   códigos de error.
5. `status.md` incluye un breve resumen que hace referencia a los archivos relevantes y
   sección de hoja de ruta.

Seguir esta lista de verificación mantiene la ejecución de la hoja de ruta auditable y garantiza que cada
El agente aporta evidencia en la que otros equipos pueden confiar.