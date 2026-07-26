## Iroha Connect Examples (Rust App/Wallet)

Run the two Rust examples end‑to‑end with a Torii node.

Prerequisites
- Torii node with `connect` enabled at `http://127.0.0.1:8080`.
- Rust toolchain (stable).
- Python 3.9+ with the `iroha-python` package installed (for the CLI helper below).

Examples
- App example: `crates/iroha_torii_shared/examples/connect_app.rs`
- Wallet example: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI helper: `python -m iroha_python.examples.connect_flow`

Order of startup
1) Terminal A — App (prints sid + tokens, connects WS, sends SignRequestTx):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   Sample output:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) Terminal B — Wallet (connect with token_wallet, replies with SignResultOk):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Sample output:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) App terminal prints the result:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  Use the new `connect_norito_decode_envelope_sign_result_alg` helper (and the
  Swift/Kotlin wrappers in this folder) to retrieve the algorithm string when
  decoding the payload.

Notes
- Examples derive demo ephemerals from `sid` so app/wallet interoperate automatically. Do not use in production.
- SDK enforces AEAD AAD binding and seq‑as‑nonce; post‑approval control frames should be encrypted.
- Relay forwarding is decentralized: Torii uses local delivery and optional Iroha node-to-node P2P broadcast;
  there is no centralized Connect relay server mode.
- Status snapshots expose relay runtime state (`policy.relay_strategy`,
  `policy.relay_effective_strategy`, `policy.relay_p2p_attached`) and P2P relay
  counters (`p2p_rebroadcasts_total`, `p2p_rebroadcast_skipped_total`).
- For Swift clients, see `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` and validate with `make swift-ci` so dashboard telemetry stays aligned with Rust examples and Buildkite metadata (`ci/xcframework-smoke:<lane>:device_tag`) remains intact.
- Python CLI helper usage:

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

  The CLI prints the typed session info, dumps the Connect status snapshot, and emits the Norito-encoded `ConnectControlOpen` frame. Pass `--send-open` to post the payload back to Torii, use `--frame-output-format binary` to write raw bytes, `--frame-json-output` for a base64-friendly JSON blob, and `--status-json-output` when you need a typed snapshot for automation. You can also load application metadata from a JSON file via `--app-metadata-file metadata.json` containing `name`, `url`, and `icon_hash` fields (see `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). Generate a fresh template with `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`. For telemetry-only runs, you can skip session creation entirely with `--status-only` and optionally dump JSON via `--status-json-output status.json`.
