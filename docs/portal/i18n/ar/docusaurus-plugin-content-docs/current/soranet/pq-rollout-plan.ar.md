---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة الطرح PQ
العنوان: خطة الاتفاق ما بعد الكم SNNet-16G
Sidebar_label: خطة التخطيط PQ
الوصف: دليل تشغيلي للمصافحة الهجين X25519+ML-KEM في SoraNet من الكناري إلى الافتراضي عبر المرحلات والعملاء وSDKs.
---

:::ملاحظة المصدر القياسي
احترام هذه الصفحة `docs/source/soranet/pq_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-16G يستكمل ما بعد الترجمة من SoraNet. تتيح المفاتيح `rollout_phase` للمشغلين إضافة ترقية حتمية من مطلوب حارس في المرحلة A الى تغطية الاغلبية في المرحلة B ثم وضع PQ سكاي في المرحلة C دون تعديل JSON/TOML الخام لكل سطح.

يغطي هذا كتاب اللعب:

- تعريفات المراحل ومفاتيح التهيئة الجديدة (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) الموصولة في codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- إشارات موامة خاصة بـ SDK وCLI حتى يبدأ كل عميل من تتبع الطرح.
- توقعات الجدولة لكاناري تتابع/العميل ولوحات الحكم التي تضبط الترقية (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks للـ rollback ومراجع لدليل fire-drill ([PQratchet runbook](./pq-ratchet-runbook.md)).

## خريطة المعالم| `rollout_phase` | مرحلة اخفاء الحجة | الاثر الافتراضي | استخداماتها |
|-----------------|-------------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (المرحلة أ) | يفترض وجود حارس PQ واحد على كل دائرة بينما تسخن المنظومة. | خط الأساس واسابيع الكناري الاولى. |
| `ramp` | `anon-majority-pq` (المرحلة ب) | آخر الاختيار نحو مرحلات PQ لكثافة > = ثلثين؛ تستمر المرحلات الكلاسيكية كخيار احتياطي. | جزر الكناري حسب المناطق للـ المرحلات؛ تبديل عرض SDK. |
| `default` | `anon-strict-pq` (المرحلة ج) | تفترض دوائر PQ فقط وتشديد الانذارات. | الترقية النهائية بعد القانون القياس عن بعد وموافقة الحكم. |

اذا قام سطح ما ايضا `anonymity_policy` صريحة انها تتغلب على المسرح لذلك المستخدم. حذف مرحلة الصريحة تعتمد على القيمة `rollout_phase` حتى يبدأون من تغيير المرحلة مرة واحدة لكل بيئة وترك العملاء يرثونها.

## مرجع التهيئة

### المنسق (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يقوم محمل الخاص بالـ Orchestrator بحل مرحلة التراجع وقت التشغيل (`crates/sorafs_orchestrator/src/lib.rs:2229`) ويعرضها عبر `sorafs_orchestrator_policy_events_total` و `sorafs_orchestrator_pq_ratio_*`. راجع `docs/examples/sorafs_rollout_stage_b.toml` و `docs/examples/sorafs_rollout_stage_c.toml` للامثلة الجديدة.

### عميل الصدأ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` سجلت الان المرحلة المبتكرة (`crates/iroha/src/client.rs:2315`) بحيث يمكن لاوامر المساعدة (مثل `iroha_cli app sorafs fetch`) ان بدأت في المرحلة الحالية مع بناء اخفاء افتراضي بنجاح.

##التمتةادوات `cargo xtask` مزدوجة يمكنك القدرة على توليد الجدول والقاطع المصنوعات اليدوية.

1. **توليد العصيريمي**

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

   تقبل الفترات لاحقاً `s` او `m` او `h` او `d`. يصدر الأمر `artifacts/soranet_pq_rollout_plan.json` وملخص Markdown (`artifacts/soranet_pq_rollout_plan.md`) ويمكن ارفاقه بطلب التغيير.

2. **تقاط التحف للتمرين مع التوقيعات**

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

   يقوم الأمر بنسخ الملفات المزودة الى `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، ويحسِب هضم من نوع BLAKE3 لكل قطعة أثرية، ويكتب `rollout_capture.json` الذي يحتوي على البيانات الوصفية والتوقيع Ed25519 على الحمولة. استخدم نفس المفتاح الخاص الذي توقعه محاضر fire-drill كي أساسي للحوكمة من التحقق بسرعة.

## أعلام مصفوفة لـ SDK وCLI| السطح | الكناري (المرحلة أ) | المنحدر (المرحلة ب) | الافتراضي (المرحلة ج) |
|---------|------------------|----------------|-------------------|
| جلب `sorafs_cli` | `--anonymity-policy stage-a` او الاعتماد على المرحلة | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| التكوين المنسق JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| تكوين عميل الصدأ (`iroha.toml`) | `rollout_phase = "canary"` (افتراضي) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` الأوامر الموقعة | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جافا/أندرويد `GatewayFetchOptions` | `setRolloutPhase("canary")`، اختياري `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`، اختياري `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`، اختياري `.ANON_STRICT_PQ` |
| مساعدين منسق جافا سكريبت | `rolloutPhase: "canary"` او `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| بايثون `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سويفت `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

تتطابق كل التبديلات في SDK مع نفس Stage Parser User في الأوركسترا (`crates/sorafs_orchestrator/src/lib.rs:365`)، ما يبقي عمليات النشر لغات متعددة في الموسيقى مع الموسيقى المهيئة.

## قائمة جدولة الكناري

1. **التجربة المبدئية (T ناقص أسبوعين)**- تأكد أن معدل انقطاع التيار الكهربائي في المرحلة A أقل من 1% خلال الأسبوعين السابقين وتغطية PQ >=70% لكل منطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - جدولة فتحة لمراجعة الحكم وتوافق على نافذة كناري.
   - تحديث `sorafs.gateway.rollout_phase = "ramp"` في التدريج (تعديل JSON الخاص بالـarchistor واعادة النشر) تشغيل التشغيل الجاف لمسار الترقية.

2. **تتابع الكناري (يوم T)**

   - منطقة واحدة في كل مرة عبر ضبط `rollout_phase = "ramp"` على منسق وبيانات خاصة بالـ Relays Share.
   - "Policy Events per Outcome" و"Brownout Rate" في لوحة PQ Ratchet (التي تستخدم الان لوحة الطرح) لضعف TTL لذاكرة التخزين المؤقت.
   - التقاط لقطات من `sorafs_cli guard-directory fetch` قبل وبعد للعمل للتأمل.

3. ** كناري العميل/SDK (T بالإضافة إلى أسبوع واحد)**

   - قلب `rollout_phase = "ramp"` في إعدادات العميل او تسارع إلى تجاوز `stage-b` لدفعات SDK المحددة.
   - التقاط فروقات القياس عن بعد (`sorafs_orchestrator_policy_events_total` مجمعة حسب `client_id` و`region`) واصحابها بسجل حوادث الطرح.

4. **العرض الترويجي الافتراضي (T بالإضافة إلى 3 أسابيع)**

   - بعد موافقة الحكم، بدّل إعدادات المنسق والـ العميل الى `rollout_phase = "default"` ودوّر قائمة المراجعة للموقع ضمن القطع الأثرية الاصدار.

## قائمة الحوكمة والمساواة| تغيير المرحلة | بوابة الترقية | الحزمة المعادلة | لوحات المعلومات والتنبيهات |
|--------------|----------------|---------------------------------|-----|
| كناري -> المنحدر *(معاينة المرحلة ب)* | معدل التراجع لمرحلة المرحلة أ أقل من 1% خلال 14 يوما، `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 لكل منطقة تمت ترقيتها، تحقق تذكرة Argon2 p95  الافتراضي *(فرض المرحلة C)* | تحقيق الاحتراق لمدة 30 يومًا لتليمترية SN16، والبقاء `sn16_handshake_downgrade_total` عند خط الأساس، و` sorafs_orchestrator_brownouts_total` صفر من خلال عملاء canary، وتوثيق بروفة للـ proxy toggle. | نص `sorafs_cli proxy set-mode --mode gateway|direct`، مخرجات `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، سجل `sorafs_cli guard-directory verify`، وزمة موقعة `cargo xtask soranet-rollout-capture --label default`. | نفس لوحة PQ Ratchet مع لوحات SN16 downgrade الموثقة في `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. || خفض رتبة الطوارئ / الاستعداد للتراجع | تم تفعيله عندما تم تمهيد عدادات الرجوع إلى إصدار أقدم، أو يفشل في التحقق من Guard-directory، أو يتم تسجيل المخزن المؤقت `/policy/proxy-toggle` احداث الرجوع إلى إصدار أقدم باستمرار. | قائمة التحقق من `docs/source/ops/soranet_transport_rollback.md`، السجلات `sorafs_cli guard-directory import` / `guard-cache prune`، `cargo xtask soranet-rollout-capture --label rollback`، التذاكر المسبقة، وقوالب الاشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json`، وكلا حزمالتنبيه (`dashboards/alerts/soranet_handshake_rules.yml`، `dashboards/alerts/soranet_privacy_rules.yml`). |

- خزين كل قطعة أثرية تحت `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` الناتجة بحيث تحتوي على حزم الحكم على لوحة النتائج وpromtool آثار وملخصات.
- ارفق ملخصات SHA256 للمساواة المرفوعة (دقائق PDF، التقاط الحزمة، لقطات الحراسة) بمحاضر الترقية حتى يمكن إعادة تشغيل موافقات البرلمان دون الوصول إلى بيئة التدريج.
- ارجع لخطة القياس عن بعد في تذكرة الترقية لا ثبات ان `docs/source/soranet/snnet16_telemetry_plan.md` هي المصدر القياسي لمفردات الرجوع إلى إصدار أقدم وحدود التنبيه.

## تحديثات لوحة القيادة والقياس عن بعد

`dashboards/grafana/soranet_pq_ratchet.json` تشمل الان لوحة تعليقات "Rollout Plan" خاصة بهذا playbook و تعرض المرحلة الحالية حتى الان مراجعات الحوكمة من تاكيد المرحلة الخاصة. حافظ على وصف اللوحة متزامنا مع المقابض المستقبلية.

للتنبيهات، تأكد من ان معلومات حالية تستخدم التسمية `stage` حتى مرحلتا الكناري والافتراضي حدود للدولة (`dashboards/alerts/soranet_handshake_rules.yml`).

## خطافات للـ rollback

### الافتراضي -> المنحدر (المرحلة ج -> المرحلة ب)1. اخفض الأوركسترا عبر `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (وتردد نفس المرحلة عبر إعدادات SDK) ليعود المرحلة B على مستوى الاسطول.
2. جبر العملاء على ملف النقل الشرطي عبر `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` مع التقاط النص كي يبقى سير العمل CPU `/policy/proxy-toggle` قابلاً للتدقيق.
3. شغّل `cargo xtask soranet-rollout-capture --label rollback-default` لارشفة diffs الخاصة بـguard-directory ومخرجات promtool ولقطات Dashboard ضمن `artifacts/soranet_pq_rollout/`.

### المنحدر -> الكناري (المرحلة ب -> المرحلة أ)

1. استورد تشغيل اللقطة الخاصة بـguard-directory الذي تم التقاطه قبل الترقية عبر `sorafs_cli guard-directory import --guard-directory guards.json` ثم اعِد `sorafs_cli guard-directory verify` كي تشمل حزمة الخفض للتجزئات.
2. تشغيل ضبط `rollout_phase = "canary"` (او override بـ `anonymity_policy stage-a`) على الأوركسترا وconfigs للـclient، ثم اعد PQ السقاطة المثقاب من [PQ Rachet runbook](./pq-ratchet-runbook.md) لا ثبات مسار الرجوع إلى إصدار أقدم.
3. ارفق لقطات PQ Ratchet وtelemetry SN16 المحدثة مع إخطارات السجل في سجل لاحق قبل اخطار الحكم.

###تذكيرات الدرابزين

- ارجع الى `docs/source/ops/soranet_transport_rollback.md` عند حدوث اي اختلاف اي تخفيف كعنصر `TODO:` في rollout Tracker للمتابعة.
- حافظ على `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` تحت كثافة `promtool test rules` قبل وبعد اي التراجع لتوثيق الانجراف في التنبيهات مع حزمة التقاط.