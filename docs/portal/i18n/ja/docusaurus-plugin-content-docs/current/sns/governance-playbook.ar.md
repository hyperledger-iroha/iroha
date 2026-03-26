---
lang: ja
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
تعكس هذه الصفحة `docs/source/sns/governance_playbook.md` وتعمل الان كمرجع بوابة
すごい。 PR をご覧ください。
:::

# دليل حوكمة خدمة أسماء سورا (SN-6)

**الحالة:** صيغ 2026-03-24 - مرجع حي لاستعداد SN-1/SN-6  
** バージョン:** SN-6「コンプライアンスと紛争解決」、SN-7「リゾルバーとゲートウェイ同期」、ADDR-1/ADDR-5  
** 応答:** 応答 [`registry-schema.md`](./registry-schema.md) 応答 API 応答[`registrar-api.md`](./registrar-api.md) ، ارشادات تجربة العناوين في [`address-display-guidelines.md`](./address-display-guidelines.md) وقواعد بنية [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md)。

يصف هذا الدليل كيف تعتمد هيئات حوكمة خدمة أسماء سورا (SNS) المواثيق، وتوافق على
ソリューションは、リゾルバーとゲートウェイを備えています。ワイズ
CLI `sns governance ...` ومانيفستات Norito
والاثار التدقيقية مرجعا تشغيليا واحدا قبل N1 (الاطلاق العام)。

## 1. いいえ

セキュリティ:

- ログインしてください。 - ログインしてください。
- 保護者は、保護者を保護します。
- スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード - スチュワード
  最高です。
- リゾルバー/ゲートウェイ、SoraDNS 、GAR 、および
  ああ。
- 国際会議 Norito
  قابلة للتدقيق。

يغطي مراحل البيتا المغلقة (N0) والاطلاق العام (N1) والتوسع (N2) المدرجة في
`roadmap.md` من خلال ربط كل سير عمل بالادلة المطلوبة ولوحات المتابعة ومسارات
ああ。

## 2. いいえ。

|ああ |ログインしてください。ログイン アカウント新規登録認証済み |
|-----|----------|--------------------------|----------|
| और देखेंスチュワード。 | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`、`sns governance charter submit`。 | رئيس المجلس + متعقب جدول اعمال الحوكمة。 |
|ガーディアン |ソフト/ハードの互換性 72 時間。 |保護者は `sns governance freeze`، ومانيفستات التجاوز المسجلة تحت `artifacts/sns/guardian/*`。 |保護者がオンコール中 (<=15 分の ACK)。 |
|スチュワードログイン して翻訳を追加するありがとうございます。 |スチュワード、`SuffixPolicyV1`、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード、スチュワード。 |スチュワード + PagerDuty の管理者です。 |
| और देखें `/v1/sns/*`、国際標準、国際標準、CLI を使用してください。 | API は ([`registrar-api.md`](./registrar-api.md))、 مقاييس `sns_registrar_status_total`、 الثباتات الدفع المؤرشفة تحت `artifacts/sns/payments/*`。 | دير مناوبة المسجل ورابط الخزينة。 |
|リゾルバ | 解決策يحافظون على SoraDNS وGAR وحالة البوابة متوافقة مع احداث المسجل؛ قاييس الشفافية。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 | SRE リゾルバー オンコール + セキュリティ ソリューション。 |
| और देखें評価は 70/30 回、紹介数は SLA です。 |ストライプ/カラー、KPI、`docs/source/sns/regulatory/`。 | مراقب المالية + مسؤول الامتثال。 |
| और देखें EU DSA (EU DSA) と KPI (KPI) を確認してください。 | `docs/source/sns/regulatory/`、`ops/drill-log.md`、`docs/source/sns/regulatory/` を確認してください。 |ありがとうございます。 |
|翻訳 / SRE 翻訳 | يعالج الحوادث (تصادمات، انحرافات فوترة، اعطال リゾルバー) وينسق رسائل العملاء، ويمتلك الادلةああ。 | `ops/drill-log.md`、スラック/戦争室、`incident/`。 | SNS + SRE のオンコール。 |

## 3. 問題を解決する|認証済み |ああ |ああ |
|------|--------|------|
|数値 + 数値 KPI | `docs/source/sns/governance_addenda/` | مواثيق موقعة مع تحكم بالنسخ، ومواثيق KPI، وقرارات الحوكمة اليها بتصويتات CLI。 |
|重要 | [`registry-schema.md`](./registry-schema.md) | Norito (`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`)。 |
| और देखें [`registrar-api.md`](./registrar-api.md) | REST/gRPC の `sns_registrar_status_total` フック。 |
| UX 版 | [`address-display-guidelines.md`](./address-display-guidelines.md) | عروض i105 (المفضلة) والمضغوطة (الخيار الثاني) المرجعية التي تعكسها المحافظ/المستكشفات。 |
| SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |尾行者を追跡します。 |
| ذكرات تنظيمية | `docs/source/sns/regulatory/` |重要なのは、EU DSA (EU DSA) のスチュワードです。 |
|ドリル | `ops/drill-log.md` | IR と IR を組み合わせてください。 |
|評価 | 評価`artifacts/sns/` |保護者はリゾルバー、KPI は CLI を使用し、`sns governance ...` を実行します。 |

يجب ان تشير كل اجراءات الحوكمة الى اثر واحد على الاقل من الجدول اعلاه حتى يتمكن
24 月 24 日。

## 4. いいえ、いいえ。

### 4.1 管理人

|ああ |ああ | CLI / セキュリティ |重要 |
|----------|----------|------|----------|
| KPI を評価 |スチュワード + スチュワード |マークダウン マークダウン `docs/source/sns/governance_addenda/YY/` | KPI フックをフックします。 |
|ニュース | ニュースऔर देखें `sns governance charter submit --input SN-CH-YYYY-NN.md` (イ18NI00000080X) | CLI は Norito は `artifacts/sns/governance/<id>/charter_motion.json` です。 |
|ガーディアン | ガーディアン保護者 + ガーディアン | `sns governance ballot cast --proposal <id>` と `sns governance guardian-ack --proposal <id>` |あなたのことを考えてください。 |
|スチュワード |スチュワード | `sns governance steward-ack --proposal <id> --signature <file>` | مطلوب قبل تغيير سياسات اللاحقات; `artifacts/sns/governance/<id>/steward_ack.json` です。 |
|翻訳 |ログイン | ログイン評価 `SuffixPolicyV1`、評価 `status.md`。 | طابع التفعيل في `sns_governance_activation_total`。 |
| और देखें翻訳 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` は卓上をドリルします。 |最高のパフォーマンスを見せてください。 |

### 4.2 を使用してください。

1. **الفحص المسبق:** يستعلم المسجل `SuffixPolicyV1` لتاكيد شريحة التسعير، الشروط
   और देखें بق جداول التسعير متزامنة مع جدول الشرائح
   3/4/5/6-9/10+ (オンライン) ロードマップ。
2. **密封入札:** プレミアム価格 72 時間コミット / 24 時間公開
   `sns governance auction commit` / `... reveal`。コミット (ハッシュ)
   فقط) تحت `artifacts/sns/auctions/<name>/commit.json` حتى يتمكن المدققون من
   ありがとうございます。
3. **التحقق من الدفع:** يتحقق المسجلون من `PaymentProofV1` مقابل تقسيمات الخزينة
   (70% のスチュワード / 30% スチュワードのカーブアウト紹介 <=10%)。 JSON Norito の例
   `artifacts/sns/payments/<tx>.json` واربطه في استجابة المسجل (`RevenueAccrualEventV1`)。
4. **フック الحوكمة:** ارفق `GovernanceHookV1` للاسماء プレミアム/ガード セキュリティ
   スチュワード。フックフック
   `sns_err_governance_missing`。
5. **التفعيل + مزامنة リゾルバー:** بمجرد ان يرسل Torii حدث التسجيل، شغل tailer
   GAR/ゾーン (バージョン 4.5) のリゾルバー。
6. **افصاح العميل:** حدث دفتر المستخدِم (ウォレット/エクスプローラー) フィクスチャ المشتركة في
   [`address-display-guidelines.md`](./address-display-guidelines.md) ، مع ضمان ان
   i105 認証/QR 認証。

### 4.3 を実行します。- ** 評価:** 評価 30 評価 + 評価 60 評価
  `SuffixPolicyV1`。 60 يوما، تتفعل تلقائيا سلسلة اعادة الفتح
  الهولندية (7 ايام، رسوم 10x تنخفض 15%/يوم) عبر `sns governance reopen`。
- ** 評価:** 評価 `RevenueAccrualEventV1`。やあ
  翻訳 (CSV/Parquet) 翻訳。 और देखें
  `artifacts/sns/treasury/<date>.json`。
- **カーブアウト紹介:** يتم تتبع نسب 紹介 الاختيارية لكل لاحقة عبر اضافة
  `referral_share` 管理人。 صدر المسجلون التقسيم النهائي ويخزنون
  مانيـفستات 紹介 بجانب اثبات الدفع.
- **وتيرة التقارير:** تنشر المالية ملاحق KPI شهرية (التسجيلات، التجديدات، ARPU،)
  (ボンド) تحت `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  يجب ان تعتمد لوحات المتابعة على الجداول المصدرة نفسها حتى تطابق ارقام Grafana
  そうです。
- ** مراجعة KPI شهرية:** يجمع فحص اول ثلاثاء قائد المالية وsteward المناوب وPM
  ああ。 افتح [لوحة KPI الخاصة بـSNS](./kpi-dashboard.md) (تضمين البوابة
  `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`) ، صدّر جداول انتاجية
  あなたのことを忘れないでください。 فعّل حادثا
  SLA ( نوافذ تجميد >72 h، ارتفاع اخطاء المسجل، انحراف ARPU)。

### 4.4 を実行します。

|ログイン | ログインああ | और देखें SLA |
|----------|----------|----------|-----|
|ソフト | ソフト |スチュワード/スチュワード | قدم تذكرة `SNS-DF-<id>` مع اثباتات الدفع، مرجع bond النزاع، والمحدد/المحددات المتاثرة. | <=4 時間です。 |
|ガーディアン | ガーディアンガーディアン | `sns governance freeze --selector <i105> --reason <text> --until <ts>` は `GuardianFreezeTicketV1`。 JSON は `artifacts/sns/guardian/<id>.json` です。 | ACK は 30 分以内、ACK は 2 時間以内。 |
|ニュース | ニュースऔर देखें保護者と絆を結ぶ。 |最高のパフォーマンスを見せてください。 |
|ログインしてください。管理人 + スチュワード | 7 番目のロードマップ (ロードマップ) は、`sns governance dispute ballot` です。ありがとうございます。 | الحكم <=7 ايام بعد ايداع 債券。 |
| और देखेंガーディアン + 保護者 |債券、債券、債券。 Norito `DisputeAppealV1` を確認してください。 | <=10 時間。 |
| فك التجميد والمعالجة |解決 + 解決策 | `sns governance unfreeze --selector <i105> --ticket <id>`、GAR/リゾルバー。 |そうです。 |

القوانين الطارئة (تجميدات يطلقها ガーディアン <=72 h) تتبع نفس التدفق لكنها تتطلب
`docs/source/sns/regulatory/` を確認してください。

### 4.5 解決策

1. **フック:** セキュリティ リゾルバー (`tools/soradns-resolver` SSE)。
   リゾルバー、テーラー、テーラー
   (`scripts/telemetry/run_soradns_transparency_tail.sh`)。
2. **تحديث قالب GAR:** يجب على البوابات تحديث قوالب GAR المشار اليها بواسطة
   `canonical_gateway_suffix()` واعادة توقيع قائمة `host_pattern`。 और देखें
   `artifacts/sns/gar/<date>.patch`。
3. **ゾーンファイル:** ゾーンファイル الموضح في `roadmap.md` (名前、ttl、cid、証明)
   Torii/SoraFS。 JSON Norito と `artifacts/sns/zonefiles/<name>/<version>.json`。
4. **فحص الشفافية:** شغل `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   重要な問題は、次のとおりです。 Prometheus を確認してください。
5. **評価:** 評価 `Sora-*` (評価 評価 評価 CSP 評価 GAR)
   وارفقها بسجل الحوكمة لكي يثبت المشغلون ان البوابة قدمت الجديد مع حواجز الحمايةああ。

## 5. いいえ

|認証済み |翻訳 | और देखें
|-------|-------|------|
| `sns_registrar_status_total{result,suffix}` | Torii | ログインしてください。 عداد نجاح/خطا للتسجيلات، التجديدات، التحويلات؛ ينبه عندما يرتفع `result="error"` لكل لاحقة。 |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii | SLO は API をサポートします`torii_norito_rpc_observability.json` です。 |
| `soradns_bundle_proof_age_seconds` と `soradns_bundle_cid_drift_total` |テーラー شفافية リゾルバー | كشف ادلة قديمة او انحراف GAR؛ `dashboards/alerts/soradns_transparency_rules.yml` です。 |
| `sns_governance_activation_total` | CLI を使用する | عداد يزداد عند تفعيل ميثاق/ملحق؛あなたのことを忘れないでください。 |
| `guardian_freeze_active` ゲージ | CLI ガーディアン |ソフト/ハードの区別 محدد؛ SRE 番号、`1` 番号、SLA 番号。 |
| KPI | 重要な KPI और देखें خصات شهرية تنشر مع المذكرات التنظيمية؛重要な [KPI およびSNS](./kpi-dashboard.md) スチュワードの評価と評価Grafana。 |

## 6. 重要なこと|ああ |ログイン アカウント新規登録認証済み |
|----------|--------------------------|----------|
| تغيير الميثاق / السياسة | مانيفست Norito موقع، نص CLI، فرق KPI، اقرار のスチュワード。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|評価 / 評価 | `RegisterNameRequestV1`、`RevenueAccrualEventV1`、認証済み。 | `artifacts/sns/payments/<tx>.json`、API を参照してください。 |
| और देखेंコミット/公開を実行します。 | `artifacts/sns/auctions/<name>/`。 |
| تجميد / فك تجميد |保護者を保護してください。 | `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|リゾルバー |ゾーンファイル/GAR は JSONL のテーラー Prometheus です。 | `artifacts/sns/resolver/<date>/` + 認証。 |
| और देखें KPI の管理。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. いいえ

|ログイン | ログインログイン して翻訳を追加する認証済み |
|----------|--------------|---------------|
| N0 - ニュース | SN-1/SN-2 CLI の保護者。 |認証コード + ACK スチュワード認証コード、ACK スチュワード認証コード、`ops/drill-log.md` 認証コード。 |
| N1 - ニュース | مزادات + شرائح اسعار ثابتة مفعلة لـ`.sora`/`.nexus`, مسجل ذاتي الخدمة، مزامنةリゾルバーをテストします。 | فرق ورقة التسعير، نتائج CI للمسجل، ملحق الدفع/KPI، مخرجات tailer الشفافية، ملاحظات تمرينああ。 |
| N2 - ニュース | `.dao`、再販業者、再販業者、再販業者、管理責任者。 | SLA セキュリティ セキュリティ セキュリティ セキュリティ スチュワード セキュリティ セキュリティ スチュワード セキュリティ セキュリティ スチュワード セキュリティ再販業者。 |

テーブルトップ مسجلة (مسار تسجيل ناجح، تجميد، عطل リゾルバ)
`ops/drill-log.md` です。

## 8. すごい

|ああ |ああ | और देखें翻訳: 翻訳: 翻訳
|------|------|------|----------------------|
|解決策/GAR 解決策 |セクション 1 | SRE リゾルバー + ガーディアン |オンコールのリゾルバーとテーラーの対応30分|
| API を使用する |セクション 1 | دير مناوبة المسجل |セキュリティ スチュワード/セキュリティ スチュワード/セキュリティ Toriiありがとうございます。 |
| زاع اسم واحد، عدم تطابق الدفع، او تصعيد عميل |セクション 2 |スチュワード + قائد الدعم |セキュリティ ソフト ソフト セキュリティ SLA セキュリティ セキュリティ セキュリティ セキュリティ セキュリティありがとうございます。 |
|重要な情報 |セクション 2 |認証済み | 認証済みصياغة خطة معالجة، حفظ مذكرة تحت `docs/source/sns/regulatory/`、جدولة جلسة مجلس متابعة。 |
|ニュース | ニュースセクション 3 |首相 | `ops/drill-log.md` のロードマップ。 |

يجب على كل الحوادث ان تنشئ `incident/YYYY-MM-DD-sns-<slug>.md` مع جداول الملكية
ログインしてください。

## 9. ああ

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS وDG وADDR)

حافظ على تحديث هذا الدليل كلما تغيرت صياغة الميثاق او اسطح CLI او عقود
ああ、ロードマップ ロードマップ ロードマップ
`docs/source/sns/governance_playbook.md` そうです。