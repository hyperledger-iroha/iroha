---
lang: ja
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce0772b1c8b387704d6b07a53c552a8b1dedc913ead40275616c052d0ea473052
source_last_modified: "2025-12-26T11:12:17.796371+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/kagami_profiles.md -->

# Kagami Iroha3 プロファイル

Kagami は Iroha 3 ネットワーク向けのプリセットを提供し、運用者が
ネットワークごとのノブを調整せずに決定論的な genesis マニフェストを
生成できるようにする。

- プロファイル: `iroha3-dev`（チェーン `iroha3-dev.local`、collectors k=1 r=1、NPoS 選択時は
  chain id から VRF seed を導出）、`iroha3-taira`（チェーン `iroha3-taira`、collectors k=3 r=3、
  NPoS 選択時に `--vrf-seed-hex` が必須）、`iroha3-nexus`（チェーン `iroha3-nexus`、collectors k=5 r=3、
  NPoS 選択時に `--vrf-seed-hex` が必須）。
- コンセンサス: Sora プロファイルネットワーク（Nexus + dataspaces）は NPoS を必須とし、
  staged cutover は許可されない。permissioned の Iroha3 配置は Sora プロファイルなしで運用すること。
- 生成: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`。
  Nexus には `--consensus-mode npos` を使用。`--vrf-seed-hex` は NPoS のみ有効（taira/nexus で必須）。
  Kagami は Iroha3 ラインで DA/RBC を固定し、概要（chain, collectors, DA/RBC, VRF seed, fingerprint）を出力する。
- 検証: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` は
  プロファイル期待値（chain id, DA/RBC, collectors, PoP coverage, consensus fingerprint）を再生する。
  taira/nexus の NPoS マニフェスト検証時のみ `--vrf-seed-hex` を指定すること。
- サンプルバンドル: 事前生成バンドルは `defaults/kagami/iroha3-{dev,taira,nexus}/` に配置
  （genesis.json, config.toml, docker-compose.yml, verify.txt, README）。
  `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` で再生成する。
- Mochi: `mochi`/`mochi-genesis` は `--genesis-profile <profile>` と `--vrf-seed-hex <hex>`
  （NPoS のみ）を受け取り Kagami へ渡し、プロファイル使用時は同じ Kagami サマリを
  stdout/stderr に出力する。

バンドルはトポロジエントリに BLS PoP を埋め込むため `kagami verify` が即時に成功する。
ローカルの smoke 実行に合わせて config 内の trusted peers/ports を調整すること。
