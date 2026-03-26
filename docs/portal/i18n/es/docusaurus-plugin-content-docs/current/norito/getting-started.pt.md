---
lang: es
direction: ltr
source: docs/portal/docs/norito/getting-started.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Primeros pasos con Norito

Esta guía rápida muestra el flujo mínimo para compilar un contrato Kotodama, inspeccionar el código de bytes Norito generado, ejecutarlo localmente y desplegarlo en un nodo Iroha.

##Requisitos previos

1. Instale una cadena de herramientas Rust (1.76 o más reciente) y luego realice la compra en este repositorio.
2. Compile o baje los binarios de soporte:
   - `koto_compile` - compilador Kotodama que emite bytecode IVM/Norito
   - `ivm_run` e `ivm_tool` - utilitarios de ejecución local e inspección
   - `iroha_cli` - usado para implementar contratos vía Torii

   El Makefile del repositorio espera esos binarios no `PATH`. Voce pode baixar artefatos precompilados o compilar a partir del código fuente. Si compila una cadena de herramientas localmente, apoye los ayudantes de Makefile para los binarios:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Garanta que um nodo Iroha esteja rodando quando chegar na etapa de implementación. Los ejemplos anteriores suponen que Torii está disponible en una URL configurada en el perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compile un contrato Kotodama

El repositorio incluye un contrato mínimo "hola mundo" en `examples/hello/hello.ko`. Compilación del código de bytes Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Banderas chave:- `--abi 1` arregla el contrato na versao ABI 1 (un unica soportada no momento).
- `--max-cycles 0` solicita ejecución sin límite; Defina un número positivo para limitar el relleno de ciclos para probar el resultado cero.

## 2. Inspección de artefacto Norito (opcional)

Utilice `ivm_tool` para verificar el cabecalho y los metadados embutidos:

```sh
ivm_tool inspect target/examples/hello.to
```

Debes ver a versao ABI, las banderas habilitadas y los puntos de entrada exportados. E uma checagem rapida antes del despliegue.

## 3. Ejecutar el contrato localmente

Ejecute el bytecode con `ivm_run` para confirmar el comportamiento sin tocar un nodo:

```sh
ivm_run target/examples/hello.to --args '{}'
```

El ejemplo `hello` registra una saudacao y emite una llamada al sistema `SET_ACCOUNT_DETAIL`. Ejecutar localmente y utilizar la misma voz en la lógica del contrato antes de publicarlo en la cadena.

## 4. Implementación de Faca a través de `iroha_cli`

Cuando esté satisfecho con el contrato, antes de implementarlo en un nodo usando la CLI. Forneca uma conta de autoridad, su chave de assinatura y un archivo `.to` o payload Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

El comando envía un paquete de manifiesto Norito + código de bytes a través de Torii e imprime el estado de la transacción resultante. Después de confirmar la transacción, el hash del código mostrado en la respuesta puede usarse para recuperar manifiestos o listar instancias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Ejecutar contra ToriiCon el bytecode registrado, usted puede invocarlo submetiendo una instrucción que hace referencia al código armado (por ejemplo, a través de `iroha_cli ledger transaction submit` o el cliente de su aplicación). Garanta que as permissoes da conta permitirm os syscalls deseados (`set_account_detail`, `transfer_asset`, etc.).

## Dicas y solución de problemas

- Utilice `make examples-run` para compilar y ejecutar los ejemplos de una vez. Substitua as variaveis de ambiente `KOTO`/`IVM` se os binarios nao estiverem no `PATH`.
- Si `koto_compile` rejeitar a versao ABI, verifique si o compilador y o nodo miram ABI v1 (rode `koto_compile --abi` sem argumentos para listar o soporte).
- O CLI aceita chaves de assinatura em hex o Base64. Para testes, voce pode usar chaves emitidas por `iroha_cli tools crypto keypair`.
- Para depurar payloads Norito, el subcomando `ivm_tool disassemble` ayuda a correlacionar instrucciones con el código fuente Kotodama.

Este flujo espelha os passos usados ​​em CI e testes de integracao. Para una mezcla más profunda de la gramática Kotodama, nos mapeamos de syscalls y nos internals de Norito, veja:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`