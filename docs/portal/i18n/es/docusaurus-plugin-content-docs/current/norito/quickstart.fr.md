---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Demarrage Norito
descripción: Construisez, validez et rolez un contrat Kotodama avec l'outillage de release et le reseau mono-pair por defecto.
babosa: /norito/inicio rápido
---

Este tutorial muestra el flujo de trabajo que acompañamos a los desarrolladores cuando descubrimos Norito y Kotodama para empezar: iniciar un trabajo determinado monopar, compilar un contrato, realizar una ejecución en seco y enviarlo a través de Torii con la CLI. de referencia.

El contrato de ejemplo escrito un par de llaves/valeur en la cuenta del apelante para que pueda verificar el efecto de borde inmediato con `iroha_cli`.

## Requisitos previos

- [Docker](https://docs.docker.com/engine/install/) con Compose V2 activo (utilícelo para finalizar el par de ejemplos definidos en `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) para construir archivos binarios auxiliares si no los descargas dos veces.
- Binarios `koto_compile`, `ivm_run` y `iroha_cli`. Podrás construir después de realizar la compra del espacio de trabajo como ci-dessous o telecargar los artefactos de lanzamiento correspondientes:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Los archivos binarios son instalados sin riesgo en el resto del espacio de trabajo.
> Ils ne lient jamais `serde`/`serde_json` ; Los códecs Norito son aplicaciones de combate a combate.

## 1. Demarrer un reseau dev mono-parEl depósito incluye un paquete Docker Compose genere par `kagami swarm` (`defaults/docker-compose.single.yml`). La génesis está conectada de forma predeterminada, el cliente de configuración y las sondas de salud hasta que Torii se pueden unir a `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur tourner (au premier plan ou en detache). Todos los comandos CLI siguientes son compatibles con este par a través de `defaults/client.toml`.

## 2. Escribir el contrato

Cree un repertorio de trabajo y registre el ejemplo Kotodama mínimo:

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

> Prefiere conservar las fuentes Kotodama en control de versión. Los ejemplos heberges sur le portail también están disponibles en la [galería de ejemplos Norito](./examples/) si vous voulez a point de part plus riche.

## 3. Compilador y ejecución en seco con IVM

Compile el contrato en código de bytes IVM/Norito (`.to`) y ejecútelo localmente para confirmar que las llamadas al sistema del host se reutilizan antes de tocar el archivo:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El corredor imprime el registro `info("Hello from Kotodama")` y efectúa la llamada al sistema `SET_ACCOUNT_DETAIL` con el host simule. Si la opción binaria `ivm_tool` está disponible, `ivm_tool inspect target/quickstart/hello.to` muestra el ABI en la parte superior, los bits de características y los puntos de entrada exportados.

## 4. Soumettre el código de bytes a través de ToriiComo nuevo y constantemente en el curso de ejecución, envíe el código de bytes a compilar un Torii con la CLI. La identidad de desarrollo por defecto está derivada de la clave pública en `defaults/client.toml`, donde la identificación de cuenta está
```
i105...
```

Utilice el archivo de configuración para guardar la URL Torii, el ID de la cadena y la clave de firma:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

La CLI codifica la transacción con Norito, la firma con la llave de desarrollo y el envío al par en curso de ejecución. Vigile los registros Docker para la llamada al sistema `set_account_detail` o seleccione la salida CLI para el hash de la transacción confirmada.

## 5. Verificador del cambio de estado

Utilice el perfil de meme CLI para recuperar el detalle de la cuenta que el contrato tiene escrito:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Debes ver la carga útil JSON agregada a Norito:

```json
{
  "hello": "world"
}
```

Si el valor está ausente, verifique que el servicio Docker compone todos los días y que el hash de la señal de transacción par `iroha` atteint el estado `Committed`.

## Etapas siguientes- Explora la [galería de ejemplos](./examples/) generada automáticamente para ver
  comment des snippets Kotodama plus avances se mappent a des syscalls Norito.
- Lisez le [guía Norito para comenzar](./getting-started) para una explicación
  Además, se han aprobado las herramientas del compilador/runner, la implementación de manifiestos y las metadonnees IVM.
- Lorsque vous iterez sur vos propres contrats, utilisez `npm run sync-norito-snippets` dans le
  Espacio de trabajo para regenerar los fragmentos telecargables según los documentos del portal y los artefactos restantes.
  sincroniza con las fuentes bajo `crates/ivm/docs/examples/`.