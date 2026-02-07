---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-despliegue-pq
título: خطة اطلاق ما بعد الكم SNNet-16G
sidebar_label: خطة اطلاق PQ
descripción: Este es un protocolo de enlace de X25519+ML-KEM con SoraNet como canario y por defecto con relés, clientes y SDK.
---

:::nota المصدر القياسي
Utilice el botón `docs/source/soranet/pq_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-16G está conectado a SoraNet. مفاتيح `rollout_phase` تسمح للمشغلين بتنسيق ترقية حتمية من متطلب guard في Stage A الى تغطية الاغلبية في Stage B ثم وضع PQ La etapa C se realiza mediante archivos JSON/TOML.

Este es el libro de jugadas:

- تعريفات المراحل ومفاتيح التهيئة الجديدة (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) الموصولة في codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Las banderas del SDK y la CLI se implementan en el cliente durante el lanzamiento.
- توقعات programación لكاناري retransmisión/cliente y ولوحات gobernanza التي تضبط الترقية (`dashboards/grafana/soranet_pq_ratchet.json`).
- Ganchos para retroceso y simulacro de incendio ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## خريطة المراحل| `rollout_phase` | مرحلة اخفاء الهوية الفعلية | الاثر الافتراضي | الاستخدام المعتاد |
|-----------------|---------------------|----------------|-----------------------|
| `canary` | `anon-guard-pq` (Etapa A) | فرض وجود guard PQ واحد على الاقل لكل circuito بينما تسخن المنظومة. | línea de base واسابيع canario الاولى. |
| `ramp` | `anon-majority-pq` (Etapa B) | تحيز الاختيار نحو relés PQ لتحقيق تغطية >= ثلثين؛ تبقى relés الكلاسيكية كخيار respaldo. | Canarias حسب المناطق للـ relés؛ alterna معاينة SDK. |
| `default` | `anon-strict-pq` (Etapa C) | فرض circuitos PQ فقط وتشديد انذارات degradación. | الترقية النهائية بعد اكتمال telemetría y gobernanza. |

Asegúrese de que el cable de alimentación `anonymity_policy` esté conectado a una fuente de alimentación. حذف المرحلة يجعلها تعتمد على قيمة `rollout_phase` حتى يتمكن المشغلون من تبديل المرحلة مرة واحدة لكل بيئة وترك clientes يرثونها.

## مرجع التهيئة

### Orquestador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Este es el cargador, el orquestador, el respaldo y el respaldo (`crates/sorafs_orchestrator/src/lib.rs:2229`) y `sorafs_orchestrator_policy_events_total` y `sorafs_orchestrator_pq_ratio_*`. راجع `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` للامثلة الجاهزة.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` يسجل الان المرحلة المحللة (`crates/iroha/src/client.rs:2315`) بحيث يمكن لاوامر المساعدة (مثل `iroha_cli app sorafs fetch`) ان تبلغ عن المرحلة الحالية مع سياسة اخفاء الهوية الافتراضية.

## الاتمتةادوات `cargo xtask` اثنان تقوم باتمتة توليد الجدول والتقاط artefactos.

1. **توليد الجدول الاقليمي**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Utilice `s`, `m`, `h` y `d`. El producto `artifacts/soranet_pq_rollout_plan.json` y el Markdown (`artifacts/soranet_pq_rollout_plan.md`) están disponibles en el mercado.

2. **التقاط artefactos للتمرين مع التوقيعات**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   يقوم الامر بنسخ الملفات المزودة الى `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, ويحسِب digests من نوع BLAKE3 لكل artefacto, ويكتب `rollout_capture.json` الذي يحتوي على metadatos وتوقيع Ed25519 على الحمولة. استخدم نفس clave privada التي توقع محاضر simulacro de incendio كي تتمكن gobernanza من التحقق بسرعة.

## Banderas adicionales para SDK y CLI| السطح | Canarias (Etapa A) | Rampa (Etapa B) | Incumplimiento (Etapa C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` او الاعتماد على المرحلة | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuración del orquestador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuración del cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (predeterminado) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos firmados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ayudantes del orquestador de JavaScript | `rolloutPhase: "canary"` y `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Pitón `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rápido `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Esto alterna entre el SDK y el analizador de escenario y el orquestador (`crates/sorafs_orchestrator/src/lib.rs:365`), y activa las funciones del SDK. المرحلة المهيئة.

## قائمة programación للكاناري

1. **Verificación previa (T menos 2 semanas)**- تاكد ان معدل brownout في Stage A اقل من 1% خلال الاسبوعين السابقين وان تغطية PQ >=70% لكل منطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - جدولة ranura لمراجعة gobernanza يوافق نافذة canario.
   - تحديث `sorafs.gateway.rollout_phase = "ramp"` para puesta en escena (تعديل JSON الخاص بالـ Orchestrator واعادة النشر) y ejecución en seco de لمسار الترقية.

2. **Relevo canario (día T)**

   - ترقية منطقة واحدة في كل مرة عبر ضبط `rollout_phase = "ramp"` على orquestador وmanifests الخاصة بالـ relés المشاركة.
   - Haga clic en "Eventos de política por resultado" y "Tasa de apagones" en PQ Ratchet (implementación de la aplicación) y en caché de protección TTL.
   - Instantáneas de التقاط من `sorafs_cli guard-directory fetch` قبل وبعد التشغيل لتخزين التدقيق.

3. **Cliente/SDK canario (T más 1 semana)**

   - Al cambiar `rollout_phase = "ramp"` al cliente y anular el código SDK `stage-b`.
   - Telemetría de telemetría (`sorafs_orchestrator_policy_events_total` مجمعة حسب `client_id` و`region`) y lanzamiento de بسجل حوادث.

4. **Promoción predeterminada (T más 3 semanas)**

   - Gobernanza de بعد موافقة, بدّل اعدادات orquestador y cliente الى `rollout_phase = "default"` y lista de verificación الاستعداد الموقع ضمن artefactos الاصدار.

## قائمة gobernanza y والادلة| تغيير المرحلة | بوابة الترقية | حزمة الادلة | Paneles de control y alertas |
|----------------------|----------------|-----------------|---------------------|
| Canarias -> Rampa *(vista previa de la etapa B)* | Fallo de tensión en la etapa A desde 1% hasta 14 minutos `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 desde el nivel de entrada de Argon2 p95  Predeterminado *(aplicación de la Etapa C)* | Burn-in de 30 días, SN16, `sn16_handshake_downgrade_total` y baseline, y ` sorafs_orchestrator_brownouts_total`, para canary العملاء, y ensayo للـ alternancia de proxy. | Aquí está `sorafs_cli proxy set-mode --mode gateway|direct`, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, `sorafs_cli guard-directory verify` y `cargo xtask soranet-rollout-capture --label default`. | Utilice PQ Ratchet para reducir la versión SN16 de `docs/source/sorafs_orchestrator_rollout.md` e `dashboards/grafana/soranet_privacy_metrics.json`. || Degradación de emergencia/preparación para reversión | Esta es una degradación del directorio de guardia y una degradación del buffer `/policy/proxy-toggle`. | Lista de verificación de `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, تذاكر الحوادث، وقوالب الاشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json`, y `dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`. |

- خزّن كل artefacto تحت `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` الناتج بحيث تحتوي حزم gobernanza على marcador وpromtool rastros y resúmenes.
- ارفق resume SHA256 للادلة المرفوعة (minutos PDF, paquete de captura, instantáneas de guardia) بمحاضر الترقية حتى يمكن اعادة تشغيل موافقات Parlamento دون الوصول الى بيئة puesta en escena.
- ارجع لخطة telemetry في تذكرة الترقية لاثبات ان `docs/source/soranet/snnet16_telemetry_plan.md` هي المصدر القياسي لمفردات downgrade y التنبيه.

## تحديثات tablero y telemetría

`dashboards/grafana/soranet_pq_ratchet.json` يتضمن الان لوحة تعليقات "Plan de implementación" تربط بهذا playbook وتعرض المرحلة الحالية حتى تمكن مراجعات من تاكيد المرحلة النشطة. Asegúrese de que las perillas no estén encendidas.

بالنسبة للتنبيهات، تاكد من ان القواعد الحالية تستخدم تسمية `stage` حتى تثير مرحلتا canary y default حدود سياسة منفصلة (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos para revertir

### Predeterminado -> Rampa (Etapa C -> Etapa B)1. اخفض orquestador عبر `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (ومزامنة نفس المرحلة عبر اعدادات SDK) ليعود Stage B على مستوى الاسطول.
2. Los clientes de على ملف النقل الامن عبر `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` مع التقاط النص كي يبقى flujo de trabajo المعالجة `/policy/proxy-toggle` قابلا للتدقيق.
3. Haga clic en `cargo xtask soranet-rollout-capture --label rollback-default` para crear diferencias entre el directorio de guardia, la herramienta de promoción y los paneles de control en `artifacts/soranet_pq_rollout/`.

### Rampa -> Canarias (Etapa B -> Etapa A)

1. Haga clic en la instantánea en el directorio de guardia y en el paquete `sorafs_cli guard-directory import --guard-directory guards.json` y en el paquete `sorafs_cli guard-directory verify`. Hash hashes.
2. Utilice `rollout_phase = "canary"` (y anule `anonymity_policy stage-a`) para orquestador y configuraciones para cliente, para realizar el taladro de trinquete PQ con [Runbook de trinquete PQ] (./pq-ratchet-runbook.md) لاثبات مسار degradación.
3. ارفق لقطات PQ Ratchet وtelemetry SN16 المحدثة مع نتائج التنبيهات في سجل الحوادث قبل اخطار.

### تذكيرات barandilla

- ارجع الى `docs/source/ops/soranet_transport_rollback.md` عند حدوث اي خفض وسجل اي مؤقت كعنصر `TODO:` في rollout tracker للمتابعة.
- Utilice `dashboards/alerts/soranet_handshake_rules.yml` e `dashboards/alerts/soranet_privacy_rules.yml` para eliminar la deriva de `promtool test rules` y revertir la deriva de la máquina. captura.