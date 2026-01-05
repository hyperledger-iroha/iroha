<!-- Japanese translation of docs/genesis.md -->

---
lang: ja
direction: ltr
source: docs/genesis.md
status: complete
translator: manual
---

# ジェネシス設定

`genesis.json` は、Iroha ネットワーク起動時に最初に実行されるトランザクション群を定義するファイルです。ファイルは以下のフィールドを持つ JSON オブジェクトです。

- `chain` – チェーン固有の識別子。
- `executor`（任意） – Executor バイトコード（`.to`）へのパス。指定した場合は、ジェネシスの最初のトランザクションとして Upgrade 命令が挿入されます。省略するとアップグレードは行われず、組み込み Executor が使用されます。
- `ivm_dir` – IVM バイトコードライブラリを格納したディレクトリ。省略時は `"."`。
- `consensus_mode` – マニフェストに記載するコンセンサスモード。必須です。Iroha3 では `"Npos"`（既定）、Iroha2 では `"Permissioned"` を使用します。
- `transactions` – ジェネシストランザクションのリスト（順番に実行されます）。各トランザクションは以下の要素を含められます:
  - `parameters` – 初期ネットワークパラメータ。
  - `instructions` – Norito でエンコードされた命令列。
  - `ivm_triggers` – IVM バイトコードを実行するトリガー。
  - `topology` – 初期ピアトポロジ。各エントリは `peer`（PeerId の文字列、つまり公開鍵）と `pop_hex` を使用します。`pop_hex` は作成時に省略できますが、署名前には必須です。
- `crypto` – `iroha_config.crypto` のスナップショット（`default_hash`, `allowed_signing`, `allowed_curve_ids`, `sm2_distid_default`, `sm_openssl_preview`）。`allowed_curve_ids` は `crypto.curves.allowed_curve_ids` を反映し、どのカーブ ID が許可されているかをマニフェスト側で公開します。`allowed_signing` に `sm2` を含める場合は、ハッシュ既定値も必ず `sm3-256` に変更してください。`sm` 機能を有効にせずビルドしたバイナリは `sm2` を含む設定を拒否します。

例（`kagami genesis generate default --consensus-mode npos` の出力。命令列は一部省略）:

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [78, 82, 84, 48, 0, 0, 19, 123, ...],
      "ivm_triggers": [],
      "topology": []
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

## SM2/SM3 向け `crypto` ブロックの準備

まずは xtask ヘルパーを使って、鍵インベントリと貼り付け用スニペットを一度に生成しましょう:

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` には次の内容が出力されます:

```toml
# アカウント鍵素材
public_key = "sm2:86264104..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]  # 検証専用に保つ場合は "sm2" を外す
allowed_curve_ids = [1]               # コントローラを解放する際に必要な ID（例: 15=SM2）を追加
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # オプション: OpenSSL/Tongsuo プロバイダ導入時のみ
```

`public_key` と `private_key` をアカウント／クライアント設定にコピーし、`genesis.json` の `crypto` ブロックも同じ内容に更新します（例: `default_hash` を `sm3-256` に設定し、`allowed_signing` に `"sm2"` を追加し、`allowed_curve_ids` を適切に設定）。Kagami はハッシュ方式やカーブ一覧と署名リストが不一致なマニフェストを拒否します。

> **ヒント:** `--snippet-out -` を指定するとスニペットを標準出力に直接表示できます。`--json-out -` で鍵インベントリも同様に出力できます。

低レベルの CLI コマンドを個別に実行したい場合は、以下の手順を利用してください:

```bash
# 1. 決定的な鍵素材を生成（JSON をディスクへ出力）
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. クライアント／設定ファイルへ貼り付けられるスニペットを再構築
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **ヒント:** 上記では `jq` を使って手作業でのコピーを省いています。`jq` が利用できない環境では `sm2-key.json` を開いて `private_key_hex` をコピーし、`crypto sm2 export` に直接渡してください。

> **移行ガイド:** 既存ネットワークで SM2/SM3/SM4 を有効化する場合は
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> を参照してください。`iroha_config` のレイヤー適用、`kagami --enable-sm`
> によるマニフェスト再生成、ロールバック手順までまとめています。

## 生成と検証

1. テンプレートを生成:
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
  ```
   `--consensus-mode` は Kagami が `parameters` ブロックにシードするコンセンサスパラメータを制御します。Iroha3 では `npos` が必須で段階的な切り替えはサポートされません。Iroha2 は `permissioned` が既定で、必要に応じて `--next-consensus-mode`/`--mode-activation-height` で `npos` を段階導入できます。`npos` を指定すると `sumeragi_npos_parameters` がシードされ、正規化/署名時に `SetParameter` 命令としてブロックへ反映されます。
2. 必要に応じて `genesis.json` を編集し、署名と検証を行う:
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   SM 系暗号を有効化したマニフェストが必要な場合は `--default-hash sm3-256` を指定し、`--allowed-signing sm2` を追加してください（複数のアルゴリズムを許可する場合は `--allowed-signing` を繰り返し指定します）。必要に応じて `--sm2-distid-default <ID>` で識別子を上書きできます。

   `irohad` を `--genesis-manifest-json` のみで起動した場合（署名済みジェネシスブロックが無いケース）、実行時の暗号設定はマニフェストの `crypto` セクションから自動的に適用されます。署名付きジェネシスブロックを併用する場合は、マニフェストと設定値が完全に一致している必要があります。

`kagami genesis sign` は JSON の整合性を検査し、`genesis.file` 経由でそのまま利用可能な Norito エンコード済みブロックを生成します。生成される `genesis.signed.nrt` は、バージョンバイトと Norito ヘッダ（ペイロードレイアウトを記述）を含むカノニカルなワイヤ形式であり、配布時は常にこちらを共有してください。互換性のために `.scale` という古い拡張子を使うこともあります。ジェネシスで Executor をアップグレードしない場合は `executor` フィールド自体を省略し、`.to` ファイルを渡さなくても構いません。

NPoS マニフェスト（`--consensus-mode npos` または Iroha2 の段階的な切り替え）を署名する場合、`kagami genesis sign` は `sumeragi_npos_parameters` を要求します。`kagami genesis generate --consensus-mode npos` で生成するか、パラメータを手動で追加してください。
既定では `kagami genesis sign` はマニフェストの `consensus_mode` を使用します。`--consensus-mode` で上書きできます。

## ジェネシスでできること

ジェネシスでは以下をサポートしています。Kagami は決められた順序でトランザクションを組み立て、すべてのピアが同じシーケンスを決定論的に実行するようにします。

- **パラメータ:** Sumeragi（block/commit 時間、許容ドリフト）、Block（最大トランザクション数）、Transaction（最大命令数・バイトコードサイズ）、Executor／スマートコントラクトの制限（燃料、メモリ、深さ）などを初期化。`Sumeragi::NextMode` と `sumeragi_npos_parameters` は `parameters` ブロックでシードされ、署名時に `SetParameter` 命令としてジェネシスブロックに反映されます。
- **ネイティブ命令:** Domain / Account / Asset Definition の登録・削除、資産の Mint/Burn/Transfer、所有権移転、メタデータ更新、権限／ロールの付与など。
- **IVM トリガー:** `ivm_triggers` を介して IVM バイトコードを実行するトリガーを登録。実行ファイルは `ivm_dir` からの相対パスで解決されます。
- **トポロジ:** 任意のトランザクション（通常は最初か最後）で `topology` 配列に `{ "peer": "<public_key>", "pop_hex": "<hex>" }` エントリを指定し、初期ピア集合を提供します。`pop_hex` は作成時に省略できますが、署名前には必須です。
- **Executor アップグレード（任意）:** `executor` を指定した場合は最初のトランザクションとして Upgrade 命令が挿入されます。指定しなければ、パラメータ／命令から開始します。

### トランザクションの順序

概念的には、ジェネシストランザクションは以下の順で処理されます。

1. （任意）Executor アップグレード
2. `transactions` に記載された各トランザクションに対して:
   - パラメータ更新
   - ネイティブ命令
   - IVM トリガー登録
   - トポロジ追加

Kagami とノード側のコードはこの順序を強制するため、同一トランザクション内でもパラメータが命令より先に適用されます。

## 推奨ワークフロー

- Kagami でテンプレートを作成:
  - 組み込み ISI のみ:  
    `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json`（Iroha3 既定。Iroha2 は `--consensus-mode permissioned`）
  - Executor アップグレードあり: `--executor <path/to/executor.to>` を追加
- `<PK>` には `iroha_crypto::Algorithm` が認識する任意のマルチハッシュを使えます。`--features gost` でビルドした Kagami なら TC26 GOST 系の鍵（例: `gost3410-2012-256-paramset-a:...`）も利用可能です。
- 編集中は `kagami genesis validate genesis.json` で検証すると便利です。
- デプロイ用に署名:  
  `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- 各ピアの設定: `genesis.file` に署名済み Norito ファイル（例: `genesis.signed.nrt`）、`genesis.public_key` に同じ `<PK>` を設定。

メモ:
- Kagami の “default” テンプレートは、サンプルドメインとアカウントの登録、簡単な資産の Mint、最小限の権限付与を含み、組み込み ISI のみで構成されています（`.to` は不要）。
- Executor アップグレードを含める場合は必ず最初のトランザクションになります。Kagami も生成／署名時にこれをチェックします。
- `kagami genesis validate` は不正な `Name`（空白を含むなど）や構造化されていない命令を署名前に検出します。

## Docker / Swarm での運用

付属の Docker Compose / Swarm ツールは、Executor の有無に応じて次のように扱います。

- Executor なし: `executor` フィールドが空または欠落している場合はそのまま署名します。
- Executor あり: コンテナ内で解決可能な絶対パスに変換し、署名まで自動で行います。

これにより、事前に IVM サンプルを用意していない開発環境でも扱いやすく、必要に応じて Executor のアップグレードにも対応できます。
