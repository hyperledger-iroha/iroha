---
lang: ja
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71c7478ed1ad61e08608dfd31275a2583cc9563754b56a3fc17080c79f9ee417
source_last_modified: "2025-11-18T09:42:31.664173+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/mochi/packaging.md -->

# MOCHI パッケージングガイド

このガイドは、MOCHI デスクトップ監督プロセスのバンドルを構築し、
生成されたアーティファクトを検査し、バンドルに同梱される runtime
オーバーライドを調整する方法を説明する。再現性のあるパッケージングと
CI 利用に焦点を当て、クイックスタートを補完する。

## 前提条件

- Rust toolchain（edition 2024 / Rust 1.82+）と、workspace 依存が
  すでにビルドされていること。
- `irohad`, `iroha_cli`, `kagami` が対象ターゲット向けにコンパイル済み。
  bundler は `target/<profile>/` のバイナリを再利用する。
- `target/` または任意の出力先に十分なディスク容量。

bundler 実行前に依存を一度ビルドする:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## バンドルの構築

リポジトリルートから専用 `xtask` コマンドを実行する:

```bash
cargo xtask mochi-bundle
```

デフォルトでは `target/mochi-bundle/` に release バンドルを生成し、
ファイル名はホスト OS とアーキテクチャから導出される（例:
`mochi-macos-aarch64-release.tar.gz`）。以下のフラグでカスタマイズできる:

- `--profile <name>` – Cargo プロファイル（`release`, `debug`, あるいは
  カスタムプロファイル）を選択。
- `--no-archive` – `.tar.gz` を作成せずに展開ディレクトリを保持（ローカル検証用）。
- `--out <path>` – `target/mochi-bundle/` の代わりに任意の出力先へ書き出す。
- `--kagami <path>` – 既存の `kagami` 実行ファイルを指定してアーカイブに含める。
  省略時は選択したプロファイルのバイナリを再利用（またはビルド）する。
- `--matrix <path>` – バンドルのメタデータを JSON マトリクスに追記（無ければ作成）。
  CI パイプラインがホスト/プロファイルごとの成果物を記録できる。エントリには
  バンドルディレクトリ、マニフェストのパスと SHA‑256、任意のアーカイブ場所、
  最新の smoke-test 結果が含まれる。
- `--smoke` – バンドル後に `mochi --help` を実行して軽量な smoke gate を通す。
  失敗時に欠落依存を公開前に検知できる。
- `--stage <path>` – 完成バンドル（および生成されたアーカイブ）を staging
  ディレクトリへコピーし、マルチプラットフォームビルドの成果物集約に使う。

このコマンドは `mochi-ui-egui`, `kagami`, `LICENSE`, サンプル設定、
`mochi/BUNDLE_README.md` をバンドルへコピーする。バイナリ横に
決定論的な `manifest.json` が生成され、CI ジョブはファイルハッシュと
サイズを追跡できる。

## バンドル構成と検証

展開バンドルは `BUNDLE_README.md` に記載された構成に従う:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

`manifest.json` は各アーティファクトの SHA‑256 ハッシュを列挙する。
別のシステムにコピーした後は次のように検証する:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI パイプラインは展開ディレクトリのキャッシュ、アーカイブ署名、
マニフェストのリリースノート同梱が可能。マニフェストには
ジェネレータのプロファイル、ターゲット triple、作成タイムスタンプが含まれ、
プロベナンス追跡を支援する。

## ランタイムオーバーライド

MOCHI はヘルパーバイナリやランタイム位置を CLI フラグまたは環境変数で
解決する:

- `--data-root` / `MOCHI_DATA_ROOT` – peer 設定、ストレージ、ログの
  ワークスペースを上書き。
- `--profile` – トポロジプリセット（`single-peer`, `four-peer-bft`）を切替。
- `--torii-start`, `--p2p-start` – サービス割り当てに使うベースポートを変更。
- `--irohad` / `MOCHI_IROHAD` – 特定の `irohad` バイナリを指定。
- `--kagami` / `MOCHI_KAGAMI` – バンドルされた `kagami` を上書き。
- `--iroha-cli` / `MOCHI_IROHA_CLI` – オプションの CLI ヘルパーを上書き。
- `--restart-mode <never|on-failure>` – 自動再起動を無効化、または
  指数バックオフポリシーを強制。
- `--restart-max <attempts>` – `on-failure` モードでの再起動試行回数を上書き。
- `--restart-backoff-ms <millis>` – 自動再起動の基準バックオフを設定。
- `MOCHI_CONFIG` – `config/local.toml` のパスを指定。

CLI ヘルプ（`mochi --help`）に全フラグが表示される。環境変数の上書きは
起動時に適用され、UI 内の Settings ダイアログと併用できる。

## CI 利用のヒント

- `cargo xtask mochi-bundle --no-archive` を実行して、ディレクトリ形式の
  バンドルを生成し、プラットフォーム固有ツール（Windows は ZIP、Unix は tar）で
  圧縮する。
- `cargo xtask mochi-bundle --matrix dist/matrix.json` でバンドルメタデータを記録し、
  リリースジョブがホスト/プロファイルの成果物を列挙する単一の JSON インデックスを
  公開できるようにする。
- 各ビルドエージェントで `cargo xtask mochi-bundle --stage /mnt/staging/mochi`
  （など）を実行し、バンドルとアーカイブを共有ディレクトリへ集約する。
  その後の publishing job がそれを消費できる。
