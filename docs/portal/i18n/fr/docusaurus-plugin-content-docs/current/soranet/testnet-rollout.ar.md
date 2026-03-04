---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : testnet-rollout
titre : Utiliser testnet pour SoraNet (SNNet-10)
sidebar_label : utiliser testnet (SNNet-10)
description: Un kit de téléchargement pour la télémétrie et le testnet pour SoraNet.
---

:::note المصدر القياسي
Utilisez SNNet-10 pour `docs/source/soranet/testnet_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-10 est compatible avec SoraNet. استخدم هذه الخطة لتحويل بند خارطة الطريق الى livrables ملموسة وrunbooks وبوابات télémétrie كي يفهم كل opérateur التوقعات قبل ان Utilisez SoraNet pour la navigation.

## مراحل الاطلاق

| المرحلة | الجدول الزمني (المستهدف) | النطاق | القطع المطلوبة |
|-------|---------|-------|----------|
| **T0 - Testnet maintenant** | T4 2026 | 20-50 relais عبر >=3 ASN يديرها مساهمون اساسيون. | Kit d'intégration Testnet, suite Smoke pour épinglage de garde, ligne de base pour + métriques PoW, et exercice de baisse de tension. |
| **T1 - عامة** | T1 2027 | >=100 relais, pour la rotation de la garde, pour la liaison de sortie, et le SDK bêta pour SoraNet pour `anon-guard-pq`. | Kit d'intégration comprenant une liste de contrôle et un répertoire SOP pour la télémétrie, ainsi que des tableaux de bord pour la télémétrie et des répétitions. |
| **T2 - Mainnet افتراضي** | T2 2027 (مشروط باكتمال SNNet-6/7/9) | شبكة الانتاج تعتمد SoraNet افتراضيا؛ تفعيل transports من نوع obfs/MASQUE وفرض PQ Ratchet. | Il s'agit d'une restauration de la gouvernance ou d'une restauration directe uniquement, ainsi que d'une rétrogradation. |

لا يوجد **مسار تجاوز** - يجب ان تشحن كل مرحلة télémétrie et gouvernance من المرحلة السابقة قبل الترقية.

## Kit pour testernet

Pour le relais opérateur, il y a des éléments suivants:

| القطعة | الوصف |
|--------------|-------------|
| `01-readme.md` | نظرة عامة، نقاط تواصل، وجدول زمني. |
| `02-checklist.md` | liste de contrôle pour la politique de garde (matériel et politique de garde). |
| `03-config-example.toml` | Le relais + orchestrateur de SoraNet est compatible avec la conformité avec SNNet-9 et le `guard_directory` utilise le hachage pour l'instantané de garde. |
| `04-telemetry.md` | Les tableaux de bord sont également disponibles pour SoraNet et SoraNet. |
| `05-incident-playbook.md` | Il y a une baisse de tension/un déclassement en cas de baisse de tension ou de déclassement. |
| `06-verification-report.md` | Vous pouvez également utiliser des tests de fumée. |

Je l'utilise pour `docs/examples/soranet_testnet_operator_kit/`. كل ترقية تحدث kit؛ ارقام الاصدار تتبع المرحلة (مثلا `testnet-kit-vT0.1`).

La version bêta (T1) est disponible en bref avec `docs/source/soranet/snnet10_beta_onboarding.md` pour la télémétrie et la télémétrie. الاشارة الى kit الحتمي وhelpers التحقق.

`cargo xtask soranet-testnet-feed` flux JSON pour les relais et les forets et les hashes قالب porte de scène. Les forets et les forets sont pour `cargo xtask soranet-testnet-drill-bundle` et l'alimentation pour `drill_log.signed = true`.

## مقاييس النجاح

La télémétrie est également utile pour la télémétrie :- `soranet_privacy_circuit_events_total` : 95 % des circuits en cas de baisse de tension et de déclassement الـ 5% المتبقية مقيدة بامدادات PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` : اقل من 1% من جلسات fetch يوميا تطلق brownout خارج forets المخطط لها.
- `soranet_privacy_gar_reports_total` : valeur de +/-10 % pour le GAR. La politique est également une question de politique.
- معدل نجاح تذاكر PoW: >=99% ضمن نافذة 3 ثواني؛ يتم الابلاغ عبر `soranet_privacy_throttles_total{scope="congestion"}`.
- Niveau (95e percentile) pour la durée de vie : <200 ms pour les circuits utilisés pour le `soranet_privacy_rtt_millis{percentile="p95"}`.

Il s'agit de tableaux de bord et de `dashboard_templates/` et `alert_templates/` ; Il s'agit de la télémétrie et des peluches de CI. استخدم `cargo xtask soranet-testnet-metrics` لتوليد التقرير الموجه للحوكمة قبل طلب الترقية.

Le stage-gate est basé sur `docs/source/soranet/snnet10_stage_gate_template.md` et le Markdown est basé sur `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Liste de contrôle للتحقق

يجب على المشغلين التوقيع على ما يلي قبل دخول كل مرحلة:

- ✅ Annonce relais موقع باستخدام enveloppe d'admission الحالي.
- ✅ Test de fumée de rotation de garde (`tools/soranet-relay --check-rotation`) ici.
- ✅ `guard_directory` يشير الى احدث artefact من `GuardDirectorySnapshotV2` et `expected_directory_hash_hex` يطابق digest اللجنة (اقلاع relay يسجل hash الذي تم التحقق منه).
- ✅ مقاييس PQ Ratchet (`sorafs_orchestrator_pq_ratio`) تبقى فوق حدود الهدف للمرحلة المطلوبة.
- ✅ La conformité est assurée par GAR avec la balise (راجع كتالوج SNNet-9).
- ✅ محاكاة انذار downgrade (تعطيل collectors وتوقع تنبيه خلال 5 دقائق).
- ✅ تنفيذ Drill لـ PoW/DoS مع خطوات تخفيف موثقة.

يوجد قالب معبأ مسبقا ضمن kit الانضمام. يرسل المشغلون التقرير المكتمل الى مكتب مساعدة قبل استلام بيانات الانتاج.

## Gouvernance والتقارير

- **ضبط التغيير:** الترقيات تتطلب موافقة Conseil de gouvernance مسجلة في محاضر المجلس ومرفقة بصفحة الحالة.
- **ملخص الحالة:** نشر تحديثات اسبوعية تلخص عدد relais et PQ et baisse de tension et العناصر المفتوحة (تخزن في `docs/source/status/soranet_testnet_digest.md` pour la lecture).
- **Rollbacks :** Il est possible de restaurer un rollback en utilisant le cache DNS/guard de 30 minutes. وقوالب تواصل العملاء.

## اصول داعمة

- `cargo xtask soranet-testnet-kit [--out <dir>]` est un kit d'installation pour `xtask/templates/soranet_testnet/` pour l'installation électrique (`docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` est un outil SNNet-10 pour la réussite/l'échec de la gouvernance. Je l'ai trouvé à `docs/examples/soranet_testnet_metrics_sample.json`.
- Pour Grafana et Alertmanager avec `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml` ; Il s'agit d'une télémétrie et d'un filtre à peluches pour CI.
- Vous pouvez rétrograder le SDK/portail vers `docs/source/soranet/templates/downgrade_communication_template.md`.
- يجب ان تستخدم ملخصات الحالة الاسبوعية `docs/source/status/soranet_testnet_weekly_digest.md` كنموذج قياسي.

Il existe des demandes d'extraction qui permettent de créer un plan de télémétrie et un plan de recherche.