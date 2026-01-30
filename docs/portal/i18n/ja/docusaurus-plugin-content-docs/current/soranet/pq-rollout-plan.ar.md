---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-rollout-plan
title: خطة اطلاق ما بعد الكم SNNet-16G
sidebar_label: خطة اطلاق PQ
description: دليل تشغيلي لترقية handshake الهجين X25519+ML-KEM في SoraNet من canary الى default عبر relays وclients وSDKs.
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/soranet/pq_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-16G يستكمل اطلاق ما بعد الكم لنقل SoraNet. مفاتيح `rollout_phase` تسمح للمشغلين بتنسيق ترقية حتمية من متطلب guard في Stage A الى تغطية الاغلبية في Stage B ثم وضع PQ الصارم في Stage C دون تعديل JSON/TOML الخام لكل سطح.

يغطي هذا playbook:

- تعريفات المراحل ومفاتيح التهيئة الجديدة (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) الموصولة في codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- مواءمة flags الخاصة بـ SDK وCLI حتى يتمكن كل client من تتبع rollout.
- توقعات scheduling لكاناري relay/client ولوحات governance التي تضبط الترقية (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks للـ rollback ومراجع لدليل fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## خريطة المراحل

| `rollout_phase` | مرحلة اخفاء الهوية الفعلية | الاثر الافتراضي | الاستخدام المعتاد |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | فرض وجود guard PQ واحد على الاقل لكل circuit بينما تسخن المنظومة. | baseline واسابيع canary الاولى. |
| `ramp`          | `anon-majority-pq` (Stage B) | تحيز الاختيار نحو relays PQ لتحقيق تغطية >= ثلثين؛ تبقى relays الكلاسيكية كخيار fallback. | canaries حسب المناطق للـ relays؛ toggles معاينة SDK. |
| `default`       | `anon-strict-pq` (Stage C) | فرض circuits PQ فقط وتشديد انذارات downgrade. | الترقية النهائية بعد اكتمال telemetry وموافقة governance. |

اذا قام سطح ما ايضا بتعيين `anonymity_policy` صريحة فانها تتغلب على المرحلة لذلك المكون. حذف المرحلة الصريحة يجعلها تعتمد على قيمة `rollout_phase` حتى يتمكن المشغلون من تبديل المرحلة مرة واحدة لكل بيئة وترك clients يرثونها.

## مرجع التهيئة

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يقوم loader الخاص بالـ orchestrator بحل مرحلة fallback وقت التشغيل (`crates/sorafs_orchestrator/src/lib.rs:2229`) ويعرضها عبر `sorafs_orchestrator_policy_events_total` و `sorafs_orchestrator_pq_ratio_*`. راجع `docs/examples/sorafs_rollout_stage_b.toml` و `docs/examples/sorafs_rollout_stage_c.toml` للامثلة الجاهزة.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` يسجل الان المرحلة المحللة (`crates/iroha/src/client.rs:2315`) بحيث يمكن لاوامر المساعدة (مثل `iroha_cli app sorafs fetch`) ان تبلغ عن المرحلة الحالية مع سياسة اخفاء الهوية الافتراضية.

## الاتمتة

ادوات `cargo xtask` اثنان تقوم باتمتة توليد الجدول والتقاط artefacts.

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

   تقبل الفترات لاحقات `s` او `m` او `h` او `d`. يصدر الامر `artifacts/soranet_pq_rollout_plan.json` وملخص Markdown (`artifacts/soranet_pq_rollout_plan.md`) يمكن ارفاقه مع طلب التغيير.

2. **التقاط artefacts للتمرين مع التوقيعات**

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

   يقوم الامر بنسخ الملفات المزودة الى `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، ويحسِب digests من نوع BLAKE3 لكل artefact، ويكتب `rollout_capture.json` الذي يحتوي على metadata وتوقيع Ed25519 على الحمولة. استخدم نفس private key التي توقع محاضر fire-drill كي تتمكن governance من التحقق بسرعة.

## مصفوفة flags لـ SDK وCLI

| السطح | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` او الاعتماد على المرحلة | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` او `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

كل toggles في SDK تتطابق مع نفس stage parser المستخدم في orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`)، ما يبقي عمليات النشر متعددة اللغات في تناغم مع المرحلة المهيئة.

## قائمة scheduling للكاناري

1. **Preflight (T minus 2 weeks)**

- تاكد ان معدل brownout في Stage A اقل من 1% خلال الاسبوعين السابقين وان تغطية PQ >=70% لكل منطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - جدولة slot لمراجعة governance يوافق نافذة canary.
   - تحديث `sorafs.gateway.rollout_phase = "ramp"` في staging (تعديل JSON الخاص بالـ orchestrator واعادة النشر) وتشغيل dry-run لمسار الترقية.

2. **Relay canary (T day)**

   - ترقية منطقة واحدة في كل مرة عبر ضبط `rollout_phase = "ramp"` على orchestrator وmanifests الخاصة بالـ relays المشاركة.
   - مراقبة "Policy Events per Outcome" و"Brownout Rate" في لوحة PQ Ratchet (التي تضم الان لوحة rollout) لمدة ضعف TTL لذاكرة guard cache.
   - التقاط snapshots من `sorafs_cli guard-directory fetch` قبل وبعد التشغيل لتخزين التدقيق.

3. **Client/SDK canary (T plus 1 week)**

   - قلب `rollout_phase = "ramp"` في اعدادات client او تمرير overrides `stage-b` لدفعات SDK المحددة.
   - التقاط فروقات telemetry (`sorafs_orchestrator_policy_events_total` مجمعة حسب `client_id` و`region`) وارفاقها بسجل حوادث rollout.

4. **Default promotion (T plus 3 weeks)**

   - بعد موافقة governance، بدّل اعدادات orchestrator والـ client الى `rollout_phase = "default"` ودوّر checklist الاستعداد الموقع ضمن artefacts الاصدار.

## قائمة governance والادلة

| تغيير المرحلة | بوابة الترقية | حزمة الادلة | Dashboards وAlerts |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | معدل brownout لمرحلة Stage A اقل من 1% خلال 14 يوما، `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 لكل منطقة تمت ترقيتها، تحقق Argon2 ticket p95 < 50 ms، وحجز slot للترقية في governance. | زوج JSON/Markdown من `cargo xtask soranet-rollout-plan`، snapshots مزدوجة من `sorafs_cli guard-directory fetch` (قبل/بعد)، حزمة موقعة `cargo xtask soranet-rollout-capture --label canary`، ومحاضر canary تشير الى [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate)، `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio)، مراجع telemetry في `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | تحقق burn-in لمدة 30 يوما لتليمترية SN16، وبقاء `sn16_handshake_downgrade_total` عند baseline، و` sorafs_orchestrator_brownouts_total` صفر خلال canary العملاء، وتوثيق rehearsal للـ proxy toggle. | نص `sorafs_cli proxy set-mode --mode gateway|direct`، مخرجات `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، سجل `sorafs_cli guard-directory verify`، وحزمة موقعة `cargo xtask soranet-rollout-capture --label default`. | نفس لوحة PQ Ratchet مع لوحات SN16 downgrade الموثقة في `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | يتم تفعيله عندما ترتفع عدادات downgrade، او يفشل تحقق guard-directory، او يسجل buffer `/policy/proxy-toggle` احداث downgrade مستمرة. | Checklist من `docs/source/ops/soranet_transport_rollback.md`، سجلات `sorafs_cli guard-directory import` / `guard-cache prune`، `cargo xtask soranet-rollout-capture --label rollback`، تذاكر الحوادث، وقوالب الاشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json`، وكلا حزمتَي التنبيه (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- خزّن كل artefact تحت `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` الناتج بحيث تحتوي حزم governance على scoreboard وpromtool traces وdigests.
- ارفق digests SHA256 للادلة المرفوعة (minutes PDF، capture bundle، guard snapshots) بمحاضر الترقية حتى يمكن اعادة تشغيل موافقات Parliament دون الوصول الى بيئة staging.
- ارجع لخطة telemetry في تذكرة الترقية لاثبات ان `docs/source/soranet/snnet16_telemetry_plan.md` هي المصدر القياسي لمفردات downgrade وحدود التنبيه.

## تحديثات dashboard وtelemetry

`dashboards/grafana/soranet_pq_ratchet.json` يتضمن الان لوحة تعليقات "Rollout Plan" تربط بهذا playbook وتعرض المرحلة الحالية حتى تتمكن مراجعات governance من تاكيد المرحلة النشطة. حافظ على وصف اللوحة متزامنا مع تغييرات knobs المستقبلية.

بالنسبة للتنبيهات، تاكد من ان القواعد الحالية تستخدم تسمية `stage` حتى تثير مرحلتا canary وdefault حدود سياسة منفصلة (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks للـ rollback

### Default -> Ramp (Stage C -> Stage B)

1. اخفض orchestrator عبر `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (ومزامنة نفس المرحلة عبر اعدادات SDK) ليعود Stage B على مستوى الاسطول.
2. اجبر clients على ملف النقل الامن عبر `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` مع التقاط النص كي يبقى workflow المعالجة `/policy/proxy-toggle` قابلا للتدقيق.
3. شغّل `cargo xtask soranet-rollout-capture --label rollback-default` لارشفة diffs الخاصة بـ guard-directory ومخرجات promtool ولقطات dashboards ضمن `artifacts/soranet_pq_rollout/`.

### Ramp -> Canary (Stage B -> Stage A)

1. استورد snapshot الخاص بـ guard-directory الذي تم التقاطه قبل الترقية عبر `sorafs_cli guard-directory import --guard-directory guards.json` ثم اعِد تشغيل `sorafs_cli guard-directory verify` كي يتضمن packet الخفض hashes.
2. اضبط `rollout_phase = "canary"` (او override بـ `anonymity_policy stage-a`) على orchestrator وconfigs للـ client، ثم اعد تشغيل PQ ratchet drill من [PQ ratchet runbook](./pq-ratchet-runbook.md) لاثبات مسار downgrade.
3. ارفق لقطات PQ Ratchet وtelemetry SN16 المحدثة مع نتائج التنبيهات في سجل الحوادث قبل اخطار governance.

### تذكيرات guardrail

- ارجع الى `docs/source/ops/soranet_transport_rollback.md` عند حدوث اي خفض وسجل اي mitigation مؤقت كعنصر `TODO:` في rollout tracker للمتابعة.
- حافظ على `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` تحت تغطية `promtool test rules` قبل وبعد اي rollback لتوثيق drift في التنبيهات مع حزمة capture.
