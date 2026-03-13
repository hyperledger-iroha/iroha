---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementação de várias fontes
título: دليل إطلاق متعدد المصادر وإدراج المزوّدين في القائمة السوداء
sidebar_label: دليل إطلاق متعدد المصادر
description: قائمة تشغيل للعمليات المرحلية متعددة المصادر ومنع المزوّدين في حالات الطوارئ.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/runbooks/multi_source_rollout.md`. Certifique-se de que o produto esteja funcionando corretamente.
:::

## الغاية

يوجّه هذا الدليل فرق SRE والمهندسين المناوبين عبر مسارين حاسمين:

1. إطلاق المُنسِّق متعدد المصادر على موجات مضبوطة.
2. Verifique o número de telefone do seu computador e verifique o valor do produto.

Você pode usar a API SF-6 (`sorafs_orchestrator`, sem API). الشرائح في البوابة, مُصدّرات التليمترية).

> **راجع أيضًا:** يتعمق [دليل تشغيل المُنسِّق](./orchestrator-ops.md) no site da empresa (التقاط لوحة النتائج, مفاتيح الإطلاق المرحلي, والرجوع للخلف). Verifique se o produto está funcionando corretamente.

## 1. التحقق قبل التنفيذ

1. **تأكيد مدخلات الحوكمة.**
   - يجب على جميع المزوّدين المرشحين نشر أظرفة `ProviderAdvertV1` مع حمولة قدرات النطاق وميزانيات التدفق. Verifique se `/v2/sorafs/providers` está funcionando corretamente.
   - يجب أن تكون لقطات التليمترية التي توفّر معدلات الكمون/الفشل أحدث من 15 دقيقة قبل كل تشغيل كناري.
2. **تهيئة الإعدادات.**
   - Para obter o JSON do arquivo `iroha_config`, use o código:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     O JSON é definido como (`max_providers`, e também é usado). Use o recurso de preparação/produção para obter mais informações.
3. **تمرين الـ fixtures القياسية.**
   - املأ متغيرات بيئة المانيفست/الرموز ونفّذ الجلب الحتمي:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Não há nenhum resumo do resumo do arquivo (hex) e do banco de dados base64 para o uso de base64. الكناري.
   - قارن `artifacts/canary.scoreboard.json` بالإصدار السابق. Isso significa que você pode obter mais de 10% de desconto.
4. **التأكد من توصيل التليمترية.**
   - Altere Grafana para `docs/examples/sorafs_fetch_dashboard.json`. Use o método `sorafs_orchestrator_*` para teste.

##2.

اتبع هذا الإجراء عندما يقدم المزوّد شرائح تالفة, أو يتجاوز المهلات بشكل مستمر, أو Não é um problema.1. **توثيق الأدلة.**
   - صدّر أحدث ملخص fetch (مخرجات `--json-out`). سجّل فهارس الشرائح الفاشلة, وأسماء المزوّدين المستعارة, وعدم تطابق digest.
   - Verifique o valor do arquivo `telemetry::sorafs.fetch.*`.
2. **تطبيق تجاوز فوري.**
   - علّم المزوّد كمُعاقب في لقطة التليمترية الموزعة على المُنسِّق (اضبط `penalty=true` أو اخفض `token_health` ou `0`). سيستبعده بناء placar التالي تلقائيًا.
   - لاختبارات smoke السريعة, مرر `--deny-provider gw-alpha` إلى `sorafs_cli fetch` لتمرين مسار الفشل دون انتظار انتشار التليمترية.
   - أعد نشر حزمة التليمترية/الإعداد المحدثة في البيئة المتأثرة (encenação → canário → produção). Você pode fazer isso em qualquer lugar.
3. **التحقق من التجاوز.**
   - أعد تشغيل fetch للـ fixture القياسي. O placar do placar é definido como `policy_denied`.
   - Use `sorafs_orchestrator_provider_failures_total` para obter mais informações sobre o produto.
4. **تصعيد الحظر طويل الأمد.**
   - إذا استمر الحظر لأكثر من 24 h, افتح تذكرة حوكمة لتدوير, أو تعليق anúncio الخاص به. حتى تمر الموافقة, أبقِ قائمة المنع وقم بتحديث لقطات التليمترية كي لا يعود المزوّد إلى placar.
5. **بروتوكول التراجع.**
   - لإعادة المزوّد, أزله من قائمة المنع, أعد النشر, eالتقط لقطة placar جديدة. أرفق التغيير بتقرير ما بعد الحادثة.

## 3. خطة الإطلاق المرحلية

| المرحلة | النطاق | الإشارات المطلوبة | معايير Go/No-Go |
|--------|--------|-------------------|-----------------|
| **Laboratório** | Máquinas de lavar louça | Equipamentos de carga CLI | luminárias de cargas úteis | تنجح كل الشرائح, تبقى عدادات فشل المزوّد عند 0, نسبة إعادة المحاولة < 5%. |
| **Encenação** | encenação بيئة كاملة لطبقة التحكم | Módulo Grafana قواعد التنبيه في وضع warning-only | Use `sorafs_orchestrator_active_fetches` para obter mais informações sobre o produto Não use o `warn/critical`. |
| **Canário** | ≤10% من حركة الإنتاج | Pager (pager) Página de destino | نسبة إعادة المحاولة < 10%, فشل المزوّدين محصور في نظراء معروفين بالضجيج, وهيستوغرام الكمون مطابق لخط أساس estadiamento ±20%. |
| **الإتاحة العامة** | 100% grátis | قواعد الـ pager فعّالة | A operação `NoHealthyProviders` dura 24 horas, e a operação do SLA é realizada por 24 horas. |

O que aconteceu:

1. Insira JSON para usar `max_providers` e verifique o valor do arquivo.
2. Use `sorafs_cli fetch` e instale o SDK do dispositivo de fixação e instale-o.
3. Artefatos de التقط الخاصة بالـ placar e resumo وأرفقها بسجل الإصدار.
4. Coloque a chave de fenda no chão para obter mais informações.

## 4. الرصد وربط الحوادث

- **المقاييس:** تأكد من أن Alertmanager يراقب `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. Não há nada de errado com isso.
- **السجلات:** وجّه أهداف `telemetry::sorafs.fetch.*` إلى مجمّع السجلات المشترك. Você pode usar o `event=complete status=failed` para triagem.
- **لوحات النتائج:** احفظ كل artefato no placar no final do jogo. O JSON é definido como um valor para o servidor e para o servidor.
- **لوحات المتابعة:** إلى مجلد الإنتاج مع قواعد O código é `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Nome de usuário- سجّل كل تغيير deny/boost في سجل عمليات التشغيل مع الطابع الزمني, والمشغل, والسبب, والحادث المرتبط.
- أخطر فرق SDK عند تغير أوزان المزوّدين, ميزانيات إعادة المحاولة لمواءمة توقعات جانب العميل.
- Verifique se o GA, `status.md` é um dispositivo de segurança que pode ser usado em uma máquina de lavar louça.