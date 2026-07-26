---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo nexus-lane
título: Lanes em Nexus
description: تصنيف منطقي للـ lanes, هندسة التهيئة, وقواعد دمج estado mundial para Sora Nexus.
---

# Coloque pistas em Nexus e WSV

> **الحالة:** مخرج NX-1 - تصنيف pistas, هندسة التهيئة, وتخطيط التخزين جاهزة للتنفيذ.  
> **المالكون:** Nexus Core WG, Governance WG  
> **Roteiro principal:** NX-1 em `roadmap.md`

Você pode usar o `docs/source/nexus_lanes.md` para usar o Sora Nexus e o SDK e o SDK Coloque lanes no seu mono-repo. تحافظ الهندسة المستهدفة على حتمية estado mundial مع السماح لمساحات البيانات (pistas) الفردية بتشغيل مجموعات مدققين عامة او خاصة باعمال معزولة.

## المفاهيم

- **Lane:** shard منطقي من ledger Nexus مع مجموعة مدققين خاصة به وbacklog تنفيذ. A chave é `LaneId`.
- **Espaço de dados:** حاوية حوكمة تجمع lane او اكثر تشترك في سياسات الامتثال والتوجيه والتسوية.
- **Lane Manifest:** واذونات التوجيه.
- **Compromisso Global:** prova تصدرها lane تلخص جذور حالة جديدة وبيانات التسوية ونقل cross-lane اختياري. حلقة NPoS العالمية ترتب compromissos.

## تصنيف pistas

Verifique as pistas e os ganchos. هندسة التهيئة (`LaneConfig`) تلتقط هذه سمات كي تتمكن وSDKs والادوات من فهم التخطيط Eu não sei.

| Nenhuma pista | الرؤية | عضوية المدققين | WSV | الحوكمة الافتراضية | سياسة التسوية | الاستخدام المعتاد |
|-----------|------------|-----------|-------------|--------------------|-------------------|-------------|
| `default_public` | عام | Sem permissão (participação global) | نسخة حالة كاملة | SORA Parlamento | `xor_global` | دفتر عام اساسي |
| `public_custom` | عام | Sem permissão e com controle de estaca | نسخة حالة كاملة | Participação em participação | `xor_lane_weighted` | Máquinas de lavar roupa |
| `private_permissioned` | Música | مجموعة مدققين ثابتة (معتمدة من الحوكمة) | Compromissos e provas | Conselho Federado | `xor_hosted_custody` | CBDC, كونسورتيوم |
| `hybrid_confidential` | Música | عضوية مختلطة؛ Provas ZK | Compromissos + افصاح انتقائي | وحدة نقود قابلة للبرمجة | `xor_dual_fund` | Máquinas de lavar roupa para uso doméstico |

يجب على كل نوع lane التصريح بما يلي:

- Alias للداتاسبيس - تجميع مقروء للبشر يربط سياسات الامتثال.
- Identificador de governança - معرف يحل عبر `Nexus.governance.modules`.
- Identificador de liquidação - معرف يستهلكه roteador de liquidação لخصم مخازن XOR.
- Metadados de metadados (ou seja, um arquivo de metadados) para `/status` e dashboards.

## هندسة تهيئة pistas (`LaneConfig`)

`LaneConfig` é um tempo de execução baseado em faixas de catálogo. لا تستبدل manifestos الحوكمة؛ بل توفر معرفات تخزين حتمية e تلميحات تيليمتري لكل lane مهيأة.

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
```

- `LaneConfig::from_catalog` não é compatível com o software (`State::set_nexus`).
- تتحول aliases para slugs بحروف صغيرة؛ Verifique o valor do código `_`. Este slug está localizado no `lane{id}`.
- `shard_id` مشتق من مفتاح metadata `da_shard_id` (الافتراضي `lane_id`) ويقود Journal مؤشر shard المثبت للحفاظ A repetição não é necessária para reiniciar/resharding.
- تضمن prefixes المفاتيح بقاء نطاقات مفاتيح WSV لكل lane منفصلة حتى عند مشاركة نفس backend.
- اسماء مقاطع Kura حتمية عبر hosts؛ يمكن للمدققين تدقيق ادلة المقاطع e manifestos دون ادوات مخصصة.
- مقاطع الدمج (`lane_{id:03}_merge`) تخزن اخر raízes para merge-hint ecommitments الحالة العالمية لتلك lane.

## تقسيم estado mundial- estado mundial المنطقي لـ Nexus هو اتحاد مساحات الحالة لكل lane. lanes العامة تحفظ حالة كاملة؛ lanes الخاصة/confidencial تصدر raízes Merkle/commitment e merge ledger.
- Verifique MV يسبق كل مفتاح ببادئة 4 vezes em `LaneConfigEntry::key_prefix`, منتجا مفاتيح مثل `[00 00 00 01] ++ PackedKey`.
- الجداول المشتركة (contas, ativos, gatilhos, سجلات الحوكمة) تخزن الادخالات مجمعة حسب بادئة lane, مما يبقي varreduras de alcance Bem.
- تعكس metadata الـ merge-ledger نفس التخطيط: كل lane تكتبroots merge-hint وroots العالمية المخفضة الى `lane_{id:03}_merge`, ما يسمح بالاحتفاظ او الازالة الموجهة عندما تتقاعد pista.
- فهارس cross-lane (aliases de contas, registros de ativos, manifestos de governança).
- **سياسة الاحتفاظ** - pistas العامة تحتفظ باجسام الكتل كاملة؛ faixas ذات compromissos فقط يمكنها ضغط الاجسام الاقدم بعد pontos de verificação لان compromissos هي المرجع. faixas confidenciais تحتفظ بسجلات مشفرة في مقاطع مخصصة كي لا تعيق workloads اخرى.
- **Ferramentas** - Use o namespace do namespace e o slug com métricas e métricas Prometheus e ارشفة مقاطع Kura.

## Roteamento e APIs

- تقبل نقاط النهاية Torii REST/gRPC `lane_id` اختيارية؛ Este é `lane_default`.
- Crie SDKs com pista e aliases com `LaneId` para criar pistas de catálogo.
- تعمل قواعد roteamento على catalog المعتمد ويمكنها اختيار lane e dataspace معا. Os aliases `LaneConfig` são encontrados em painéis e logs.

## Liquidação والرسوم

- كل lane تدفع رسوم XOR لمجموعة المدققين العالمية. يمكن لل lanes تحصيل رموز gás محلية لكن يجب ان تودع ما يعادل XOR مع compromissos.
- provas de depósito e metadados e depósito (مثلا تحويل الى vault الرسوم العالمية).
- roteador de liquidação الموحد (NX-3) يخصم buffers باستخدام نفس بادئات lane, بحيث تتطابق تيليمتري التسوية مع هندسة التخزين.

## Governança

- تعلن pistas وحدة الحوكمة عبر catálogo. تحمل `LaneConfigEntry` alias وslug الاصليين لابقاء تيليمتري ومسارات التدقيق مقروءة.
- يوزع Nexus manifestos de registro موقعة للـ lane تشمل `LaneId` وربط espaço de dados e identificador de governança e identificador de liquidação e metadados.
- تستمر hooks الترقية runtime في فرض سياسات الحوكمة (`gov_upgrade_id` افتراضيا) وتسجيل diffs عبر telemetry bridge (احداث `nexus.config.diff`).

## Telemetria وstatus

- يكشف `/status` عن aliases para pistas وربط dataspaces وhandles الحوكمة وملفات التسوية, مشتقة من catalog و`LaneConfig`.
- تعرض مقاييس agendador (`nexus_scheduler_lane_teu_*`) aliases/slugs ليتمكن المشغلون من ربط backlog وضغط TEU بسرعة.
- `nexus_lane_configured_total` يحصي عدد ادخالات lane المشتقة ويعاد حسابه عند تغير التهيئة. تبعث التيليمتري diffs موقعة عندما تتغير هندسة pistas.
- تتضمن medidores backlog للداتاسبيس metadados alias/descrição لمساعدة المشغلين على ربط ضغط الطوابير بالمجالات التجارية.

## Nome de usuário Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` são usados ​​em `iroha_data_model::nexus` e podem ser encontrados em Norito Isso manifesta e SDKs.
- `LaneConfig` يعيش في `iroha_config::parameters::actual::Nexus` ويشتق تلقائيا من catalog; A codificação Norito não é um auxiliar para o tempo de execução.
- تظل تهيئة المستخدم (`iroha_config::parameters::user::Nexus`) تقبل واصفات lane وdataspace التصريحية؛ يقوم التحليل الان باشتقاق الهندسة ورفض aliases غير صالحة او IDs pistas مكررة.

## العمل المتبقي

- O roteador de liquidação (NX-3) é um roteador de liquidação que armazena buffers XOR ou slug lane.
- توسيع tooling الادارية لسرد colunas famílias وضغط lanes المتقاعدة وفحص سجلات الكتل لكل lane باستخدام namespace ذي slug.
- انهاء خوارزمية الدمج (ordenação, poda, detecção de conflitos) e luminárias رجعية لاعادة التشغيل cross-lane.
- اضافة hooks امتثال لـ whitelists/blacklists وسياسات النقود القابلة للبرمجة (متابعة تحت NX-12).

---

*É necessário que o NX-1 seja compatível com o NX-1 e o NX-2 com o NX-18. Use o rastreador `roadmap.md` e o rastreador para obter informações sobre como usar o rastreador. القانونية.*