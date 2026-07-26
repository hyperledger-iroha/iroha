---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lançamento testnet
título: Testnet no SoraNet (SNNet-10)
sidebar_label: Testnet (SNNet-10)
description: Você pode usar o kit de teste de telemetria e testnet da SoraNet.
---

:::note المصدر القياسي
Você pode usar o SNNet-10 em `docs/source/soranet/testnet_rollout_plan.md`. Não se preocupe, o problema pode ser alterado.
:::

SNNet-10 é um recurso que permite que o SoraNet seja instalado no SoraNet. استخدم هذه الخطة لتحويل بند خارطة الطريق الى entregáveis, runbooks e telemetria كي يفهم كل operador التوقعات Você também pode usar o SoraNet no site.

## مراحل الاطلاق

| المرحلة | الجدول الزمني (المستهدف) | النطاق | القطع المطلوبة |
|-------|-------------------|-------|---------|
| **T0 - Testnet مغلق** | 4º trimestre de 2026 | 20-50 relés são >=3 ASNs. | Kit de integração Testnet, conjunto de fumaça para fixação de proteção, linha de base para + métricas PoW e broca de brownout. |
| **T1 - بيتا عامة** | 1º trimestre de 2027 | >=100 relés, rotação de guarda, ligação de saída, e SDK betas são suportados pelo SoraNet com `anon-guard-pq`. | Kit de integração, lista de verificação para o diretório de diretório SOP, painéis de controle, telemetria, ensaio e ensaio. |
| **T2 - Mainnet افتراضي** | 2º trimestre de 2027 (referência SNNet-6/7/9) | شبكة الانتاج تعتمد SoraNet افتراضيا؛ Os transportes são feitos com obfs/MASQUE e catraca PQ. | Isso significa governança, rollback para somente direto, downgrade e downgrade. |

لا يوجد **مسار تجاوز** - يجب ان تشحن كل مرحلة telemetria e governança من المرحلة السابقة قبل الترقية.

## Kit de teste para testnet

O relé do operador deve ser definido como:

| القطعة | الوصف |
|----------|------------|
| `01-readme.md` | نظرة عامة, نقاط تواصل, وجدول زمني. |
| `02-checklist.md` | checklist é uma lista de verificação (hardware, امكانية الوصول للشبكة, تحقق من política de guarda). |
| `03-config-example.toml` | O relé + orquestrador é usado no SoraNet para garantir a conformidade com o SNNet-9 e o `guard_directory` para o hash do guard snapshot. |
| `04-telemetry.md` | Você pode criar painéis de controle no SoraNet e no site. |
| `05-incident-playbook.md` | Evite que o brownout/downgrade seja interrompido. |
| `06-verification-report.md` | Você pode fazer testes de fumaça. |

Remova o produto em `docs/examples/soranet_testnet_operator_kit/`. كل ترقية تحدث kit; Verifique o valor do arquivo (exemplo `testnet-kit-vT0.1`).

بالنسبة لمشغلي beta العامة (T1), brief مختصر في `docs/source/soranet/snnet10_beta_onboarding.md` يلخص المتطلبات ومخرجات telemetria وسير عمل التسليم مع Kit de ajuda e ajudantes.

`cargo xtask soranet-testnet-feed` é feed JSON يجمع نافذة الترقية وقائمة relés وتقرير المقاييس وادلة drills وhashes للمرفقات المشار اليها No palco-gate. Você pode usar brocas e usar o `cargo xtask soranet-testnet-drill-bundle` para alimentar o `drill_log.signed = true`.

## مقاييس النجاح

Você pode usar a telemetria para obter informações sobre a telemetria e usar a tecnologia de telemetria:- `soranet_privacy_circuit_events_total`: 95% dos circuitos تكتمل بدون brownout e downgrade; Mais de 5% do valor total do PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: Mais de 1% da busca fetch يوميا تطلق brownout خارج drills المخطط لها.
- `soranet_privacy_gar_reports_total`: valor de +/-10% da taxa de juros do GAR. A política de segurança é uma política de segurança.
- معدل نجاح تذاكر PoW: >=99% ضمن نافذة 3 ثواني؛ O código é `soranet_privacy_throttles_total{scope="congestion"}`.
- Percentil (percentil 95) Tempo médio: <200 ms بعد اكتمال circuitos بالكامل, ملتقط عبر `soranet_privacy_rtt_millis{percentile="p95"}`.

Os painéis de controle são usados ​​em `dashboard_templates/` e `alert_templates/`; Você pode usar telemetria e usar fiapos no CI. Use `cargo xtask soranet-testnet-metrics` para remover o problema.

O stage-gate está no `docs/source/soranet/snnet10_stage_gate_template.md`, e o Markdown está definido para `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist

يجب على المشغلين التوقيع, ما يلي قبل دخول كل مرحلة:

- ✅ Anúncio de retransmissão موقع باستخدام envelope de admissão الحالي.
- ✅ Teste de fumaça de rotação da guarda (`tools/soranet-relay --check-rotation`) ناجح.
- ✅ `guard_directory` يشير الى احدث artefato de `GuardDirectorySnapshotV2` e `expected_directory_hash_hex` يطابق digest اللجنة (اقلاع relé يسجل hash الذي تم التحقق sim).
- ✅ Catraca PQ de metal (`sorafs_orchestrator_pq_ratio`) تبقى فوق حدود الهدف للمرحلة المطلوبة.
- ✅ تهيئة compliance الخاصة بـ GAR تطابق اخر tag (راجع كتالوج SNNet-9).
- ✅ محاكاة انذار downgrade (coletores تعطيل وتوقع تنبيه خلال 5 دقائق).
- ✅ Faça uma broca para PoW/DoS que você possa usar.

Este é o kit mais adequado para você. يرسل المشغلون التقرير المكتمل الى مكتب مساعدة governança قبل استلام بيانات الانتاج.

## Governança

- **ضبط التغيير:** الترقيات تتطلب موافقة Conselho de Governança مسجلة في محاضر المجلس ومرفقة بصفحة الحالة.
- **ملخص الحالة:** نشر تحديثات اسبوعية تلخص عدد relés ونسبة PQ وحوادث brownout والعناصر المفتوحة (Isto está em `docs/source/status/soranet_testnet_digest.md`).
- **Rollbacks:** الحفاظ على خطة rollback Altere o DNS/guard cache e o cache de segurança.

## اصول داعمة

- `cargo xtask soranet-testnet-kit [--out <dir>]` é um kit de reposição de `xtask/templates/soranet_testnet/` para o modelo (`docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` يقيم مقاييس نجاح SNNet-10 ويصدر تقرير pass/fail منظم مناسب لمراجعات governança. Use a chave de segurança em `docs/examples/soranet_testnet_metrics_sample.json`.
- O Grafana e o Alertmanager são baseados em `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; Você pode usar telemetria e usar fiapos no CI.
- Faça o downgrade do SDK/portal em `docs/source/soranet/templates/downgrade_communication_template.md`.
- يجب ان تستخدم ملخصات الحالة الاسبوعية `docs/source/status/soranet_testnet_weekly_digest.md` كنموذج قياسي.

Não receber pull requests pode ser um plano de telemetria e um plano de telemetria.