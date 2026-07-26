---
lang: pt
direction: ltr
source: docs/connect_examples_readme.md
status: complete
translator: manual
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-11-02T04:40:28.804038+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/connect_examples_readme.md (Iroha Connect Examples) -->

## Exemplos de Iroha Connect (aplicativo / carteira em Rust)

Execute os dois exemplos em Rust de ponta a ponta contra um nó Torii.

Pré‑requisitos
- Nó Torii com `connect` habilitado em `http://127.0.0.1:8080`.
- Toolchain de Rust (stable).
- Python 3.9+ com o pacote `iroha-python` instalado (para o helper de CLI abaixo).

Exemplos
- Exemplo de app: `crates/iroha_torii_shared/examples/connect_app.rs`
- Exemplo de carteira: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Helper de CLI em Python: `python -m iroha_python.examples.connect_flow`

Ordem de inicialização
1) Terminal A — App (imprime `sid` + tokens, conecta o WS, envia `SignRequestTx`):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   Saída de exemplo:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) Terminal B — Carteira (conecta usando `token_wallet` e responde com `SignResultOk`):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Saída de exemplo:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) O terminal da app imprime o resultado:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  Use o novo helper `connect_norito_decode_envelope_sign_result_alg` (e os wrappers em
  Swift/Kotlin neste diretório) para recuperar a string do algoritmo ao decodificar o
  payload.

Notas
- Os exemplos derivam chaves efêmeras de demonstração a partir de `sid`, de forma que app
  e carteira interoperam automaticamente. Não use isso em produção.
- O SDK aplica o binding AEAD AAD e `seq` como nonce; frames de controle após a aprovação
  devem ser criptografados.
- Para clientes em Swift, consulte `docs/connect_swift_integration.md` /
  `docs/connect_swift_ios.md` e valide com `make swift-ci` para que a telemetria dos
  dashboards permaneça alinhada com os exemplos em Rust e os metadados do Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) continuem corretos.
- Uso do helper de CLI em Python:

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

  O CLI imprime as informações tipadas da sessão, gera um snapshot de status do Connect e
  emite o frame `ConnectControlOpen` codificado em Norito. Use `--send-open` para enviar o
  payload de volta ao Torii, `--frame-output-format binary` para gravar bytes brutos,
  `--frame-json-output` para obter um blob JSON adequado a base64 e `--status-json-output`
  quando precisar de um snapshot tipado para automação. Você também pode carregar metadados
  do aplicativo a partir de um arquivo JSON via `--app-metadata-file metadata.json`
  contendo os campos `name`, `url` e `icon_hash` (veja
  `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). Gere um novo
  template com
  `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`.
  Para execuções apenas de telemetria, é possível pular completamente a criação de sessão
  usando `--status-only` e, opcionalmente, gerar JSON com
  `--status-json-output status.json`.

