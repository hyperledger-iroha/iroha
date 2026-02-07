---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードプラン
タイトル: خطة تنفيذ عقدة SoraFS
サイドバーラベル: خطة تنفيذ العقدة
説明: SF-3 の開発と開発、開発、開発、開発。
---

:::note ノート
テストは `docs/source/sorafs/sorafs_node_plan.md` です。スフィンクス、スフィンクス、スフィンクス。
:::

SF-3 のクレートを確認 `sorafs-node` セキュリティ Iroha/Torii を確認SoraFS。 ستخدم هذه الخطة بجانب [دليل تخزين العقدة](node-storage.md)، و[سياسة قبول الموفّرين](provider-admission-policy.md) ، و[خارطة طريق سوق سعة التخزين](storage-capacity-marketplace.md) عند ترتيب التسليمات.

## النطاق المستهدف (المرحلة M1)

1. ** تكامل مخزن القطع.** تغليف `sorafs_car::ChunkStore` بواجهة خلفية دائمة تخزن بايتات القطع وملفات マニフェストPoR を使用してください。
2. ** قايط نهاية البوابة.** توفير نقاط نهاية HTTP لـ Norito لإرسال pin وجلب القطع وأخذ عينات PoR Torii です。
3. **توصيل الإعدادات.** إضافة بنية إعداد `SoraFsStorage` (مفتاح التفعيل، السعة، المجلدات، حدود التوازي) وتمريرها عبر `iroha_config` و`iroha_core` و`iroha_torii`。
4. **الحصص/الجدولة.** فرض حدود القرص/التوازي التي يحددها المشغل ووضع الطلبات في طوابير مع背圧。
5. ** إصدار مقاييس/سجلات لنجاح pin وزمن جلب القطع واستغلال السعة ونتائج عينات PoR。

## いいえ

### A. 木枠箱

|ああ |ああ | और देखें
|------|----------|----------|
| `crates/sorafs_node` および `config` و`store` و`gateway` و`scheduler` و`telemetry`。 | और देखें Torii を確認してください。 |
| `StorageConfig` は `SoraFsStorage` (ユーザー → 実際の → デフォルト)。 |構成 WG | 構成 WG Norito/`iroha_config` を確認してください。 |
| `NodeHandle` は Torii ピン/フェッチです。 | और देखें最高のパフォーマンスを見せてください。 |

### B. خزن قطع دائم

|ああ |ああ | और देखें
|------|----------|----------|
| `sorafs_car::ChunkStore` マニフェスト (`sled`/`sqlite`)。 | और देखें評価: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
| PoR الوصفية (64 KiB/4 KiB) باستخدام `ChunkStore::sample_leaves`。 | और देखें دعم إعادة التشغيل؛ فشل بسرعة عند التلف. |
| تنفيذ إعادة فحص السلامة عند البدء (إعادة تجزئة マニフェスト وحذف pins غير المكتملة)。 | और देखें Torii を確認してください。 |

### C. ナオミ

| और देखेंああ |ああ |
|-----------|-----------|------|
| `POST /sorafs/pin` | `PinProposalV1` マニフェスト マニフェスト マニフェスト マニフェスト CID マニフェスト。 |チャンカーをダウンロードしてください。 |
| `GET /sorafs/chunks/{cid}` + 範囲 | `Content-Chunker` を確認してください。 | ستخدام المجدول مع ميزانيات البث (ربطها بقدرات النطاق SF-2d)。 |
| `POST /sorafs/por/sample` | PoR マニフェストを確認してください。 | Norito JSON を参照してください。 |
| `GET /sorafs/telemetry` |重要: PoR をフェッチします。 |ニュース/ニュース/ニュース。 |

`sorafs_node::por`، حيث يسجل المتبع كل `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` لكي تعكس مقاييس `CapacityMeter` أحكام الحوكمة من دون منطق Torii مخصص.【crates/sorafs_node/src/scheduler.rs#L147】

重要:

- アクサム Torii حمولات `norito::json`。
- أضف مخططات Norito للاستجابات (`PinResultV1` و`FetchErrorV1` وبنى التليمترية)。

- ✅ أصبح المسار `/v1/sorafs/por/ingestion/{manifest_digest_hex}` يعرض عمق الـ backlog وأقدم epoch/deadline وأحدث طوابع النجاح/الفشل لكل `sorafs_node::NodeHandle::por_ingestion_status`Torii`torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` للّوحات.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. ああ、

|ああ |翻訳 |
|------|----------|
|認証済み |最高のパフォーマンスを見せてください。ピンは `max_capacity_bytes` です。重要な情報は、次のとおりです。 |
|フェッチ | (`max_parallel_fetches`) は SF-2d を開発しました。 |
|ピン | ピンتحديد عدد مهام الإدخال المعلقة؛ Norito が表示されます。 |
| PoR | عامل خلفي يعمل وفق `por_sample_interval_secs`。 |

### E. ニュース

回答 (Prometheus):- `sorafs_pin_success_total`、`sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (هيستوغرام مع وسوم `result`)
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

回答 / 回答:

- テスト Norito テスト (`StorageTelemetryV1`)。
- 90% の割合で、PoR を取得します。

### F. アナスタシア

1. **اختبارات وحدات.** ديمومة مخزن القطع، حسابات الحصة، ثوابت المجدول (انظر `crates/sorafs_node/src/scheduler.rs`)。
2. **اختبارات تكامل** (`crates/sorafs_node/tests`)。 دورة pin → fetch، الاستعادة بعد إعادة التشغيل، رفض الحصص، والتحقق من إثباتات أخذ عينات PoR。
3. **اختبارات تكامل Torii.** تشغيل Torii مع تفعيل التخزين وتجربة نقاط النهاية HTTP بر `assert_cmd`。
4. ** خارطة طريق الفوضى.** تدريبات مستقبلية تحاكي نفاد القرص، بطء IO، وإزالة الموفّرين.

## ああ

- SF-2b — SF-2b を開発しました。
- SF-2c — ロシア連邦。
- SF-2d 広告 — 広告 広告 + 広告 広告。

## معايير إغلاق المرحلة

- `cargo run -p sorafs_node --example pin_fetch` の備品。
- ログイン Torii ログイン `--features sorafs-storage` ログインしてください。
- تحديث الوثائق ([دليل تخزين العقدة](node-storage.md)) مع افتراضيات الإعداد وأمثلة CLI؛ランブックは、次のとおりです。
- ステージングと PoR のパフォーマンス。

## خرجات الوثائق والعمليات

- تحديث [مرجع تخزين العقدة](node-storage.md) مع افتراضيات الإعداد، استخدام CLI، وخطواتああ。
- SF-3 [ランブック عمليات العقدة](node-operations.md) متوافقا مع التنفيذ مع تطور SF-3。
- API インターフェイス `/sorafs/*` インターフェイス OpenAPI インターフェイスTorii。