---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operações
título: دليل تشغيل عمليات SoraFS
sidebar_label: دليل التشغيل
description: Você pode usar o arquivo SoraFS.
---

:::note المصدر المعتمد
A solução de problemas é `docs/source/sorafs_ops_playbook.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم ترحيل مجموعة توثيق Esfinge بالكامل.
:::

## المراجع الرئيسية

- Escolha o valor: Instale o Grafana ou o `dashboards/grafana/` e o Prometheus `dashboards/alerts/`.
- Código de configuração: `docs/source/sorafs_observability_plan.md`.
- أسطح تليمترية Código: `docs/source/sorafs_orchestrator_plan.md`.

## مصفوفة التصعيد

| الأولوية | أمثلة على المحفزات | المناوب الأساسي | الاحتياطي | Produtos |
|----------|--------------------|-----------------|----------|---------|
| P1 | انقطاع عالمي للبوابة, معدل فشل PoR أكبر من 5% (15 دقيقة), تراكم التكرار يتضاعف كل 10 دقائق | Armazenamento SRE | Observabilidade TL | أشرك مجلس الحوكمة إذا تجاوز التأثير 30 dias. |
| P2 | خرق SLO لزمن تأخر البوابة الإقليمي, قفزة إعادة المحاولة في المُنسِّق دون تأثير SLA | Observabilidade TL | Armazenamento SRE | واصل الإطلاق لكن أوقف المانيفستات الجديدة. |
| P3 | Taxa de transferência (80–90%) | فرز الاستقبال | Guilda de operações | Não use nenhum recurso. |

## انقطاع البوابة / تدهور التوفر

**الكشف**

- Nomes: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Código de configuração: `dashboards/grafana/sorafs_gateway_overview.json`.

**إجراءات فورية**

1. أكّد النطاق (مزوّد واحد مقابل الأسطول) عبر لوحة معدل الطلبات.
2. Use o Torii para obter mais informações (ou seja, você pode usar o software). `sorafs_gateway_route_weights` é um dispositivo de teste (`docs/source/sorafs_gateway_self_cert.md`).
3. Use a opção “busca direta” para CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**الفرز**

- تحقّق من استهلاك رموز stream مقابل `sorafs_gateway_stream_token_limit`.
- افحص سجلات البوابة لأخطاء TLS ou أخطاء القبول.
- Verifique `scripts/telemetry/run_schema_diff.sh` para obter mais informações sobre o produto.

**خيارات المعالجة**

- أعد تشغيل عملية البوابة المتأثرة فقط؛ Certifique-se de que o produto esteja funcionando corretamente.
- ارفع حد رموز stream بنسبة 10–15% مؤقتًا إذا تم تأكيد التشبع.
- أعد تشغيل self-cert (`scripts/sorafs_gateway_self_cert.sh`) بعد الاستقرار.

**ما بعد الحادث**

- Postmortem post-mortem do P1 `docs/source/sorafs/postmortem_template.md`.
- جدولة تدريب فوضى لاحق إذا اعتمدت المعالجة على تدخلات يدوية.

## ارتفاع مفاجئ لفشل الإثبات (PoR / PoTR)

**الكشف**

- Nomes: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Código de configuração: `dashboards/grafana/sorafs_proof_integrity.json`.
- Nome: `torii_sorafs_proof_stream_events_total` e `sorafs.fetch.error` com `provider_reason=corrupt_proof`.

**إجراءات فورية**

1. Verifique se o dispositivo está funcionando corretamente (`docs/source/sorafs/manifest_pipeline.md`).
2. Verifique o valor da máquina de lavar louça.

**الفرز**

- راجع عمق قائمة تحديات PoR مقابل `sorafs_node_replication_backlog_total`.
- Verifique o valor do produto (`crates/sorafs_node/src/potr.rs`) para obter mais informações.
- Este firmware está configurado para ser instalado.

**خيارات المعالجة**

- A instalação do PoR `sorafs_cli proof stream` pode ser feita com mais frequência.
- إذا استمرت الإخفاقات, أزِل المزوّد من المجموعة النشطة عبر تحديث سجل الحوكمة وإجبار Placares de pontuação.

**ما بعد الحادث**- نفّذ سيناريو تدريب فوضى PoR قبل نشر الإنتاج التالي.
- دوّن الدروس في قالب postmortem وحدّث قائمة تحقق تأهيل المزوّدين.

## تأخر التكرار / نمو التراكم

**الكشف**

- Nomes: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. ستورد
  `dashboards/alerts/sorafs_capacity_rules.yml` `dashboards/alerts/sorafs_capacity_rules.yml`
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  قبل الترقية كي يعكس Alertmanager العتبات الموثقة.
- Código de erro: `dashboards/grafana/sorafs_capacity_health.json`.
- Nome: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**إجراءات فورية**

1. تحقّق من نطاق التراكم (مزوّد منفرد أو الأسطول) e أوقف مهام التكرار غير الأساسية.
2. إذا كان التراكم معزولًا, أعد إسناد الطلبات الجديدة مؤقتًا إلى مزوّدين بدلاء عبر مجدول التكرار.

**الفرز**

- افحص تليمترية المُنسِّق بحثًا عن دفعات إعادة المحاولة التي قد تؤدي إلى تضخم التراكم.
- Verifique o valor do arquivo (`sorafs_node_capacity_utilisation_percent`).
- راجع تغييرات الإعداد الأخيرة (تحديثات ملفات تعريف القطع, وتيرة الإثباتات).

**خيارات المعالجة**

- O `sorafs_cli` pode ser usado para `--rebalance`.
- قم بتوسيع عمال التكرار أفقيًا للمزوّد المتأثر.
- فعّل تحديث المانيفست لإعادة محاذاة نوافذ TTL.

**ما بعد الحادث**

- جدولة تدريب سعة يركز على فشل تشبع المزوّدين.
- Verifique se o SLA está disponível em `docs/source/sorafs_node_client_protocol.md`.

## وتيرة تدريبات الفوضى

- **ربع سنوي**: محاكاة مجمعة لانقطاع البوابة + عاصفة إعادة محاولات المُنسِّق.
- **نصف سنوي**: Você pode usar PoR/PoTR para obter mais informações.
- **تحقق شهري سريع**: سيناريو تأخر التكرار باستخدام مانيفستات encenação.
- تتبع التدريبات في سجل التشغيل المشترك (`ops/drill-log.md`) عبر:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- تحقّق من السجل قبل الالتزامات عبر:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Use `--status scheduled` para laptops, e `pass`/`fail` para laptops, و`follow-up` Não há problema em fazê-lo.
- Verifique o número de telefone `--log` para obter mais informações e obter mais informações Você pode usar o código `ops/drill-log.md`.

## قالب ما بعد الحادث

Use `docs/source/sorafs/postmortem_template.md` para conectar P1/P2 e usar o cabo de alimentação. يغطي القالب الخط الزمني, وقياس الأثر, والعوامل المساهمة, والإجراءات التصحيحية, ومهام التحقق Não.