---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-operator-onboarding
title: Sora Nexus data-space آپریٹر آن بورڈنگ
説明: `docs/source/sora_nexus_operator_onboarding.md` کا آئینہ، جو Nexus آپریٹرز کے لئے エンドツーエンドの ریلیز چیک لسٹ کو ٹریک کرتا ❁❁❁❁
---

:::note ٩ینونیکل ماخذ
یہ صفحہ `docs/source/sora_nexus_operator_onboarding.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus データスペース オペレーターのオンボーディング

エンド ツー エンド فلو کو محفوظ کرتی ہے جس پر Sora Nexus データスペース آپریٹرز کو ریلیز کے اعلان ٩ے بعد عمل کرنا ہوتا ہے۔デュアルトラック Runbook (`docs/source/release_dual_track_runbook.md`) アーティファクト選択ノート (`docs/source/release_artifact_selection.md`)バンドル/イメージマニフェスト設定テンプレートレーンの期待値

## سامعین اور پیشگی شرائط
- Nexus プログラムのデータ空間割り当て (レーン インデックス、データ空間 ID/エイリアス、ルーティング ポリシー要件)。
- リリース エンジニアリング、署名付きリリース アーティファクト、および (tarball、イメージ、マニフェスト、署名、公開キー)。
- 検証者/オブザーバー キーマテリアルの検証 (Ed25519 ノード ID バリデーター BLS コンセンサスキー + PoP)機密機能が切り替わります）。
- آپ انموجودہ Sora Nexus ピア تک رسائی کر سکتے ہیں جو آپ کے نوڈ کا ブートストラップ کریں گے۔

## مرحلہ 1 - ریلیز پروفائل کی تصدیق
1. ネットワーク エイリアス チェーン ID شناخت کریں جو آپ کو دیا گیا ہے۔
2. チェックアウト `scripts/select_release_profile.py --network <alias>` (`--chain-id <id>`) チェックアウトヘルパー `release/network_profiles.toml` دیکھ کر デプロイ ہونے والا پروفائل پرنٹ کرتا ہے۔そら Nexus کے لئے جواب `iroha3` ہونا چاہئے۔リリース エンジニアリング リリース エンジニアリング リリース エンジニアリング
3. ریلیز اعلان میں دیا گیا バージョンタグ نوٹ کریں (مثلاً `iroha3-v3.2.0`);成果物 マニフェスト 成果物 成果物 成果物 マニフェスト 成果物

## مرحلہ 2 - アーティファクト حاصل کریں اور ویریفائی کریں
1. `iroha3` バンドル (`<profile>-<version>-<os>.tar.zst`) コンパニオン ファイル (`.sha256`、`.sig/.pub`、 `<profile>-<version>-manifest.json`、`<profile>-<version>-image.json` コンテナー ڈپلائے کر رہے ہیں)۔
2. 整合性の確保:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   ハードウェア バックアップ KMS を使用する `openssl` を使用する 検証者を使用する
3. tar ボール `PROFILE.toml` JSON マニフェストの内容:
   - `profile = "iroha3"`
   - `version`, `commit`, اور `built_at` فیلڈز ریلیز اعلان سے ملتے ہیں۔
   - OS/アーキテクチャの展開ターゲットとの一致
4. コンテナイメージの保存 保存 `<profile>-<version>-<os>-image.tar` 保存 ハッシュ/署名 検証 `<profile>-<version>-image.json` 保存イメージID

## مرحلہ 3 - テンプレート構成の設定
1. バンドル抽出 کریں اور `config/` کو اس جگہ کاپی کریں جہاں نوڈ اپنی 構成 پڑھے گا۔
2. `config/` テンプレートのテンプレート:
   - `public_key`/`private_key` پروڈکشن Ed25519 キー سے بدلیں۔ HSM キー、秘密キー、ディスク キー、HSM キー、秘密キー、ディスク キーHSM コネクタの構成
   - `trusted_peers`、`network.address` `torii.address` インターフェイス ブートストラップ ピアऔर देखें
   - `client.toml` オペレーター側 Torii エンドポイント (TLS 構成) プロビジョニング 資格情報और देखें
3. バンドル チェーン ID チェーン ID チェーン ID ガバナンス チェーン グローバル レーン 正規チェーン識別子 چاہتا ہے۔
4. 空 پروفائل فلیگ کے ساتھ اسٹارٹ کرنے کا ارادہ رکھیں: `irohad --sora --config <path>`。構成ローダー SoraFS マルチレーンの拒否拒否## 4 - データスペースのメタデータとルーティング
1. `config/config.toml` 番号 `[nexus]` 番号 Nexus 評議会番号 番号データ空間カタログ番号 一致番号:
   - `lane_count` موجودہ epoch میں فعال レーン کی مجموعی تعداد کے برابر ہونا چاہئے۔
   - `[[nexus.lane_catalog]]` 回答 `[[nexus.dataspace_catalog]]` 回答 回答 `index`/`id` 回答 別名 ہونے और देखेंグローバル エントリ فٹائیں؛議会評議会のデータスペース دیئے ہیں تو اپنے 委任されたエイリアス شامل کریں۔
   - ہر データスペース انٹری میں `fault_tolerance (f)` شامل ہونا یقینی بنائیں؛レーンリレー委員会 کا سائز `3f+1` ہوتا ہے۔
2. `[[nexus.routing_policy.rules]]` کو اپنی دی گئی پالیسی کے مطابق اپ ڈیٹ کریں۔デフォルトのテンプレート ガバナンス手順 レーン `1` 契約の展開 レーン `2` ルート ルートデータスペースのデータスペース レーン別名 پرجائے۔リリース エンジニアリング リリース エンジニアリング リリース エンジニアリング
3. `[nexus.da]`、`[nexus.da.audit]`、`[nexus.da.recovery]` しきい値آپریٹرز سے توقع ہے کہ وہ 議会承認 ویلیوز رکھیں؛ صرف اسی وقت بدلیں جب نئی پالیسی منظور ہو۔
4. 設定と操作トラッカーの管理デュアルトラック リリース ランブック オンボーディング チケット ساتھ موثر `config.toml` (秘密は編集済み) منسلک کرنے کا تقاضا کرتا ہے۔

## مرحلہ 5 - پری فلائٹ ویلیڈیشن
1. 組み込みの構成バリデータを使用する:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   解決済みの構成 カタログ/ルーティング エントリ 構成 ジェネシス 構成 失敗 構成
2. コンテナーの展開 (`docker load -i <profile>-<version>-<os>-image.tar` イメージ) イメージ (`--sora` イメージ) ٩رنا نہ بھولیں)۔
3. ログ プレースホルダー レーン/データ空間識別子 警告 警告アプリケーション 4 つ - 展開 テンプレート プレースホルダ ID 4 つ目 4 つ目 4 つ目 4 つ目
4. ローカルスモーク手順 (مثلاً `iroha_cli` سے `FindNetworkStatus` クエリ بھیجیں، تصدیق کریں کہ テレメトリ エンドポイント) `nexus_lane_state_total` ストリーミング キーを公開する 回転/インポートする تصدیق کریں)۔

## مرحلہ 6 - カットオーバーのハンドオフ
1. 検査 `manifest.json` 署名アーティファクトのリリース チケットの検査 監査員の検査 検査の検査
2. Nexus 操作要旨:
   - ノード ID (ピア ID、ホスト名、Torii エンドポイント)。
   - レーン/データスペースカタログとルーティングポリシー
   - 検証されたバイナリ/イメージのハッシュ
3. ピア入場 (ゴシップの種とレーンの割り当て) `@nexus-core` のアクセス許可منظوری ملنے سے پہلے نیٹ ورک join نہ کریں؛ Sora Nexus 決定的なレーン占有率 نافذ کرتا ہے اور 更新された入学マニフェスト چاہتا ہے۔
4. ライブ チャット ライブ チャット ランブック チャット リリース タグ チャット リリース タグ チャット 繰り返しをオーバーライドします。ベースライン شروع ہو۔

## और देखें
- [ ] リリース プロファイル `iroha3` ٩ے طور پر validate ہو چکا ہے۔
- [ ] バンドル/画像のハッシュ、署名の検証、 ہیں۔
- [ ] キー、ピア アドレス、Torii エンドポイント、接続数、接続数。
- [ ] Nexus レーン/データスペース カタログ、ルーティング ポリシー評議会の割り当て、一致、一致
- [ ] 構成バリデータ (`irohad --sora --config ... --trace-config`) 警告メッセージ
- [ ] マニフェスト/署名オンボーディング チケット میں آرکائیو اور Ops کو اطلاع دے دی گئی ہے۔

Nexus 移行フェーズとテレメトリの期待値 ٩ے وسیع تر سیاق کے لئے [Nexus 移行メモ](./nexus-transition-notes) دیکھیں۔