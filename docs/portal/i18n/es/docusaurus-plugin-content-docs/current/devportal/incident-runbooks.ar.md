---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# كتيبات الحوادث وتمارين rollback

## الغرض

بند خارطة الطريق **DOCS-9** يتطلب كتيبات اجرائية وخطة تدريب حتى يتمكن مشغلو البوابة من
التعافي من فشل النشر دون تخمين. تغطي هذه الملاحظة ثلاثة حوادث عالية الاشارة—فشل النشر،
تدهور النسخ، وانقطاع التحليلات—وتوثق تمارين ربع سنوية تثبت ان rollback للalias
والتحقق الاصطناعي ما زالا يعملان end to end.

### مواد ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) — سير packaging وsigning وترقية alias.
- [`devportal/observability`](./observability) — release tags والتحليلات والprobes المذكورة ادناه.
- `docs/source/sorafs_node_client_protocol.md`
  و [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — telemetria السجل وحدود التصعيد.
- `docs/portal/scripts/sorafs-pin-release.sh` و `npm run probe:*` helpers
  المشار اليها عبر قوائم التحقق.

### القياس عن بعد والادوات المشتركة

| Signal / Tool | الغرض |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | يكشف توقف النسخ وخرق SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | يقيس عمق backlog وزمن الاكتمال لاغراض triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | يوضح اعطال gateway التي غالبا ما تتبع deploy سيئا. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | probes اصطناعية تقوم بعمل gate للreleases وتتحقق من rollbacks. |
| `npm run check:links` | بوابة الروابط المكسورة؛ تستخدم بعد كل mitigation. |
| `sorafs_cli manifest submit ... --alias-*` (مغلفة عبر `scripts/sorafs-pin-release.sh`) | آلية ترقية/اعادة alias. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | تجمع telemetria refusals/alias/TLS/replication. تنبيهات PagerDuty تشير الى هذه اللوحات كدليل. |

## Runbook - فشل نشر او artefact سيئ

### شروط الاطلاق

- فشل probes للpreview/production (`npm run probe:portal -- --expect-release=...`).
- تنبيهات Grafana على `torii_sorafs_gateway_refusals_total` او
  `torii_sorafs_manifest_submit_total{status="error"}` بعد rollout.
- QA يدوي يلاحظ مسارات مكسورة او فشل proxy Try it مباشرة بعد ترقية alias.

### احتواء فوري

1. **تجميد النشر:** ضع `DEPLOY_FREEZE=1` على pipeline في CI (input للworkflow في GitHub)
   او اوقف Jenkins job حتى لا تخرج artefacts اضافية.
2. **التقاط artefacts:** حمل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, ومخرجات probes من build الفاشل حتى يشير rollback
   الى digests الدقيقة.
3. **اخطار اصحاب المصلحة:** storage SRE وlead لـ Docs/DevRel وضابط الحوكمة المناوب
   (خصوصا عند تاثير `docs.sora`).

### اجراء rollback

1. تحديد manifest الاخير المعروف انه جيد (LKG). يقوم workflow الانتاجي بتخزينه في
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. اعادة ربط alias بهذا manifest عبر helper الشحن:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. سجل ملخص rollback في تذكرة الحادثة مع digests الخاصة بmanifest LKG وmanifest الفاشل.

### التحقق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` و `sorafs_cli proof verify ...`
   (انظر دليل النشر) لتاكيد ان manifest المعاد ترقيته ما زال يطابق CAR المؤرشف.
4. `npm run probe:tryit-proxy` لضمان ان proxy Try-It في staging عاد للعمل.

### ما بعد الحادثة

1. اعادة تفعيل pipeline النشر فقط بعد فهم السبب الجذري.
2. تحديث قسم "Lessons learned" في [`devportal/deploy-guide`](./deploy-guide)
   بملاحظات جديدة عند الحاجة.
3. فتح defects لاختبارات فشلت (probe, link checker, الخ).

## Runbook - تدهور النسخ

### شروط الاطلاق

- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` لمدة 10 دقائق.
- `torii_sorafs_replication_backlog_total > 10` لمدة 10 دقائق (انظر
  `pin-registry-ops.md`).
- الحوكمة تبلغ عن بطء توفر alias بعد release.

### Triage

1. راجع dashboards [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) لتحديد ما اذا كان
   backlog محصورا في فئة تخزين او اسطول providers.
2. تحقق من سجلات Torii بحثا عن `sorafs_registry::submit_manifest` لمعرفة ما اذا كانت
   submissions تفشل.
3. افحص صحة النسخ عبر `sorafs_cli manifest status --manifest ...` (يعرض النتائج لكل provider).

### Mitigation

1. اعادة اصدار manifest بعدد نسخ اعلى (`--pin-min-replicas 7`) عبر
   `scripts/sorafs-pin-release.sh` ليوزع scheduler الحمل على عدد اكبر من providers.
   سجل digest الجديد في سجل الحادثة.
2. اذا كان backlog مرتبطا بprovider واحد، عطله مؤقتا عبر replication scheduler
   (موثق في `pin-registry-ops.md`) وقدم manifest جديد يجبر باقي providers على تحديث alias.
3. عندما تكون حداثة alias اهم من parity النسخ، اعد ربط alias الى manifest دافئ
   staged (`docs-preview`)، ثم انشر manifest متابعة بعد ان ينظف SRE الـ backlog.

### التعافي والاغلاق

1. راقب `torii_sorafs_replication_sla_total{outcome="missed"}` لضمان استقرار العد.
2. التقط مخرجات `sorafs_cli manifest status` كدليل على عودة كل replica للامتثال.
3. انشئ او حدث post-mortem لـ backlog النسخ مع الخطوات التالية
   (توسيع providers، ضبط chunker، الخ).

## Runbook - انقطاع التحليلات او القياس عن بعد

### شروط الاطلاق

- `npm run probe:portal` ينجح لكن dashboards تتوقف عن ابتلاع احداث
  `AnalyticsTracker` لاكثر من 15 دقيقة.
- Privacy review ترصد زيادة غير متوقعة في الاحداث المسقطة.
- `npm run probe:tryit-proxy` يفشل على مسارات `/probe/analytics`.

### الاستجابة

1. تحقق من inputs وقت البناء: `DOCS_ANALYTICS_ENDPOINT` و
   `DOCS_ANALYTICS_SAMPLE_RATE` داخل artefact الاصدار (`build/release.json`).
2. اعد تشغيل `npm run probe:portal` مع توجيه `DOCS_ANALYTICS_ENDPOINT` الى
   collector في staging لتاكيد ان tracker ما زال يرسل payloads.
3. اذا كانت collectors متوقفة، اضبط `DOCS_ANALYTICS_ENDPOINT=""` واعمل rebuild
   ليقوم tracker بعمل short-circuit؛ سجل نافذة الانقطاع في timeline الحادثة.
4. تحقق ان `scripts/check-links.mjs` ما زال يقوم بعمل fingerprint لـ `checksums.sha256`
   (انقطاعات التحليلات يجب *الا* تمنع التحقق من sitemap).
5. بعد تعافي collector، شغل `npm run test:widgets` لتشغيل unit tests الخاصة بanalytics helper
   قبل اعادة النشر.

### ما بعد الحادثة

1. تحديث [`devportal/observability`](./observability) باي قيود جديدة للcollector او
   متطلبات sampling.
2. اصدر اخطار حوكمة اذا تم فقدان او تنقيح بيانات التحليلات خارج السياسة.

## تمارين المرونة الربع سنوية

شغل كلا التمرينين خلال **اول ثلاثاء من كل ربع** (Jan/Apr/Jul/Oct)
او مباشرة بعد اي تغيير كبير في البنية التحتية. خزّن artefacts تحت
`artifacts/devportal/drills/<YYYYMMDD>/`.

| التمرين | الخطوات | الدليل |
| ----- | ----- | -------- |
| تمرين rollback للalias | 1. اعادة تشغيل rollback الخاص بـ "Failed deployment" باستخدام احدث manifest انتاجي.<br/>2. اعادة الربط الى الانتاج بعد نجاح probes.<br/>3. تسجيل `portal.manifest.submit.summary.json` وسجلات probes في مجلد التمرين. | `rollback.submit.json`, مخرجات probes، وrelease tag للتمرين. |
| تدقيق التحقق الاصطناعي | 1. تشغيل `npm run probe:portal` و `npm run probe:tryit-proxy` ضد الانتاج وstaging.<br/>2. تشغيل `npm run check:links` و ارشفة `build/link-report.json`.<br/>3. ارفاق screenshots/exports من لوحات Grafana لتاكيد نجاح probes. | سجلات probes + `link-report.json` تشير الى fingerprint للmanifest. |

صعّد التمارين الفائتة الى مدير Docs/DevRel ومراجعة حوكمة SRE، لان خارطة الطريق تتطلب
دليلا ربع سنوي حتميا على ان rollback للalias و probes البوابة ما تزال سليمة.

## تنسيق PagerDuty و on-call

- خدمة PagerDuty **Docs Portal Publishing** تملك التنبيهات المولدة من
  `dashboards/grafana/docs_portal.json`. القواعد `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, و `DocsPortal/TLSExpiry` تقوم بpaging لـ Docs/DevRel
  الرئيسي مع Storage SRE كاحتياطي.
- عند النداء، ارفق `DOCS_RELEASE_TAG`، وارفق screenshots للوحات Grafana المتاثرة،
  واربط مخرجات probe/link-check في ملاحظات الحادثة قبل بدء mitigation.
- بعد mitigation (rollback او redeploy)، اعد تشغيل `npm run probe:portal`,
  `npm run check:links`, والتقط snapshots جديدة من Grafana تظهر عودة المقاييس
  ضمن العتبات. ارفق جميع الادلة بحادثة PagerDuty قبل اغلاقها.
- اذا اطلق تنبيهان في نفس الوقت (مثلا TLS expiry مع backlog)، تعامل مع refusals اولا
  (ايقاف publishing)، نفذ اجراء rollback، ثم عالج TLS/backlog مع Storage SRE على bridge.
