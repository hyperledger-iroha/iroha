---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスオペレーション
title: Nexus آپریشنز رن بُک
説明: Nexus آپریٹر ورک فلو کا فیلڈ کے لئے تیار خلاصہ، جو `docs/source/nexus_operations.md` کی عکاسی کرتا ❁❁❁❁
---

اس صفحے کو `docs/source/nexus_operations.md` کے تیز ریفرنس کے طور پر استعمال کریں۔ یہ آپریشنل چیک لسٹ، تبدیلی کے انتظام کے フック ، اور ٹیلیمیٹری کوریج کی ضروریات کو سمیٹتا ہے جن پر Nexus آپریٹرز کو عمل کرنا ہوتا ہے۔

## سائیکل چیک لسٹ

| |ありがとう | और देखें
|----------|----------|----------|
| और देखें ریلیز ہیشز/سگنیچرز کی تصدیق کریں، `profile = "iroha3"` کنفرم کریں، اور کنفیگ ٹیمپلیٹسいいえ| `scripts/select_release_profile.py` チェックサム لاگ، دستخط شدہ مینفسٹ بَنڈل۔ |
| ٩یٹلاگ الائنمنٹ | `[nexus]` کیٹلاگ، روٹنگ پالیسی اور DA تھریش ہولڈز کو کونسل کے جاری کردہ مینفسٹ کے مطابق اپ ڈیٹ کریں، پھر `--trace-config` کیپچر کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ جو オンボーディング ٹکٹ کے ساتھ محفوظ ہے۔ |
| और देखें `irohad --sora --config ... --trace-config` چلائیں، CLI اسموک (`FindNetworkStatus`) چلائیں، ٹیلیمیٹری ایکسپورٹس کی توثیق ٩ریں، اور ایڈمیشن کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager کنفرمیشن۔ |
| और देखेंダッシュボード/アラート مانیٹر کریں، گورننس کی cadence کے مطابق کیز روٹیٹ کریں، اور جب مینفسٹ بدلے تو構成/ランブック| سہ ماہی ریویو منٹس، ڈیش بورڈ اسکرین شاٹس، روٹیشن ٹکٹ IDs۔ |

オンボーディング (کلیدوں کی تبدیلی، روٹنگ ٹیمپلیٹس، ریلیز پروفائل کے مراحل) `docs/source/sora_nexus_operator_onboarding.md` میں موجود ہے۔

## ありがとうございます

1. **ریلیز اپ ڈیٹس** - `status.md`/`roadmap.md` میں اعلانات ٹریک کریں؛ PR スタッフ募集 スタッフ募集 スタッフ募集
2. **レーン مینفسٹ تبدیلیاں** - Space Directory سے دستخط شدہ بَنڈلز کی تصدیق کریں اور انہیں `docs/source/project_tracker/nexus_config_deltas/` کے تحت محفوظ کریں۔
3. ** - `config/config.toml` میں ہر تبدیلی کے لئے レーン/データスペース کا حوالہ دینے والا ٹکٹ ضروری ہے۔ جب نوڈز شامل ہوں یا اپ گریڈ ہوں تو موثر کنفیگ کی ریڈیکٹڈ کاپی محفوظ کریں۔
4. **ロールバック訓練** - 停止/復元/発煙の訓練نتائج `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں لاگ کریں۔
5. **コンプライアンスの承認** - プライベート/CBDC レーン پالیسی یا ٹیلیمیٹری 編集ノブ بدلنے سے پہلے コンプライアンス منظوری درکار ہے (دیکھیں) `docs/source/cbdc_lane_playbook.md`)。

## ٹیلیمیٹری اور SLO

- ダッシュボード: `dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`、SDK مخصوص ویوز (مثلاً `android_operator_console.json`)。
- アラート: `dashboards/alerts/nexus_audit_rules.yml` および Torii/Norito トランスポート (`dashboards/alerts/torii_norito_rpc_rules.yml`)。
- دیکھنے کے لئے میٹرکس:
  - `nexus_lane_height{lane_id}` - スロットスロットのスロット数
  - `nexus_da_backlog_chunks{lane_id}` - レーン مخصوص تھریش ہولڈز سے اوپر الرٹ کریں (ڈیفالٹ 64 public / 8 private)。
  - `nexus_settlement_latency_seconds{lane_id}` - セキュリティ P99 900 ミリ秒 (パブリック) 1200 ミリ秒 (プライベート)
  - `torii_request_failures_total{scheme="norito_rpc"}` - エラー 5 エラー率 > 2% ہو تو الرٹ کریں۔
  - `telemetry_redaction_override_total` - セクション 2 یقینی بنائیں کہ は کے لئے コンプライアンス ٹکٹس ہوں۔ をオーバーライドします。
- [Nexus テレメトリ修復計画](./nexus-telemetry-remediation) 問題を解決するہوا فارم آپریشنز ریویو نوٹس کے ساتھ منسلک کریں۔

## और देखें

| और देखेंフォローする意味 |
|----------|-----------|----------|
|セクション 1 |データ空間の分離 15 件の決済 15 件の管理 ガバナンス 15 件の決済 15 件の管理| Nexus プライマリ + リリース エンジニアリング + コンプライアンスمیں کمیونیکیشن جاری کریں، RCA <=5 کاروباری دن۔ |
|セクション 2 |レーン バックログ SLA ステータス 死角 >30 ロールアウト ロールアウト| Nexus プライマリ + SRE کو پیج کریں، <=4 گھنٹے میں مٹیگیٹ کریں، 2 کاروباری دن کے اندر フォローアップありがとうございます|
|セクション 3 |ドリフト (ドキュメント アラート)。 |トラッカー、スプリント、修正、修正、修正|

レーン/データスペース ID とデータスペース ID とデータスペース ID とデータスペース ID とデータスペース ID とデータスペース ID。フォローアップ ٹاسکس/مالکان درج ہونا ضروری ہیں۔

## और देखें

- バンドル/マニフェスト/テレメトリのエクスポート `artifacts/nexus/<lane>/<date>/` のエクスポート
- 編集された設定 + `--trace-config` آؤٹ پٹ محفوظ رکھیں۔
- 構成設定 مینفسٹ تبدیلیاں ہوں تو کونسل منٹس + دستخط شدہ فیصلے منسلک کریں۔
- Nexus میٹرکس کے لئے متعلقہ Prometheus ہفتہ وار スナップショット 12 ماہ تک محفوظ کریں۔
- ランブックの編集 کو `docs/source/project_tracker/nexus_config_deltas/README.md` میں ریکارڈ کریں تاکہ آڈیٹرز جان سکیں ذمہ داریاں کب بدلیں۔

## 大事な- 概要: [Nexus 概要](./nexus-overview)
- 仕様: [Nexus仕様](./nexus-spec)
- レーン形状: [Nexus レーン モデル](./nexus-lane-model)
- 移行およびルーティング シム: [Nexus 移行ノート](./nexus-transition-notes)
- オペレーターのオンボーディング: [Sora Nexus オペレーターのオンボーディング](./nexus-operator-onboarding)
- テレメトリ修復: [Nexus テレメトリ修復計画](./nexus-telemetry-remediation)