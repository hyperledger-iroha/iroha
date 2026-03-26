---
lang: es
direction: ltr
source: docs/portal/docs/norito/getting-started.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Начало работы с Norito

Este краткое руководство показывает minимальный рабочий процесс компиляции контракта Kotodama, проверки сгенерированного байткода Norito, la configuración local y la implementación en el uso Iroha.

## Требования

1. Instale la cadena de herramientas de Rust (1.76 o más) y cierre este repositorio.
2. Соберите или скачайте вспомогательные бинарники:
   - `koto_compile` - compilador Kotodama, generador de batería IVM/Norito
   - `ivm_run` y `ivm_tool` - Utilidades locales de captura e inspección
   - `iroha_cli` - Utilizado para implementar contratos con Torii

   El repositorio Makefile contiene estos archivos binarios en `PATH`. Вы moжете скачать готовые артефакты или собрать их из исходников. Si está compilando la cadena de herramientas localmente, utilice el asistente Makefile con binarios:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Asegúrese de utilizar Iroha antes de implementarlo. Primero, no predisponga, ya que Torii inserta la URL en el perfil `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Compilación del contrato Kotodama

En el repositorio hay un contrato minimo "hola mundo" en `examples/hello/hello.ko`. Compilación en el bloque Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Ключевые флаги:- `--abi 1` contrato físico en ABI versión 1 (versión actualizada en momento de inicio).
- `--max-cycles 0` запрашивает неограниченное выполнение; установите положительное число, чтобы ограничить padding циклов для zero-knowledge доказательств.

## 2. Проверить артефакт Norito (opcional)

Utilice `ivm_tool` para comprobar los valores y metadanos existentes:

```sh
ivm_tool inspect target/examples/hello.to
```

Вы увидите версию ABI, включенные флаги y eksportirovannye puntos de entrada. Esta es una verificación de cordura exhaustiva antes del despliegue.

## 3. Cerrar el contrato local

Introduzca el código de fuente `ivm_run` y podrá utilizar el siguiente código:

```sh
ivm_run target/examples/hello.to --args '{}'
```

Primero, `hello` muestra el registro y la llamada al sistema `SET_ACCOUNT_DETAIL`. Los mensajes de texto locales se aplican a través de iteraciones lógicas de contrato de publicidad en cadena.

## 4. Деплой через `iroha_cli`

Para establecer un contrato, instale el dispositivo en la CLI. Consulte el autor de la cuenta, haga clic en el archivo libre `.to`, libre de carga útil Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

El comando ejecuta el paquete de manifiesto Norito + el conjunto de componentes Torii y cambia el estado de la transmisión. Después de usar este código, es posible utilizar un manifestante o una instalación específica:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Запуск через ToriiDespués de registrarse en el sistema, puede utilizar su propio sistema de instrucciones, cómo utilizar el código binario (por ejemplo, (por ejemplo `iroha_cli ledger transaction submit` o por una cuenta de cliente). Tenga en cuenta que esta cuenta tiene varias llamadas al sistema (`set_account_detail`, `transfer_asset` y t.d.).

## Problemas relacionados con la restauración y la restauración

- Utilice `make examples-run` para conectar y desconectar los primeros pulsadores. Utilice la configuración permanente `KOTO`/`IVM`, ni binarias no conectadas a `PATH`.
- Si `koto_compile` incluye la versión ABI, compruebe qué compilador y usuario utiliza en ABI v1 (descarga `koto_compile --abi` sin аргументов, чтобы увидеть поддержку).
- CLI configura las teclas en hexadecimal o Base64. Para realizar la prueba, es posible utilizar teclas de acceso `iroha_cli tools crypto keypair`.
- Al instalar cargas útiles Norito en el comando `ivm_tool disassemble`, el equipo puede seguir instrucciones de instalación Kotodama.

Esto se aplica a CI y pruebas integradas. Para el mayor grupo de gramáticas Kotodama, mapeo de llamadas al sistema y datos Norito como:

-`docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`