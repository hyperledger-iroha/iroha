---
lang: es
direction: ltr
source: docs/portal/docs/norito/getting-started.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Primeros pasos con Norito

Esta guía rápida muestra el flujo mínimo para compilar un contrato Kotodama, inspeccionar el bytecode Norito generado, ejecutarlo localmente y desplegarlo en un nodo de Iroha.

##Requisitos previos

1. Instale la cadena de herramientas de Rust (1.76 o más reciente) y clone este repositorio.
2. Construye o descarga los binarios de soporte:
   - `koto_compile` - compilador Kotodama que emite bytecode IVM/Norito
   - `ivm_run` y `ivm_tool` - utilidades de ejecución local e inspección
   - `iroha_cli` - se usa para el despliegue de contratos vía Torii

   El Makefile del repositorio espera estos binarios en `PATH`. Puedes descargar artefactos precompilados o compilarlos desde el código fuente. Si compila la cadena de herramientas localmente, apunta los ayudantes del Makefile a los binarios:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Asegúrese de que un nodo de Iroha esté en ejecución cuando llegues al paso de despliegue. Los ejemplos de abajo suponen que Torii es accesible en la URL configurada en su perfil de `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compila un contrato Kotodama

El repositorio incluye un contrato mínimo "hello world" en `examples/hello/hello.ko`. Compila un código de bytes Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Claves de opciones:- `--abi 1` fija el contrato a la versión ABI 1 (la única soportada al momento de escribir).
- `--max-cycles 0` solicita ejecucion sin limites; establece un número positivo para acotar el padding de ciclos para pruebas de conocimiento cero.

## 2. Inspecciona el artefacto Norito (opcional)

Usa `ivm_tool` para verificar la cabecera y los metadatos incrustados:

```sh
ivm_tool inspect target/examples/hello.to
```

Deberias ver la versión ABI, las banderas habilitadas y los puntos de entrada exportados. Es una comprobacion rapida antes del despliegue.

## 3. Ejecuta el contrato localmente

Ejecuta el bytecode con `ivm_run` para confirmar el comportamiento sin tocar un nodo:

```sh
ivm_run target/examples/hello.to --args '{}'
```

El ejemplo `hello` registra un saludo y emite una llamada al sistema `SET_ACCOUNT_DETAIL`. Ejecutarlo localmente es útil mientras iteras sobre la lógica del contrato antes de publicarlo on-chain.

## 4. Despliega vía `iroha_cli`

Cuando esté satisfecho con el contrato, despliega en un nodo usando el CLI. Proporciona una cuenta de autoridad, su clave de firma y un archivo `.to` o payload Base64:

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

El comando envía un paquete de manifiesto Norito + bytecode por Torii y muestra el estado de la transacción resultante. Una vez confirmada, el hash de código mostrado en la respuesta puede usarse para recuperar manifiestos o listar instancias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```## 5. Ejecuta contra Torii

Con el bytecode registrado, puedes invocarlo enviando una instrucción que haga referencia al código almacenado (p. ej., mediante `iroha_cli ledger transaction submit` o tu cliente de aplicación). Asegúrese de que los permisos de la cuenta permitan las llamadas al sistema deseadas (`set_account_detail`, `transfer_asset`, etc.).

## Consejos y solución de problemas

- Usa `make examples-run` para compilar y ejecutar los ejemplos en un solo paso. Sobrescriba las variables de entorno `KOTO`/`IVM` si los binarios no están en `PATH`.
- Si `koto_compile` rechaza la versión ABI, verifique que el compilador y el nodo apunten a ABI v1 (ejecuta `koto_compile --abi` sin argumentos para listar soporte).
- El CLI acepta claves de firma en hexadecimal o Base64. Para pruebas, puede usar las claves emitidas por `iroha_cli tools crypto keypair`.
- Al depurar payloads Norito, el subcomando `ivm_tool disassemble` ayuda a correlacionar instrucciones con el código fuente Kotodama.

Este flujo refleja los pasos usados ​​en CI y en las pruebas de integración. Para un análisis más profundo de la gramática de Kotodama, los mapeos de syscalls y los internals de Norito, consulta:

-`docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`