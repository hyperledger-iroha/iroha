---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# كتيبات الحوادث وتمارين reversión

## الغرض

بند خارطة الطريق **DOCS-9** يتطلب كتيبات اجرائية وخطة تدريب حتى يتمكن مشغلو البوابة من
التعافي من فشل النشر دون تخمين. تغطي هذه الملاحظة ثلاثة حوادث عالية الاشارة—فشل النشر،
تدهور النسخ، وانقطاع التحليلات—وتوثق تمارين ربع سنوية تثبت ان rollback للalias
والتحقق الاصطناعي ما زالا يعملان de extremo a extremo.

### مواد ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) — Empaquetado, firma y alias.
- [`devportal/observability`](./observability) — etiquetas de lanzamiento y sondas de lanzamiento.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — telemetría السجل وحدود التصعيد.
- Ayudantes `docs/portal/scripts/sorafs-pin-release.sh` y `npm run probe:*`
  المشار اليها عبر قوائم التحقق.

### القياس عن بعد والادوات المشتركة| Señal / Herramienta | الغرض |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (cumplido/incumplido/pendiente) | يكشف توقف النسخ وخرق SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | يقيس عمق trabajo pendiente y clasificación. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | يوضح اعطال gateway التي غالبا ما تتبع implementar سيئا. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | sondas اصطناعية تقوم بعمل gate للreleases وتتحقق من rollbacks. |
| `npm run check:links` | بوابة الروابط المكسورة؛ تستخدم بعد كل mitigación. |
| `sorafs_cli manifest submit ... --alias-*` (desde `scripts/sorafs-pin-release.sh`) | آلية ترقية/اعادة alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | تجمع rechazos de telemetría/alias/TLS/replicación. تنبيهات PagerDuty تشير الى هذه اللوحات كدليل. |

## Runbook - فشل نشر او artefacto سيئ

### شروط الاطلاق

- Sondas de vista previa/producción (`npm run probe:portal -- --expect-release=...`).
- Actualizaciones Grafana a `torii_sorafs_gateway_refusals_total` y
  `torii_sorafs_manifest_submit_total{status="error"}` بعد implementación.
- Control de calidad يدوي يلاحظ مسارات مكسورة او فشل proxy Pruébelo مباشرة بعد ترقية alias.

### احتواء فوري

1. **تجميد النشر:** Utilice `DEPLOY_FREEZE=1` en la canalización de CI (flujo de trabajo de entrada en GitHub)
   El trabajo de Jenkins es un trabajo de artefactos.
2. **التقاط artefactos:** حمل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, sondas y sondas para compilación y reversión
   الى digiere الدقيقة.
3. **اخطار اصحاب المصلحة:** almacenamiento SRE y líder en Docs/DevRel وضابط الحوكمة المناوب
   (خصوصا عند تاثير `docs.sora`).

### reversión1. تحديد manifiesto الاخير المعروف انه جيد (LKG). يقوم flujo de trabajo الانتاجي بتخزينه في
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. اعادة ربط alias بهذا manifiesto عبر ayudante الشحن:

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

3. Haga una reversión de los resúmenes del manifiesto LKG y del manifiesto.

### التحقق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (انظر دليل النشر) لتاكيد ان manifiesto المعاد ترقيته ما زال يطابق CAR المؤرشف.
4. `npm run probe:tryit-proxy` utiliza el proxy Try-It para la puesta en escena.

### ما بعد الحادثة

1. اعادة تفعيل tubería النشر فقط بعد فهم السبب الجذري.
2. تحديث قسم "Lecciones aprendidas" en [`devportal/deploy-guide`](./deploy-guide)
   بملاحظات جديدة عند الحاجة.
3. فتح defectos لاختبارات فشلت (sonda, verificador de enlaces, الخ).

## Runbook - تدهور النسخ

### شروط الاطلاق

- Contenido: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  abrazadera_min(suma(torii_sorafs_replication_sla_total{resultado=~"cumplido|perdido"}), 1) <
  0,95` por 10 días.
- `torii_sorafs_replication_backlog_total > 10` de 10 días (más
  `pin-registry-ops.md`).
- الحوكمة تبلغ عن بطء توفر alias بعد liberación.

### Triaje

1. راجع paneles de control [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) لتحديد ما اذا كان
   acumulación de proveedores.
2. تحقق من سجلات Torii بحثا عن `sorafs_registry::submit_manifest` لمعرفة ما اذا كانت
   presentaciones تفشل.
3. افحص صحة النسخ عبر `sorafs_cli manifest status --manifest ...` (يعرض النتائج لكل proveedor).

### Mitigación1. اعادة اصدار manifiesto بعدد نسخ اعلى (`--pin-min-replicas 7`) عبر
   `scripts/sorafs-pin-release.sh` Este programador está diseñado para proveedores.
   سجل digest الجديد في سجل الحادثة.
2. اذا كان backlog مرتبطا بprovider y عطله مؤقتا عبر replicación planificador
   (موثق في `pin-registry-ops.md`) Un manifiesto es un proveedor de un alias.
3. عندما تكون حداثة alias اهم من paridad النسخ، اعد ربط alias الى manifiesto دافئ
   en escena (`docs-preview`) ، ثم انشر manifest متابعة بعد ان ينظف SRE الـ backlog.

### التعافي والاغلاق

1. راقب `torii_sorafs_replication_sla_total{outcome="missed"}` لضمان استقرار العد.
2. التقط مخرجات `sorafs_cli manifest status` كدليل على عودة كل replica للامتثال.
3. انشئ او حدث post-mortem لـ backlog النسخ مع الخطوات التالية
   (Proveedores de توسيع, ضبط fragmentador, خ).

## Runbook - انقطاع التحليلات او القياس عن بعد

### شروط الاطلاق

- `npm run probe:portal` ينجح لكن paneles de control تتوقف عن ابتلاع احداث
  `AnalyticsTracker` Después de 15 días.
- Revisión de privacidad ترصد زيادة غير متوقعة في الاحداث المسقطة.
- `npm run probe:tryit-proxy` يفشل على مسارات `/probe/analytics`.

### الاستجابة1. تحقق من inputs y البناء: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` داخل artefacto الاصدار (`build/release.json`).
2. Presione `npm run probe:portal` y presione `DOCS_ANALYTICS_ENDPOINT`.
   colector في puesta en escena لتاكيد ان rastreador ما زال يرسل cargas útiles.
3. Los coleccionistas de اذا كانت متوقفة، اضبط `DOCS_ANALYTICS_ENDPOINT=""` y la reconstrucción
   ليقوم tracker بعمل cortocircuito؛ سجل نافذة الانقطاع في línea de tiempo الحادثة.
4. تحقق ان `scripts/check-links.mjs` ما زال يقوم بعمل huella digital لـ `checksums.sha256`
   (انقطاعات التحليلات يجب *الا* تمنع التحقق من mapa del sitio).
5. Recolector de datos, `npm run test:widgets` para pruebas unitarias y ayudante de análisis
   قبل اعادة النشر.

### ما بعد الحادثة

1. تحديث [`devportal/observability`](./observability) باي قيود جديدة للcollector او
   Muestreo de متطلبات.
2. اصدر اخطار حوكمة اذا تم فقدان او تنقيح بيانات التحليلات خارج السياسة.

## تمارين المرونة الربع سنوية

شغل كلا التمرينين خلال **اول ثلاثاء من كل ربع** (enero/abril/julio/octubre)
او مباشرة بعد اي تغيير كبير في البنية التحتية. خزّن artefactos تحت
`artifacts/devportal/drills/<YYYYMMDD>/`.| التمرين | الخطوات | الدليل |
| ----- | ----- | -------- |
| تمرين reversión للalias | 1. اعادة تشغيل rollback الخاص بـ "Implementación fallida" باستخدام احدث manifiesto انتاجي.2. اعادة الربط الى الانتاج بعد نجاح sondas.3. تسجيل `portal.manifest.submit.summary.json` وسجلات sondas في مجلد التمرين. | `rollback.submit.json`, sondas de prueba y etiqueta de liberación. |
| تدقيق التحقق الاصطناعي | 1. تشغيل `npm run probe:portal` و `npm run probe:tryit-proxy` ضد الانتاج وstaging.2. Utilice `npm run check:links` y conecte `build/link-report.json`.3. ارفاق capturas de pantalla/exportaciones de Grafana لتاكيد نجاح sondas. | سجلات sondas + `link-report.json` تشير الى للmanifest de huellas dactilares. |

Para obtener más información, consulte Docs/DevRel y SRE, para obtener más información.
دليلا ربع سنوي حتميا على ان rollback للalias and probes البوابة ما تزال سليمة.

## تنسيق PagerDuty y de guardia- خدمة PagerDuty **Docs Portal Publishing** تملك التنبيهات المولدة من
  `dashboards/grafana/docs_portal.json`. Nombre `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry` se encuentran en paginación en Docs/DevRel
  الرئيسي مع Almacenamiento SRE كاحتياطي.
- عند النداء، ارفق `DOCS_RELEASE_TAG`, y capturas de pantalla للوحات Grafana المتاثرة,
  واربط مخرجات sonda/verificación de enlace في ملاحظات الحادثة قبل بدء mitigación.
- Mitigación adicional (reversión y reimplementación), según `npm run probe:portal`,
  `npm run check:links`, y las instantáneas de Grafana están disponibles en el sitio web.
  ضمن العتبات. Utilice PagerDuty para ejecutar la función PagerDuty.
- اذا اطلق تنبيهان في نفس الوقت (مثلا TLS expiration مع backlog), تعامل مع rechazos اولا
  (publicación de) ، نفذ اجراء rollback, ثم عالج TLS/backlog مع Storage SRE على bridge.