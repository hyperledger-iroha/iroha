---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-carril-predeterminado-inicio rápido
título: البدء السريع لـ lane الافتراضي (NX-5)
sidebar_label: البدء السريع لـ lane الافتراضي
descripción: Utilice el respaldo del carril de respaldo de Nexus y el Torii y los SDK de lane_id de carriles.
---

:::nota المصدر الرسمي
Utilice el código `docs/source/quickstart/default_lane.md`. حافظ على النسختين متطابقتين حتى يصل مسح التوطين إلى البوابة.
:::

# البدء السريع لـ carril الافتراضي (NX-5)

> **سياق خارطة الطريق:** NX-5 - تكامل carril العام الافتراضي. Utilice el respaldo `nexus.routing_policy.default_lane` para instalar REST/gRPC en Torii y SDK con `lane_id`. بأمان عندما تنتمي الحركة إلى lane العام canonical. يوجه هذا الدليل المشغلين لإعداد الكتالوج، والتحقق من fallback في `/status`, y اختبار سلوك العميل من البداية للنهاية.

## المتطلبات المسبقة

- Nombre Sora/Nexus de `irohad` (tipo `irohad --sora --config ...`).
- Para obtener más información, consulte el enlace `nexus.*`.
- `iroha_cli` مهيأ للتحدث معنقود الهدف.
- `curl`/`jq` (أو ما يعادله) لفحص حمولة `/status` في Torii.

## 1. وصف كتالوج carril y espacio de datos

Hay carriles y espacios de datos aquí. El nombre de la línea (modelo `defaults/nexus/config.toml`) es el siguiente:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```يجب أن يكون كل `index` فريدا ومتتاليا. معرفات espacio de datos هي قيم 64-بت؛ وتستخدم الأمثلة أعلاه القيم الرقمية نفسها كفهارس lane للوضوح.

## 2. ضبط افتراضات التوجيه والتجاوزات الاختيارية

قسم `nexus.routing_policy` يتحكم في lane الاحتياطي ويسمح بتجاوز التوجيه لتعليمات محددة أو بادئات الحسابات. Esta es la configuración del programador `default_lane` y `default_dataspace`. El enrutador debe ser `crates/iroha_core/src/queue/router.rs` y el enrutador debe conectarse a Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

عند إضافة carriles جديدة لاحقا، حدّث الكتالوج أولا ثم وسّع قواعد التوجيه. يجب أن يظل lane الاحتياطي يشير إلى lane العام الذي يحمل غالبية حركة المستخدمين حتى تبقى SDKs القديمة متوافقة.

## 3. إقلاع عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

تسجل العقدة سياسة التوجيه المشتقة أثناء الإقلاع. تظهر أي أخطاء تحقق (فهارس مفقودة، alias مكررة، معرفات dataspace غير صالحة) قبل بدء chismes.

## 4. تأكيد حالة حوكمة carril

بمجرد أن تصبح العقدة online, استخدم أداة CLI للتحقق من أن lane الافتراضي مختوم (manifiesto محمّل) y للحركة. تعرض النظرة الملخصة صفا لكل carril:

```bash
iroha_cli app nexus lane-report --summary
```

Salida de ejemplo:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Utilice el carril `sealed` para ejecutar el runbook y el carril del servidor. Consulte `--fail-on-sealed` desde CI.

## 5. فحص حمولة حالة ToriiUtilice `/status` para configurar el programador y el carril. Utilice `curl`/`jq` para obtener información sobre el carril y el código de acceso del carril:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Salida de muestra:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Aquí está el programador del carril `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

هذا يؤكد أن لقطة TEU وبيانات alias ورايات manifiesto تتطابق مع الإعداد. نفس الحمولة تستخدمها لوحات Grafana لعرض lane-ingest.

## 6. اختبار افتراضات العميل

- **Rust/CLI.** `iroha_cli` y crate عميل Rust يحذفان حقل `lane_id` عندما لا تمرر `--lane-id` / `LaneSelector`. Hay un enrutador de cola como `default_lane`. Utilice el cable `--lane-id`/`--dataspace-id` en el carril correspondiente.
- **JS/Swift/Android.** أحدث إصدارات SDK تعامل `laneId`/`lane_id` كاختيارية y تعود إلى المعلنة في `/status`. حافظ على سياسة التوجيه متزامنة بين puesta en escena y producción حتى لا تحتاج تطبيقات الهاتف لإعادة تهيئة طارئة.
- **Pruebas de canalización/SSE.** مرشحات أحداث المعاملات تقبل الشرط `tx_lane_id == <u32>` (انظر `docs/source/pipeline.md`). Utilice `/v2/pipeline/events/transactions` para cambiar el carril y cambiar el carril.

## 7. المراقبة وروابط الحوكمة- `/status` incluye `nexus_lane_governance_sealed_total` y `nexus_lane_governance_sealed_aliases` con Alertmanager para mostrar el manifiesto de carril. ابق هذه التنبيهات مفعلة حتى في devnets.
- خريطة القياس للـ planificador y لوحة حوكمة lanes (`dashboards/grafana/nexus_lanes.json`) تتوقع حقول alias/slug من الكتالوج. إذا اعدت تسمية alias, اعد تسمية دلائل Kura المقابلة كي يحافظ المدققون على مسارات حتمية (متابعة تحت NX-1).
- موافقات البرلمان لـ carriles الافتراضية يجب ان تتضمن خطة rollback. El hash del manifiesto y el archivo de ejecución del runbook están relacionados con los archivos de comandos.