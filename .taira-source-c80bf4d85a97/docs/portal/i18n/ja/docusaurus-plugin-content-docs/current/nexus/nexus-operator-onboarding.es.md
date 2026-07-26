---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-operator-onboarding
title: Sora Nexus のデータスペースのオペラドールの組み込み
説明: `docs/source/sora_nexus_operator_onboarding.md` の説明、Nexus のエンドツーエンドのパラ オペラドールのリリースに関するチェックリスト。
---

:::ノート フエンテ カノニカ
エスタ・ページナ・リフレジャ`docs/source/sora_nexus_operator_onboarding.md`。 Manten ambas copias alineadas hasta que las ediciones localizadas lleguen al portal。
:::

# Sora Nexus のデータスペースのオペラドールの組み込み

エンドツーエンドのデータ スペースの操作を確認するには、Nexus をリリースする必要があります。 (`docs/source/release_dual_track_runbook.md`) とアーティファクトの選択に関するノート (`docs/source/release_artifact_selection.md`) によるランブックの補完は、線形バンドル/イメージの説明、マニフェスト、プラントの設定、予測、レーン、グローバル、ノード、およびノードの選択に関するものです。リネア。

## 視聴者と前提条件
- プログラム Nexus とデータスペースの割り当てに関するポリシーがあります (レーンのインデックス、データスペース ID/エイリアス、ルーティングの政治要件)。
- リリース エンジニアリングおよびリリース エンジニアリング (tarball、imagenes、マニフェスト、firmas、llaves publicas) のリリース公開にアクセスします。
- バリデータ/オブザーバーの製造に関する一般的な資料 (ID25519 のノード、BLS と PoP のコンセンサス検証ツール、機密機能の切り替えに関する情報) があります。
- ソラ Nexus のブートストラップが存在し、ピアが存在するプエデス アルカンザール。

## 手順 1 - リリースの確認
1. レッドチェーン ID のエイリアスを識別します。
2. エステ リポジトリのチェックアウトを解除する `scripts/select_release_profile.py --network <alias>` (o `--chain-id <id>`) を取り出します。ヘルパーは、`release/network_profiles.toml` を参照して、デスプレガーを実行します。パラ ソラ Nexus ラ レスプエスタ デベ サー `iroha3`。非常に重要な場合は、リリース エンジニアリングにご連絡ください。
3. バージョンのタグの参照とリリースの注記 (por ejemplo `iroha3-v3.2.0`)。成果物をダウンロードしてマニフェストを表示します。

## パソ 2 - 回復と有効なアーティファクト
1. バンドル `iroha3` (`<profile>-<version>-<os>.tar.zst`) およびアーカイブ コンパネロス (`.sha256`、オプションの `.sig/.pub`、`<profile>-<version>-manifest.json`、および `<profile>-<version>-image.json` si) をダウンロードします。デスプリエガス・コンテネドール）。
2. 債務不履行の完全性の検証:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Reemplaza `openssl` は、米国の KMS ハードウェア管理システムの検証を行っています。
3. Inspecciona `PROFILE.toml` dentro del tarball y los マニフェストの JSON パラ確認:
   - `profile = "iroha3"`
   - ロス・カンポス `version`、`commit` および `built_at` はリリースと同時に発表されました。
   - OS/アーキテクチャは、オブジェクトの一致と一致します。
4. 画像を保存し、`<profile>-<version>-<os>-image.tar` のハッシュ/ファームの検証を繰り返し、画像 ID 登録 `<profile>-<version>-image.json` を確認します。

## 手順 3 - 植物の構成を準備する
1. 余分なバンドルとコピア `config/` を使用して構成を実行します。
2. Trata los archives bajo `config/` como plantillas:
   - Reemplaza `public_key`/`private_key` は Ed25519 を製造しています。エリミナは、HSM でディスコでプライベートな時間を過ごすことができます。 HSM コネクターの実際の構成。
   - Ajusta `trusted_peers`、`network.address` および `torii.address` は、インターフェイスにアクセス可能であり、ブートストラップ アサインドのピアを失います。
   - エンドポイント Torii の実際の操作 (TLS アプリケーションの構成を含む) は、ツールの操作としてのプロビジョニングに対する資格情報を取得します。
3. マンティエン エル チェーン ID は、ガバナンスに関する明確な明示的な管理を目的としてバンドルされます: エル レーン グローバル エスペラ ユニ アイデンティティ デ カデナ カノニコ ユニコ。
4. Planea iniciar el nodo con el flag de perfil Sora: `irohad --sora --config <path>`。 SoraFS のローダーの設定が再調整され、マルチレーンのフラグが表示されます。## 手順 4 - データ空間のルーティングにおける線形メタデータ
1. `config/config.toml` セクション `[nexus]` とデータスペースの比例カタログの編集 Nexus 評議会:
   - `lane_count` は、実際のエポカでの合計のアクセス可能性を示します。
   - `[[nexus.lane_catalog]]` と `[[nexus.dataspace_catalog]]` デベ コンテナー、`index`/`id` ユニコ、エイリアス アコルダスを失います。存在するグローバルな要素を削除することはできません。 agrega tus alias delegados si el consejo assigno data-spaces adicionales。
   - `fault_tolerance (f)` を含むデータスペースの確保。ロス コミテ レーン リレー セキュリティ ディメンション en `3f+1`。
2. 実際に政治を担当する `[[nexus.routing_policy.rules]]` をキャプチャします。 La plantilla pordefeto enruta instrucciones de gobernanza al LANE `1` y despliegues de contratos al LANE `2`;データスペースを変更し、レーンやエイリアスを修正するために、トラフィックの宛先を変更する必要があります。定期的にリリース エンジニアリングを調整します。
3. `[nexus.da]`、`[nexus.da.audit]`、および `[nexus.da.recovery]` の改訂。オペラドールのマンテンガンを見て、コンセホのアプロバドスを見てください。アジャスタロスは政治の現実を確認します。
4. 操作の最終的な構成を登録します。必要な付属品 `config.toml` のリリースとオンボーディングのチケットの有効なリリースのランブック。

## パソ 5 - 事前検証
1. 統合された設定を有効にするには、次の手順を実行します。
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   カタログ/ルーティングの初期設定の結果は、偶然には一致しません。
2. `docker load -i <profile>-<version>-<os>-image.tar` (`--sora` を含む) を参照してください。
3. Revisa は、レーン/データスペースのプレースホルダーを識別するための広告を記録します。 4 番目の報告書: 生産上の制限はなく、ID のプレースホルダーは植物のようなウィーンに依存しません。
4. ローカル煙の処理 (p. ej.、`FindNetworkStatus` と `iroha_cli` を参照、`nexus_lane_state_total` およびストリーミング サービスまたは輸入セグンの検証に関するエンドポイントの確認)対応）。

## パソ 6 - カットオーバーと引き継ぎ
1. Guarda el `manifest.json` verificado y los artefactos de arma en el ticket de release para que los Auditores puedan reproducir tus verificaciones.
2. Nexus の操作の開始リストと概要を通知します。含まれます:
   - ノードの識別 (ピア ID、ホスト名、エンドポイント Torii)。
   - レーン/データスペースとルーティングの政治的カタログの有効性。
   - ハッシュ・デ・ロス・ビナリオス/イマージェンス・ク・ベリフィカステ。
3. `@nexus-core` に関する最終的な承認 (ゴシップの種と割り当て) を調整します。赤いハスタ・レシビルの不正行為をする必要はありません。 Sora Nexus アプリケーションは、実際のマニフェストを承認する必要があります。
4. 生体内での運用手順の実際の実行手順は、ベースラインの導入とそのタグのリリースに関するオーバーライドを決定します。

## 参照用チェックリスト
- [ ] `iroha3` のリリースに関する有効な情報。
- [ ] ハッシュとバンドル/画像検証の企業。
- [ ] ラベス、エンドポイントのピア Torii の実際の生産価値。
- [ ] レーン/データスペースのカタログと、Nexus のルーティングの政治情報が、割り当てられた情報と一致します。
- [ ] 設定の有効性 (`irohad --sora --config ... --trace-config`) は広告を表示しません。
- [ ] オンボーディングおよび運用通知のチケットのマニフェスト/アーカイブ。

Nexus の移行に関する追加の状況に関するパラメタ、テレメトリの期待、改訂 [Nexus 移行ノート](./nexus-transition-notes)。