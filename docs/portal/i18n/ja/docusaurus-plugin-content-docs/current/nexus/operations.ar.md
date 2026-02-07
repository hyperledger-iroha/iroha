---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスオペレーション
タイトル: دليل تشغيل Nexus
説明: ملخص عملي لخطوات تشغيل مشغل Nexus، يعكس `docs/source/nexus_operations.md`。
---

ستخدم هذه الصفحة كمرجع سريع موازي لـ `docs/source/nexus_operations.md`。 فهي تلخص قائمة التشغيل، ونقاط إدارة التغيير، ومتطلبات تغطية القياس التي يجب على مشغلي Nexus です。

## قائمة دورة الحياة

|ログイン | ログインऔर देखेंああ |
|----------|----------|----------|
|重要な情報 |重要な情報は、`profile = "iroha3"` に準拠しています。 | `scripts/select_release_profile.py` チェックサムを確認してください。 |
| और देखें حدّث كتالوج `[nexus]`، سياسة التوجيه، وعتبات DA وفق بيان المجلس، ثم التقط `--trace-config`。 | `irohad --sora --config ... --trace-config` オンボーディングです。 |
| فحص الدخان والتحويل | CLI (`FindNetworkStatus`) を使用して、`irohad --sora --config ... --trace-config` を使用してください。 |アラートマネージャー。 |
|ニュース | ニュース構成/ランブックを確認してください。 | حاضر مراجعة ربع سنوية، لقطات لوحات، أرقام تذاكر التدوير. |

يبقى オンボーディング التفصيلي (استبدال المفاتيح، قوالب التوجيه، خطوات ملف الإصدار) في `docs/source/sora_nexus_operator_onboarding.md`。

## いいえ

1. **評価** - 評価`status.md`/`roadmap.md`評価オンボーディングの PR を担当します。
2. **レーン** - 宇宙ディレクトリ`docs/source/project_tracker/nexus_config_deltas/`。
3. **レーン/データスペース - `config/config.toml` レーン/データスペース。 حفظ نسخة منقحة من الإعداد الفعلي عند انضمام أو ترقية العقد.
4. **تمارين الرجوع** - درّب ربع سنوي على إجراءات الإيقاف/الاستعادة/فحص الدخان؛ دوّن النتائج تحت `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`。
5. **موافقات الامتثال** - レーン الخاصة/CBDC على موافقة امتثال قبل تعديل سياسة DA أو مفاتيح تنقيح القياس (انظر `docs/source/cbdc_lane_playbook.md`)。

## と SLO

- バージョン: `dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`、SDK (`android_operator_console.json`)。
- バージョン: `dashboards/alerts/nexus_audit_rules.yml` および Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`)。
- 回答:
  - `nexus_lane_height{lane_id}` - テスト。
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل レーン (公共 64 / 民間 8)。
  - `nexus_settlement_latency_seconds{lane_id}` - P99 900 ミリ秒 (パブリック) と 1200 ミリ秒 (プライベート)。
  - `torii_request_failures_total{scheme="norito_rpc"}` - 評価は 2% です。
  - `telemetry_redaction_override_total` - セクション 2 فوري؛最高のパフォーマンスを見せてください。
- فذ قائمة معالجة القياس في [خطة معالجة قياس Nexus](./nexus-telemetry-remediation) على الأقل فصليا أرفق النموذج المكتمل بملاحظات مراجعة التشغيل。

## صفوفة الحوادث

|ああ | और देखें और देखें
|----------|-----------|----------|
|セクション 1 |データスペースは、15 番目のデータスペースに基づいています。 | Nexus プライマリ + リリース エンジニアリング + コンプライアンス セキュリティ レベル <=60、RCA <=5 レベル。 |
|セクション 2 | SLA のレーン数が 30 を超えると、ロールアウトが行われます。 | Nexus プライマリ + SRE 値 <=4 値。 |
|セクション 3 | انحراف غير معطل (docs، تنبيهات)。 |重要な問題は、次のとおりです。 |

ID とレーン/データスペースの管理والمقاييس/السجلات الداعمة، ومهام المتابعة والمالكين。

## أرشيف الأدلة

- خزّن الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`。
- حتفظ بالتهيئات المنقحة + مخرجات `--trace-config` لكل إصدار.
- أرفق محاضر المجلس + القرارات الموقعة عند تطبيق تغييرات الإعداد أو البيانات。
- ニュース Prometheus ニュース ニュース Nexus 12 ニュース。
- دوّن تعديلات الدليل في `docs/source/project_tracker/nexus_config_deltas/README.md` حتى يعرف المدققون متى تغيرت المسؤوليات.

## عرض المزيد

- 説明: [Nexus 概要](./nexus-overview)
- 説明: [Nexus 仕様](./nexus-spec)
- レーン: [Nexus レーン モデル](./nexus-lane-model)
- 回答: [Nexus 移行メモ](./nexus-transition-notes)
- オンボーディング المشغلين: [Sora Nexus オペレーターのオンボーディング](./nexus-operator-onboarding)
- メッセージ: [Nexus テレメトリ修復計画](./nexus-telemetry-remediation)