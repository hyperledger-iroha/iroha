---
lang: ja
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2026-01-03T18:08:00.656311+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI バンドルツール

MOCHI には軽量パッケージング ワークフローが付属しているため、開発者は
カスタム CI スクリプトを配線する必要のないポータブル デスクトップ バンドル。 `xtask`
サブコマンドは、コンパイル、レイアウト、ハッシュ、および (オプションで) アーカイブを処理します。
一発で作成。

## バンドルの生成

```bash
cargo xtask mochi-bundle
```

デフォルトでは、このコマンドはリリース バイナリをビルドし、以下にバンドルをアセンブルします。
`target/mochi-bundle/`、`mochi-<os>-<arch>-release.tar.gz` アーカイブを発行します
決定的な `manifest.json` と並んで。マニフェストには、すべてのファイルがリストされます。
そのサイズと SHA-256 ハッシュにより、CI パイプラインが検証を再実行したり公開したりできるようになります
証明書。ヘルパーは、`mochi` デスクトップ シェルと
ワークスペース `kagami` バイナリが存在するため、genesis の生成は
箱。

### フラグ

|旗 |説明 |
|---------------------|----------------------------------------------------------------------------|
| `--out <dir>` |出力ディレクトリをオーバーライドします (デフォルトは `target/mochi-bundle`)。         |
| `--profile <name>` |特定の Cargo プロファイルを使用してビルドします (テスト用の `debug` など)。              |
| `--no-archive` | `.tar.gz` アーカイブをスキップし、準備されたフォルダーのみを残します。               |
| `--kagami <path>` | `iroha_kagami` をビルドする代わりに、明示的な `kagami` バイナリを使用します。         |
| `--matrix <path>` | CI 来歴追跡のためにバンドル メタデータを JSON マトリックスに追加します。         |
| `--smoke` |パッケージ化されたバンドルから基本的な実行ゲートとして `mochi --help` を実行します。      |
| `--stage <dir>` |完成したバンドル (存在する場合はアーカイブ) をステージング フォルダーにコピーします。 |

`--stage` は、各ビルド エージェントがビルド エージェントをアップロードする CI パイプラインを対象としています。
アーティファクトを共有場所に移動します。ヘルパーはバンドル ディレクトリを再作成し、
生成されたアーカイブをステージング ディレクトリにコピーして、ジョブを公開できるようにします。
シェルスクリプトを使用せずにプラットフォーム固有の出力を収集します。

バンドル内のレイアウトは意図的にシンプルになっています。

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### ランタイムオーバーライド

パッケージ化された `mochi` 実行可能ファイルは、ほとんどのコマンド ライン オーバーライドを受け入れます。
共通のスーパーバイザ設定。編集する代わりにこれらのフラグを使用します
実験時は `config/local.toml`:

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

CLI 値はすべて、`config/local.toml` エントリおよび環境よりも優先されます。
変数。

## スナップショットの自動化

`manifest.json` は、生成タイムスタンプ、ターゲット トリプル、貨物プロファイル、
そして完全なファイルインベントリ。パイプラインはマニフェストを比較して、いつであるかを検出できます。
新しいアーティファクトが表示されたり、リリース アセットと一緒に JSON をアップロードしたり、
バンドルをオペレーターにプロモートする前にハッシュを実行します。

ヘルパーはべき等です。コマンドを再実行するとマニフェストが更新され、
`target/mochi-bundle/` を単一として保持し、以前のアーカイブを上書きします。
現在のマシン上の最新バンドルの信頼できる情報源。