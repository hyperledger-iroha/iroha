---
lang: es
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-08T10:55:43+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guía contribuyente

¡Gracias por tomarse el tiempo para contribuir con Iroha 2!

Lea esta guía para saber cómo puede contribuir y qué pautas esperamos que siga. Esto incluye las pautas sobre código y documentación, así como nuestras convenciones sobre el flujo de trabajo de git.

Leer estas pautas le ahorrará tiempo más adelante.

## ¿Cómo puedo contribuir?

Hay muchas maneras en las que podrías contribuir a nuestro proyecto:

- Informar [errores](#reporting-bugs) y [vulnerabilidades](#reporting-vulnerabilities)
- [Sugerir mejoras](#suggesting-improvements) e implementarlas
- [Hacer preguntas](#asking-questions) e interactuar con la comunidad

¿Nuevo en nuestro proyecto? [Haz tu primera contribución](#your-first-code-contribution)!

### TL;DR

- Busque [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240).
- Horquilla [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Solucione el problema de su elección.
- Asegúrese de seguir nuestras [guías de estilo](#style-guides) para obtener código y documentación.
- Escribir [pruebas](https://doc.rust-lang.org/cargo/commands/cargo-test.html). Asegúrese de que todos pasen (`cargo test --workspace`). Si toca la pila de criptografía SM, ejecute también `cargo test -p iroha_crypto --features "sm sm_proptest"` para ejecutar el arnés de propiedad/fuzz opcional.
  - Nota: Las pruebas que ejercitan el ejecutor IVM sintetizarán automáticamente un código de bytes de ejecutor determinista mínimo si `defaults/executor.to` no está presente. No se requiere ningún paso previo para ejecutar pruebas. Para generar el código de bytes canónico para la paridad, puede ejecutar:
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- Si cambia las cajas de derivación/proc-macro, ejecute las suites de interfaz de usuario trybuild a través de
  `make check-proc-macro-ui` (o
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) y actualizar
  Accesorios `.stderr` cuando los diagnósticos cambian para mantener los mensajes estables.
- Ejecute `make dev-workflow` (envoltorio alrededor de `scripts/dev_workflow.sh`) para ejecutar fmt/clippy/build/test con `--locked` más `swift test`; espere que `cargo test --workspace` tarde horas y use `--skip-tests` solo para bucles locales rápidos. Consulte `docs/source/dev_workflow.md` para obtener el runbook completo.
- Aplique barreras de seguridad con `make check-agents-guardrails` para bloquear ediciones de `Cargo.lock` y nuevas cajas de espacio de trabajo, `make check-dependency-discipline` para fallar en nuevas dependencias a menos que se permita explícitamente, y `make check-missing-docs` para evitar nuevas calzas `#[allow(missing_docs)]`, documentos faltantes a nivel de caja en cajas tocadas o nuevos públicos. elementos sin comentarios de documentos (la guardia actualiza `docs/source/agents/missing_docs_inventory.{json,md}` a través de `scripts/inventory_missing_docs.py`). Agregue `make check-tests-guard` para que las funciones modificadas fallen a menos que las pruebas unitarias hagan referencia a ellas (bloques `#[cfg(test)]`/`#[test]` en línea o caja `tests/`; recuentos de cobertura existentes) y `make check-docs-tests-metrics` para que los cambios en la hoja de ruta se combinen con documentos, pruebas y métricas/paneles. Mantenga la aplicación de TODO a través de `make check-todo-guard` para que los marcadores TODO no se eliminen sin los documentos/pruebas que los acompañan. `make check-env-config-surface` regenera el inventario de alternancia de entorno y ahora falla cuando aparecen nuevas cuñas de entorno de **producción** en relación con `AGENTS_BASE_REF`; configure `ENV_CONFIG_GUARD_ALLOW=1` solo después de documentar las adiciones intencionales en el rastreador de migración. `make check-serde-guard` actualiza el inventario de servidores y falla en instantáneas obsoletas o visitas de nueva producción `serde`/`serde_json`; configure `SERDE_GUARD_ALLOW=1` solo con un plan de migración aprobado. Mantenga visibles los grandes aplazamientos a través de rutas de navegación TODO y tickets de seguimiento en lugar de aplazarlos en silencio. Ejecute `make check-std-only` para capturar los cfgs `no_std`/`wasm32` e `make check-status-sync` para garantizar que los elementos abiertos `roadmap.md` permanezcan solo abiertos y que los cambios de hoja de ruta/estado lleguen juntos; configure `STATUS_SYNC_ALLOW_UNPAIRED=1` solo para correcciones de errores tipográficos poco comunes después de fijar `AGENTS_BASE_REF`. Para una única invocación, utilice `make agents-preflight` para ejecutar todas las barreras de seguridad juntas.
- Ejecute guardias de serialización locales antes de presionar: `make guards`.
  - Esto niega el `serde_json` directo en el código de producción, no permite nuevos departamentos de servicio directo fuera de la lista de permitidos y evita que los ayudantes ad hoc de AoS/NCB se encuentren fuera de `crates/norito`.
- Opcionalmente, ejecute la matriz de funciones Norito localmente: `make norito-matrix` (utiliza un subconjunto rápido).
  - Para una cobertura completa, ejecute `scripts/run_norito_feature_matrix.sh` sin `--fast`.
  - Para incluir un humo aguas abajo por combo (caja predeterminada `iroha_data_model`): `make norito-matrix-downstream` o `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- Para cajas de proc-macro, agregue un arnés de interfaz de usuario `trybuild` (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) y confirme los diagnósticos `.stderr` para los casos defectuosos. Mantenga los diagnósticos estables y sin pánico; actualice los accesorios con `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` y protéjalos con `cfg(all(feature = "trybuild-tests", not(coverage)))`.
- Realizar rutinas de confirmación previa, como formato y regeneración de artefactos (consulte [`pre-commit.sample`](./hooks/pre-commit.sample))
- Con `upstream` configurado para rastrear [repositorio Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha), `git pull -r upstream main`, `git commit -s`, `git push <your-fork>` y [crear una extracción solicitud](https://github.com/hyperledger-iroha/iroha/compare) a la sucursal `main`. Asegúrese de que siga las [pautas de solicitud de extracción] (#pull-request-etiquette).

### Inicio rápido del flujo de trabajo de AGENTES

- Ejecute `make dev-workflow` (contenedor alrededor de `scripts/dev_workflow.sh`, documentado en `docs/source/dev_workflow.md`). Incluye `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build/test --workspace --locked` (las pruebas pueden tardar varias horas) e `swift test`.
- Utilice `scripts/dev_workflow.sh --skip-tests` o `--skip-swift` para iteraciones más rápidas; Vuelva a ejecutar la secuencia completa antes de abrir una solicitud de extracción.
- Guardrails: evite tocar `Cargo.lock`, agregar nuevos miembros del espacio de trabajo, introducir nuevas dependencias, agregar nuevos shims `#[allow(missing_docs)]`, omitir documentos a nivel de caja, omitir pruebas al cambiar funciones, eliminar marcadores TODO sin documentos/pruebas o reintroducir `no_std`/`wasm32` cfgs sin aprobación. Ejecute `make check-agents-guardrails` (o `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) más `make check-dependency-discipline`, `make check-missing-docs` (actualiza `docs/source/agents/missing_docs_inventory.{json,md}`), `make check-tests-guard` (falla cuando las funciones de producción cambian sin evidencia de prueba unitaria; ya sea que las pruebas cambien en la diferencia o las pruebas existentes deben hacer referencia a la función), `make check-docs-tests-metrics` (falla cuando los cambios en la hoja de ruta carecen de actualizaciones de documentos/pruebas/métricas), `make check-todo-guard`, `make check-env-config-surface` (falla en inventarios obsoletos o nuevos cambios de entorno de producción; anular con `ENV_CONFIG_GUARD_ALLOW=1` solo después de actualizar los documentos) e `make check-serde-guard` (falla en inventarios de servidores obsoletos o accesos de servidores nuevos de producción; anule con `SERDE_GUARD_ALLOW=1` solo con un plan de migración aprobado) localmente para señal temprana, `make check-std-only` para la protección estándar únicamente y mantenga `roadmap.md`/`status.md` sincronizado con `make check-status-sync` (configurado `STATUS_SYNC_ALLOW_UNPAIRED=1` solo para correcciones de errores tipográficos poco comunes de estado después de fijar `AGENTS_BASE_REF`). Utilice `make agents-preflight` si desea que un solo comando ejecute todos los guardias antes de abrir un PR.

### Informar errores

Un *error* es un error, falla de diseño, fallo o falla en Iroha que hace que se produzca un resultado o comportamiento incorrecto, inesperado o no deseado.

Realizamos un seguimiento de los errores Iroha a través de [Problemas de GitHub] (https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) etiquetados con la etiqueta `Bug`.

Cuando crea un nuevo problema, hay una plantilla que debe completar. Aquí está la lista de verificación de lo que debe hacer cuando informa errores:
- [] Agregue la etiqueta `Bug`
- [] Explica el problema.
- [] Proporcione un ejemplo de trabajo mínimo
- [] Adjunte una captura de pantalla

<detalles> <summary>Ejemplo de trabajo mínimo</summary>

Para cada error, debe proporcionar un [ejemplo de trabajo mínimo](https://en.wikipedia.org/wiki/Minimal_working_example). Por ejemplo:

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</detalles>

---
**Nota:** Problemas como documentación desactualizada, documentación insuficiente o solicitudes de funciones deben usar las etiquetas `Documentation` o `Enhancement`. No son errores.

---

### Informar de vulnerabilidades

Si bien somos proactivos en la prevención de problemas de seguridad, es posible que usted se encuentre con una vulnerabilidad de seguridad antes que nosotros.

- Antes de la primera versión principal (2.0), todas las vulnerabilidades se consideran errores, así que no dude en enviarlas como errores [siguiendo las instrucciones anteriores] (#reporting-bugs).
- Después de la primera versión importante, utilice nuestro [programa de recompensas por errores] (https://hackerone.com/hyperledger) para enviar vulnerabilidades y obtener su recompensa.

:exclamación: Para minimizar el daño causado por una vulnerabilidad de seguridad sin parches, debe divulgar la vulnerabilidad directamente a Hyperledger lo antes posible y **evitar divulgar la misma vulnerabilidad públicamente** durante un período de tiempo razonable.

Si tiene alguna pregunta sobre nuestro manejo de las vulnerabilidades de seguridad, no dude en comunicarse con cualquiera de los mantenedores actualmente activos en los mensajes privados de Rocket.Chat.

### Sugerir mejoras

Cree [un problema](https://github.com/hyperledger-iroha/iroha/issues/new) en GitHub con las etiquetas apropiadas (`Optimization`, `Enhancement`) y describa la mejora que sugiere. Puede dejarnos esta idea a nosotros o a otra persona para que la desarrollemos, o puede implementarla usted mismo.

Si tiene la intención de implementar la sugerencia usted mismo, haga lo siguiente:

1. Asígnate el problema que creaste **antes** de comenzar a trabajar en él.
2. Trabaje en la función que sugirió y siga nuestras [directrices para código y documentación] (#style-guides).
3. Cuando esté listo para abrir una solicitud de extracción, asegúrese de seguir las [pautas de solicitud de extracción] (#pull-request-etiquette) y márquela como implementando el problema creado anteriormente:

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. Si su cambio requiere un cambio de API, use la etiqueta `api-changes`.

   **Nota:** las funciones que requieren cambios de API pueden tardar más en implementarse y aprobarse, ya que requieren que los creadores de la biblioteca Iroha actualicen su código.### Hacer preguntas

Una pregunta es cualquier discusión que no sea un error ni una característica o solicitud de optimización.

<detalles> <summary> ¿Cómo hago una pregunta? </summary>

Publique sus preguntas en [una de nuestras plataformas de mensajería instantánea] (#contact) para que el personal y los miembros de la comunidad puedan ayudarlo de manera oportuna.

Tú, como parte de la comunidad antes mencionada, deberías considerar ayudar a los demás también. Si decides ayudar, por favor hazlo de manera [respetuosa](CODE_OF_CONDUCT.md).

</detalles>

## Tu primera contribución de código

1. Encuentre un problema apto para principiantes entre los problemas con la etiqueta [bueno-primer-número](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue).
2. Asegúrate de que nadie más esté trabajando en los temas que has elegido comprobando que no esté asignado a nadie.
3. Asígnate el problema para que otros puedan ver que alguien está trabajando en él.
4. Lea nuestra [Guía de estilo Rust](#rust-style-guide) antes de comenzar a escribir código.
5. Cuando esté listo para realizar los cambios, lea las [directrices de solicitud de extracción] (#pull-request-etiquette).

## Etiqueta de solicitud de extracción

Por favor [bifurque](https://docs.github.com/en/get-started/quickstart/fork-a-repo) el [repositorio](https://github.com/hyperledger-iroha/iroha/tree/main) y [cree una rama de funciones](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) para sus contribuciones. Cuando trabaje con **PR de horquillas**, consulte [este manual](https://help.github.com/articles/checking-out-pull-requests-locally).

#### Trabajando en la contribución del código:
- Siga la [Guía de estilo Rust](#rust-style-guide) y la [Guía de estilo de documentación](#documentation-style-guide).
- Asegúrese de que el código que ha escrito esté cubierto por pruebas. Si solucionó un error, convierta el ejemplo de trabajo mínimo que reproduce el error en una prueba.
- Al tocar cajas de derivación/proc-macro, ejecute `make check-proc-macro-ui` (o
  filtro con `PROC_MACRO_UI_CRATES="crate1 crate2"`) así que intente crear accesorios de interfaz de usuario
  permanezca sincronizado y los diagnósticos permanezcan estables.
- Documentar nuevas API públicas (nivel de caja `//!` e `///` en elementos nuevos) y ejecutar
  `make check-missing-docs` para verificar la barandilla. Mencione los documentos/pruebas que
  agregado en la descripción de su solicitud de extracción.

#### Comprometiendo tu trabajo:
- Siga la [Guía de estilo de Git](#git-workflow).
- Aplasta tus confirmaciones [ya sea antes](https://www.git-tower.com/learn/git/faq/git-squash/) o [durante la fusión](https://rietta.com/blog/github-merge-types/).
- Si durante la preparación de su solicitud de extracción su sucursal quedó desactualizada, vuelva a establecerla localmente con `git pull --rebase upstream main`. Alternativamente, puede usar el menú desplegable para el botón `Update branch` y elegir la opción `Update with rebase`.

  Con el fin de facilitar este proceso para todos, trate de no tener más de un puñado de confirmaciones para una solicitud de extracción y evite reutilizar ramas de funciones.

#### Creando una solicitud de extracción:
- Utilice una descripción de solicitud de extracción adecuada siguiendo las instrucciones de la sección [Etiqueta de solicitud de extracción] (#pull-request-etiquette). Evite desviarse de estas pautas si es posible.
- Agregue un [título de solicitud de extracción] con el formato adecuado (#pull-request-titles).
- Si cree que su código no está listo para fusionarse, pero desea que los mantenedores lo revisen, cree un borrador de solicitud de extracción.

#### Fusionando tu trabajo:
- Una solicitud de extracción debe pasar todas las comprobaciones automatizadas antes de fusionarse. Como mínimo, el código debe estar formateado, pasando todas las pruebas y sin pelusas `clippy` pendientes.
- Una solicitud de extracción no se puede fusionar sin dos revisiones de aprobación de los mantenedores activos.
- Cada solicitud de extracción notificará automáticamente a los propietarios del código. Puede encontrar una lista actualizada de los mantenedores actuales en [MAINTAINERS.md](MAINTAINERS.md).

#### Revisar la etiqueta:
- No resuelvas una conversación por tu cuenta. Deje que el revisor tome una decisión.
- Reconocer los comentarios de la revisión e interactuar con el revisor (estar de acuerdo, en desacuerdo, aclarar, explicar, etc.). No ignores los comentarios.
- Para sugerencias de cambio de código simples, si las aplicas directamente, puedes resolver la conversación.
- Evite sobrescribir sus confirmaciones anteriores al realizar nuevos cambios. Confunde lo que ha cambiado desde la última revisión y obliga al revisor a empezar desde cero. Las confirmaciones se aplastan antes de fusionarse automáticamente.

### Títulos de solicitud de extracción

Analizamos los títulos de todas las solicitudes de extracción fusionadas para generar registros de cambios. También verificamos que el título siga la convención mediante la verificación *`check-PR-title`*.

Para pasar la verificación *`check-PR-title`*, el título de la solicitud de extracción debe cumplir con las siguientes pautas:

<detalles> <summary> Expandir para leer las pautas detalladas del título</summary>

1. Siga el formato [confirmaciones convencionales](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers).

2. Si la solicitud de extracción tiene una única confirmación, el título del PR debe ser el mismo que el mensaje de confirmación.

</detalles>

### Flujo de trabajo de Git

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) el [repositorio](https://github.com/hyperledger-iroha/iroha/tree/main) y [crear una rama de funciones](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) para sus contribuciones.
- [Configure el control remoto](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) para sincronizar su bifurcación con el [repositorio Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Utilice el [Flujo de trabajo de Git Rebase] (https://git-rebase.io/). Evite el uso de `git pull`. Utilice `git pull --rebase` en su lugar.
- Utilice los [git hooks] proporcionados (./hooks/) para facilitar el proceso de desarrollo.

Siga estas pautas de confirmación:

- **Aprobar cada confirmación**. Si no lo hace, [DCO](https://github.com/apps/dco) no le permitirá fusionarse.

  Utilice `git commit -s` para agregar automáticamente `Signed-off-by: $NAME <$EMAIL>` como la línea final de su mensaje de confirmación. Su nombre y correo electrónico deben ser los mismos que se especifican en su cuenta de GitHub.

  También le recomendamos que firme sus confirmaciones con la clave GPG utilizando `git commit -sS` ([obtenga más información](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)).

  Puede utilizar [el gancho `commit-msg`](./hooks/) para cerrar automáticamente sus confirmaciones.

- Los mensajes de confirmación deben seguir [confirmaciones convencionales](https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) y el mismo esquema de nomenclatura que para [títulos de solicitud de extracción](#pull-request-titles). Esto significa:
  - **Usar tiempo presente** ("Agregar función", no "Función agregada")
  - **Utilice el modo imperativo** ("Implementar en la ventana acoplable...", no "Implementar en la ventana acoplable...")
- Escribe un mensaje de confirmación significativo.
- Intente que el mensaje de confirmación sea breve.
- Si necesita tener un mensaje de confirmación más largo:
  - Limite la primera línea de su mensaje de confirmación a 50 caracteres o menos.
  - La primera línea de tu mensaje de confirmación debe contener el resumen del trabajo que has realizado. Si necesita más de una línea, deje una línea en blanco entre cada párrafo y describa sus cambios en el medio. La última línea debe ser la despedida.
- Si modifica el esquema (verifique generando el esquema con `kagami schema` y diff), debe realizar todos los cambios en el esquema en una confirmación separada con el mensaje `[schema]`.
- Intente ceñirse a una confirmación por cada cambio significativo.
  - Si solucionó varios problemas en un PR, proporcióneles confirmaciones por separado.
  - Como se mencionó anteriormente, los cambios en `schema` y la API deben realizarse en confirmaciones apropiadas separadas del resto de su trabajo.
  - Agregar pruebas de funcionalidad en la misma confirmación que esa funcionalidad.

## Pruebas y puntos de referencia

- Para ejecutar las pruebas basadas en código fuente, ejecute [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) en la raíz Iroha. Tenga en cuenta que este es un proceso largo.
- Para ejecutar pruebas comparativas, ejecute [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) desde la raíz Iroha. Para ayudar a depurar los resultados de las pruebas comparativas, configure la variable de entorno `debug_assertions` así: `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- Si está trabajando en un componente en particular, tenga en cuenta que cuando ejecuta `cargo test` en un [espacio de trabajo](https://doc.rust-lang.org/cargo/reference/workspaces.html), solo ejecutará las pruebas para ese espacio de trabajo, que generalmente no incluye ninguna [prueba de integración](https://www.testingxperts.com/blog/what-is-integration-testing).
- Si desea probar sus cambios en una red mínima, el [`docker-compose.yml`](defaults/docker-compose.yml) proporcionado crea una red de 4 pares Iroha en contenedores acoplables que se pueden usar para probar el consenso y la lógica relacionada con la propagación de activos. Recomendamos interactuar con esa red utilizando [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) o la CLI del cliente Iroha incluida.
- No eliminar las pruebas fallidas. Incluso las pruebas que se ignoran eventualmente se ejecutarán en nuestra canalización.
- Si es posible, compare su código antes y después de realizar los cambios, ya que una regresión significativa del rendimiento puede interrumpir las instalaciones de los usuarios existentes.

### Comprobaciones de guardia de serialización

Ejecute `make guards` para validar las políticas del repositorio localmente:

- Lista de denegación directa `serde_json` en fuentes de producción (preferir `norito::json`).
- Prohibir dependencias/importaciones directas de `serde`/`serde_json` fuera de la lista de permitidos.
- Evitar la reintroducción de ayudantes ad hoc de AoS/NCB fuera de `crates/norito`.

### Pruebas de depuración

<detalles> <summary> Amplíe para aprender cómo cambiar el nivel de registro o escribir registros en un JSON.</summary>

Si una de sus pruebas falla, es posible que desee disminuir el nivel máximo de registro. De forma predeterminada, Iroha solo registra mensajes de nivel `INFO`, pero conserva la capacidad de producir registros de nivel `DEBUG` e `TRACE`. Esta configuración se puede cambiar usando la variable de entorno `LOG_LEVEL` para pruebas basadas en código o usando el punto final `/configuration` en uno de los pares en una red implementada.Si bien los registros impresos en `stdout` son suficientes, puede resultarle más conveniente producir registros con formato `json` en un archivo separado y analizarlos usando [node-bunyan](https://www.npmjs.com/package/bunyan) o [rust-bunyan](https://crates.io/crates/bunyan).

Configure la variable de entorno `LOG_FILE_PATH` en una ubicación adecuada para almacenar los registros y analizarlos utilizando los paquetes anteriores.

</detalles>

### Depuración usando la consola Tokio

<detalles> <summary> Amplíe para aprender cómo compilar Iroha con soporte para la consola Tokio.</summary>

A veces puede resultar útil para la depuración analizar las tareas de Tokio usando [tokio-console](https://github.com/tokio-rs/console).

En este caso deberías compilar Iroha con soporte de consola Tokio así:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

El puerto para la consola Tokio se puede configurar a través del parámetro de configuración `LOG_TOKIO_CONSOLE_ADDR` (o variable de entorno).
El uso de la consola de Tokio requiere que el nivel de registro sea `TRACE`, se puede habilitar a través del parámetro de configuración o la variable de entorno `LOG_LEVEL`.

Ejemplo de ejecución de Iroha con soporte de consola tokio usando `scripts/test_env.sh`:

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</detalles>

### Perfilado

<detalles> <summary> Amplíe para aprender cómo generar el perfil Iroha. </summary>

Para optimizar el rendimiento, es útil crear el perfil Iroha.

Actualmente, las compilaciones de perfiles requieren una cadena de herramientas nocturna. Para preparar uno, compile Iroha con el perfil y la función `profiling` usando `cargo +nightly`:

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

Luego inicie Iroha y adjunte el perfilador de su elección al pid Iroha.

Alternativamente, es posible construir Iroha dentro de la ventana acoplable con soporte de perfilador y perfilar Iroha de esta manera.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

Por ej. usando perf (disponible solo en Linux):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

Para poder observar el perfil del ejecutor durante la creación de perfiles Iroha, el ejecutor debe compilarse sin eliminar símbolos.
Se puede hacer ejecutando:

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

Con la función de creación de perfiles habilitada, Iroha expone el punto final a perfiles pprof de desecho:

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</detalles>

## Guías de estilo

Siga estas pautas cuando realice contribuciones de código a nuestro proyecto:

### Guía de estilo de Git

:libro: [Leer pautas de git](#git-workflow)

### Guía de estilo oxidado

<detalles> <summary> :libro: Lea las pautas del código</summary>

- Utilice `cargo fmt --all` (edición 2024) para formatear el código.

Directrices del código:

- A menos que se especifique lo contrario, consulte [Mejores prácticas de oxidación](https://github.com/mre/idiomatic-rust).
- Utilice el estilo `mod.rs`. Los [módulos con nombre propio](https://rust-lang.github.io/rust-clippy/master/) no pasarán el análisis estático, excepto como pruebas [`trybuild`](https://crates.io/crates/trybuild).
- Utilice una estructura de módulos de dominio primero.

  Ejemplo: no hagas `constants::logger`. En su lugar, invierta la jerarquía y coloque primero el objeto para el que se utiliza: `iroha_logger::constants`.
- Utilice [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) con un mensaje de error explícito o prueba de infalibilidad en lugar de `unwrap`.
- Nunca ignores un error. Si no puede `panic` y no puede recuperarlo, al menos debe registrarse en el registro.
- Prefiero devolver un `Result` en lugar de `panic!`.
- Agrupar espacialmente la funcionalidad relacionada, preferiblemente dentro de módulos apropiados.

  Por ejemplo, en lugar de tener un bloque con definiciones `struct` y luego `impl` para cada estructura individual, es mejor tener los `impl` relacionados con ese `struct` al lado.
- Declarar antes de la implementación: declaraciones y constantes `use` en la parte superior, pruebas unitarias en la parte inferior.
- Intente evitar declaraciones `use` si el nombre importado se usa solo una vez. Esto facilita mover su código a un archivo diferente.
- No silenciar las pelusas `clippy` indiscriminadamente. Si es así, explique su razonamiento con un comentario (o mensaje `expect`).
- Prefiera `#[outer_attribute]` a `#![inner_attribute]` si alguno está disponible.
- Si su función no muta ninguna de sus entradas (y no debería mutar nada más), márquela como `#[must_use]`.
- Evite `Box<dyn Error>` si es posible (preferimos escritura fuerte).
- Si su función es getter/setter, márquela como `#[inline]`.
- Si su función es un constructor (es decir, está creando un nuevo valor a partir de los parámetros de entrada y llama a `default()`), márquelo como `#[inline]`.
- Evite vincular su código a estructuras de datos concretas; `rustc` es lo suficientemente inteligente como para convertir un `Vec<InstructionExpr>` en `impl IntoIterator<Item = InstructionExpr>` y viceversa cuando sea necesario.

Pautas de nomenclatura:
- Utilice sólo palabras completas en nombres de estructuras, variables, métodos, rasgos, constantes y módulos *públicos*. Sin embargo, se permiten abreviaturas si:
  - El nombre es local (por ejemplo, argumentos de cierre).
  - El nombre está abreviado según la convención de Rust (por ejemplo, `len`, `typ`).
  - El nombre es una abreviatura aceptada (por ejemplo, `tx`, `wsv`, etc.); consulte el [glosario del proyecto](https://docs.iroha.tech/reference/glossary.html) para conocer las abreviaturas canónicas.
  - El nombre completo habría sido sombreado por una variable local (por ejemplo, `msg <- message`).
  - El nombre completo habría hecho que el código fuera engorroso con más de 5 o 6 palabras (por ejemplo, `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- Si cambia las convenciones de nomenclatura, asegúrese de que el nuevo nombre que ha elegido sea _mucho_ más claro que el que teníamos antes.

Pautas para comentarios:
- Cuando escriba comentarios que no sean de documento, en lugar de describir *qué* hace su función, intente explicar *por qué* hace algo de una manera particular. Esto le ahorrará tiempo a usted y al revisor.
- Puedes dejar marcadores `TODO` en el código siempre que hagas referencia a un problema que hayas creado para ello. No crear un problema significa que no se fusiona.

Usamos dependencias ancladas. Siga estas pautas para el control de versiones:

- Si su trabajo depende de una caja en particular, vea si no estaba ya instalada usando [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (use `bat` o `grep`), e intente usar esa versión, en lugar de la última versión.
- Utilice la versión completa "X.Y.Z" en `Cargo.toml`.
- Proporcionar mejoras de versión en un PR separado.

</detalles>

### Guía de estilo de documentación

<detalles> <summary> :libro: Lea las pautas de documentación</summary>


- Utilice el formato [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- Prefiere la sintaxis de comentarios de una sola línea. Utilice `///` arriba de los módulos en línea y `//!` para los módulos basados ​​en archivos.
- Si puede vincular a los documentos de una estructura/módulo/función, hágalo.
- Si puedes dar un ejemplo de uso, hazlo. Esto [también es una prueba](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- Si una función puede generar errores o entrar en pánico, evite los verbos modales. Ejemplo: `Fails if disk IO fails` en lugar de `Can possibly fail, if disk IO happens to fail`.
- Si una función puede generar errores o generar pánico por más de un motivo, utilice una lista con viñetas de condiciones de falla, con las variantes `Error` apropiadas (si corresponde).
- Funciones *hacer* cosas. Utilice el modo imperativo.
- Las estructuras *son* cosas. Vaya al grano. Por ejemplo, `Log level for reloading from the environment` es mejor que `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`.
- Las estructuras tienen campos, que también *son* cosas.
- Los módulos *contienen* cosas, y lo sabemos. Vaya al grano. Ejemplo: utilice `Logger-related traits.` en lugar de `Module which contains logger-related logic`.


</detalles>

## Contacto

Los miembros de nuestra comunidad están activos en:

| Servicio | Enlace |
|-----------------------|--------------------------------------------------------------------|
| Desbordamiento de pila | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| Lista de correo | https://lists.lfdecentralizedtrust.org/g/iroha |
| Telegrama | https://t.me/hyperledgeriroha |
| Discordia | https://discord.com/channels/905194001349627914/905205848547155968 |

---