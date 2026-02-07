---
lang: ja
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T15:38:30.660147+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM 構成の移行

# SM 設定の移行

SM2/SM3/SM4 機能セットを展開するには、単にコンパイルするだけでは不十分です。
`sm` 機能フラグ。ノードは、階層化されたノードの背後にある機能をゲートします。
`iroha_config` プロファイルと、ジェネシス マニフェストに一致するものが含まれることを期待します
デフォルト。このノートでは、プロモーション時に推奨されるワークフローについて説明します。
既存のネットワークを「Ed25519 のみ」から「SM 対応」に変更します。

## 1. ビルドプロファイルを確認する

- `--features sm` を使用してバイナリをコンパイルします。次の場合にのみ `sm-ffi-openssl` を追加します。
  OpenSSL/Tongsuo プレビュー パスを実行することを計画しています。 `sm` なしでビルドする
  設定が有効であっても、アドミッション中に `sm2` 署名を拒否する機能
  彼ら。
- CI が `sm` アーティファクトを公開し、すべての検証ステップ (`cargo) を確認します。
  test -p iroha_crypto --features sm`、統合フィクスチャ、ファズ スイート) パス
  デプロイする予定の正確なバイナリ上で。

## 2. レイヤー構成のオーバーライド

`iroha_config` は、`defaults` → `user` → `actual` の 3 つの層を適用します。 SMを発送する
オペレーターがバリデーターに配布する `actual` プロファイル内でオーバーライドされ、
開発者のデフォルトが変更されないように、`user` を Ed25519 のみのままにしておきます。

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

同じブロックを「kagami Genesis」経由で `defaults/genesis` マニフェストにコピーします。
必要に応じて …` (add `--allowed-signing sm2 --default-hash sm3-256` を生成します
オーバーライド) なので、`parameters` ブロックと挿入されたメタデータは
ランタイム構成。マニフェストと構成が変更された場合、ピアは起動を拒否します。
スナップショットが分岐します。

## 3. Genesis マニフェストを再生成する

- すべての場合に `kagami genesis generate --consensus-mode <mode>` を実行します
  環境に追加し、更新された JSON を TOML オーバーライドとともにコミットします。
- マニフェスト (`kagami genesis sign …`) に署名し、`.nrt` ペイロードを配布します。
  未署名の JSON マニフェストからブートストラップするノードは、ランタイム暗号を派生します
  ファイルから直接設定を行う - 同じ一貫性が維持される
  小切手。

## 4. トラフィックの前に検証する

- 新しいバイナリと構成を使用してステージング クラスターをプロビジョニングし、次のことを確認します。
  - ピアが再起動すると、`/status` は `crypto.sm_helpers_available = true` を公開します。
  - `sm2` が存在しない間、Torii アドミッションは依然として SM2 署名を拒否します。
    `allowed_signing` は、リストにある場合に混合 Ed25519/SM2 バッチを受け入れます。
    両方のアルゴリズムが含まれています。
  - `iroha_cli tools crypto sm2 export …` は、新しいキー マテリアルを介してシードされたキー マテリアルを往復します。
    デフォルト。
- SM2 決定論的署名をカバーする統合スモーク スクリプトを実行し、
  ホスト/VM の一貫性を確認するための SM3 ハッシュ。

## 5. ロールバック計画- 反転を文書化します。`sm2` を `allowed_signing` から削除して復元します。
  `default_hash = "blake2b-256"`。同じ `actual` を通じて変更をプッシュします。
  パイプラインをプロファイルするため、すべてのバリデーターが単調に反転します。
- SM マニフェストをディスク上に保持します。不一致の設定とジェネシスを認識するピア
  データの開始が拒否されるため、部分的なロールバックが防止されます。
- OpenSSL/Tongsuo プレビューが関係する場合は、無効にする手順を含めます。
  `crypto.enable_sm_openssl_preview` から共有オブジェクトを削除します
  ランタイム環境。

## 参考資料

- [`docs/genesis.md`](../../genesis.md) – ジェネシスマニフェストの構造と
  `crypto` ブロック。
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  `iroha_config` セクションとデフォルトの概要。
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – 終了
  SM 暗号化を出荷するためのエンド オペレーター チェックリスト。