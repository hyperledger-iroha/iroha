---
lang: es
direction: ltr
source: docs/connect_examples_readme.md
status: complete
translator: manual
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-11-02T04:40:28.804038+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/connect_examples_readme.md (Iroha Connect Examples) -->

## Ejemplos de Iroha Connect (aplicación / wallet en Rust)

Ejecuta los dos ejemplos en Rust de extremo a extremo contra un nodo Torii.

Requisitos previos
- Nodo Torii con `connect` habilitado en `http://127.0.0.1:8080`.
- Toolchain de Rust (stable).
- Python 3.9+ con el paquete `iroha-python` instalado (para el helper de CLI de abajo).

Ejemplos
- Ejemplo de aplicación: `crates/iroha_torii_shared/examples/connect_app.rs`
- Ejemplo de wallet: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Helper de CLI en Python: `python -m iroha_python.examples.connect_flow`

Orden de arranque
1) Terminal A — App (imprime `sid` + tokens, conecta el WS, envía `SignRequestTx`):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   Salida de ejemplo:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) Terminal B — Wallet (se conecta con `token_wallet` y responde con `SignResultOk`):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Salida de ejemplo:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) La terminal de la app imprime el resultado:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  Usa el nuevo helper `connect_norito_decode_envelope_sign_result_alg` (y los wrappers de
  Swift/Kotlin de esta carpeta) para recuperar la cadena de algoritmo al decodificar el
  payload.

Notas
- Los ejemplos derivan claves efímeras de demo a partir de `sid`, de modo que app y wallet
  interoperan automáticamente. No lo uses en producción.
- El SDK impone el binding AEAD AAD y `seq` como nonce; los control frames posteriores a
  la aprobación deben ir cifrados.
- Para clientes en Swift, consulta `docs/connect_swift_integration.md` /
  `docs/connect_swift_ios.md` y valida con `make swift-ci` para que la telemetría de los
  dashboards se mantenga alineada con los ejemplos en Rust y los metadatos de Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) sigan siendo coherentes.
- Uso del helper de CLI en Python:

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  El CLI imprime la información tipada de la sesión, vuelca una instantánea de estado de
  Connect y emite el frame Norito‑codificado `ConnectControlOpen`. Pasa `--send-open` para
  enviar de vuelta el payload a Torii, usa `--frame-output-format binary` para escribir
  bytes crudos, `--frame-json-output` para obtener un blob JSON apto para base64 y
  `--status-json-output` cuando necesites una instantánea tipada para automatización.
  También puedes cargar metadatos de la aplicación desde un archivo JSON mediante
  `--app-metadata-file metadata.json` que contenga los campos `name`, `url` e `icon_hash`
  (ver `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). Genera
  una plantilla nueva con
  `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`.
  Para ejecuciones solo de telemetría puedes omitir completamente la creación de sesión con
  `--status-only` y, opcionalmente, volcar JSON con
  `--status-json-output status.json`.

