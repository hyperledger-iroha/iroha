---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "خطة طرح وتوافق anuncios لمزودي SoraFS"
---

> مقتبس من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة طرح وتوافق anuncios لمزودي SoraFS

تنسق هذه الخطة الانتقال من anuncios المسموحة لمزودي التخزين إلى سطح `ProviderAdvertV1`
Lea los trozos de agua y los trozos. وهي تركز على
ثلاثة مخرجات رئيسية:

- **دليل المشغل.** خطوات يجب على مزودي التخزين إكمالها قبل تفعيل كل puerta.
- **تغطية التليمترية.** لوحات معلومات وتنبيهات تستخدمها Observabilidad y operaciones
  للتأكد من أن الشبكة تقبل anuncios المتوافقة فقط.
- **الجدول الزمني للتوافق.** تواريخ واضحة لرفض sobres القديمة حتى تتمكن
  فرق SDK y herramientas من التخطيط لإصداراتها.

يتماشى الطرح مع معالم SF-2b/2c في
[خارطة طريق هجرة SoraFS](./migration-roadmap) ويفترض أن سياسة القبول في
[política de admisión de proveedores](./provider-admission-policy) مطبقة بالفعل.

## الجدول الزمني للمراحل| المرحلة | النافذة (الهدف) | السلوك | إجراءات المشغل | تركيز الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 - الملاحظة الأساسية** | حتى **2025-03-31** | يقبل Torii incluye anuncios, contenido y cargas útiles de `ProviderAdvertV1`. تسجل سجلات ingestión تحذيرات عندما تهمل anuncios `chunk_range_fetch` أو `profile_aliases` القياسية. | - إعادة توليد anuncios عبر canalización نشر anuncio del proveedor (ProviderAdvertV1 + sobre de gobernanza) مع ضمان `profile_id=sorafs.sf1@1.0.0` و`profile_aliases` قياسية و`signature_strict=true`. - تشغيل اختبارات `sorafs_fetch` محليا؛ يجب triage تحذيرات capacidades غير المعروفة. | Introduzca el código Grafana (es decir, el nombre del usuario) y el código de configuración de la unidad. |
| **R1 - بوابة التحذير** | **2025-04-01 → 2025-05-15** | يستمر Torii في قبول anuncios القديمة لكنه يزيد `torii_sorafs_admission_total{result="warn"}` عندما يفتقد carga útil `chunk_range_fetch` أو يحمل capacidades غير معروفة دون `allow_unknown_capabilities=true`. يفشل herramientas CLI الآن في إعادة التوليد ما لم يوجد الـ mango القياسي. | - Publicidad de puesta en escena y producción de cargas útiles `CapabilityType::ChunkRangeFetch`, y pruebas de GREASE en `allow_unknown_capabilities=true`. - توثيق الاستعلامات الجديدة للتليمترية في runbooks التشغيل. | ترقية paneles de control إلى دوران de guardia؛ Para obtener más información, consulte `warn` un 5 % del tiempo de funcionamiento durante 15 días. || **R2 - Cumplimiento** | **2025-05-16 → 2025-06-30** | يرفض Torii anuncios التي تفتقد sobres الحوكمة أو الـ manija القياسي للملف الشخصي أو capacidad `chunk_range_fetch`. لم تعد maneja القديمة `namespace-name` تُحلل. Estas son las capacidades de la opción de inscripción de GREASE en `reason="unknown_capability"`. | - التأكد من وجود sobres الإنتاج ضمن `torii.sorafs.admission_envelopes_dir` وتدوير أي anuncios قديمة متبقية. - Los SDK manejan los alias de los usuarios. | Alertas de buscapersonas: `torii_sorafs_admission_total{result="reject"}` > 0 لمدة 5 دقائق يستدعي تدخل المشغل. تتبع نسبة القبول وهيستوغرامات أسباب القبول. |
| **R3 - إيقاف القديمة** | **اعتبارا desde 2025-07-01** | تخلى Discovery عن دعم anuncios الثنائية التي لا تضبط `signature_strict=true` أو التي تفتقد `profile_aliases`. يقوم Torii caché de descubrimiento بحذف الإدخالات القديمة التي تجاوزت fecha límite للتجديد دون تحديث. | - جدولة نافذة desmantelamiento النهائية لمكدسات المزودين القديمة. - Use GREASE `--allow-unknown` para taladros y taladros. - تحديث playbooks للحوادث لاعتبار مخرجات تحذير `sorafs_fetch` مانعا قبل الإصدارات. | تشديد التنبيهات: أي نتيجة `warn` تنبه de guardia. Utilice el descubrimiento de JSON y las capacidades de su servidor. |

## قائمة تحقق المشغل1. **جرد anuncios.** احصر كل anuncio منشور وسجل:
   - مسار الـ envolvente gobernante (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` y `profile_aliases` en el anuncio.
   - Capacidades de قائمة (تتوقع على الأقل `torii_gateway` y `chunk_range_fetch`).
   - علم `allow_unknown_capabilities` (مطلوب عندما توجد TLV محجوزة من proveedor).
2. **إعادة التوليد باستخدام herramientas المزود.**
   - أعد بناء carga útil عبر ناشر anuncio del proveedor, مع التأكد من:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` desde `max_span` desde arriba
     - `allow_unknown_capabilities=<true|false>` Compatible y TLV de GRASA
   - تحقق عبر `/v2/sorafs/providers` y `sorafs_fetch`; يجب triaje تحذيرات
     capacidades غير المعروفة.
3. **التحقق من جاهزية de múltiples fuentes.**
   - Aquí `sorafs_fetch` o `--provider-advert=<path>`. يفشل CLI الآن عندما
     يغيب `chunk_range_fetch` ويطبع تحذيرات عند تجاهل capacidades غير المعروفة.
     Utilice JSON y utilice archivos JSON.
4. **تجهيز التجديدات.**
   - Sobres de من نوع `ProviderAdmissionRenewalV1` قبل 30 يوما على الأقل من
     cumplimiento de la puerta de enlace (R2). يجب أن تحافظ التجديدات على الـ mango القياسي
     ومجموعة capacidades؛ Y también hay participación, puntos finales y metadatos.
5. **التواصل مع الفرق المعتمدة.**
   - يجب على ملاك SDK إطلاق نسخ تُظهر التحذيرات للمشغلين عندما تُرفض anuncios.
   - يعلن DevRel كل انتقال مرحلة؛ أدرج روابط paneles de control y منطق العتبات أدناه.
6. **Paneles de control y aplicaciones.**- Configuración de Grafana y configuración **SoraFS / Implementación del proveedor** con UID
     `sorafs-provider-admission`.
   - تأكد من أن قواعد التنبيه تشير إلى قناة `sorafs-advert-rollout` المشتركة
     في puesta en escena y producción.

## التليمترية ولوحات المعلومات

La información del fabricante es `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — يعد القبول والرفض ونتائج التحذير.
  Utilice `missing_envelope`, `unknown_capability`, `stale` y `policy_violation`.

Actualización Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
قم باستيراد الملف إلى مستودع paneles de control المشترك (`observability/dashboards`) مع
تحديث UID لمصدر البيانات فقط قبل النشر.

يُنشر اللوح تحت مجلد Grafana **SoraFS / Provider Rollout** مع UID ثابت
`sorafs-provider-admission`. قواعد التنبيه `sorafs-admission-warn` (advertencia) y
`sorafs-admission-reject` (crítico) مُعدة مسبقا لاستخدام سياسة الإشعار
`sorafs-advert-rollout`؛ عدل جهة الاتصال إذا تغيّرت قائمة الوجهات بدلا من تحرير
JSON الخاص باللوحة.

Nombres de usuario Grafana:| اللوحة | الاستعلام | الملاحظات |
|-------|-------|-------|
| **Tasa de resultados de admisión** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط مكدس لعرض aceptar vs advertir vs rechazar. تنبيه عند advertir > 0.05 * total (advertencia) o rechazar > 0 (crítico). |
| **Proporción de advertencia** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سلسلة زمنية وحيدة تغذي عتبة buscapersonas (نسبة تحذير 5% خلال 15 دقيقة). |
| **Motivos del rechazo** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Clasificación del triaje mediante runbook أرفق روابط لخطوات التخفيف. |
| **Actualizar deuda** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | تشير إلى مزودين فاتتهم مهلة التجديد؛ قارن مع سجلات caché de descubrimiento. |

Artefactos en la CLI de los paneles de control:

- `sorafs_fetch --provider-metrics-out` يكتب عدادات `failures` و `successes` و
  `disabled` Está aquí. استوردها في paneles de control ad-hoc لمراقبة ensayos en seco في
  orquestador قبل تبديل مزودي الإنتاج.
- Utilice `chunk_retry_rate` e `provider_failure_rate` para crear archivos JSON.
  estrangulamiento أو أعراض cargas útiles obsoletas التي غالبا ما تسبق رفض القبول.

### تخطيط لوحة Grafana

تنشر Observabilidad لوحة مخصصة — **SoraFS Admisión del proveedor
Lanzamiento** (`sorafs-provider-admission`) — Aquí **SoraFS / Lanzamiento del proveedor**
مع معرفات اللوحات القياسية التالية:- Panel 1: *Tasa de resultados de admisión* (área apilada, وحدة "ops/min").
- Panel 2 — *Relación de advertencia* (serie única) ، مع التعبير
  `suma(tasa(torii_sorafs_admission_total{result="warn"}[5m])) /
   suma(tasa(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motivos de rechazo* (serie temporal مجمعة حسب `reason`)، مرتبة حسب
  `rate(...[5m])`.
- Panel 4 — *Actualizar deuda* (estadística)، يعكس الاستعلام أعلاه ومُعلق بمهل actualizar
  المستخرجة من libro mayor de migración.

انسخ (أو أنشئ) Esqueleto JSON في مستودع لوحات البنية التحتية
`observability/dashboards/sorafs_provider_admission.json`, muestra el código UID
البيانات؛ معرفات اللوحات وقواعد التنبيه يُرجع إليها runbooks أدناه، لذا تجنب
إعادة ترقيمها دون تحديث هذا المستند.

للتسهيل، يوفر المستودع تعريفا مرجعيا للوحة في
`docs/source/grafana_sorafs_admission.json`؛ Nombre del usuario: Grafana
كنقطة انطلاق للاختبارات المحلية.

### قواعد تنبيه Prometheus

أضف مجموعة القواعد التالية إلى
`observability/prometheus/sorafs_admission.rules.yml` (أنشئ الملف إن كانت هذه
أول مجموعة قواعد SoraFS) and ضمنها في إعدادات Prometheus. Producto `<pagerduty>`
بعلامة التوجيه الفعلية لدوام المناوبة.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Número `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
Para obtener más información, consulte el enlace `promtool check rules`.

## مصفوفة التوافق| خصائص anuncio | R0 | R1 | R2 | R3 |
|--------------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` Otros alias قياسية, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Capacidad de غياب `chunk_range_fetch` | ⚠️ Avisar (ingesta + telemetría) | ⚠️ Advertir | ❌ Rechazar (`reason="missing_capability"`) | ❌ Rechazar |
| Capacidad de TLV según `allow_unknown_capabilities=true` | ✅ | ⚠️ Advertir (`reason="unknown_capability"`) | ❌ Rechazar | ❌ Rechazar |
| `refresh_deadline` Hombre | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar |
| `signature_strict=false` (accesorios de diagnóstico) | ✅ (للتطوير فقط) | ⚠️ Advertir | ⚠️ Advertir | ❌ Rechazar |

كل الأوقات بتوقيت UTC. تواريخ aplicación de la ley معكوسة في libro mayor de migración ولن تتغير
بدون تصويت المجلس؛ أي تغيير يتطلب تحديث هذا الملف والـ libro mayor في نفس PR.

> **Mostrar información:** Para R1, `result="warn"`.
> `torii_sorafs_admission_total`. تتبع رقعة ingest في Torii التي تضيف هذه التسمية
> ضمن مهام تليمترية SF-2؛ وحتى ذلك الحين استخدم أخذ عينات من السجلات لمراقبة

## التواصل ومعالجة الحوادث- **رسالة حالة أسبوعية.** يرسل DevRel ملخصا موجزا لمقاييس القبول والتحذيرات
  والمواعيد القادمة.
- **استجابة للحوادث.** إذا انطلقت تنبيهات `reject`, يقوم de guardia بما يلي:
  1. جلب anuncio المخالف عبر descubrimiento في Torii (`/v2/sorafs/providers`).
  2. إعادة تشغيل تحقق anuncio في tubería المزود ومقارنة النتائج مع
     `/v2/sorafs/providers` لإعادة إنتاج الخطأ.
  3. التنسيق مع المزود لتدوير anuncio قبل اقتراب مهلة actualizar التالية.
- **تجميد التغييرات.** لا تغيرات على esquema للـ capacidades خلال R1/R2 ما لم
  يوافق عليها فريق lanzamiento؛ يجب جدولة تجارب GREASE ضمن نافذة الصيانة الأسبوعية
  وتسجيلها في libro de migración.

## المراجع

- [SoraFS Protocolo de cliente/nodo](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de admisión de proveedores](./provider-admission-policy)
- [Hoja de ruta de migración](./migration-roadmap)
- [Extensiones de múltiples fuentes de anuncios de proveedores](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)