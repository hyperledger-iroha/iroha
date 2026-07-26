---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスの概要
title: ソラ Nexus
説明: ملخص عالي المستوى لمعمارية Iroha 3 (Sora Nexus) مع مؤشرات إلى وثائق المستودعありがとう。
---

Nexus (Iroha 3) Iroha 2 を参照してください。 SDK をダウンロードしてください。 تعكس هذه الصفحة الملخص الجديد `docs/source/nexus_overview.md` في المستودع الأحادي حتى يتمكن قراء البوابة重要な情報を入力してください。

## いいえ

- **Iroha 2** - ログインしてください。
- **Iroha 3 / Sora Nexus** - الشبكة العامة متعددة المسارات حيث يسجل المشغلون مساحات世界のゲーム (DS) は、ゲームをプレイします。
- (IVM + ツールチェーン Kotodama) SDK の開発ABI のフィクスチャ Norito は、次のとおりです。セキュリティ `iroha3-<version>-<os>.tar.zst` セキュリティ Nexus `docs/source/sora_nexus_operator_onboarding.md` を確認してください。

## ああ、

|ああ、ああ |ログイン | ログイン
|----------|-----------|--------------|
|重要な情報 (DS) | طاق تنفيذ/تخزين محدد بالحوكمة يمتلك مسارا واحدا أو أكثر، ويعلن مجموعات المدققين وفئة拡張子 + DA。 | [Nexus 仕様](./nexus-spec) です。 |
|レーン | شريحة تنفيذ حتمية تصدر تعهدات يرتبها حلقة NPoS العالمية.レーン `default_public` و`public_custom` و`private_permissioned` و`hybrid_confidential`。 | يلتقط [نموذج Lane](./nexus-lane-model) الهندسة وبوادئ التخزين والاحتفاظ。 |
|認証済み |プレースホルダー ومراحل توجيه وتعبئة بملفين تتبع كيف تتطور عمليات النشر أحادية المسار إلى Nexus。 | توثق [ملاحظات الانتقال](./nexus-transition-notes) كل مرحلة ترحيل。 |
|宇宙ディレクトリ | DS をプレイしてください。重要な問題は、次のとおりです。 | `docs/source/project_tracker/nexus_config_deltas/` を確認してください。 |
| كتالوج المسارات | قسم الإعداد `[nexus]` يربط معرّفات レーン بالأسماء المستعارة وسياسات التوجيه وعتبات DA。 يقوم `irohad --sora --config … --trace-config` بطباعة الكتالوج المحسوم لأغراض التدقيق. | `docs/source/sora_nexus_operator_onboarding.md` を参照してください。 |
| और देखें XOR と CBDC の組み合わせ。 | يوضح `docs/source/cbdc_lane_playbook.md` مقابض السياسة وبوابات القياس. |
|セキュリティ/SLO | `dashboards/grafana/nexus_*.json` を使用して、DA を使用してください。ありがとうございます。 | يوضح [خطة معالجة القياس](./nexus-telemetry-remediation) اللوحات والتنبيهات وأدلة التدقيق. |

## いいえ|ログイン | ログインログイン | ログインログイン して翻訳を追加する
|------|------|------|
| N0 - ニュース |レジストラ يديره المجلس (`.sora`) は、 انضمام يدوي للمشغلين، كتالوج مسارات ثابت です。 | DS と عمليات تسليم حوكمة مجربة。 |
| N1 - ニュース | يضيف لاحقات `.nexus`، مزادات، レジストラ بالخدمة الذاتية، توصيل تسوية XOR。 |リゾルバー/ゲートウェイのセキュリティを強化します。 |
| N2 - ニュース | يقدم `.dao`، واجهات API للموزعين، تحليلات، بوابة نزاعات، بطاقات أشرفين。 |重要な情報を確認してください。 |
| NX-12/13/14 | NX-12/13/14最高のパフォーマンスを見せてください。 | [Nexus 概要](./nexus-overview) + [Nexus 操作](./nexus-operations) を確認してください。 |

## سؤوليات المشغلين

1. ** - أبقِ `config/config.toml` متزامنا مع كتالوج المسارات ومساحات البيانات المنشور؛ `--trace-config` を確認してください。
2. ** - スペース ディレクトリ - スペース ディレクトリ。
3. **評価** - 評価 `nexus_lanes.json` و`nexus_settlement.json` 評価 SDK 評価PagerDuty を使用して、PagerDuty を使用してください。
4. ** 管理者** - 管理 [Nexus 操作](./nexus-operations) وقدّم تقارير RCAすごいです。
5. ** الجاهزية للحوكمة** - الحضر تصويتات مجلس Nexus التي تؤثر على مساراتك وتدرّب على عليمات الرجوع ربع سنويا (يتم تتبعها عبر `docs/source/project_tracker/nexus_config_deltas/`)。

## ああ

- 番号: `docs/source/nexus_overview.md`
- バージョン: [./nexus-spec](./nexus-spec)
- 番号: [./nexus-lane-model](./nexus-lane-model)
- メッセージ: [./nexus-transition-notes](./nexus-transition-notes)
- セキュリティ: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- バージョン: [./nexus-operations](./nexus-operations)
- メッセージ: `docs/source/sora_nexus_operator_onboarding.md`