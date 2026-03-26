---
lang: he
direction: rtl
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Inicio rapido de Norito
תיאור: Crea, valida y despliega un contrato Kotodama con las herramientas de release y la red predeterminada de un solo peer.
Slug: /norito/Quickstart
---

Este recorrido refleja el flujo que esperamos que sigan los desarrolladores al aprender Norito y Kotodama por primera vez: arrancar una red determinista de un solo peer, compilar un dry porlo, en hair Torii con el CLI de referencia.

El contrato de ejemplo escribe un par clave/valor en la cuenta del llamador para que puedas verificar el efecto lateral de inmediato con `iroha_cli`.

## דרישות קודמות

- [Docker](https://docs.docker.com/engine/install/) עם Compose V2 habilitado (se usa para iniciar el peer de muestra definido en `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+) para construir los binarios auxiliares si no descargas los publicados.
- Binarios `koto_compile`, `ivm_run` ו-`iroha_cli`. Puedes construirlos desde el checkout del workspace como se muestra abajo או הורדת חפצי חפצים של כתב שחרור:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Los binarios anteriores son seguros de instalar junto con el resto del workspace.
> Nunca enlazan con `serde`/`serde_json`; los codec Norito זה אפליקן מקצה לקצה.

## 1. Inicia una red dev de un solo peer

המאגר כולל חבילה של Docker Compose generado por `kagami swarm` (`defaults/docker-compose.single.yml`). Conecta el genesis por defecto, la configuracion del cliente y los probes health para que Torii accessible sea en `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deja el contenedor corriendo (en primer plano o desacoplado). Todas las llamadas posteriores del CLI apuntan a este peer mediaante `defaults/client.toml`.

## 2. Redacta el contrato

Crea un directorio de trabajo y guarda el emplo minimo de Kotodama:

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

> התקנת הוראות הפעלה Kotodama בשליטה בגרסאות. Los ejemplos alojados en el portal tambien estan disponibles en la [galeria de ejemplos Norito](./examples/) סי quieres un punto de partida mas completo.

## 3. Compila y haz dry-run con IVM

אגד את קוד הבתים IVM/Norito (`.to`) והוצאת קוד גישה למערכת ההפעלה של המארח.

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El runner imprime el log `info("Hello from Kotodama")` y ejecuta el syscall `SET_ACCOUNT_DETAIL` contra el host simulado. זה אופציונלי בינארי `ivm_tool` esta disponible, `ivm_tool inspect target/quickstart/hello.to` muestra el encabezado ABI, los bits de features y los entrypoints exportados.

## 4. Envia el bytecode באמצעות Torii

עם קוד התקדמות, חיבור קוד בייט ל-Torii בשימוש ב-CLI. La identidad de desarrollo por defecto se deriva de la clave publica en `defaults/client.toml`, por lo que el ID de cuenta es
```
soraカタカナ...
```

Usa el archivo de configuracion para suministrar la URL de Torii, el chain ID y la clave de firma:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```El CLI codifica la transaccion con Norito, la firma con la clave de desarrollo y la envia al peer en ejecucion. Observa los logs de Docker para el syscall `set_account_detail` o monitorea la salida del CLI para el hash de transaccion comprometida.

## 5. Verifica el cambio de estado

Usa el mismo perfil del CLI para obtener el חשבון פרטי que escribio el contrato:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Deberias ver elloadload JSON respaldado por Norito:

```json
{
  "hello": "world"
}
```

Si falta el valor, confirma que el servicio de Docker compose sigue en ejecucion y que el hash de transaccion reportado por `iroha` llego al estado `Committed`.

## Siguiente pasos

- Explora la [galeria de ejemplos](./examples/) autogenerada para ver
  como fragmentos Kotodama mas avanzados se mapean a syscalls Norito.
- Lee la [guia de inicio de Norito](./getting-started) עבור הסבר אחד
  mas profunda del tooling de compilador/ranner, el despliegue de manifests y la metadata de IVM.
- Cuando iteres en tus propios contratos, ארה"ב `npm run sync-norito-snippets` en el
  סביבת עבודה ל-regenererar קטעי טקסט להורדה דה modo que los docs del portal y los artefactos
  se mantengan sincronizados con las fuentes en `crates/ivm/docs/examples/`.