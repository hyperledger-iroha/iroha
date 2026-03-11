---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elástico-carril
título: تجهيز carril المرن (NX-7)
sidebar_label: تجهيز carril المرن
Descripción: Hay manifiestos de arranque en el carril Nexus y lanzamientos y lanzamientos.
---

:::nota المصدر الرسمي
Utilice el botón `docs/source/nexus_elastic_lane.md`. حافظ على النسختين متطابقتين حتى يصل مسح الترجمة الى البوابة.
:::

# مجموعة ادوات تجهيز carril المرن (NX-7)

> **عنصر خارطة الطريق:** NX-7 - ادوات تجهيز carril المرن  
> **الحالة:** الادوات مكتملة - تولد manifiestos, مقتطفات الكتالوج، حمولات Norito, اختبارات humo,
> Paquete de paquetes لاختبارات الحمل يجمع الان بوابة زمن الاستجابة لكل slot + manifiestos الادلة كي تنشر اختبارات حمل المدققين
> دون سكربتات مخصصة.

هذا الدليل يوجه المشغلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh` الذي يقوم بأتمتة توليد manifest لـ lane ومقتطفات كتالوج lane/dataspace وادلة lanzamiento. Para cambiar los carriles de Nexus (عامة او خاصة) دون تحرير عدة ملفات يدويا y دون اعادة اشتقاق هندسة الكتالوج يدويا.

## 1. المتطلبات المسبقة1. موافقة الحوكمة على alias لـ lane و dataspace ومجموعة المدققين وتحمّل الاعطال (`f`) وسياسة asentamiento.
2. قائمة نهائية بالمدققين (معرفات الحسابات) وقائمة espacios de nombres المحمية.
3. وصول الى مستودع تهيئة العقد كي تتمكن من اضافة المقتطفات المولدة.
4. مسارات لسجل manifiesta الخاص بـ carril (انظر `nexus.registry.manifest_directory` y `cache_directory`).
5. Haga clic en el botón de PagerDuty en el carril y coloque el botón en el carril.

## 2. توليد artefactos للـ carril

شغّل المساعد من جذر المستودع:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

اهم الاعلام:

- `--lane-id` Índice de datos del índice `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` están ubicados en el espacio de datos (افتراضيا يستخدم id الخاص بالlane عند الحذف).
- `--validator` يمكن تكراره او قراءته من `--validators-file`.
- `--route-instruction` / `--route-account` تصدر قواعد توجيه جاهزة للصق.
- `--metadata key=value` (او `--telemetry-contact/channel/runbook`) تلتقط جهات اتصال الـ runbook لكي تعرض اللوحات اصحابها الصحيحين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` تضيف gancho runtime-upgrade الى manifest عندما يحتاج lane الى ضوابط تشغيل ممتدة.
- `--encode-space-directory` يستدعي `cargo xtask space-directory encode` تلقائيا. Utilice `--space-directory-out` para conectar y desconectar `.to`.

ينتج السكربت ثلاثة artefactos داخل `--output-dir` (الافتراضي هو المجلد الحالي), مع رابع اختياري عند تفعيل codificación:1. `<slug>.manifest.json` - manifiesto de carril, quórum, espacios de nombres y enlaces de actualización de tiempo de ejecución de gancho.
2. `<slug>.catalog.toml` - مقتطف TOML يحتوي `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` واي قواعد توجيه مطلوبة. Utilice `fault_tolerance` y un espacio de datos para el relé de carril (`3f+1`).
3. `<slug>.summary.json` - ملخص تدقيق يصف الهندسة (slug والقطاعات والبيانات) وخطوات rollout المطلوبة والامر الدقيق `cargo xtask space-directory encode` (por ejemplo, `space_directory_encode.command`). ارفق هذا JSON بتذكرة incorporación كدليل.
4. `<slug>.manifest.to` - يصدر عند تفعيل `--encode-space-directory`؛ Aquí están `iroha app space-directory manifest publish` y Torii.

Utilice `--dry-run` para JSON/المقتطفات دون كتابة ملفات، e `--force` para لاعادة الكتابة فوق artefactos.

## 3. تطبيق التغييرات1. Utilice el JSON de manifiesto `nexus.registry.manifest_directory` (directorio de caché o directorio de registro remoto). التزم بالملف اذا كانت manifiesta تُدار بالنسخ في مستودع التهيئة.
2. الحق مقتطف الكتالوج في `config/config.toml` (او `config.d/*.toml` المناسب). Utilice el `nexus.lane_count` y el `lane_id + 1` y el `nexus.routing_policy.rules` en el carril.
3. شفّر (اذا تجاوزت `--encode-space-directory`) y manifiesto del directorio espacial باستخدام الامر الملتقط في resumen (`space_directory_encode.command`). Esta es la carga útil `.manifest.to` y la otra es Torii y la configuración del sistema. Aquí está `iroha app space-directory manifest publish`.
4. Haga clic en `irohad --sora --config path/to/config.toml --trace-config` y realice el seguimiento del lanzamiento. هذا يثبت ان الهندسة الجديدة تطابق slug/قطاعات Kura المولدة.
5. اعد تشغيل المدققين المخصصين للlane بعد نشر تغييرات manifest/الكتالوج. احتفظ بملف resumen JSON في التذكرة للتدقيقات المستقبلية.

## 4. Paquete بناء لتوزيع السجل

قم بتجميع manifiesto المولد y superposición لكي يتمكن المشغلون من توزيع بيانات حوكمة lanes دون تعديل configs على كل مضيف. Paquete completo de manifiestos, configuración y superposición de archivos comprimidos en `nexus.registry.cache_directory` لنقل sin conexión:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

المخرجات:1. `manifests/<slug>.manifest.json` - انسخها الى `nexus.registry.manifest_directory` المهيأ.
2. `cache/governance_catalog.json` - ضعها في `nexus.registry.cache_directory`. كل ادخال `--module` يصبح تعريفا لوحدة قابلة للتبديل، ما يتيح تبديل وحدات الحوكمة (NX-2) عبر تحديث overlay Esta es la versión `config.toml`.
3. `summary.json`: incluye hashes, superposición y errores.
4. اختياري `registry_bundle.tar.*` - جاهز لـ SCP او S3 او متبعات artefactos.

زامن المجلد بالكامل (او الارشيف) لكل مدقق، وفكّه على hosts معزولة، وانسخ manifiestos + superposición الكاش الى مسارات السجل قبل Utilice el código Torii.

## 5. اختبارات humo للمدققين

بعد اعادة تشغيل Torii, شغّل مساعد smoke الجديد للتحقق من ان ان lane يبلّغ `manifest_ready=true`, وان المقاييس تعرض عدد carriles المتوقع، وان مقياس sellado خال. يجب ان تعرض lanes التي تتطلب manifiestos قيمة `manifest_path` غير فارغة؛ Para obtener más información, consulte el manifiesto del NX-7:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

اضف `--insecure` عند اختبار بيئات autofirmado. يخرج السكربت برمز غير صفري اذا كانت carril مفقودة او sellado او اذا انحرفت المقاييس/القياس عن القيم المتوقعة. Utilice `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` y `--max-headroom-events` en el carril (ارتفاع الكتلة/النهائية/backlog/headroom) ضمن حدود التشغيل، واربطها مع `--max-slot-p95` / `--max-slot-p99` (مع `--min-slot-samples`) لفرض اهداف Utilice la ranura del NX-18 para acceder a ella.Los dispositivos con espacio de aire (CI) que se encuentran en el punto final Torii están conectados al punto final:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Los accesorios son `fixtures/nexus/lanes/`, son artefactos, son necesarios el bootstrap y son los manifiestos de pelusa los que son necesarios. تنفذ CI نفس التدفق عبر `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) لاثبات ان مساعد smoke الخاص بـ NX-7 يبقى متوافقا مع صيغة payload المنشورة وللتأكد من ان resúmenes/superposiciones الخاصة بالbundle قابلة لاعادة الانتاج.