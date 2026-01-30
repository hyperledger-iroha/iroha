---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9bb37c2863bca52511fce863e1057ae6e9bcbe861ba8f25a2677e94d17ac5817
source_last_modified: "2025-11-14T04:43:20.832960+00:00"
translation_last_reviewed: 2026-01-30
---

# Primeros pasos con Norito

Esta guia rapida muestra el flujo minimo para compilar un contrato Kotodama, inspeccionar el bytecode Norito generado, ejecutarlo localmente y desplegarlo en un nodo de Iroha.

## Requisitos previos

1. Instala la toolchain de Rust (1.76 o mas reciente) y clona este repositorio.
2. Construye o descarga los binarios de soporte:
   - `koto_compile` - compilador Kotodama que emite bytecode IVM/Norito
   - `ivm_run` y `ivm_tool` - utilidades de ejecucion local e inspeccion
   - `iroha_cli` - se usa para el despliegue de contratos via Torii

   El Makefile del repositorio espera estos binarios en `PATH`. Puedes descargar artefactos precompilados o compilarlos desde el codigo fuente. Si compilas la toolchain localmente, apunta los helpers del Makefile a los binarios:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Asegurate de que un nodo de Iroha este en ejecucion cuando llegues al paso de despliegue. Los ejemplos de abajo asumen que Torii es accesible en la URL configurada en tu perfil de `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compila un contrato Kotodama

El repositorio incluye un contrato minimo "hello world" en `examples/hello/hello.ko`. Compilalo a bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Opciones clave:

- `--abi 1` fija el contrato a la version ABI 1 (la unica soportada al momento de escribir).
- `--max-cycles 0` solicita ejecucion sin limites; establece un numero positivo para acotar el padding de ciclos para pruebas de conocimiento cero.

## 2. Inspecciona el artefacto Norito (opcional)

Usa `ivm_tool` para verificar la cabecera y los metadatos incrustados:

```sh
ivm_tool inspect target/examples/hello.to
```

Deberias ver la version ABI, los flags habilitados y los entry points exportados. Es una comprobacion rapida antes del despliegue.

## 3. Ejecuta el contrato localmente

Ejecuta el bytecode con `ivm_run` para confirmar el comportamiento sin tocar un nodo:

```sh
ivm_run target/examples/hello.to --args '{}'
```

El ejemplo `hello` registra un saludo y emite un syscall `SET_ACCOUNT_DETAIL`. Ejecutarlo localmente es util mientras iteras sobre la logica del contrato antes de publicarlo on-chain.

## 4. Despliega via `iroha_cli`

Cuando estes satisfecho con el contrato, despliega en un nodo usando el CLI. Proporciona una cuenta de autoridad, su clave de firma y un archivo `.to` o payload Base64:

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

El comando envia un bundle de manifiesto Norito + bytecode por Torii y muestra el estado de la transaccion resultante. Una vez confirmada, el hash de codigo mostrado en la respuesta puede usarse para recuperar manifiestos o listar instancias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Ejecuta contra Torii

Con el bytecode registrado, puedes invocarlo enviando una instruccion que haga referencia al codigo almacenado (p. ej., mediante `iroha_cli ledger transaction submit` o tu cliente de aplicacion). Asegurate de que los permisos de la cuenta permitan los syscalls deseados (`set_account_detail`, `transfer_asset`, etc.).

## Consejos y solucion de problemas

- Usa `make examples-run` para compilar y ejecutar los ejemplos en un solo paso. Sobrescribe las variables de entorno `KOTO`/`IVM` si los binarios no estan en `PATH`.
- Si `koto_compile` rechaza la version ABI, verifica que el compilador y el nodo apunten a ABI v1 (ejecuta `koto_compile --abi` sin argumentos para listar soporte).
- El CLI acepta claves de firma en hex o Base64. Para pruebas, puedes usar las claves emitidas por `iroha_cli tools crypto keypair`.
- Al depurar payloads Norito, el subcomando `ivm_tool disassemble` ayuda a correlacionar instrucciones con el codigo fuente Kotodama.

Este flujo refleja los pasos usados en CI y en las pruebas de integracion. Para un analisis mas profundo de la gramatica de Kotodama, los mapeos de syscalls y los internals de Norito, consulta:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
