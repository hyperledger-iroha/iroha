---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Быстрый старт Norito
descripción: Soberite, proteja y limpie el contrato Kotodama con instrumentos confiables y un cómodo juego de sábanas.
babosa: /norito/inicio rápido
---

Este tutorial de este proceso, ya que debemos seguir los pasos previos a la conexión con Norito e Kotodama: поднять детерминированную одноузловую сеть, скомпилировать контракть, сделать локальный dry-run, затем отправить его через Torii con la CLI estándar.

El primer contrato que se realiza con un clic o cierre en la cuenta de usuario es necesario para asegurarse de que no se produzcan efectos negativos. помощью `iroha_cli`.

## Требования

- [Docker](https://docs.docker.com/engine/install/) con el nuevo Compose V2 (utilizado para iniciar el par de muestra, adaptado a `defaults/docker-compose.single.yml`).
- Cadena de herramientas Rust (1.76+) para archivos binarios que no se pueden descargar.
- Binarios `koto_compile`, `ivm_run` y `iroha_cli`. Si puedo acceder al espacio de trabajo de pago, no podemos descartar artefactos de lanzamiento compatibles:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Estos archivos binarios se pueden instalar correctamente en el espacio de trabajo habitual.
> Они никогда не линкуются с `serde`/`serde_json`; Los códigos Norito se ejecutan de un extremo a otro.

## 1. Запустите одноузловую dev сетьEn los repositorios está Docker Compose Bundle, diseñado `kagami swarm` (`defaults/docker-compose.single.yml`). En el caso de дефолтную genesis, configuración del cliente y sondas de salud, que Torii están disponibles en `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Coloque el contenido en primer plano (en el teléfono o en primer plano). Cualquiera que sea el usuario de la CLI está conectado a este peer como `defaults/client.toml`.

## 2. Nuevo contrato

Utilice un director de escritorio y un programa de televisión minimal por primera vez Kotodama:

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

> Pruebe los dispositivos Kotodama en el sistema de control versión anterior. Primeros ajustes en el portal, tales como descargas en [galería de inicios Norito](./examples/), o ninguna otra opción богатый стартовый набор.

## 3. Compilación y funcionamiento en seco con IVM

Compile el contrato con el bloque de motor IVM/Norito (`.to`) y descargue ego localmente, según sea necesario. syscalls es un proceso de configuración de conjuntos:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El corredor envía el registro `info("Hello from Kotodama")` y envía la llamada al sistema `SET_ACCOUNT_DETAIL` para proteger el host. Además del binario opcional `ivm_tool`, el comando `ivm_tool inspect target/quickstart/hello.to` incluye encabezado ABI, bits de función y puntos de entrada deportivos.

## 4. Отправьте байткод через ToriiCuando utilice un robot, compárelo en Torii mediante CLI. La identificación del desarrollador completa con el clic público en `defaults/client.toml`, cuenta de identificación del usuario:
```
<i105-account-id>
```

Utilice un archivo de configuración, introduzca la URL Torii, ID de cadena y haga clic en:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

El código CLI para la transmisión Norito, puede conectar un programa de desarrollo y ejecutar un peer robotizado. Seleccione el registro Docker para syscall `set_account_detail` o monitoree la CLI para su transmisión comprometida.

## 5. Проверьте изменение состояния

Utilice el perfil CLI, consulte los detalles de la cuenta y el contrato anterior:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

Para verificar la carga útil JSON en la base Norito:

```json
{
  "hello": "world"
}
```

Si no está disponible, consulte el servicio Docker. `iroha`, достиг состояния `Committed`.

## Следующие шаги- Изучите авто-сгенерированную [galerею примеров](./examples/), чтобы увидеть,
  как более продвинутые сниппеты Kotodama маппятся на syscalls Norito.
- Прочитайте [Norito guía de introducción](./getting-started) para todo el mundo
  объяснения инструментов компилятора/раннера, деплоя manifests and метаданных IVM.
- Antes de conectar sus contratos, utilice `npm run sync-norito-snippets` en
  espacio de trabajo, cómo regenerar fragmentos de archivos y descargar documentos, portales y artefactos
  синхронизированными с исходниками в `crates/ivm/docs/examples/`.