---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Inicio rápido de Norito
descripción: Crea, valida y despliega un contrato Kotodama con las herramientas de liberación y la red predeterminada de un solo peer.
babosa: /norito/inicio rápido
---

Este recorrido refleja el flujo que esperamos que sigan los desarrolladores al aprender Norito y Kotodama por primera vez: arrancar una red determinista de un solo peer, compilar un contrato, hacer un dry-run local y luego enviarlo por Torii con el CLI de referencia.

El contrato de ejemplo escribe un par clave/valor en la cuenta del llamador para que puedas verificar el efecto lateral de inmediato con `iroha_cli`.

##Requisitos previos

- [Docker](https://docs.docker.com/engine/install/) con Compose V2 habilitado (se usa para iniciar el par de muestra definido en `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+) para construir los binarios auxiliares si no descargas los publicados.
- Binarios `koto_compile`, `ivm_run` e `iroha_cli`. Puedes construirlos desde el checkout del espacio de trabajo como se muestra abajo o descargar los artefactos del lanzamiento correspondiente:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Los binarios anteriores son seguros de instalar junto con el resto del espacio de trabajo.
> Nunca enlacen con `serde`/`serde_json`; Los códecs Norito se aplican de extremo a extremo.

## 1. Inicia una red dev de un solo peerEl repositorio incluye un paquete de Docker Compose generado por `kagami swarm` (`defaults/docker-compose.single.yml`). Conecta la génesis por defecto, la configuración del cliente y los sondas de salud para que Torii sea accesible en `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deja el contenedor corriendo (en primer plano o desacoplado). Todas las llamadas posteriores del CLI apuntan a este par mediante `defaults/client.toml`.

## 2. Redacta el contrato

Crea un directorio de trabajo y guarda el ejemplo mínimo de Kotodama:

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

> Prefiere mantener las fuentes de Kotodama en control de versiones. Los ejemplos alojados en el portal también están disponibles en la [galeria de ejemplos Norito](./examples/) si quieres un punto de partida más completo.

## 3. Compila y haz dry-run con IVM

Compila el contrato a bytecode IVM/Norito (`.to`) y ejecutalo localmente para confirmar que las llamadas al sistema del host funcionan antes de tocar la red:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El corredor imprime el registro `info("Hello from Kotodama")` y ejecuta el syscall `SET_ACCOUNT_DETAIL` contra el host simulado. Si el binario opcional `ivm_tool` está disponible, `ivm_tool inspect target/quickstart/hello.to` muestra el encabezado ABI, los bits de características y los puntos de entrada exportados.

## 4. Envía el bytecode vía ToriiCon el nodo aún corriendo, envió el bytecode compilado a Torii usando el CLI. La identidad de desarrollo por defecto se deriva de la clave pública en `defaults/client.toml`, por lo que el ID de cuenta es
```
<i105-account-id>
```

Utilice el archivo de configuración para proporcionar la URL de Torii, el ID de cadena y la clave de firma:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

El CLI codifica la transacción con Norito, la firma con la clave de desarrollo y la envía al peer en ejecución. Observe los registros de Docker para el syscall `set_account_detail` o monitoree la salida del CLI para el hash de transacción comprometida.

## 5. Verifica el cambio de estado

Usa el mismo perfil del CLI para obtener el detalle de cuenta que escribe el contrato:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

Deberias ver el JSON de carga útil respaldado por Norito:

```json
{
  "hello": "world"
}
```

Si falta el valor, confirma que el servicio de Docker compose sigue en ejecución y que el hash de transacción reportado por `iroha` llego al estado `Committed`.

## Siguientes pasos- Explora la [galería de ejemplos](./examples/) autogenerada para ver
  como fragmentos Kotodama mas avanzados se mapean a syscalls Norito.
- Lee la [guía de inicio de Norito](./getting-started) para una explicación
  Más profundo del tooling de compilador/runner, el despliegue de manifests y los metadata de IVM.
- Cuando iteres en tus propios contratos, usa `npm run sync-norito-snippets` en el
  espacio de trabajo para regenerar fragmentos descargables de modo que los documentos del portal y los artefactos
  se mantienen sincronizados con las fuentes en `crates/ivm/docs/examples/`.