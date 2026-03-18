---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-operator-onboarding
title: データスペース操作の統合 Sora Nexus
説明: `docs/source/sora_nexus_operator_onboarding.md` のミロワール、Nexus のオペレーターのリリースのチェックリストに適しています。
---

:::note ソースカノニク
Cette ページは `docs/source/sora_nexus_operator_onboarding.md` を反映します。 Gardez les deux は、ポータル上のローカル版の到着を確認するためにコピーします。
:::

# データスペース操作の統合 Sora Nexus

データスペース Sora Nexus は、リリースのアナウンスを行う前に、データ スペースのオペレーターの状況をガイドでキャプチャします。二重音声 (`docs/source/release_dual_track_runbook.md`) および選択の成果物に関するメモ (`docs/source/release_artifact_selection.md`) および決定コメントの整列、バンドル/イメージ、電荷、マニフェスト、およびテンプレートの設定に関する完全なファイル ランブックは、グローバルなレーンと前方の制御に役立ちます。

## 対象読者と前提条件
- プログラム Nexus を承認し、データ スペースの影響を確認します (レーンのインデックス、データ スペース ID/エイリアスとルーティングのポリシーの実行)。
- リリース エンジニアリング (tarball、イメージ、マニフェスト、署名、公開ファイル) に従ってリリース公開の署名を作成します。
- バリデータ/オブザーバーの役割を果たし、生産に必要な材料を取得します (識別子 Ed25519、コンセンサス BLS + PoP の検証者、および秘密保持の切り替えを行います)。
- Vous pouvez joindre les ペア Sora Nexus 存在者 qui bootstrappen votre noeud。

## Etape 1 - リリースのプロファイルの確認
1. 必要なチェーン ID のエイリアスを特定します。
2. Lancez `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) デポをチェックアウトします。ヘルパーは、`release/network_profiles.toml` を参照し、デプロイメントの主要なプロファイルを作成します。 Sora Nexus la reponse doit etre `iroha3` を注ぎます。最高の価値を注ぎ、アレテスとコンタクトをリリースエンジニアリングします。
3. リリース時のバージョン参照に関するタグの注記 (`iroha3-v3.2.0` など)。人工物やマニフェストを回復するために使用します。

## Etape 2 - 回復者と遺物の検証者
1. Telecharger バンドル `iroha3` (`<profile>-<version>-<os>.tar.zst`) およびその他のコンポーネント (`.sha256`、オプションネル `.sig/.pub`、`<profile>-<version>-manifest.json`、および `<profile>-<version>-image.json` を展開)デコンテニュール）。
2. Validez l'integrite avant decompresser:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Remplacez `openssl` は、KMS マテリアルを使用する組織の検証を承認します。
3. tarball などのマニフェストで `PROFILE.toml` を検査し、JSON を注ぐ確認者を検査します。
   - `profile = "iroha3"`
   - Les champs `version`、`commit` および `built_at` のリリース発表。
   - L'OS/アーキテクチャは、展開の投票に対応します。
4. 画像管理を利用し、`<profile>-<version>-<os>-image.tar` の検証ハッシュ/署名を繰り返し、`<profile>-<version>-image.json` の画像 ID 登録を確認します。

## Etape 3 - テンプレートの構成を作成する
1. バンドルと `config/` の配置と構成のコピー。
2. Traitez les ficiers sous `config/` comme des templates:
   - Remplacez `public_key`/`private_key` は Ed25519 の生産品です。 HSM の情報源を保護するための情報を提供します。ポインタとコネクタ HSM の設定を簡単に確認できます。
   - Ajustez `trusted_peers`、`network.address` および `torii.address` は、アクセス可能なインターフェイスおよびペアのブートストラップ割り当てを反映します。
   - 時間 `client.toml` のエンドポイント Torii の管理 (適用可能な構成 TLS を含む) および識別子のプロビジョニング オペレーションを実行します。
3. チェーン ID を 4 つ以上保持し、ガバナンスを明示的にバンドルするための指示を保持します - グローバルなチェーン ID を特定せずに、独自のチェーンを保持します。
4. Planifiez le demarrage du noeud avec le flag de profile Sora: `irohad --sora --config <path>`。 SoraFS またはマルチレーン サイル フラグが存在しません。## Etape 4 - データスペースとルーティングのメタデータの調整
1. `config/config.toml` セクション `[nexus]` に対応するデータスペース カタログ Nexus を編集します。
   - `lane_count` 時代の流れを汲む活動を行ってください。
   - `[[nexus.lane_catalog]]` と `[[nexus.dataspace_catalog]]` のメインディッシュは、`index`/`id` のユニークな、別名コンベナスのコンテンツです。必要なものはすべて、世界中に存在します。データスペースの追加属性を管理するためのエイリアスデリゲ。
   - `fault_tolerance (f)` を含むデータスペースの主要な内容を保証します。レ コミテ レーン リレー ソントは `3f+1` をディメンションします。
2. Mettez a jour `[[nexus.routing_policy.rules]]` は、キャプチャー ラ politique qui vous a ete 属性です。デフォルトのルートのテンプレートは、レーン `1` とレーン `2` に対する統治の指示および制御の展開を示します。トラフィックの運命を修正するために、データ スペースを変更し、レーンとエイリアスを修正します。 Coordonnez avec Release Engineering は、規則を変更する前に開発します。
3. Revoyez les seuils `[nexus.da]`、`[nexus.da.audit]`、`[nexus.da.recovery]`。管理者は管理者を承認し、管理者は評価を承認します。 ajustez-les uniquement si une politique mise a jour a ete retifiee.
4. トラッカー操作の最終設定を登録します。 Le runbook de release a double voie exige d'attacher le `config.toml` effectif (secrets rediges) au ticket d'onboarding。

## Etape 5 - プリフライトの検証
1. 事前に再調査を行って構成の検証を実行します。
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Cela は、構成の解決策とエコー、メイン カタログ/ルーティングの一貫性を欠き、生成と構成の相違を解決します。
2. コンテニュールを展開し、画像上の画像コマンドを実行し、平均料金 `docker load -i <profile>-<version>-<os>-image.tar` (`--sora` を含む) を実行します。
3. レーン/データスペースのプレースホルダの識別情報を監視するためのログを検証します。これにより、テープ 4 の開発が開始され、ID に依存する開発環境が作成され、プレースホルダーはテンプレートを参照できるようになります。
4. 煙のロケールの投票手順を実行します (例: soumettre une requete `FindNetworkStatus` avec `iroha_cli`、確認者はエンドポイントのテレメトリー公開者 `nexus_lane_state_total`、および検証者はストリーミング ソント トーナメントまたは輸入者セロン レスを確認します)緊急事態）。

## Etape 6 - カットオーバーとハンドオフ
1. Stockez le `manifest.json` は、署名とチケットのリリースを監査し、検証を再生産するための成果物を検証します。
2. Informez Nexus 操作は開始前に行われます。含まれます:
   - 識別情報 (ピア ID、ホスト名、エンドポイント Torii)。
   - カタログレーン/データスペースと政治の戦略における有効性。
   - ハッシュ デ バイネール/画像が検証されます。
3. ペアの入場フィナーレ (ゴシップの種と感情の変化) avec `@nexus-core`。 Ne rejoignez pas le reseau avant d'avoir recu l'approbation; Sora Nexus アップリケは、職業を決定するためにレーンと必要なマニフェストを提出し、入学手続きをミスします。
4. 必要に応じて、定期的にランブックを実行し、導入と注記のタグとリリースを上書きして、プロチェーンの反復パート セットのベースラインをオーバーライドします。

## 参照用チェックリスト
- [ ] リリース有効プロファイルは `iroha3` です。
- [ ] バンドル/イメージのハッシュと署名が検証されます。
- [ ] クレス、ペアとエンドポイントのアドレス Torii は、ジュール アン ヴァルール プロダクションに誤りがあります。
- [ ] カタログ レーン/データスペースとルート政治 Nexus の通信員、影響力の影響。
- [ ] 構成の検証 (`irohad --sora --config ... --trace-config`) は回避できません。
- [ ] オンボーディングのチケットと運用通知に関するマニフェスト/署名のアーカイブ。

状況に加えて、移行に関する大規模なフェーズ Nexus およびテレメトリの注意事項を確認し、[Nexus 移行ノート](./nexus-transition-notes) を参照してください。