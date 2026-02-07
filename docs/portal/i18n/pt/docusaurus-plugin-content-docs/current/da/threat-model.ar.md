---
lang: pt
direction: ltr
source: docs/portal/docs/da/threat-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
`docs/source/da/threat_model.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# نموذج تهديدات توفر البيانات em Sora Nexus

_اخر مراجعة: 2026-01-19 -- المراجعة القادمة المجدولة: 2026-04-19_

وتيرة الصيانة: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). يجب ان تظهر كل
A chave `status.md` pode ser usada para remover a água e o cabo.

## الهدف والنطاق

Disponibilidade de dados (DA) em Taikai e blobs Nexus e blobs
Certifique-se de que o produto esteja funcionando corretamente e sem problemas. هذا النموذج
يثبت عمل الهندسة لDA-1 (البنية ونموذج التهديدات) ويعد خط اساس لمهام DA اللاحقة
(DA-2 em vez de DA-10).

Nome de usuário:
- Ingerir em Torii e metadados Norito.
- اشجار تخزين blobs المدعومة بSoraFS (طبقات quente/frio) وسياسات التكرار.
- تعهدات كتل Nexus (formatos de transmissão, provas, APIs de cliente leve).
- aplicação de ganchos لPDP/PoTR الخاصة بحمولات DA.
- تدفقات المشغل (fixação, despejo, corte) وخطوط الرصد.
- موافقات الحوكمة التي تقبل او تزيل مشغلي DA والمحتوى.

خارج النطاق لهذا المستند:
- نمذجة الاقتصاد الكاملة (ضمن مسار DA-7).
- Coloque o SoraFS no lugar do SoraFS.
- O SDK do SDK está disponível para download.

## نظرة عامة معمارية

1. **Efetuar:** Adicione blobs à API ingest como Torii. تقوم العقدة
   Usando blobs, e manifestando Norito (blob, lane, época, codec de código),
   Os pedaços são cortados no SoraFS.
2. **الاعلان:** تنتشر نوايا pin وتلميحات التكرار الى مزودي التخزين عبر
   registro (mercado SoraFS) pode ser usado para quente/frio.
3. **الالتزام:** يدرج sequenciadores em Nexus armazenam blobs (CID + جذور KZG اختيارية)
   Isso é tudo. Obter clientes leves, hash, dados e metadados
   A disponibilidade depende da disponibilidade.
4. **التكرار:** تسحب عقد التخزين الحصص/chunks المخصصة, وتلبي تحديات PDP/PoTR,
   وتقوم بترقية البيانات بين طبقات quente ou frio حسب السياسة.
5. **الجلب:** يسترجع المستهلكون البيانات عبر SoraFS e بوابات DA-aware مع
   As provas são as mais importantes para você.
6. **الحوكمة:** يوافق البرلمان ولجنة الاشراف على DA على المشغلين وجداول aluguel
   وتصعيدات aplicação. تحفظ ادوات الحوكمة عبر نفس مسار DA لضمان شفافية
   العملية.

## الاصول والمالكون

مقياس الاثر: **حرج** يكسر سلامة/حيوية الدفتر؛ **عال** يحجب backfill DA e
العملاء؛ **متوسط** يخفض الجودة لكنه قابل للاسترجاع؛ **منخفض** اثر محدود.| الاصل | الوصف | السلامة | الاتاحة | السرية | المالك |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Blobs Taikai e lane وادوات الحوكمة المخزنة em SoraFS | حرج | حرج | Medição | DA WG / Equipe de Armazenamento |
| Manifestos Norito DA | Metadados مصنفة تصف blobs | حرج | sim | Medição | GT de Protocolo Central |
| تعهدات الكتل | CIDs + جذور KZG داخل كتل Nexus | حرج | sim | منخفض | GT de Protocolo Central |
| Projeto PDP/PoTR | وتيرة aplicação لنسخ DA | sim | sim | منخفض | Equipe de armazenamento |
| سجل المشغلين | Máquinas de lavar roupa | sim | sim | منخفض | Conselho de Governança |
| Aluguer de casas e serviços | Máquinas de lavar roupa DA e equipamentos | sim | Medição | منخفض | GT Tesouraria |
| لوحات الرصد | SLOs DA | Medição | sim | منخفض | SRE / Observabilidade |
| Notícias | طلبات لاعادة ترطيب pedaços المفقودة | Medição | Medição | منخفض | Equipe de armazenamento |

## الخصوم والقدرات

| الفاعل | القدرات | الدوافع | الملاحظات |
| --- | --- | --- | --- |
| عميل خبيث | Os blobs são definidos como manifestos para serem ingeridos por DoS ou ingeridos. | تعطيل بث Taikai, حقن بيانات غير صحيحة. | Não há problema. |
| Máquinas de lavar louça | اسقاط نسخ مخصصة, تزوير provas PDP/PoTR, تواطؤ. | تقليص احتفاظ DA, تجنب aluguel, احتجاز البيانات. | Não se preocupe, você pode fazer isso. |
| sequenciador مخترق | Você pode alterar os metadados. | اخفاء ارسال DA, خلق عدم اتساق. | محدود باغلبية الاجماع. |
| مشغل داخلي | Certifique-se de que você está usando o telefone para obter mais informações. | مكسب اقتصادي, تخريب. | وصول الى بنية quente/frio. |
| خصم شبكة | تقسيم العقد, تاخير التكرار, حقن حركة MITM. | خفض الاتاحة, تدهور SLOs. | Não use TLS para obter informações/serviços. |
| مهاجم الرصد | Você pode usar/descompactar. | اخفاء انقطاعات DA. | يتطلب وصولا لخط التليمترية. |

## حدود الثقة

- **حد الدخول:** العميل الى امتداد DA is Torii. يتطلب auth على مستوى الطلب،
  تحديد المعدل, والتحقق من payload.
- **حد التكرار:** عقد التخزين تتبادل pedaços e provas. العقد متصادقة لكنها قد
  تتصرف بشكل بيزنطي.
- **حد الدفتر:** بيانات الكتلة الملتزمة مقابل التخزين خارج السلسلة. الاجماع
  يحمي السلامة, لكن الاتاحة تتطلب aplicação خارج السلسلة.
- **حد الحوكمة:** قرارات Conselho/Parlamento لاعتماد المشغلين والميزانيات
  e. الاختراق هنا يؤثر مباشرة على نشر DA.
- **حد الرصد:** جمع métricas/logs المصدرة الى لوحات/تنبيهات. العبث يخفي
  الانقطاعات او الهجمات.

## سيناريوهات التهديد والضوابط

### هجمات مسار ingerir

**Efetuar:** Você pode usar payloads Norito e blobs para obter mais informações
Os metadados e os metadados estão disponíveis.**الضوابط**
- تحقق صارم من esquema Norito مع تفاوض الاصدار؛ رفض الاعلام غير المعروفة.
- Verifique o valor do endpoint ingest em Torii.
- Aumentar o tamanho do pedaço e usar o chunker SoraFS.
- خط القبول لا يحفظ manifests الا بعد تطابق checksum السلامة.
- Replay cache حتمي (`ReplayCache`) يتتبع نوافذ `(lane, epoch, sequence)`،
  يحفظ marcas de água alta
  aproveita propriedade وfuzz تغطي impressões digitais مختلفة وارسال خارج الترتيب.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**الثغرات المتبقية**
- يجب ان يمر Torii ingest replay cache في مسار القبول ويحفظ مؤشرات seqüência عبر
  اعادة التشغيل.
- مخططات Norito DA تملك الان fuzz chicote مخصصا (`fuzz/da_ingest_schema.rs`) لفحص
  ثوابت codificar/decodificar; Não há nenhum problema com isso.

### احتجاز التكرار

**السيناريو:** مشغلو التخزين البيزنطيون يقبلون مهام pin لكنهم يسقطون pedaços,
Você pode usar PDP/PoTR para obter informações e recursos.

**الضوابط**
- جدول تحديات PDP/PoTR يمتد لحمولات DA مع تغطية لكل época.
- تكرار متعدد المصادر مع عتبات quorum; orquestrador يكتشف shards المفقودة
  ويطلق اصلاحات.
- cortando حوكمي مرتبط بفشل provas والنسخ المفقودة.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) Recibos de dinheiro
  Commitments DA (SignedBlockWire, `.norito`, e JSON) e agrupar bundle JSON
  للحوكمة, وتفشل عند فقدان/عدم تطابق tickets no Alertmanager تنبيهات.

**الثغرات المتبقية**
- chicote de fios em `integration_tests/src/da/pdp_potr.rs` (مغطى عبر
  `integration_tests/tests/da/pdp_potr_simulation.rs`)
  Você pode usar o PDP/PoTR para obter mais informações. استمر في
  توسيعه مع DA-5 é uma prova de prova.
- سياسة despejo لطبقة frio تحتاج سجل تدقيق موقع لمنع الاسقاطات الخفية.

### العبث بالتعهدات

**السيناريو:** sequenciador مخترق ينشر كتل تحذف او تغير تعهدات DA, ما يسبب فشل
fetch e pode ser usado para clientes leves.

**الضوابط**
- الاجماع يطابق مقترحات الكتل مع قوائم ارسال DA; النظراء يرفضون المقترحات
  Você pode fazer isso sem problemas.
- clientes leves não possuem provas de inclusão, mas manipulam o fetch.
- سجل تدقيق يقارن recibos الارسال بتعهدات الكتل.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) Recibos de dinheiro
  Commitments DA (SignedBlockWire, `.norito`, e JSON) e agrupar bundle JSON
  للحوكمة, وتفشل عند فقدان/عدم تطابق tickets no Alertmanager تنبيهات.

**الثغرات المتبقية**
- مغطاة بمهمة المصالحة + gancho Alertmanager; Como definir o pacote JSON
  كافتراضي.

### تقسيم الشبكة والرقابة

**السيناريو:** خصم يقسم شبكة التكرار, مانعا العقد من الحصول على pedaços
A solução e o problema são PDP/PoTR.

**الضوابط**
- متطلبات مزودين multi-região تضمن مسارات شبكة متنوعة.
- Não há jitter e fallback que possam causar problemas.
- لوحات الرصد تراقب عمق التكرار ونجاح التحديات وزمن fetch مع عتبات تنبيه.

**الثغرات المتبقية**
- محاكاة التقسيم لاحداث Taikai المباشرة ما زالت مفقودة؛ Faça testes de imersão.
- سياسة حجز سعة اصلاح لم توثق بعد.

### اساءة داخلية**السيناريو:** مشغل لديه وصول للregistry يتلاعب بسياسات الاحتفاظ, ويمنح
whitelist لمزودين خبيثين, او يخفي التنبيهات.

**الضوابط**
- اعمال الحوكمة تتطلب تواقيع متعددة الاطراف وسجلات Norito موثقة.
- تغييرات السياسة تصدر احداثا للرصد وسجلات ارشيف.
- خط الرصد يفرض سجلات Norito somente acréscimo com encadeamento de hash.
- اتمتة مراجعة الوصول ربع السنوية (`cargo xtask da-privilege-audit`) تتفقد
  Como manifesto/repetição (مع مسارات يحددها المشغلون), e وتعلم العناصر المفقودة/
  غير-دليل/قابلة للكتابة عالميا, وتنتج bundle JSON موقع للوحات الحوكمة.

**الثغرات المتبقية**
- ادلة العبث في اللوحات تحتاج snapshots موقعة.

## سجل المخاطر المتبقية

| الخطر | الاحتمال | الاثر | المالك | خطة التخفيف |
| --- | --- | --- | --- | --- |
| Repetir manifestos DA e sequência de cache em DA-2 | ممكن | Medição | GT de Protocolo Central | Usando cache de sequência + nonce em DA-2; Faça isso com cuidado. |
| تواطؤ PDP/PoTR عند اختراق >f عقد | غير محتمل | sim | Equipe de armazenamento | اشتقاق جدول تحديات جديد مع amostragem entre provedores; Certifique-se de usar o arnês. |
| فجوة تدقيق despejo em frio | ممكن | sim | Equipe SRE/Armazenamento | ربط سجلات موقعة ووصولات despejo on-chain عمليات; الرصد عبر اللوحات. |
| sequenciador de software | ممكن | sim | GT de Protocolo Central | `cargo xtask da-commitment-reconcile` não recebe recibos e compromissos (SignedBlockWire/`.norito`/JSON) e envia recibos para obter tickets e tickets متطابقة. |
| مقاومة التقسيم لبث Taikai المباشر | ممكن | حرج | Rede TL | تنفيذ تدريبات تقسيم; حجز سعة اصلاح؛ توثيق SOP للفشل. |
| Produtos de limpeza | غير محتمل | sim | Conselho de Governança | `cargo xtask da-privilege-audit` é um arquivo (dirs manifest/replay + مسارات اضافية) com JSON موقع + gate لوحة؛ تثبيت artefatos التدقيق على السلسلة. |

## المتابعات المطلوبة

1. Selecione Norito para obter o DA e o arquivo (منقولة الى DA-2).
2. Limpe o cache de repetição do Torii DA ingest e a sequência do arquivo para a sequência.
3. **مكتمل (2026-02-05):** chicote de fios PDP/PoTR يمارس تواطؤ + تقسيم مع نمذجة
   QoS do backlog; راجع `integration_tests/src/da/pdp_potr.rs` (مع اختبارات في
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para laptop e laptop
   Não há problema.
4. **مكتمل (2026-05-29):** `cargo xtask da-commitment-reconcile` recibos de dinheiro
   Commitments DA (SignedBlockWire/`.norito`/JSON)
   `artifacts/da/commitment_reconciliation.json`, ومربوط بAlertmanager/حزم
   Não há omissão/adulteração (`xtask/src/da.rs`).
5. **مكتمل (2026-05-29):** `cargo xtask da-privilege-audit` يتجول في carretel
   manifest/replay (مع مسارات يحددها المشغلون), ويحدد العناصر المفقودة/غير
   دليل/قابلة للكتابة عالميا, وينتج bundle JSON موقع للحوكمة
   (`artifacts/da/privilege_audit.json`).

**Isso é:**- cache de repetição واستمرار المؤشرات تم انجازهما em DA-2. راجع التنفيذ في
  `crates/iroha_core/src/da/replay_cache.rs` (cache de cache) e Torii em
  `crates/iroha_torii/src/da/ingest.rs` O número verifica a impressão digital em `/v1/da/ingest`.
- Transmitir streaming PDP/PoTR تمارس عبر aproveitar fluxo de prova aqui
  `crates/sorafs_car/tests/sorafs_cli.rs`, um dispositivo de teste para PoR/PDP/PoTR e um dispositivo PoR/PDP/PoTR
  O código de barras está disponível para download.
- Capacidade de نتائج e imersão de reparo موجودة في
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, بينما مصفوفة embeber Sumeragi
  O código está em `docs/source/sumeragi_soak_matrix.md` (não disponível).
  Existem artefatos que você pode usar em qualquer lugar.
- Atribuição de privilégios + auditoria de privilégios em `docs/automation/da/README.md`
  والاوامر الجديدة `cargo xtask da-commitment-reconcile` /
  `cargo xtask da-privilege-audit`; استخدم المخارج الافتراضية تحت
  `artifacts/da/` não é compatível com o produto.

## Melhorar a QoS e QoS (2026-02)

Para usar o DA-1 #3, um chicote de fios de segurança para PDP/PoTR pode ser usado
`integration_tests/src/da/pdp_potr.rs` (مغطي بواسطة
`integration_tests/tests/da/pdp_potr_simulation.rs`). يقوم arnês بتوزيع العقد
على ثلاث مناطق, ويحقن التقسيم/التواطؤ حسب احتمالات roadmap, ويتتبع تاخر PoTR,
O backlog de pendências não é quente. تشغيل السيناريو
Período (12 épocas, 18 anos PDP + نافذتان PoTR por época) انتج المقاييس
Nome:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| المقياس | القيمة | الملاحظات |
| --- | --- | --- |
| Falhas de PDP detectadas | 48/49 (98,0%) | As partições mais importantes Isso pode causar instabilidade e jitter. |
| PDP significa latência de detecção | 0,0 épocas | يتم اظهار الاعطال ضمن época do ano. |
| Falhas PoTR detectadas | 28/77 (36,4%) | يتم اطلاق الاكتشاف عند فقدان العقدة >=2 نوافذ PoTR, مما يترك معظم الاحداث في سجل المخاطر المتبقية. |
| PoTR significa latência de detecção | Épocas 2.0 | يطابق عتبة تاخر مقدارها época مضمنة في تصعيد الارشفة. |
| Pico na fila de reparos | 38 manifestos | يرتفع backlog عندما تتكدس partições اسرع من اربع اصلاحات متاحة لكل época. |
| Latência de resposta p95 | 30.068ms | O intervalo de 30 dias com jitter +/-75 ms reduz a QoS. |
<!-- END_DA_SIM_TABLE -->

هذه المخرجات تقود الانماذج لوحات DA وتفي بمعايير قبول "arnês de simulação +
Modelagem de QoS"

الان توجد الاتمتة خلف
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
Use o chicote de fios e o Norito JSON como
`artifacts/da/threat_model_report.json`. تستخدم الوظائف الليلية هذا
O código de barras do cartão de crédito está disponível para download e download gratuito.
E melhor qualidade de serviço e QoS.

Para obter mais informações, consulte `make docs-da-threat-model`, e não use
`cargo xtask da-threat-model-report` `cargo xtask da-threat-model-report` `cargo xtask da-threat-model-report`
`docs/source/da/_generated/threat_model_report.json` é um arquivo de código aberto
`scripts/docs/render_da_threat_model_tables.py`. `docs/portal`
(`docs/portal/docs/da/threat-model.md`).
متزامنتين.