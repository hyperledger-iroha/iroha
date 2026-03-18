---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c21b5a607b18e4cf97f1be2247fc5fc92e5f23a846999720e50a1b06af5b4a9f
source_last_modified: "2025-12-21T01:14:24.621301+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-operator-onboarding
title: Sora Nexus data-space オペレーターのオンボーディング
description: `docs/source/sora_nexus_operator_onboarding.md` のミラーで、Nexusオペレーター向けのエンドツーエンド release checklist を追跡します。
---

:::note 正本
このページは `docs/source/sora_nexus_operator_onboarding.md` を反映しています。ローカライズ版がポータルに到達するまで両方のコピーを揃えてください。
:::

# Sora Nexus Data-Space Operator Onboarding

このガイドは、release が発表された後に Sora Nexus data-space オペレーターが従うべきエンドツーエンドの手順をまとめています。二系統の runbook (`docs/source/release_dual_track_runbook.md`) とアーティファクト選定ノート (`docs/source/release_artifact_selection.md`) を補完し、ノードをオンラインにする前にダウンロード済みの bundle/image、manifest、設定テンプレートをグローバル lane の期待値と整合させる方法を説明します。

## 対象読者と前提条件
- Nexus Program に承認され、data-space の割り当て (lane index、data-space ID/alias、routing policy 要件) を受けている。
- Release Engineering が公開した署名付き release artifacts (tarball、image、manifest、signature、public key) にアクセスできる。
- validator/observer のロール向けに本番鍵素材を生成/受領している (Ed25519 ノードアイデンティティ、validator 向け BLS コンセンサスキー + PoP、機密機能のトグルなど)。
- ノードを bootstrap する既存の Sora Nexus peers に到達できる。

## ステップ 1 - release プロファイルの確認
1. 提供された network alias または chain ID を特定する。
2. このリポジトリの checkout で `scripts/select_release_profile.py --network <alias>` (または `--chain-id <id>`) を実行する。helper は `release/network_profiles.toml` を参照してデプロイすべきプロファイルを出力する。Sora Nexus の場合は `iroha3` でなければならない。それ以外の値なら停止して Release Engineering に連絡する。
3. release アナウンスで参照された version tag (例: `iroha3-v3.2.0`) を控える。これを artifacts と manifests の取得に使う。

## ステップ 2 - artifacts の取得と検証
1. `iroha3` bundle (`<profile>-<version>-<os>.tar.zst`) と付随ファイル (`.sha256`、任意の `.sig/.pub`、`<profile>-<version>-manifest.json`、コンテナ運用の場合は `<profile>-<version>-image.json`) をダウンロードする。
2. 展開前に整合性を検証する:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   ハードウェア KMS を使う場合は、`openssl` を組織承認の検証ツールに置き換える。
3. tarball 内の `PROFILE.toml` と JSON manifests を確認し、次を満たすことを確認する:
   - `profile = "iroha3"`
   - `version`、`commit`、`built_at` が release アナウンスと一致する。
   - OS/アーキテクチャが配備先と一致する。
4. コンテナ image を使う場合は、`<profile>-<version>-<os>-image.tar` の hash/signature を検証し、`<profile>-<version>-image.json` に記録された image ID を確認する。

## ステップ 3 - テンプレートからの設定準備
1. bundle を展開し、`config/` をノードが設定を読む場所にコピーする。
2. `config/` 配下のファイルはテンプレートとして扱う:
   - `public_key`/`private_key` を本番 Ed25519 キーに置き換える。ノードが HSM から鍵を取得する場合は、ディスク上の秘密鍵を削除し、設定を HSM コネクタに向ける。
   - `trusted_peers`、`network.address`、`torii.address` を、到達可能なインタフェースと割り当てられた bootstrap peers に合わせて調整する。
   - `client.toml` を、運用向け Torii endpoint (必要なら TLS 設定を含む) と運用ツールの認証情報に合わせて更新する。
3. Governance が明示的に指示しない限り、bundle に含まれる chain ID を維持する - グローバル lane は単一のカノニカル chain identifier を期待する。
4. ノード起動時に Sora プロファイルフラグを付ける: `irohad --sora --config <path>`。フラグが無い場合、設定ローダは SoraFS や multi-lane の設定を拒否する。

## ステップ 4 - data-space メタデータと routing の整合
1. `config/config.toml` を編集し、`[nexus]` セクションが Nexus Council 提供の data-space カタログに一致するようにする:
   - `lane_count` は現在のエポックで有効な lanes の総数と一致する必要がある。
   - `[[nexus.lane_catalog]]` と `[[nexus.dataspace_catalog]]` の各エントリは、固有の `index`/`id` と合意済み aliases を含む必要がある。既存のグローバルエントリは削除しない。評議会が追加の data-space を割り当てた場合は、委任された aliases を追加する。
   - 各 dataspace エントリに `fault_tolerance (f)` が含まれていることを確認する。lane-relay 委員会のサイズは `3f+1`。
2. `[[nexus.routing_policy.rules]]` を更新して与えられたポリシーを反映する。デフォルトテンプレートは governance の指示を lane `1` に、コントラクトデプロイを lane `2` にルーティングする。data-space 宛てのトラフィックが適切な lane と alias に送られるようにルールを追加/変更する。ルール順の変更は Release Engineering と調整する。
3. `[nexus.da]`、`[nexus.da.audit]`、`[nexus.da.recovery]` の閾値を確認する。オペレーターは評議会承認の値を維持することが期待される。更新されたポリシーが批准された場合のみ変更する。
4. 最終設定を運用トラッカーに記録する。二系統 release runbook は、実効 `config.toml` (秘密情報はマスク) をオンボーディングチケットに添付することを求める。

## ステップ 5 - プリフライト検証
1. ネットワーク参加前に内蔵の設定バリデータを実行する:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   これは解決済み設定を出力し、カタログ/ルーティングの不整合や genesis と config の不一致があれば早期に失敗する。
2. コンテナを使う場合は、`docker load -i <profile>-<version>-<os>-image.tar` で読み込んだ後に同じコマンドをイメージ内で実行する ( `--sora` を含める)。
3. placeholder の lane/data-space 識別子に関する警告ログがないか確認する。あればステップ 4 に戻る - 本番デプロイはテンプレートの placeholder ID に依存してはならない。
4. ローカルの smoke 手順を実行する (例: `iroha_cli` で `FindNetworkStatus` クエリを送信し、テレメトリ endpoint が `nexus_lane_state_total` を公開していることを確認し、ストリーミング鍵がローテーション/インポートされていることを検証する)。

## ステップ 6 - カットオーバーと引き継ぎ
1. 検証済み `manifest.json` と署名 artifacts を release チケットに保存し、監査が検証を再現できるようにする。
2. ノードの導入準備ができたことを Nexus Operations に通知し、以下を含める:
   - ノードのアイデンティティ (peer ID、hostnames、Torii endpoint)。
   - 有効な lane/data-space カタログと routing ポリシーの値。
   - 検証済みバイナリ/イメージの hashes。
3. `@nexus-core` と最終 peer admission (gossip seeds と lane 割り当て) を調整する。承認を受けるまでネットワークに参加しないこと。Sora Nexus は lane 占有を決定的に強制し、更新済み admissions manifest を要求する。
4. ノード稼働後、導入した overrides を runbook に反映し、次の反復がこの baseline から始められるよう release tag を記録する。

## 参照チェックリスト
- [ ] release プロファイルが `iroha3` で検証済み。
- [ ] bundle/image の hashes と signatures を検証済み。
- [ ] 鍵、peer アドレス、Torii endpoints を本番値に更新済み。
- [ ] Nexus の lane/dataspace カタログと routing ポリシーが評議会の割り当てと一致。
- [ ] 設定バリデータ (`irohad --sora --config ... --trace-config`) が警告なしで通過。
- [ ] manifests/signatures をオンボーディングチケットに保存し Ops を通知済み。

Nexus 移行フェーズとテレメトリ期待値の詳細は [Nexus transition notes](./nexus-transition-notes) を参照してください。
