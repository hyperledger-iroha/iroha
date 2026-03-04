---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: armazenamento de nó
título: تصميم تخزين عقدة SoraFS
sidebar_label: تصميم تخزين العقدة
description: معمارية التخزين والحصص وخطافات دورة الحياة لعُقد Torii المستضيفة لبيانات SoraFS.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/sorafs_node_storage.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة وثائق Sphinx القديمة.
:::

## تصميم تخزين عقدة SoraFS (مسودة)

Você pode usar o Iroha (Torii) para obter o valor de Iroha (Torii).
SoraFS é uma função que pode ser removida do computador e do computador. وهي تكمل مواصفة descoberta
`sorafs_node_client_protocol.md` Dispositivos elétricos de montagem para SF-1b عبر تفصيل معمارية جانب
التخزين وضوابط الموارد وتوصيلات الإعداد التي يجب أن تصل إلى العقدة ومسارات
É Torii. توجد التدريبات العملية للمشغلين في
[Runbook عمليات العقدة](./node-operations).

### الأهداف

- O valor do item Iroha é o mesmo que o Iroha. التأثير
  على مسؤوليات دفتر الأستاذ الأساسية.
- إبقاء وحدة التخزين حتمية ومدفوعة بـ Norito: manifestos وخطط القطع وجذور
  Prova de recuperabilidade (PoR) é uma prova de recuperação de dados.
- أو fetch.
- إرجاع الصحة/التليمترية (عينات PoR, زمن جلب القطع, ضغط القرص) إلى الحوكمة والعملاء.

### المعمارية عالية المستوى

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Palavras-chave:

- **Gateway**: تعرض نقاط نهاية Norito HTTP لمقترحات pin وطلبات fetch للقطع وأخذ عينات PoR والتليمترية. Verifique se o Norito está funcionando corretamente. Verifique se o endereço HTTP está no Torii.
- **Registro de Pins**: Você pode manifestar o código em `iroha_data_model::sorafs` e `iroha_core`. Não há manifest يسجل السجل digest para manifest وdigest لخطة القطع وجذر PoR وأعلام قدرات المزوّد.
- **Chunk Storage**: `ChunkStore` على القرص يستقبل manifests موقعة, ويبني خطط القطع باستخدام `ChunkProfile::DEFAULT`, ويخزن القطع في تخطيط حتمي. ترتبط كل قطعة ببصمة محتوى وبيانات PoR وصفية كي يمكن إعادة التحقق دون قراءة الملف Então.
- **Quota/Scheduler**: يفرض حدود المشغل (أقصى بايتات قرص, أقصى pins معلقة, أقصى عمليات fetch متوازية، TTL (para)) e IO (IO) não podem ser usados. O agendador é o PoR e o programador de CPU é usado.

### الإعداد

أضف قسما جديدا إلى `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # وسم اختياري مقروء
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```- `enabled`: مفتاح مشاركة. Não false تعيد البوابة 503 لنقاط نهاية التخزين ولا تعلن العقدة نفسها في descoberta.
- `data_dir`: المجلد الجذري لبيانات القطع وأشجار PoR e fetch. Código `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: Você pode usar o código de segurança. Você pode usar pinos de segurança para isso.
- `max_parallel_fetches`: O agendador do IO é configurado para ser executado no IO e no IO.
- `max_pins`: Você pode definir os pinos do manifesto como parte do arquivo/الضغط.
- `por_sample_interval_secs`: é um recurso que pode ser usado para PoR. Isso significa que `N` é um arquivo (que é o manifesto do manifesto) e que está sendo executado. O código `N` é usado para gerar metadados `profile.sample_multiplier` (ou seja, `1-4`). Não há nenhuma substituição de arquivo / نصا واحدا, أو كائنا مع substitui o valor do arquivo, como `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: بنية يستخدمها مولد adverts لملء حقول `ProviderAdvertV1` (stake pointer, إشارات QoS, tópicos). Você também pode usar o cabo de alimentação do computador para obter mais informações.

توصيلات الإعداد:

- `[sorafs.storage]` é usado para `iroha_config` para `SorafsStorage` e não pode ser removido.
- `iroha_core` و`iroha_torii` بتمرير إعداد التخزين إلى Builder الخاص بالبوابة ومخزن القطع عند sim.
- توجد substitui للتطوير/الاختبار (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), لكن نشر الإنتاج يجب أن يعتمد على ملف الإعداد.

### CLI

Você pode usar Torii HTTP para criar uma caixa `sorafs_node` e CLI para criar uma caixa 【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` usa o manifesto `.to` para substituir Norito por carga útil. Você pode usar o chunking do manifesto, o resumo do resumo, o resumo e o resumo A configuração JSON do `chunk_fetch_specs` é usada para definir o valor do arquivo no site.
- `export` gera o manifesto e o manifest/payload do arquivo (o JSON é definido) para que você possa usá-lo إنتاج fixtures عبر البيئات.

Você pode usar Norito JSON para stdout, mas não usar scripts. تغطي اختبارات التكامل الـ CLI لضمان أن manifests وpayloads تعمل round-trip بشكل صحيح قبل وصول واجهات Torii.【crates/sorafs_node/tests/cli.rs:1】

> HTTP
>
> تعرض بوابة Torii الآن مساعدات قراءة فقط مدعومة بنفس `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — Crie o manifesto Norito (base64) em digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — يعيد خطة القطع الحتمية JSON (`chunk_fetch_specs`) para downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> تعكس هذه النقاط خرج CLI بحيث يمكن للخطوط التحويل من scripts محلية إلى فحوصات HTTP دون تغيير المحللات.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### دورة حياة العقدة1. **Efetuar**:
   - عند تفعيل التخزين تهيئ العقدة مخزن القطع بالمجلد والسعة المهيأة. يشمل ذلك التحقق أو إنشاء قاعدة بيانات PoR للـ manifesto e إعادة تشغيل manifestos المثبتة لتسخين الكاش.
   - تسجيل مسارات بوابة SoraFS (não Norito JSON POST/GET para pin وfetch وأخذ عينات PoR والتليمترية).
   - تشغيل عامل أخذ عينات PoR ومراقب الحصص.
2. **Descoberta/Anúncios**:
   - توليد مستندات `ProviderAdvertV1` باستخدام السعة/الصحة الحالية, وتوقيعها بالمفتاح المعتمد من المجلس, ونشرها عبر قناة descoberta. Verifique se o `profile_aliases` é compatível com o produto e com o produto.
3. ** alfinete **:
   - تستقبل البوابة manifesto موقعا (يشمل خطة القطع وجذر PoR وتواقيع المجلس). Você pode usar aliases (`sorafs.sf1@1.0.0` مطلوب) e obter o manifesto do arquivo.
   - تحقق من الحصص. Se você precisar de um pino/pino, você deve usar um pino (Norito).
   - بث بيانات القطع إلى `ChunkStore` مع التحقق من digests أثناء الإدخال. Você pode usar PoR e metadados no manifesto do registro.
4. **تدفق buscar**:
   - تقديم طلبات نطاق القطع من القرص. Use o agendador `max_parallel_fetches` e `429`.
   - إصدار تليمترية منظمة (Norito JSON) تتضمن زمن الاستجابة والبايتات المخدومة وعدادات Coloque-o no lugar.
5. **أخذ عينات PoR**:
   - يختار العامل manifesta بنسبة للوزن (مثل البايتات المخزنة) ويجري أخذ عينات حتميا باستخدام شجرة PoR الخاصة بمخزن القطع.
   - حفظ النتائج لأغراض تدقيق الحوكمة وإدراج الملخصات في anúncios الخاصة بالمزوّد/نقاط التليمترية.
6. **الإخلاء/تطبيق الحصص**:
   - عند بلوغ السعة ترفض العقدة pinos الجديدة افتراضيا. Não há nenhum problema de segurança (como TTL ou LRU) sem nenhum problema. حاليا يفترض التصميم حصصا صارمة وعمليات unpin يطلقها المشغل.

### تكامل إعلان السعة والجدولة- Você pode usar Torii para `CapacityDeclarationRecord` para `/v1/sorafs/capacity/declare` ou `CapacityManager` Verifique se o chunker/lane está no local certo. يكشف المدير لقطات read-only للتليمترية (`GET /v1/sorafs/capacity/state`) ويفرض حجوزات لكل ملف تعريف أو lane قبل قبول أوامر جديدة.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Altere o endpoint `/v1/sorafs/capacity/schedule` para `ReplicationOrderV1` no final. عندما يستهدف الأمر المزوّد المحلي, يتحقق المدير من التكرار, ويفحص سعة chunker/lane, ويحجز الشريحة, O `ReplicationPlan` é usado para orquestração e orquestração. يتم الإقرار بالأوامر الخاصة بمزوّدين آخرين باستجابة `ignored` لتسهيل سير العمل متعدد مشغلين.【crates/iroha_torii/src/routing.rs:4845】
- تقوم hooks الإكمال (مثل ما يحدث بعد نجاح الإدخال) باستدعاء `POST /v1/sorafs/capacity/complete` لإطلاق الحجوزات عبر `CapacityManager::complete_order`. Use o `ReplicationRelease` (alteração e chunker/lane) para orquestrar a orquestração Votação em janeiro. O que você precisa saber é que você pode fazer isso sem parar. 【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Use o `TelemetryAccumulator` para usar o `NodeHandle::update_telemetry`, o que significa que você pode usar o PoR/uptime e o tempo de atividade O programador `CapacityTelemetryV1` é o planejador interno.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### التكاملات والعمل المستقبلي

- **الحوكمة**: توسيع `sorafs_pin_registry_tracker.md` بتليمترية التخزين (معدل نجاح PoR, استغلال القرص). Não há anúncios de PoR em anúncios.
- **SDKs للعملاء**: تعريض إعداد التخزين الجديد (حدود القرص, alias) لتمكين أدوات الإدارة من bootstrap العقد برمجيا.
- **التليمترية**: دمج مع مكدس المقاييس الحالي (Prometheus / OpenTelemetry) حتى تظهر مقاييس التخزين في لوحات المراقبة.
- **الأمان**: تشغيل وحدة التخزين داخل مجموعة مهام async مخصصة مع contrapressão, والنظر em sandboxing لقراءات القطع عبر io_uring أو مجموعات tokio المحدودة لمنع العملاء الخبيثين من استنزاف الموارد.

يحافظ هذا التصميم على اختيارية وحدة التخزين وحتميتها, ويمنح المشغلين المفاتيح اللازمة A chave de segurança é SoraFS. A solução de problemas é `iroha_config` e `iroha_core` e `iroha_torii` e Norito, Verifique se há algum problema com isso.