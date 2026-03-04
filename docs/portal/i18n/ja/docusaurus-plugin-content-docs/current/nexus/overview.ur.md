---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスの概要
title: Sora Nexus کا جائزہ
説明: Iroha 3 (Sora Nexus) کی معماری کا اعلی سطحی خلاصہ اور مونو ریپو کی کینونیکل دستاویزات کے حوالے۔
---

Nexus (Iroha 3) Iroha 2 マルチレーンの機能SDK の開発と開発 ٹولنگ کے ساتھ の開発یہ صفحہ مونو ریپو میں نئے خلاصے `docs/source/nexus_overview.md` کی عکاسی کرتا ہے تاکہ پورٹل کے قارئین جلد سمجھ سکیں کہ معماری کے اجزا کیسے جڑتے ہیں۔

## いいえ

- **Iroha 2** - کنسورشیم یا نجی نیٹ ورکس کے لئے خود میزبان ڈپلائمنٹس۔
- **Iroha 3 / Sora Nexus** - پبلک マルチレーン نیٹ ورک جہاں آپریٹرز ڈیٹا اسپیسز (DS) رجسٹر ٹرتے ہیں اور مشترکہ گورننس، سیٹلمنٹ، اور آبزرویبیلٹی ٹولنگ وراثت میں لیتے ہیں۔
- ワークスペース (IVM + Kotodama ツールチェーン) の開発と SDK の開発ABI اپڈیٹس اور Norito فکسچرز قابلِ نقل رہتے ہیں۔ آپریٹرز Nexus میں شامل ہونے کے لئے `iroha3-<version>-<os>.tar.zst` بَنڈل ڈاؤن لوڈ کرتے ہیں؛ فل اسکرین چیک لسٹ کے لئے `docs/source/sora_nexus_operator_onboarding.md` دیکھیں۔

## いいえ

|ああ |ああ | और देखें
|----------|-----------|--------------|
| ویٹا اسپیس (DS) |レーン数 رکھتا ہے، ویلیڈیٹر سیٹس، پرائیویسی کلاس، اور فیس + DA پالیسی کا اعلان کرتا ہے۔ | مینفسٹ اسکیمہ کے لئے [Nexus 仕様](./nexus-spec) دیکھیں۔ |
|レーン | جرا کا ڈیٹرمنسٹک شَارڈ؛コミットメント جاری کرتا ہے جنہیں عالمی NPoS رنگ ترتیب دیتا ہے۔レーン `default_public`、`public_custom`、`private_permissioned`、`hybrid_confidential` レーン| [レーンモデル](./nexus-lane-model) جیومیٹری، اسٹوریج پری فکسز، اور ریٹینشن کو بیان کرتا ہے۔ |
| ٹرانزیشن پلان |プレースホルダ識別子、ルーティング フェーズ、デュアル プロファイル パッケージング、シングル レーン、Nexus میںありがとうございます| [移行メモ](./nexus-transition-notes) ہر مائیگریشن مرحلہ دستاویز کرتی ہیں۔ |
|宇宙ディレクトリ | رجسٹری کنٹریکٹ جو DS مینفسٹس + ورژنز اسٹور کرتا ہے۔ آپریٹرز شامل ہونے سے پہلے کیٹلاگ انٹریز کو اس ڈائریکٹری کے ساتھ ملاتے ہیں۔ | مینفسٹ ڈف ٹریکر `docs/source/project_tracker/nexus_config_deltas/` کے تحت موجود ہے۔ |
|レーンカタログ | `[nexus]` レーン ID とエイリアス、ルーティング、DA しきい値、および DA しきい値`irohad --sora --config … --trace-config` 解決されたカタログ پرنٹ کرتا ہے۔ | CLI ウォークスルー `docs/source/sora_nexus_operator_onboarding.md` の説明|
|決済ルーター | XOR は、CBDC レーンとレーンを示します。 | `docs/source/cbdc_lane_playbook.md` ノブ اور ٹیلیمیٹری ゲート کی وضاحت کرتا ہے۔ |
|テレメトリ/SLO | `dashboards/grafana/nexus_*.json` 数値 + レーン高さ DA バックログ決済レイテンシ ガバナンス キューの深さ| [テレメトリ修復計画](./nexus-telemetry-remediation) ویش بورڈز، الرٹس اور آڈٹ ثبوت کی وضاحت کرتا ہے۔ |

## آؤٹ اسنیپ شاٹ

| |うーんऔर देखें
|------|------|------|
| N0 - 認証済み | 認証済みکونسل-مینجڈ registrar (`.sora`) ، مینول آپریٹر آن بورڈنگ، جامد レーン カタログ۔ | دستخط شدہ DS مینفسٹس + گورننس ہینڈ آفز کی مشق۔ |
| N1 - ニュース | `.nexus` セルフサービス レジストラ XOR 決済配線 شامل کرتا ہے۔ |リゾルバ/ゲートウェイの同期 調整 調停 紛争 卓上訓練|
| N2 - ニュース | `.dao`、リセラー API、分析、紛争ポータル、スチュワード スコアカード|コンプライアンス関連のアーティファクト 政策陪審ツールキット 財務省透明性レポート|
| NX-12/13/14ゲート |コンプライアンス エンジン、テレメトリ ダッシュボード、ドキュメント、およびドキュメントの管理| [Nexus 概要](./nexus-overview) + [Nexus 操作](./nexus-operations)、ダッシュボード、有線、ポリシー エンジンの統合。 |

## آپریٹر کی ذمہ داریاں

1. ** حفظانِ صحت** - `config/config.toml` کو شائع شدہ LANE データスペース کیٹلاگ کے ساتھ ہم آہنگうわーہر ریلیز ٹکٹ کے ساتھ `--trace-config` آؤٹ پٹ محفوظ کریں۔
2. **مینفسٹ ٹریکنگ** - شامل ہونے یا نوڈ اپ گریڈ سے پہلے کیٹلاگ انٹریز کو تازہ ترین Space Directory بَنڈل سے ہم آہنگ کریں۔
3. **ٹیلیمیٹری کوریج** - `nexus_lanes.json`، `nexus_settlement.json` اور متعلقہ SDK ڈیش بورڈز کو 公開 کریں؛ PagerDuty の修復 修復 修復 修復 修復 修復 PagerDuty の修復
4. **حادثہ رپورٹنگ** - [Nexus 操作](./nexus-operations) میں سیوریٹی میٹرکس کی پیروی کریں اور RCA ラジオ局
5. ** レーン レーン پر اثر انداز ہونے والی Nexus کونسل ووٹس میں حصہ لیں اور سہ ماہی ロールバック ہدایات کی مشق کریں (ٹریکنگ `docs/source/project_tracker/nexus_config_deltas/` کے ذریعے ہوتی ہے)۔

## زید دیکھیں- 番号: `docs/source/nexus_overview.md`
- バージョン: [./nexus-spec](./nexus-spec)
- レーン جیومیٹری: [./nexus-lane-model](./nexus-lane-model)
- ٹرانزیشن پلان: [./nexus-transition-notes](./nexus-transition-notes)
- 修復 پلان: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- ランブック: [./nexus-operations](./nexus-operations)
- オンボーディング中のユーザー: `docs/source/sora_nexus_operator_onboarding.md`