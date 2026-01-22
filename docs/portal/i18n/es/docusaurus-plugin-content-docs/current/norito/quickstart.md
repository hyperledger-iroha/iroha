---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Inicio rapido de Norito
description: Crea, valida y despliega un contrato Kotodama con las herramientas de release y la red predeterminada de un solo peer.
slug: /norito/quickstart
---

Este recorrido refleja el flujo que esperamos que sigan los desarrolladores al aprender Norito y Kotodama por primera vez: arrancar una red determinista de un solo peer, compilar un contrato, hacer un dry-run local y luego enviarlo por Torii con el CLI de referencia.

El contrato de ejemplo escribe un par clave/valor en la cuenta del llamador para que puedas verificar el efecto lateral de inmediato con `iroha_cli`.

## Requisitos previos

- [Docker](https://docs.docker.com/engine/install/) con Compose V2 habilitado (se usa para iniciar el peer de muestra definido en `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+) para construir los binarios auxiliares si no descargas los publicados.
- Binarios `koto_compile`, `ivm_run` e `iroha_cli`. Puedes construirlos desde el checkout del workspace como se muestra abajo o descargar los artifacts del release correspondiente:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Los binarios anteriores son seguros de instalar junto con el resto del workspace.
> Nunca enlazan con `serde`/`serde_json`; los codecs Norito se aplican end-to-end.

## 1. Inicia una red dev de un solo peer

El repositorio incluye un bundle de Docker Compose generado por `kagami swarm` (`defaults/docker-compose.single.yml`). Conecta el genesis por defecto, la configuracion del cliente y los health probes para que Torii sea accesible en `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deja el contenedor corriendo (en primer plano o desacoplado). Todas las llamadas posteriores del CLI apuntan a este peer mediante `defaults/client.toml`.

## 2. Redacta el contrato

Crea un directorio de trabajo y guarda el ejemplo minimo de Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Prefiere mantener los fuentes de Kotodama en control de versiones. Los ejemplos alojados en el portal tambien estan disponibles en la [galeria de ejemplos Norito](./examples/) si quieres un punto de partida mas completo.

## 3. Compila y haz dry-run con IVM

Compila el contrato a bytecode IVM/Norito (`.to`) y ejecutalo localmente para confirmar que los syscalls del host funcionan antes de tocar la red:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El runner imprime el log `info("Hello from Kotodama")` y ejecuta el syscall `SET_ACCOUNT_DETAIL` contra el host simulado. Si el binario opcional `ivm_tool` esta disponible, `ivm_tool inspect target/quickstart/hello.to` muestra el encabezado ABI, los bits de features y los entrypoints exportados.

## 4. Envia el bytecode via Torii

Con el nodo aun corriendo, envia el bytecode compilado a Torii usando el CLI. La identidad de desarrollo por defecto se deriva de la clave publica en `defaults/client.toml`, por lo que el ID de cuenta es
```
ih58...
```

Usa el archivo de configuracion para suministrar la URL de Torii, el chain ID y la clave de firma:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

El CLI codifica la transaccion con Norito, la firma con la clave de desarrollo y la envia al peer en ejecucion. Observa los logs de Docker para el syscall `set_account_detail` o monitorea la salida del CLI para el hash de transaccion comprometida.

## 5. Verifica el cambio de estado

Usa el mismo perfil del CLI para obtener el account detail que escribio el contrato:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Deberias ver el payload JSON respaldado por Norito:

```json
{
  "hello": "world"
}
```

Si falta el valor, confirma que el servicio de Docker compose sigue en ejecucion y que el hash de transaccion reportado por `iroha` llego al estado `Committed`.

## Siguientes pasos

- Explora la [galeria de ejemplos](./examples/) autogenerada para ver
  como fragmentos Kotodama mas avanzados se mapean a syscalls Norito.
- Lee la [guia de inicio de Norito](./getting-started) para una explicacion
  mas profunda del tooling de compilador/runner, el despliegue de manifests y la metadata de IVM.
- Cuando iteres en tus propios contratos, usa `npm run sync-norito-snippets` en el
  workspace para regenerar snippets descargables de modo que los docs del portal y los artefactos
  se mantengan sincronizados con las fuentes en `crates/ivm/docs/examples/`.
