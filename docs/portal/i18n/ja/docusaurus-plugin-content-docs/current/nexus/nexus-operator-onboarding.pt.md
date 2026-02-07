---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-operator-onboarding
title: データスペース ソラ Nexus のオペラドールの統合
説明: `docs/source/sora_nexus_operator_onboarding.md` のエスペル、エンドツーエンドのパラ オペラドール Nexus のリリース チェックリストを作成します。
---

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/sora_nexus_operator_onboarding.md`。マンテンハは、デュアス コピアス アリンハダスとして、エディコエス ローカリザダス チェゲム アオ ポータルとして食べました。
:::

# データ空間のオペラドールの統合 Sora Nexus

データスペース Sora Nexus を開発し、確実にリリースできるようにエンドツーエンドの操作を実行します。 (`docs/source/release_dual_track_runbook.md`) と、アーティファトの選択に関するノート (`docs/source/release_artifact_selection.md`) を介して、ランブックを補完し、アリンハル バンドル/画像バイシャドの詳細を説明し、グローバルな生産性を期待できる設定テンプレートのマニフェストを作成します。

## 視聴と前提条件
- プログラム Nexus を使用して、データ空間の属性を受け取ります (レーンのインデックス、データ空間 ID/エイリアス、ルーティングの政治要件)。
- リリース エンジニアリング (tarball、imagen、マニフェスト、assinaturas、chaves publicas) をリリースする際には、アセッサー オスの芸術作品を積極的にリリースしてください。
- バリデータ/オブザーバーの文書を作成するための資料を取得します (Ed25519 の識別子、BLS + PoP のコンセンサス検証ツール、再帰的な秘密の切り替えを許可します)。
- Voce consegue alcancar os ピア Sora Nexus 存在 que farao bootstrap do seu no.

## Etapa 1 - リリースの実行確認
1. チェーン ID のエイリアスを識別します。
2. `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) em um checkout deste repositorio を実行します。 O ヘルパーは、`release/network_profiles.toml` をインプリメントまたはデプロイ時に実行します。パラ Sora Nexus、レスポスタ開発者 `iroha3`。決勝戦の勇敢さをパラパラと、リリース エンジニアリングと並行して進めます。
3. リリースの通知はありません (`iroha3-v3.2.0` など)。声を上げてバスカーの芸術品を明らかにします。

## Etapa 2 - 回復と有効な芸術品
1. バンドル `iroha3` (`<profile>-<version>-<os>.tar.zst`) を使用し、arquivos companheiros (`.sha256`、オプションの `.sig/.pub`、`<profile>-<version>-manifest.json`、`<profile>-<version>-image.json` の音声ファイザーを展開) を使用するcom contenores）。
2. 圧縮解除前の統合を検証します。
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   `openssl` は、KMS com ハードウェアの承認を取得し、米国の組織をサポートします。
3. `PROFILE.toml` dentro do tarball e os マニフェストの JSON パラ確認を検査します。
   - `profile = "iroha3"`
   - OS は `version`、`commit`、`built_at` に対応し、リリースを発表します。
   - OS/アーキテチュラ対応のデプロイメント。
4. 画像を保存して、`<profile>-<version>-<os>-image.tar` のハッシュ/分析を確認し、画像 ID 登録 `<profile>-<version>-image.json` を確認します。

## Etapa 3 - 一部の dos テンプレートの設定を準備する
1. バンドルとコピー `config/` をローカル オンデマンドに追加し、構成を変更できません。
2. Trate os arquivos sob `config/` como テンプレート:
   - `public_key`/`private_key` は、Ed25519 を製造するための製品です。 HSM の一部を削除することはできません。コネクター HSM のアポンタの構成を実現します。
   - `trusted_peers`、`network.address`、`torii.address` のインターフェイスを参照して、ブートストラップ クエリの形式で OS ピアを調整します。
   - `client.toml` com o endpoint Torii パラ オペラ (TLS 設定などの機能を含む) を、ツール操作用の認証情報としてプロビジョニングします。
3. Mantenha はチェーン ID をバンドルせず、ガバナンスに関する明確な指示を提供します - レーン グローバル エスペラウム ユニコ識別子カノニコ デ カディア。
4. Planeje iniciar o no com o flag de perfil Sora: `irohad --sora --config <path>`.ローダーの設定を変更して SoraFS をマルチレーン設定し、フラグを設定してください。## Etapa 4 - データ空間ルーティングの Alinhar メタデータ
1. `config/config.toml` を安全な `[nexus]` に対応するデータスペースのカタログとして編集します。 Nexus 評議会:
   - `lane_count` は、実際の状況を把握できるよう、全体的な目標を設定します。
   - `[[nexus.lane_catalog]]` と `[[nexus.dataspace_catalog]]` は、`index`/`id` unico e os エイリアス acordados を開発します。ナオは、存在する世界全体としての困難を抱えています。 adicione seus エイリアス delegados se o conselho atribuiu データスペース adicionais。
   - `fault_tolerance (f)` を含むデータスペースの確認;レーンリレー SAO ディメンションアドス em `3f+1` をコミテします。
2. `[[nexus.routing_policy.rules]]` を、政治的な問題を把握できるようにします。 O テンプレート パドラオ ローテイア レーン `1` での統治命令は、レーン `2` での制御を展開します。旅行先のデータスペースを変更したり、レーンや別名を修正したりできます。 Coordene com のリリース エンジニアリングは、計画を変更する前に行われます。
3. `[nexus.da]`、`[nexus.da.audit]`、および `[nexus.da.recovery]` の OS 制限を改訂します。エスペラーゼ ケ オス オペラドール マンテナム オス ヴァロール アプロバドス ペロ コンセリョ。政治は正しいと信じてください。
4. オペラ座トラッカーの最終構成を登録します。 exige anexar 経由のリリースのランブック、`config.toml` efetivo (com segredos redigidos)、オンボーディングのチケット。

## Etapa 5 - バリダカオ飛行前
1. 設定を有効にする手順を実行します。
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   カタログ/ルーティング フォームの一貫性が失われ、生成と構成の相違が発生する可能性があるため、問題解決のための構成を実行する必要があります。
2. com contendores を展開し、mesmo commando dentro da imagem apos carregala com `docker load -i <profile>-<version>-<os>-image.tar` (`--sora` を含む) を実行します。
3. Verifique は、レーン/データスペースのプレースホルダーを明確に識別してログを記録します。さあ、Etapa 4 を元に戻します - 開発環境に依存する DOS ID プレースホルダを会社の OS テンプレートに展開します。
4. ローカル煙の手順を実行します (例、enviar uma query `FindNetworkStatus` com `iroha_cli`、Telemetria expoem `nexus_lane_state_total`、e verificar que as Chaves de Streaming foram rotacionadas ou importadas conme)必要です）。

## Etapa 6 - カットオーバーとハンドオフ
1. `manifest.json` は、認証された時点での芸術作品を検証するためのリリース パラケーター チケットはありません。
2. Nexus の操作は開始前に実行されないことを通知します。含む:
   - ID は実行しません (ピア ID、ホスト名、エンドポイント Torii)。
   - レーン/データスペースと政治のルーティングのカタログを作成します。
   - ビナリオ/画像検証のハッシュ。
3. `@nexus-core` で最終的なピアの承認 (ゴシップの種とレーンの情報) を調整します。ナオ・エントレ・ナ・レデは、レシーバー・アプロヴァカオを食べました。 Sora Nexus アプリケーションは、レーンの決定性を確認し、マニフェストを許可します。
4. 目標をデポジットし、Runbook の設定を変更して、最初の説明と注釈のタグを上書きし、リリース パラメータを近似的に変更し、ベースラインの一部を作成します。

## 参照用チェックリスト
- [ ] `iroha3` のリリースに関する有効な情報。
- [ ] ハッシュと結合はバンドル/イメージの検証を行います。
- [ ] Chaves、ピアとエンドポイント Torii の生産性を向上させます。
- [ ] レーン/データスペースのカタログとルーティングの政治情報は、Nexus に対応しており、コンセルホの属性に対応しています。
- [ ] 設定の有効性 (`irohad --sora --config ... --trace-config`) は必須です。
- [ ] マニフェスト/アーカイブには、オンボーディングおよび運用通知のチケットはありません。

Nexus とテレメトリの期待に関する移行の状況を確認し、[Nexus 移行ノート](./nexus-transition-notes) を改訂してください。