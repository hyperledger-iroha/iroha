---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
`docs/source/da/replication_policy.md`。 بق النسختين متزامنتين حتى يتم
ありがとうございます。
:::

# アーティファクト (DA-4)

_番号: 番号 -- 番号: コア プロトコル WG / ストレージ チーム / SRE_

يطبق خط انابيب ingest الخاص بـ DA اهداف احتفاظ حتمية لكل فئة blob مذكورة في
`roadmap.md` (DA-4)。 يرفض Torii الاحتفاظ باغلفة التي يزودها
المتصل اذا لم تطابق السياسة المكونة، ما يضمن ان كل عقدة مدقق/تخزين تحتفظ بعدد
重要な問題は、次のとおりです。

## ああ、ああ

|ブロブ |ホット |寒いです |ログイン | ログインऔर देखें और देखें
|----------|---------------|---------------|-----|-------------|---------------|
| `taikai_segment` | 24 件 | 14日5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 件 | 7 ヤス | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 件 | 180 日 | 3 | `cold` | `da.governance` |
| _デフォルト (كل الفئات الاخرى)_ | 6 件 | 30日3 | `warm` | `da.default` |

تدمج هذه القيم في `torii.da_ingest.replication_policy` وتطبق على جميع
`/v2/da/ingest`。 يعيد Torii كتابة マニフェスト مع ملف الاحتفاظ المفروض ويصدر
SDK をダウンロードする
そうです。

### فئات توفر タイカイ

マニフェスト توجيه Taikai (`taikai.trm`) عن `availability_class`
(`hot`、`warm`、`cold`)。 يفرض Torii السياسة المطابقة قبل التقسيم بحيث يمكن
ストリームをストリーミングします。回答:

| और देखेंホット |寒いです |ログイン | ログインऔर देखें और देखें
|-----------|---------------|---------------|-----|-------------|---------------|
| `hot` | 24 件 | 14日5 | `hot` | `da.taikai.live` |
| `warm` | 6 件 | 30日4 | `warm` | `da.taikai.warm` |
| `cold` | 1 件 | 180 日 | 3 | `cold` | `da.taikai.archive` |

`hot` を確認してください。 قم
認証済み
`torii.da_ingest.replication_policy.taikai_availability` ذا كانت شبكتك تستخدم
そうです。

## ああ

`torii.da_ingest.replication_policy` قالب *デフォルト* مع
مصفوفة は لكل فئة をオーバーライドします。 معرفات الفئة غير حساسة لحالة الاحرف وتقبل
`taikai_segment`、`nexus_lane_sidecar`、`governance_artifact`、`custom:<u16>`
すごいね。 `hot`、`warm`、`cold`。

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

あなたのことを忘れないでください。オーバーライド
ああ`default_retention` を参照してください。

يمكن تجاوز فئات توفر Taikai بشكل مستقل عبر
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## और देखें

- يستبدل Torii `RetentionPolicy` الذي يقدمه المستخدم بالملف المفروض قبل التقسيم
  マニフェスト。
- ترفضは、 المبنية مسبقا التي تعلن ملف احتفاظ غير مطابق بـ
  `400 schema mismatch` حتى لا تتمكن العملاء المتقادمة من اضعاف العقد.
- يتم تسجيل كل حدث オーバーライド (`blob_class`、السياسة المرسلة مقابل المتوقعة)
  ロールアウト。راجع [خطة ingest لتوفر البيانات](ingest-plan.md) (قائمة التحقق) للبوابة المحدثة
ありがとうございます。

## سير عمل اعادة التكرار (متابعة DA-4)

और देखें يجب على المشغلين ايضا اثبات ان マニフェスト
الحية واوامر التكرار تبقى متسقة مع المكونة حتى يتمكن SoraFS من اعادة
ブロブのブロック。

1. **راقب الانحراف.** يصدر Torii
   `overriding DA retention policy to match configured network baseline` 認証
   سل المتصل قيما قديمة للاحتفاظ。 قرن هذا السجل مع قياسات
   `torii_sorafs_replication_*` は、次のことを意味します。
2. **فرق النية مقابل النسخ الحية.** استخدم مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   يحمل الامر `torii.da_ingest.replication_policy` من الاعدادات المقدمة،
   マニフェスト (JSON Norito) ペイロード
   `ReplicationOrderV1` ダイジェストとマニフェスト。評価:

   - `policy_mismatch` - マニフェスト يختلف عن السياسة المفروضة
     (لا يجب ان يحدث ذلك الا اذا كان Torii مكونا بشكل خاطئ)。
   - `replica_shortfall` - ログイン してください。
     `RetentionPolicy.required_replicas` 認証済み。

   حالة خروج غير صفرية تعني نقصا نشطا حتى تتمكن اتـمتة CI/オンコール من التنبيه
   فورا。 JSON 認証 `docs/examples/da_manifest_review_template.md`
   重要です。
3. **اطلق اعادة التكرار.** عندما يبلغ التدقيق عن نقص، اصدر `ReplicationOrderV1`
   جديدا عبر ادوات الحوكمة الموصوفة في
   [SoraFS ストレージ容量マーケットプレイス](../sorafs/storage-capacity-marketplace.md)
   واعِد تشغيل التدقيق حتى تتقارب مجموعة النسخ.重要な情報を確認してください。
   CLI مع `iroha app da prove-availability` حتى يتمكن SRE のダイジェスト
   PDP。

評価 `integration_tests/tests/da/replication_policy.rs` 評価さいきょう
حزمة بارسال سياسة احتفاظ غير متطابقة الى `/v2/da/ingest` وتتحقق من ان
マニフェストは、マニフェストを表示します。