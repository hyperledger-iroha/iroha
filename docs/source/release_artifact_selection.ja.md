---
lang: ja
direction: ltr
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

# Iroha リリース成果物の選定

このノートは、各リリースプロファイルでオペレーターがデプロイすべき成果物 (bundle とコンテナイメージ) を明確にします。

## プロファイル

- **iroha2 (Self-hosted networks)** — `defaults/genesis.json` と `defaults/client.toml` に一致する単一レーン構成。
- **iroha3 (SORA Nexus)** — `defaults/nexus/*` テンプレートを使う Nexus のマルチレーン構成。

## Bundles (バイナリ)

Bundles は `scripts/build_release_bundle.sh` を `--profile` に `iroha2` または `iroha3` を指定して生成します。

各 tarball には以下が含まれます:

- `bin/` — デプロイ用プロファイルでビルドした `irohad`、`iroha`、`kagami`。
- `config/` — プロファイル別の genesis/client 設定 (single vs. nexus)。Nexus bundle はレーンと DA パラメータを含む `config.toml` を同梱します。
- `PROFILE.toml` — プロファイル、設定、バージョン、コミット、OS/arch、有効な機能セットのメタデータ。
- tarball と同じ場所に出力されるメタデータ成果物:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` と `.pub` (`--signing-key` 指定時)
  - `<profile>-<version>-manifest.json` (tarball パス、ハッシュ、署名の詳細を記録)

## コンテナイメージ

コンテナイメージは `scripts/build_release_image.sh` を同じ profile/config 引数で実行して生成します。

出力:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- 署名/公開鍵 (任意, `*.sig`/`*.pub`)
- `<profile>-<version>-image.json` (タグ、イメージ ID、ハッシュ、署名メタデータを記録)

## 正しい成果物の選び方

1. デプロイ対象を判断します:
   - **SORA Nexus / multi-lane** -> `iroha3` の bundle と image を使用。
   - **Self-hosted single-lane** -> `iroha2` の成果物を使用。
   - 迷ったら `scripts/select_release_profile.py --network <alias>` または `--chain-id <id>` を実行します。helper は `release/network_profiles.toml` に従ってネットワークを適切なプロファイルへ対応付けます。
2. 目的の tarball と manifest ファイルをダウンロードします。展開前に SHA256 ハッシュと署名を検証します:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. bundle を展開 (`tar --use-compress-program=zstd -xf <tar>`) し、`bin/` をデプロイ PATH に配置します。必要に応じてローカル設定の overrides を適用します。
4. コンテナデプロイの場合は `docker load -i <profile>-<version>-<os>-image.tar` でイメージを読み込みます。読み込み前にハッシュ/署名を上記と同様に検証します。

## Nexus 設定チェックリスト

- `config/config.toml` に `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]`, `[nexus.da]` セクションが含まれていること。
- レーンのルーティング規則がガバナンスの期待 (`nexus.routing_policy`) と一致していることを確認。
- DA 閾値 (`nexus.da`) と fusion パラメータ (`nexus.fusion`) が評議会承認の設定と整合していることを確認。

## Single-lane 設定チェックリスト

- `config/config.d` (存在する場合) は single-lane の override のみで、`[nexus]` セクションを含めない。
- `config/client.toml` が意図した Torii エンドポイントと peer リストを参照していることを確認。
- Genesis は self-hosted ネットワーク向けの canonical domains/assets を保持すること。

## ツールのクイックリファレンス

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — 成果物選定後の Sora Nexus data-space オペレーター向け end-to-end onboarding フロー。
