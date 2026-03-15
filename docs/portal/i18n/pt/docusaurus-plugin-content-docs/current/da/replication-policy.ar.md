---
lang: pt
direction: ltr
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
`docs/source/da/replication_policy.md`. ابق النسختين متزامنتين حتى يتم
سحب الوثائق القديمة.
:::

# سياسة تكرار توفر البيانات (DA-4)

_الحالة: قيد التنفيذ -- Nome: Core Protocol WG / Storage Team / SRE_

يطبق خط انابيب ingest الخاص بـ DA اهداف احتفاظ حتمية لكل فئة blob مذكورة في
`roadmap.md` (referência DA-4). Use Torii para obter mais informações
Você pode fazer isso com uma chave de fenda / uma chave de fenda
Não há nada de errado com você.

## السياسة الافتراضية

| blob | Quente | Frio | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|----------|------------|-------------|----------------|-------------|-------------|
| `taikai_segment` | 24 dias | 14 dias | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 dias | 7 dias | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 dias | 180 dias | 3 | `cold` | `da.governance` |
| _Padrão (كل الفئات الاخرى)_ | 6 dias | 30 dias | 3 | `warm` | `da.default` |

Você pode usar o `torii.da_ingest.replication_policy` para obter mais informações
طلبات `/v1/da/ingest`. يعيد Torii كتابة manifestos مع ملف الاحتفاظ المفروض ويصدر
Faça o download do pacote de software com SDKs
المتقادمة.

### فئات توفر Taikai

Manifestos توجيه Taikai (`taikai.trm`) عن `availability_class`
(`hot`, `warm`, e `cold`). Use Torii para obter mais informações
للمشغلين توسيع عدد النسخ لكل stream دون تعديل الجدول العام. Informações:

| فئة التوفر | Quente | Frio | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|------------|------------|-------------|----------------|-------------|-------------|
| `hot` | 24 dias | 14 dias | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 dias | 30 dias | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 dia | 180 dias | 3 | `cold` | `da.taikai.archive` |

O código `hot` deve ser removido do computador. قم
بتجاوز الافتراضيات عبر
`torii.da_ingest.replication_policy.taikai_availability` `torii.da_ingest.replication_policy.taikai_availability` `torii.da_ingest.replication_policy.taikai_availability`
Não há problema.

## الاعداد

Use o `torii.da_ingest.replication_policy` para definir *default*
مصفوفة substitui o فئة. معرفات الفئة غير حساسة لحالة الاحرف وتقبل
`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, e `custom:<u16>`
للامتدادات المعتمدة حوكما. O código é `hot`, `warm`, e `cold`.

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

Faça isso com a ajuda de uma pessoa. لتشديد فئة, حدّث substituir
المطابق؛ Verifique se o dispositivo está conectado a `default_retention`.

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

## دلالات الانفاذ

- يستبدل Torii `RetentionPolicy` الذي يقدمه المستخدم بالملف المفروض قبل التقسيم
  E o manifesto.
- ترفض manifestos المبنية مسبقا التي تعلن ملف احتفاظ غير مطابق بـ
  `400 schema mismatch` é uma ferramenta que pode ser usada para evitar problemas.
- Não é possível substituir a substituição (`blob_class`, a opção de substituição)
  لاظهار المتصلين غير الملتزمين اثناء rollout.

راجع [خطة ingerir لتوفر البيانات](ingest-plan.md) (قائمة التحقق) للبوابة المحدثة
Não use nada.

## سير عمل اعادة التكرار (متابعة DA-4)

Não há nada que você possa fazer. يجب على المشغلين ايضا اثبات ان manifestos
A solução de problemas de segurança do computador é o SoraFS com o nome SoraFS
Deixe os blobs de lado.

1. **راقب الانحراف.** Torii
   `overriding DA retention policy to match configured network baseline` `overriding DA retention policy to match configured network baseline`
   يرسل المتصل قيما قديمة للاحتفاظ. قرن هذا السجل مع قياسات
   `torii_sorafs_replication_*` não pode ser usado nem removido.
2. **فرق النية مقابل النسخ الحية.** استخدم مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Use o `torii.da_ingest.replication_policy` para obter mais informações
   Como definir o manifesto (JSON e Norito) e como definir payloads
   `ReplicationOrderV1` é um resumo do manifesto. يلخص الشرطان التاليان:

   - `policy_mismatch` - ملف الاحتفاظ في manifest يختلف عن السياسة المفروضة
     (Não há nenhum problema com o Torii).
   - `replica_shortfall` - امر التكرار الحي يطلب نسخا اقل من
     `RetentionPolicy.required_replicas` e eu não consigo mais.حالة خروج غير صفرية تعني نقصا نشطا حتى تتمكن اتـمتة CI/on-call من التنبيه
   فورا. Definir JSON usando `docs/examples/da_manifest_review_template.md`
   لتصويت البرلمان.
3. **اطلق اعادة التكرار.** عندما يبلغ التدقيق عن نقص, اصدر `ReplicationOrderV1`
   جديدا عبر ادوات الحوكمة الموصوفة في
   [Mercado de capacidade de armazenamento SoraFS](../sorafs/storage-capacity-marketplace.md)
   Verifique se o produto está funcionando corretamente. للتجاوزات الطارئة, اربط مخرجات
   CLI com `iroha app da prove-availability` fornece SREs para o resumo
   e PDP.

A solução de problemas é `integration_tests/tests/da/replication_policy.rs`; تقوم
A solução de problemas de segurança do computador é o `/v1/da/ingest` e o valor de `/v1/da/ingest`
manifesto. يعرض الملف المفروض بدلا من نية المتصل.