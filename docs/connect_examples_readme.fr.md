---
lang: fr
direction: ltr
source: docs/connect_examples_readme.md
status: complete
translator: manual
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-11-02T04:40:28.804038+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/connect_examples_readme.md (Iroha Connect Examples) -->

## Exemples Iroha Connect (application / wallet Rust)

Lancez les deux exemples Rust de bout en bout contre un nœud Torii.

Prérequis
- Nœud Torii avec `connect` activé sur `http://127.0.0.1:8080`.
- Toolchain Rust (stable).
- Python 3.9+ avec le package `iroha-python` installé (pour le helper CLI ci‑dessous).

Exemples
- Exemple application : `crates/iroha_torii_shared/examples/connect_app.rs`
- Exemple wallet : `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Helper CLI Python : `python -m iroha_python.examples.connect_flow`

Ordre de démarrage
1) Terminal A — App (affiche `sid` + tokens, connecte le WS, envoie `SignRequestTx`) :

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   Exemple de sortie :

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) Terminal B — Wallet (se connecte avec `token_wallet`, répond avec `SignResultOk`) :

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Exemple de sortie :

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) Le terminal de l’app affiche le résultat :

    app: got SignResultOk algo=ed25519 sig=deadbeef

  Utilisez le nouveau helper `connect_norito_decode_envelope_sign_result_alg` (et les
  wrappers Swift/Kotlin dans ce dossier) pour récupérer la chaîne d’algorithme lors du
  décodage du payload.

Notes
- Les exemples dérivent des éphémères de démonstration à partir de `sid`, ce qui permet à
  l’app et au wallet d’interopérer automatiquement. Ne pas utiliser en production.
- Le SDK impose le binding AEAD (AAD) et `seq` comme nonce ; les frames de contrôle après
  approbation doivent être chiffrés.
- Pour les clients Swift, consultez `docs/connect_swift_integration.md` /
  `docs/connect_swift_ios.md` et validez avec `make swift-ci` afin que la télémétrie des
  dashboards reste alignée avec les exemples Rust et que les métadonnées Buildkite
  (`ci/xcframework-smoke:<lane>:device_tag`) restent cohérentes.
- Utilisation du helper CLI Python :

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

  Le CLI affiche les informations typées de la session, produit un instantané d’état
  Connect et émet le frame `ConnectControlOpen` encodé en Norito. Passez `--send-open` pour
  renvoyer le payload vers Torii, `--frame-output-format binary` pour écrire les octets
  bruts, `--frame-json-output` pour obtenir un blob JSON encodé en base64 et
  `--status-json-output` lorsque vous avez besoin d’un snapshot typé pour l’automatisation.
  Vous pouvez aussi charger les métadonnées de l’application depuis un fichier JSON via
  `--app-metadata-file metadata.json` contenant les champs `name`, `url` et `icon_hash`
  (voir `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). Pour
  générer un nouveau template :
  `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`.
  Pour les exécutions purement télémétriques, vous pouvez sauter complètement la création
  de session avec `--status-only` et, si besoin, générer du JSON via
  `--status-json-output status.json`.
