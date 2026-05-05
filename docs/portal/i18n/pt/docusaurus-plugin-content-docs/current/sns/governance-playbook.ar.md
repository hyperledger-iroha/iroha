---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
Verifique o valor `docs/source/sns/governance_playbook.md` e verifique o valor do arquivo
Eu. Não há problema em PRs.
:::

# دليل حوكمة خدمة أسماء سورا (SN-6)

**Edição:** 2026-03-24 - Data de lançamento SN-1/SN-6  
**روابط خارطة الطريق:** SN-6 "Conformidade e resolução de disputas", SN-7 "Resolver e sincronização de gateway", سياسة العناوين ADDR-1/ADDR-5  
**Alterações:** O valor da API está em [`registry-schema.md`](./registry-schema.md) , na API do site [`registrar-api.md`](./registrar-api.md), ارشادات تجربة em [`address-display-guidelines.md`](./address-display-guidelines.md), وقواعد بنية O código é [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Você pode usar o serviço de streaming de notícias (SNS) para obter mais informações.
Você pode usar o resolvedor e o gateway para resolver o problema. sim
Você pode usar o CLI `sns governance ...` e o Norito
والاثار التدقيقية مرجعا تشغيليا e قبل N1 (الاطلاق العام).

## 1. النطاق والجمهور

يستهدف المستند:

- اعضاء مجلس الحوكمة الذين يصوتون على المواثيق, وسياسات اللاحقات, ونتائج النزاعات.
- اعضاء مجلس guardião الذين يصدرون تجميدات طارئة ويراجعون التراجعات.
- stewards اللاحقات الذين يديرون طوابير المسجل, ويوافقون على المزادات, ويديرون
  تقسيمات الايرادات.
- Resolver/gateway de solução de problemas no SoraDNS, e GAR, e وحواجز
  التليمترية.
- فرق الامتثال والخزينة والدعم التي يجب ان تثبت ان كل اجراء حوكمة ترك اثار Norito
  قابلة للتدقيق.

يغطي مراحل البيتا المغلقة (N0) والاطلاق العام (N1) والتوسع (N2) المدرجة في
`roadmap.md` é um dispositivo de armazenamento de dados que pode ser usado para armazenar e armazenar dados.
التصعيد.

##2.| الدور | Produtos de informática | Máquinas e equipamentos | التصعيد |
|------|----------------------|-------------|---------|
| مجلس الحوكمة | يصيغ ويصادق على المواثيق, وسياسات اللاحقات, واحكام النزاعات, e mordomos. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, você pode usar o `sns governance charter submit`. | رئيس المجلس + متعقب جدول اعمال الحوكمة. |
| Guardião principal | يصدر تجميدات macio/duro, وقوانين طارئة, ومراجعات 72 h. | Guarde o guardião do `sns governance freeze`, e o guardião do `artifacts/sns/guardian/*`. | دورية guardião de plantão (<=15 min ACK). |
| comissários de bordo | يديرون طوابير المسجل, والمزادات, وشرائح التسعير, واتصالات العملاء؛ ويقرون بالامتثال. | Você pode usar o steward em `SuffixPolicyV1`, você pode usar o steward para saber mais sobre o assunto. | قائد برنامج steward + PagerDuty خاص بكل لاحقة. |
| Ferramentas e equipamentos | Selecione `/v1/sns/*`, verifique o código, clique no link e clique na CLI. | API API ([`registrar-api.md`](./registrar-api.md)), مقاييس `sns_registrar_status_total`, اثباتات الدفع المؤرشفة تحت `artifacts/sns/payments/*`. | Isso significa que você pode fazer isso. |
| Resolvedores de problemas e soluções | يحافظون على SoraDNS e GAR e وحالة البوابة متوافقة مع احداث المسجل؛ ويبثون مقاييس الشفافية. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolvedor SRE de plantão + جسر عمليات البوابة. |
| Produtos e serviços | 70/30, e referência, e SLA. | Você pode usar o Stripe/الخزينة, o KPI definido como `docs/source/sns/regulatory/`. | مراقب المالية + مسؤول الامتثال. |
| Produtos de higiene pessoal | تتبع الالتزامات العالمية (EU DSA), وتحدث مواثيق KPI, وتقدم الافصاحات. | Você pode usar `docs/source/sns/regulatory/`, عروض مرجعية, e `ops/drill-log.md`. | قائد برنامج الامتثال. |
| الدعم / SRE عند الطلب | يعالج الحوادث (تصادمات, انحرافات فوترة, اعطال resolvedor), وينسق رسائل العملاء, ويمتلك الادلة التشغيلية. | قوالب الحوادث, `ops/drill-log.md`, ادلة مختبر مرحلية, ونصوص Slack/war-room المؤرشفة تحت `incident/`. | دورية de plantão لـSNS + ادارة SRE. |

## 3. الاثار المرجعية e مصادر البيانات| الاثر | الموقع | الغرض |
|-------|--------|-------|
| Indicador + Indicador KPI | `docs/source/sns/governance_addenda/` | Ele pode ser usado para definir o KPI e o KPI e o CLI. |
| مخطط السجل | [`registry-schema.md`](./registry-schema.md) | Verifique Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| عقد المسجل | [`registrar-api.md`](./registrar-api.md) | Você pode usar REST/gRPC, usar `sns_registrar_status_total` e usar ganchos. |
| UX UX | [`address-display-guidelines.md`](./address-display-guidelines.md) | عروض i105 (المفضلة) والمضغوطة (الخيار الثاني) المرجعية التي تعكسها المحافظ/المستكشفات. |
| Aplicativo SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | اشتقاق المضيفات الحتمي, سير عمل tailer للشفافية, وقواعد التنبيه. |
| Máquinas de lavar louça | `docs/source/sns/regulatory/` | ملاحظات استقبال حسب الولاية (مثل EU DSA), اقرارات steward, ملاحق قوالب. |
| brocas | `ops/drill-log.md` | سجل لتجارب الفوضى وIR المطلوبة قبل الخروج من المراحل. |
| تخزين الاثار | `artifacts/sns/` | Você pode usar o Guardian, o resolvedor de software, o KPI e o arquivo CLI no `sns governance ...`. |

Não há nada que você possa fazer sobre o dinheiro e o dinheiro que você precisa.
Você pode fazer isso em 24 dias.

## 4. ادلة دورة الحياة

### 4.1 حركات الميثاق وsteward

| الخطوة | المالك | CLI / الدليل | Produtos |
|----|--------|---------------|---------|
| صياغة الملحق وفروق KPI | مقرر المجلس + قائد mordomo | O Markdown é definido como `docs/source/sns/governance_addenda/YY/` | تضمين معرفات مواثيق KPI, ganchos تليمترية, وشروط التفعيل. |
| تقديم الاقتراح | رئيس المجلس | `sns governance charter submit --input SN-CH-YYYY-NN.md` (ou `CharterMotionV1`) | Use CLI para Norito em `artifacts/sns/governance/<id>/charter_motion.json`. |
| Guardião e guardião | المجلس + guardiões | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | ارفاق محاضر مجزأة واثباتات النصاب. |
| Mordomo | Comissários de bordo | `sns governance steward-ack --proposal <id> --signature <file>` | مطلوب قبل تغيير سياسات اللاحقات; Verifique o `artifacts/sns/governance/<id>/steward_ack.json`. |
| التفعيل | عمليات المسجل | Se você usar `SuffixPolicyV1`, você pode usar o código de barras, e usar o `status.md`. | Você pode fazer isso em `sns_governance_activation_total`. |
| سجل التدقيق | الامتثال | Você pode usar `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` e usar brocas para bancada. | Verifique o valor do produto e do produto. |

### 4.2 Perguntas e respostas1. **الفحص المسبق:** يستعلم المسجل `SuffixPolicyV1` لتاكيد شريحة التسعير, الشروط
   المتاحة, ونوافذ السماح/الاسترداد. ابق جداول التسعير متزامنة مع جدول الشرائح
   3/4/5/6-9/10+ (الشريحة الاساسية + معاملات اللاحقة) الموثقة neste roteiro.
2. ** lance selado: ** لمجموعات premium, 72 h de commit / 24 h de revelação
   Modelo `sns governance auction commit` / `... reveal`. انشر قائمة commits (hashes
   فقط) تحت `artifacts/sns/auctions/<name>/commit.json` حتى يتمكن المدققون من
   التحقق من العشوائية.
3. **التحقق من الدفع:** يتحقق المسجلون من `PaymentProofV1` مقابل تقسيمات الخزينة
   (70% خزينة / 30% administrador com referência separada 72 h, ارتفاع اخطاء المسجل, انحراف ARPU).

### 4.4 Perguntas e respostas e| المرحلة | المالك | Produtos e serviços | SLA |
|--------|--------|------------------|-----|
| Produtos macios | mordomo / الدعم | Você pode usar `SNS-DF-<id>` para obter títulos de dívida e títulos de dívida. | <=4 horas de semana. |
| تذكرة guardião | Guardião principal | `sns governance freeze --selector <i105> --reason <text> --until <ts>` é `GuardianFreezeTicketV1`. O JSON é definido como `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h تنفيذ. |
| تصديق المجلس | مجلس الحوكمة | يوافق او يرفض التجميدات, ويوثق القرار مع رابط لتذكرة guardião وبصمة bond النزاع. | جلسة المجلس التالية او تصويت غير متزامن. |
| لجنة التحكيم | الامتثال + mordomo | No final de 7 meses (roteiro do mapa) você pode encontrar o `sns governance dispute ballot`. ارفق ايصالات التصويت المجهولة بحزمة الحادث. | الحكم <=7 ايام بعد ايداع bond. |
| ستئناف | guardião + المجلس | يضاعف الاستئناف vínculo ويعيد عملية المحلفين; Verifique se o Norito `DisputeAppealV1` está funcionando corretamente. | <=10 dias. |
| فك التجميد والمعالجة | Resolver + resolver | Em `sns governance unfreeze --selector <i105> --ticket <id>`, você pode usar o GAR/resolver. | مباشرة بعد الحكم. |

القوانين الطارئة (تجميدات يطلقها guardião <=72 h) تتبع نفس التدفق لكنها تتطلب
A máquina de lavar roupa e a máquina de lavar roupa são `docs/source/sns/regulatory/`.

### 4.5 Solução de resolução de problemas

1. **Hook الحدث:** يرسل كل حدث تسجيل الى دفق احداث resolvedor (`tools/soradns-resolver` SSE).
   يشترك فريق resolvedor ويسجل الفروقات عبر tailer الشفافية
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **تحديث قالب GAR:** يجب على البوابات تحديث قوالب GAR المشار اليها بواسطة
   `canonical_gateway_suffix()` é o mesmo que `host_pattern`. خزن الفروقات في
   `artifacts/sns/gar/<date>.patch`.
3. **نشر zonefile:** Use o zonefile do arquivo em `roadmap.md` (nome, ttl, cid, prova)
   Use o Torii/SoraFS. O JSON Norito é o `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Especificação:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   Não se preocupe com isso. Verifique se o Prometheus está configurado para ser removido.
5. **تدقيق البوابة:** سجل عينات رؤوس `Sora-*` (سياسة التخزين المؤقت, CSP, ملخص GAR)
   وارفقها بسجل الحوكمة لكي يثبت المشغلون ان البوابة قدمت الاسم الجديد مع حواجز الحماية المقصودة.

## 5. Nome do usuário

| الاشارة | المصدر | الوصف / الاجراء |
|--------|--------|-----------------|
| `sns_registrar_status_total{result,suffix}` | معالجات مسجل Torii | عداد نجاح/خطا للتسجيلات, التجديدات, التجميدات, التحويلات; Verifique se o `result="error"` está danificado. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Mecanismo Torii | SLO para API de API; Verifique se o produto está em `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | alfaiataria resolvedor | تكشف ادلة قديمة او انحراف GAR; O item está em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI الحوكمة | عداد يزداد عند تفعيل ميثاق/ملحق؛ Verifique se o dispositivo está funcionando corretamente. |
| Medidor `guardian_freeze_active` | Guardião CLI | يتتبع نوافذ تجميد macio/duro para fazer isso O SRE não possui o código `1` ou o SLA. |
| Indicadores de KPI | المالية / الوثائق | ملخصات شهرية تنشر مع المذكرات التنظيمية؛ تضمها البوابة عبر [KPI KPI الخاصة بـSNS](./kpi-dashboard.md) ليتمكن stewards والمنظمون من الوصول لنفس عرض Grafana. |## 6. متطلبات الادلة والتدقيق

| الاجراء | الادلة التي يجب ارشفتها | التخزين |
|--------|--------------------------|--------|
| تغيير الميثاق / السياسة | مانيفست Norito موقع, نص CLI, فرق KPI, اقرار steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| تسجيل / تجديد | Use `RegisterNameRequestV1`, `RevenueAccrualEventV1`, por favor. | `artifacts/sns/payments/<tx>.json`, API API definida. |
| Meio | مانيفستات commit/reveal, بذرة العشوائية, جدول حساب الفائز. | `artifacts/sns/auctions/<name>/`. |
| تجميد / فك تجميد | Guardião de تذكرة, تجزئة تصويت المجلس, رابط سجل الحادث, قالب تواصل العملاء. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Resolver resolver | Para zonefile/GAR, use JSONL no tailer, como Prometheus. | `artifacts/sns/resolver/<date>/` + تقارير الشفافية. |
| الاستقبال التنظيمي | مذكرة استقبال, متعقب المواعيد النهائية, اقرار steward, ملخص تغيير KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. قائمة بوابات المرحلة

| المرحلة | معايير الخروج | حزمة الادلة |
|---------|-------------|------------|
| N0 - بيتا مغلقة | مخطط سجل SN-1/SN-2, CLI مسجل يدوي, تدريب guardião مكتمل. | حركة الميثاق + ACK steward; |
| N1 - اطلاق عام | مزادات + شرائح اسعار ثابتة مفعلة لـ`.sora`/`.nexus`, مسجل ذاتي الخدمة, مزامنة Execute o resolvedor, o problema. | Para definir o valor do CI, o valor do KPI/KPI, o valor do CI é menor, o valor do valor é menor. |
| N2 - توسع | `.dao`, revendedor de واجهات, بوابة النزاعات, بطاقات تقييم steward, لوحات تحليلات. | لقطات شاشة للبوابة, مقاييس SLA للنزاعات, صادرات بطاقات تقييم steward, ميثاق حوكمة محدث Revendedor do يشير لسياسات. |

تتطلب مخارج المراحل تدريبات tabletop مسجلة (مسار تسجيل ناجح, تجميد, عطل resolver)
A solução está em `ops/drill-log.md`.

## 8. الاستجابة للحوادث والتصعيد

| المشغل | الشدة | المالك الفوري | Produtos de informática |
|--------|-------|---------------|----------------------|
| Resolver/GAR e soluções de resolução | 1º de setembro | Resolvedor SRE + guardião de مجلس | استدعاء للresolver de plantão, التقاط مخرجات tailer, تقرير ما اذا كان يجب تجميد الاسماء المتاثرة, نشر تحديث Mais de 30 min. |
| تعطل المسجل, فشل الفوترة, او اخطاء API واسعة | 1º de setembro | مدير مناوبة المسجل | بوثيقة Não. |
| Máquinas de lavar e secar roupa | 2 de setembro | mordomo + قائد الدعم | جمع اثباتات الدفع, تحديد الحاجة الى تجميد soft, الرد على مقدم الطلب ضمن SLA, تسجيل النتيجة في متعقب النزاع. |
| Máquinas de lavar louça | 2 de setembro | جهة اتصال الامتثال | Verifique se o produto está conectado ao `docs/source/sns/regulatory/`, se necessário. |
| Máquinas de lavar e secar | 3 de setembro | PM البرنامج | تنفيذ السيناريو المبرمج من `ops/drill-log.md`, ارشفة الاثار, ووسم الفجوات كمهام في roadmap. |

Use o código de barras `incident/YYYY-MM-DD-sns-<slug>.md` para obter mais informações
وسجلات الاوامر ومراجع الادلة المنتجة عبر هذا الدليل.

## 9. المراجع- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (para SNS e DG e ADDR)

Você pode usar o aplicativo CLI e CLI para fazer isso.
التليمترية; يجب ان تطابق عناصر roadmap التي تشير الى
`docs/source/sns/governance_playbook.md` está danificado.