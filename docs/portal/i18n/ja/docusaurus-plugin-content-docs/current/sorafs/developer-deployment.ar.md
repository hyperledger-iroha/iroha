---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者デプロイメント
タイトル: ملاحظات نشر SoraFS
サイドバーラベル: 重要
説明: قائمة تحقق لترقية خط أنابيب SoraFS من CI إلى الإنتاج.
---

:::note ノート
テストは `docs/source/sorafs/developer/deployment.md` です。 حرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة。
:::

# 大事な

يعزز مسار تغليف SoraFS الحتمية، لذا فإن الانتقال من CI إلى الإنتاج يتطلب أساسًا ضوابطそうです。最高のパフォーマンスを見せてください。

## قبل التشغيل

- **مواءمة السجل** — أكد أن ملفات chunker والمانيفستات تشير إلى نفس ثلاثية `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`)。
- **سياسة القبول** — راجع إعلانات المزوّدين الموقّعة وأدلة エイリアス المطلوبة لـ `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`)。
- **دليل تشغيل سجل التثبيت** — احتفظ بـ `docs/source/sorafs/runbooks/pin_registry_ops.md` للسيناريوهات الاستردادية (تدوير alias، إخفاقات)または）。

## إعدادات البيئة

- يجب على البوابات تفعيل نقطة نهاية بث الأدلة (`POST /v2/sorafs/proof/stream`) حتى يتمكن CLI من إصدار ملخصاتああ。
- 回答 `sorafs_alias_cache` 評価 `iroha_config` 評価 CLI 評価(`sorafs_cli manifest submit --alias-*`)。
- وفّر رموز البث (أو بيانات اعتماد Torii) عبر مدير أسرار آمن.
- فعّل مصدّرات التليمترية (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) وأرسلها إلى حزمة Prometheus/OTel الخاصةああ。

## いいえ

1. **ブルー/グリーン**
   - 回答 `manifest submit --summary-out` を表示します。
   - راقب `torii_sorafs_gateway_refusals_total` لاكتشاف عدم تطابق القدرات مبكرًا.
2.***************
   - ニュース `sorafs_cli proof stream` ニュース最高のパフォーマンスを見せてください。
   - يجب أن يكون `proof verify` جزءًا من اختبار スモーク بعد التثبيت لضمان أن CAR المستضاف لدى المزوّدين لاダイジェスト版。
3. **
   - `docs/examples/sorafs_proof_streaming_dashboard.json` と Grafana。
   - チャンク範囲 (`docs/source/sorafs/runbooks/pin_registry_ops.md`) のチャンク範囲。
4. ** 最高のパフォーマンス**
   - ببع خطوات الإطلاق المرحلي في `docs/source/sorafs/runbooks/multi_source_rollout.md` عند تفعيل المُنسِّق، وأرشِف آرتيفاكتاتスコアボード/スコアボード/スコアボード。

## और देखें

- 回答 `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` ストリーム トークン。
  - `dispute_revocation_runbook.md` 認証済み。
  - `sorafs_node_ops.md` は。
  - `multi_source_rollout.md` は、最高のパフォーマンスを提供します。
- 管理ツール GovernanceLog 管理ツール PoR トラッカー管理ツールأداء المزوّدين。

## ああ、

- 国際宇宙ステーション (`sorafs_car::multi_fetch`) 国際宇宙ステーション (SF-6b)。
- PDP/PoTR と SF-13/SF-14 の互換性CLI を使用して、オンライン インターフェイスを使用してください。

من خلال الجمع بين ملاحظات النشر هذه وبين دليل البدء السريع ووصفات CI، يمكن للفرق الانتقال من جاجارب المحلية إلى خطوط أنابيب SoraFS جاهزة للإنتاج بعملية قابلة للتكرار وقابلةすごい。