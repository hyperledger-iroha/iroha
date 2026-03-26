---
lang: es
direction: ltr
source: docs/portal/docs/norito/getting-started.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Premio principal de Norito

Esta guía rápida presenta el flujo de trabajo mínimo para compilar un contrato Kotodama, inspeccionar el género del código de bytes Norito, la ubicación del ejecutor y el implementador en un nuevo Iroha.

## Requisitos previos

1. Instale la cadena de herramientas Rust (1.76 o más) y recupere este depósito.
2. Construya o telecargue los binarios de soporte:
   - `koto_compile` - compilador Kotodama que contiene el código de bytes IVM/Norito
   - `ivm_run` e `ivm_tool` - utilidades de ejecución local e inspección
   - `iroha_cli` - utilizar para implementar contratos a través de Torii

   Le Makefile du depot atiende estos binarios en `PATH`. Podrás telecargar artefactos precompilados o construir después de las fuentes. Si compila la ubicación de la cadena de herramientas, indique los ayudantes de Makefile frente a los binarios:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Asegúrese de que un nud Iroha esté en curso de ejecución cuando atteignez la etapa de implementación. Los ejemplos anteriores suponen que Torii está accesible a la URL configurada en su perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compilador con contrato Kotodama

El depósito dispone de un contrato mínimo "hola mundo" en `examples/hello/hello.ko`. Compile el código de bytes Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Opciones de elementos:- `--abi 1` canceló el contrato en la versión ABI 1 (el único soporte en el momento de la redacción).
- `--max-cycles 0` exige una ejecución sin límite; Fixez un nombre positif pour borner le padding de Cycles pour les preuves de connaissance zero.

## 2. Inspeccione el artefacto Norito (opcional)

Utilice `ivm_tool` para verificar el contenido y los metadones enteros:

```sh
ivm_tool inspect target/examples/hello.to
```

Deberá consultar la versión ABI, las banderas activas y los puntos de entrada exportados. Es un control rápido antes del despliegue.

## 3. Ejecutar la ubicación del contrato

Ejecute el código de bytes con `ivm_run` para confirmar el comportamiento sin tocar un botón:

```sh
ivm_run target/examples/hello.to --args '{}'
```

El ejemplo `hello` escribe un saludo y emite una llamada al sistema `SET_ACCOUNT_DETAIL`. La configuración local es útil mientras ingresa en la lógica del contrato antes de publicarlo en la cadena.

## 4. Implementador a través de `iroha_cli`

Cuando esté satisfecho con el contrato, implementelo en un lugar a través de la CLI. Introduzca una cuenta de autorización, una clave de firma y un archivo `.to` con una carga útil Base64:

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

El comando requiere un paquete manifiesto Norito + código de bytes a través de Torii y muestra el estado de la transacción resultante. Una vez validada la transacción, el hash de código que se muestra en la respuesta puede servir para recuperar manifiestos o listar instancias:```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Ejecutor vía Torii

Con el registro de bytecode, puede solicitar alguna instrucción que haga referencia al código almacenado (p. ej., a través de `iroha_cli ledger transaction submit` o su aplicación cliente). Asegúrese de que los permisos de la cuenta autoricen las llamadas al sistema deseadas (`set_account_detail`, `transfer_asset`, etc.).

## Consejos y despacho

- Utilice `make examples-run` para compilar y ejecutar ejemplos en un solo comando. Recargue las variables de entorno `KOTO`/`IVM` si los binarios no están en `PATH`.
- Si `koto_compile` rechaza la versión ABI, verifique que el compilador y el nud cible bien ABI v1 (ejecute `koto_compile --abi` sin argumentos para listar el soporte).
- La CLI acepta las claves de firma en hexadecimal o Base64. Para las pruebas, puede utilizar las luces emitidas por `iroha_cli tools crypto keypair`.
- Durante la depuración de cargas útiles Norito, el sous-commande `ivm_tool disassemble` ayuda a correlacionar las instrucciones con la fuente Kotodama.

Este flujo refleja las etapas utilizadas en CI y en las pruebas de integración. Para analizar más la gramática Kotodama, las asignaciones de llamadas al sistema y los componentes internos Norito, consulte:

-`docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`