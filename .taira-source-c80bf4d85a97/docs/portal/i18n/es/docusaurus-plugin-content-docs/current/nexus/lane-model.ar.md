---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-carril-nexus
título: نموذج carriles في Nexus
descripción: تصنيف منطقي للـ lanes، هندسة التهيئة، وقواعد دمج world-state لـ Sora Nexus.
---

# carriles nuevos en Nexus y WSV

> **الحالة:** مخرج NX-1 - تصنيف carriles, هندسة التهيئة, وتخطيط التخزين جاهزة للتنفيذ.  
> **المالكون:** Nexus Grupo de trabajo central, Grupo de trabajo de gobernanza  
> **Hoja de ruta principal:** NX-1 según `roadmap.md`

Utilice el software `docs/source/nexus_lanes.md` y el SDK y el software de Sora Nexus. ارشادات carriles دون الغوص في شجرة mono-repo. تحافظ الهندسة المستهدفة على حتمية estado mundial مع السماح لمساحات البيانات (carriles) الفردية بتشغيل مجموعات مدققين عامة او خاصة باعمال معزولة.

## المفاهيم

- **Lane:** fragmento del libro mayor Nexus de la acumulación de trabajos pendientes. معرف بـ `LaneId` ثابت.
- **Espacio de datos:** حاوية حوكمة تجمع lane او اكثر تشترك في سياسات الامتثال والتوجيه والتسوية.
- **Manifiesto de carril:** بيانات وصفية تتحكم بها الحوكمة تصف المدققين وسياسة DA ورمز الغاز وقواعد التسوية واذونات التوجيه.
- **Compromiso global:** prueba تصدرها carril تلخص جذور حالة جديدة وبيانات التسوية ونقل carril cruzado اختياري. حلقة NPoS العالمية ترتب compromisos.

## carriles تصنيف

تصف انواع carriles قانونيا رؤيتها وسطح الحوكمة وhooks التسوية. Programador (`LaneConfig`) Programador de software, SDK y SDK de terceros مخصص.| نوع carril | الرؤية | عضوية المدققين | Actualizar WSV | الحوكمة الافتراضية | سياسة التسوية | الاستخدام المعتاد |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | عام | Sin permiso (participación global) | نسخة حالة كاملة | SORA Parlamento | `xor_global` | دفتر عام اساسي |
| `public_custom` | عام | Sin permiso y con participación | نسخة حالة كاملة | Participación en وحدة مرجحة بال | `xor_lane_weighted` | تطبيقات عامة عالية السعة |
| `private_permissioned` | مقيد | مجموعة مدققين ثابتة (معتمدة من الحوكمة) | Compromisos y pruebas | Consejo federado | `xor_hosted_custody` | CBDC, اعمال كونسورتيوم |
| `hybrid_confidential` | مقيد | عضوية مختلطة؛ تغلف Pruebas ZK | Compromisos + افصاح انتقائي | وحدة نقود قابلة للبرمجة | `xor_dual_fund` | نقود قابلة للبرمجة مع حفظ الخصوصية |

يجب على كل نوع carril التصريح بما يلي:

- Alias للداتاسبيس - تجميع مقروء للبشر يربط سياسات الامتثال.
- Identificador de gobernanza - معرف يحل عبر `Nexus.governance.modules`.
- Identificador de liquidación - معرف يستهلكه enrutador de liquidación لخصم مخازن XOR.
- Metadatos تيليمتري اختيارية (وصف، جهة اتصال، مجال عمل) تظهر عبر `/status` y tableros.

## carriles هندسة تهيئة (`LaneConfig`)

`LaneConfig` Este es el tiempo de ejecución de las líneas del catálogo. لا تستبدل manifiesta الحوكمة؛ بل توفر معرفات تخزين حتمية وتلميحات تيليمتري لكل lane مهيأة.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```- `LaneConfig::from_catalog` يعيد حساب الهندسة عند تحميل التهيئة (`State::set_nexus`).
- تتحول alias الى babosas بحروف صغيرة؛ تكاثف الاحرف غير الابجدية الرقمية المتتالية الى `_`. اذا نتج slug فارغ نعود الى `lane{id}`.
- `shard_id` مشتق من مفتاح metadatos `da_shard_id` (الافتراضي `lane_id`) y diario مؤشر shard المثبت للحفاظ على repetición حتمي ل DA عبر reinicia/reharding.
- Prefijos تضمن المفاتيح بقاء نطاقات مفاتيح WSV لكل lane منفصلة حتى عند مشاركة نفس backend.
- اسماء مقاطع Kura حتمية عبر anfitriones؛ يمكن للمدققين تدقيق ادلة المقاطع وmanifests دون ادوات مخصصة.
- مقاطع الدمج (`lane_{id:03}_merge`) تخزن اخر root ل merge-hint وcommitments الحالة العالمية لتلك lane.

## تقسيم estado mundial- estado mundial المنطقي لـ Nexus هو اتحاد مساحات الحالة لكل lane. carriles العامة تحفظ حالة كاملة؛ carriles الخاصة/confidencial تصدر raíces Merkle/compromiso الى fusionar libro mayor.
- La MV está conectada a 4 baterías de `LaneConfigEntry::key_prefix`, y está conectada a `[00 00 00 01] ++ PackedKey`.
- الجداول المشتركة (cuentas, activos, activadores, سجلات الحوكمة) تخزن الادخالات مجمعة حسب بادئة lane, مما يبقي escaneos de rango حتمية.
- Metadatos de fusión en el libro de contabilidad de fusión: en el carril, sugerencias de fusión de raíces y sugerencias de fusión de raíces en el carril, `lane_{id:03}_merge`, en lugar de `lane_{id:03}_merge` بالاحتفاظ او الازالة الموجهة عندما تتقاعد carril.
- فهارس cross-lane (alias de cuentas, registros de activos, manifiestos de gobernanza) تخزن بادئات lane صريحة كي يتمكن المشغلون من مطابقة الادخالات بسرعة.
- **سياسة الاحتفاظ** - carriles العامة تحتفظ باجسام الكتل كاملة؛ carriles ذات compromisos فقط يمكنها ضغط الاجسام الاقدم بعد puntos de control لان compromisos هي المرجع. carriles confidenciales.
- **Herramientas** - يجب على ادوات الصيانة (`kagami`, اوامر admin في CLI) الرجوع الى namespace ذو slug عند اظهار metrics او تسميات Prometheus او ارشفة مقاطع Kura.

## Enrutamiento y API- Soporte técnico Torii REST/gRPC y soporte `lane_id` غيابها يعني `lane_default`.
- تعرض SDKs محددات carril y تطابق alias الودية مع `LaneId` باستخدام catálogo carriles.
- تعمل قواعد enrutamiento على catálogo المعتمد ويمكنها اختيار carril y espacio de datos معا. Hay alias `LaneConfig` que se utilizan en paneles y registros.

## Liquidación والرسوم

- كل carril تدفع رسوم XOR لمجموعة المدققين العالمية. يمكن لل carriles تحصيل رموز gas محلية لكن يجب ان تودع ما يعادل XOR مع compromisos.
- pruebas التسوية تشمل المبلغ وmetadatos التحويل ودليل depósito en garantía (مثلا تحويل الى vault الرسوم العالمية).
- enrutador de liquidación الموحد (NX-3) يخصم buffers باستخدام نفس بادئات lane، بحيث تتطابق تيليمتري التسوية مع هندسة التخزين.

## Gobernanza

- تعلن carriles وحدة الحوكمة عبر catálogo. Utilice el alias `LaneConfigEntry` y el slug para crear archivos y archivos adjuntos.
- Los manifiestos de registro Nexus incluyen el carril `LaneId`, el espacio de datos, el identificador de gobernanza, el identificador de liquidación y los metadatos.
- تستمر ganchos ترقية runtime في فرض سياسات الحوكمة (`gov_upgrade_id` افتراضيا) y تسجيل diffs عبر telemetry bridge (احداث `nexus.config.diff`).

## Telemetría y estado- `/status` incluye alias de carriles, espacios de datos y identificadores, y nombres de catálogos y `LaneConfig`.
- تعرض مقاييس planificador (`nexus_scheduler_lane_teu_*`) alias/slugs ليتمكن المشغلون من ربط backlog y TEU بسرعة.
- `nexus_lane_configured_total` يحصي عدد ادخالات lane المشتقة ويعاد حسابه عند تغير التهيئة. تبعث التيليمتري diferencia entre carriles موقعة عندما تتغير هندسة.
- تتضمن indicadores pendientes للداتاسبيس alias/descripción de metadatos لمساعدة المشغلين على ربط ضغط الطوابير بالمجالات التجارية.

## التهيئة وانواع Norito

- `LaneCatalog`, `LaneConfig`, y `DataSpaceCatalog` están conectados a `iroha_data_model::nexus` y están conectados a Norito en manifiestos وSDK.
- `LaneConfig` يعيش في `iroha_config::parameters::actual::Nexus` ويشتق تلقائيا من catálogo؛ Esta es la codificación Norito del asistente de tiempo de ejecución.
- تظل تهيئة المستخدم (`iroha_config::parameters::user::Nexus`) تقبل واصفات lane anddataspace التصريحية؛ يقوم التحليل الان باشتقاق الهندسة ورفض alias غير صالحة او ID carriles مكررة.

## العمل المتبقي

- Enrutador de asentamiento de دمج تحديثات (NX-3) مع الهندسة الجديدة كي توسم خصومات وايصالات buffers XOR بslug lane.
- Herramientas para crear familias de columnas, carriles, espacios de nombres y slug.
- انهاء خوارزمية الدمج (ordenamiento, poda, detección de conflictos) y accesorios رجعية لاعادة التشغيل entre carriles.
- اضافة ganchos امتثال لـ whitelists/blacklists وسياسات النقود القابلة للبرمجة (متابعة تحت NX-12).

---* Presione el botón NX-1, NX-2 y NX-18. يرجى اظهار الاسئلة في `roadmap.md` او tracker ليبقى البوابة متوافقة مع الوثائق القانونية.*