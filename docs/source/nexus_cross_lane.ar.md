---
lang: ar
direction: rtl
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_cross_lane.md -->

# التزامات cross-lane في Nexus وخط الاثبات

> **الحالة:** مخرج NX-4 — خط التزامات cross-lane والاثباتات (الهدف الربع الرابع 2025).  
> **المالكون:** Nexus Core WG / Cryptography WG / Networking TL.  
> **بنود خارطة الطريق ذات الصلة:** NX-1 (هندسة lane)، NX-3 (settlement router)، NX-4 (هذا المستند)، NX-8 (global scheduler)، NX-11 (SDK conformance).

تصف هذه المذكرة كيف تتحول بيانات التنفيذ لكل lane الى التزام عالمي قابل للتحقق. تربط بين settlement router الحالي (`crates/settlement_router`)، وlane block builder (`crates/iroha_core/src/block.rs`)، واسطح التلِمتري/الحالة، وخطافات LaneRelay/DA المخطط لها التي لا تزال مطلوبة لخارطة الطريق **NX-4**.

## الاهداف

- انتاج `LaneBlockCommitment` حتمي لكل lane block يلتقط settlement والسيولة وبيانات التباين دون تسريب حالة خاصة.
- ترحيل هذه الالتزامات (ومعها attestations DA) الى حلقة NPoS العالمية كي يتمكن merge ledger من ترتيب وتحقق وحفظ تحديثات cross-lane.
- عرض نفس الحمولة عبر Torii والتلِمتري كي يتمكن المشغلون وSDKs والمدققون من اعادة تشغيل الخط دون ادوات مخصصة.
- تحديد الثوابت وحزم الادلة المطلوبة لانجاز NX-4: اثباتات lane، attestations DA، دمج merge ledger، وتغطية الانحدار.

## المكونات والاسطح

| المكون | المسؤولية | مراجع التنفيذ |
|-----------|----------------|---------------------------|
| منفذ lane و settlement router | تسعير تحويلات XOR، تجميع receipts لكل معاملة، وتطبيق سياسة buffer | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| lane block builder | تفريغ `SettlementAccumulator`s واصدار `LaneBlockCommitment`s مع lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | تجميع QCs لكل lane + اثباتات DA، بثها عبر `iroha_p2p` وتغذية merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | التحقق من QCs الخاصة بالlane، تقليل merge hints، وحفظ التزامات world-state | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status ولوحات المتابعة | عرض `lane_commitments` و`lane_settlement_commitments` و`lane_relay_envelopes` وgauge الخاصة بالscheduler ولوحات Grafana | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| تخزين الادلة | ارشفة `LaneBlockCommitment`s وقطع RBC ولقطات Alertmanager للقيام بالتدقيق | `docs/settlement-router.md`, `artifacts/nexus/*` (حزمة مستقبلية) |

## هياكل البيانات وتخطيط الحمولة

توجد الحمولة القياسية في `crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` — تجزئة المعاملة او id يقدمه المستدعي.
- `local_amount_micro` — خصم رمز الغاز الخاص بالdataspace.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` — قيود XOR الحتمية وهامش الامان لكل receipt (`due - after haircut`).
- `timestamp_ms` — طابع زمني UTC بالميلي ثانية يتم التقاطه اثناء settlement.

ترث receipts قواعد التسعير الحتمية من `SettlementEngine` وتتم تجميعها داخل كل `LaneBlockCommitment`.

### `LaneSwapMetadata`

بيانات اختيارية تسجل المعلمات المستخدمة اثناء التسعير:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- bucket `liquidity_profile` (Tier1-Tier3).
- سلسلة `twap_local_per_xor` كي يتمكن المدققون من اعادة حساب التحويلات بدقة.

### `LaneBlockCommitment`

ملخص لكل lane محفوظ مع كل كتلة:

- الراس: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- المجاميع: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- `swap_metadata` اختياري.
- متجه `receipts` مرتب.

هذه الهياكل تشتق بالفعل `NoritoSerialize`/`NoritoDeserialize`، لذا يمكن بثها on-chain او عبر Torii او عبر fixtures دون انحراف في المخطط.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (انظر `crates/iroha_data_model/src/nexus/relay.rs`) يجمع `BlockHeader` الخاص
بالlane، و`commit QC (`Qc`)` اختياري، وتجزيء `DaCommitmentBundle` اختياري، و`LaneBlockCommitment`
الكامل، وعدد بايتات RBC لكل lane. يحفظ الـ envelope قيمة `settlement_hash` مشتقة من Norito
(عبر `compute_settlement_hash`) كي يتمكن المستلمون من التحقق من حمولة settlement قبل تمريرها
الى merge ledger. يجب رفض الـ envelope عند فشل `verify` (عدم تطابق QC subject او DA hash او
settlement hash)، وعند فشل `verify_with_quorum` (اخطاء طول bitmap للموقعين/الquorum)، او عند
تعذر التحقق من توقيع QC المجمع مقابل roster اللجنة الخاصة بالdataspace. تغطي preimage الخاصة
بالـ QC تجزئة lane block مع `parent_state_root` و`post_state_root`، بحيث يتم التحقق من العضوية
وصحة state-root معا.

### اختيار لجنة lane

تتحقق QCs الخاصة بـ lane relay ضد لجنة لكل dataspace. حجم اللجنة هو `3f+1` حيث يتم ضبط `f` في
كتالوج dataspace (`fault_tolerance`). تجمع المدققين هو مدققو dataspace: manifests حوكمة lane
لـ lanes admin-managed وسجلات staking للـ lanes العامة في حالة stake-elected. يتم سحب عضوية
اللجنة بشكل حتمي لكل حقبة باستخدام بذرة حقبة VRF المرتبطة بـ `dataspace_id` و`lane_id` (ثابتة
خلال الحقبة). اذا كان التجمع اصغر من `3f+1` تتوقف نهائية lane relay حتى عودة quorum. يمكن
للمشغلين توسيع التجمع باستخدام تعليمة multisig الادارية `SetLaneRelayEmergencyValidators`
(يتطلب `CanManagePeers` و `nexus.lane_relay_emergency.enabled = true`، وهي معطلة افتراضيا). عند
التفعيل يجب ان تكون السلطة حساب multisig يستوفي الحدود الدنيا المضبوطة
(`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`، الافتراضي 3-of-5). يتم تخزين
الـ overrides لكل dataspace وتطبيقها فقط عندما يكون التجمع دون quorum، ويتم مسحها عند ارسال قائمة
مدققين فارغة. عندما يتم ضبط `expires_at_height` تتجاهل عملية التحقق الـ override عندما يتجاوز
`block_height` في lane relay envelope ارتفاع الانتهاء. يسجل عداد التلِمتري
`lane_relay_emergency_override_total{lane,dataspace,outcome}` ما اذا تم تطبيق الـ override
(`applied`) او كان مفقودا/منتهيا/غير كاف/معطلا اثناء التحقق.

## دورة حياة الالتزام

1. **تسعير وتجهيز receipts.**  
   تقوم واجهة settlement (`SettlementEngine`, `SettlementAccumulator`) بتسجيل `PendingSettlement`
   لكل معاملة. يخزن كل سجل مدخلات TWAP وملف السيولة والطوابع الزمنية ومبالغ XOR ليصبح لاحقا
   `LaneSettlementReceipt`.

2. **اغلاق receipts داخل الكتلة.**  
   اثناء `BlockBuilder::finalize` يقوم كل زوج `(lane_id, dataspace_id)` بتفريغ accumulator الخاص به.
   ينشئ الـ builder قيمة `LaneBlockCommitment`، وينسخ قائمة receipts، ويجمع المجاميع، ويخزن
   metadatas swap الاختيارية (عبر `SwapEvidence`). يتم دفع المتجه الناتج الى خانة حالة Sumeragi
   (`crates/iroha_core/src/sumeragi/status.rs`) كي تكشفه Torii والتلِمتري فورا.

3. **حزم relay واثباتات DA.**  
   يقوم `LaneRelayBroadcaster` الان باستهلاك `LaneRelayEnvelope`s الصادرة اثناء اغلاق الكتلة
   ويبثها كاطارات `NetworkMessage::LaneRelay` ذات اولوية عالية. يتم التحقق من الـ envelopes،
   وازالة التكرار حسب `(lane_id,dataspace_id,height,settlement_hash)`، وتخزينها في لقطة حالة
   Sumeragi (`/v2/sumeragi/status`) للمشغلين والمدققين. سيستمر الـ broadcaster بالتطور لاضافة
   artefacts DA (اثباتات chunk لـ RBC، وheaders Norito، وmanifests SoraFS/Object) وتغذية merge ring
   بدون حجب head-of-line.

4. **الترتيب العالمي و merge ledger.**  
   تتحقق حلقة NPoS من كل relay envelope: التحقق من `lane_qc` مقابل لجنة dataspace، اعادة حساب
   مجاميع settlement، التحقق من اثباتات DA، ثم تمرير tip الخاص بالlane الى merge ledger الموضح في
   `docs/source/merge_ledger.md`. عند ختم merge entry يصبح hash الخاص بـ world-state
   (`global_state_root`) ملتزما بكل `LaneBlockCommitment`.

5. **الحفظ والافصاح.**  
   يكتب Kura lane block وmerge entry و`LaneBlockCommitment` بشكل ذري كي يتمكن replay من اعادة بناء
   نفس التخفيض. يعرض `/v2/sumeragi/status`:
   - `lane_commitments` (metadata التنفيذ).
   - `lane_settlement_commitments` (الحمولة الموصوفة هنا).
   - `lane_relay_envelopes` (relay headers وQCs وDA digests وsettlement hash وعدادات بايت RBC).
  تقرأ اللوحات (`dashboards/grafana/nexus_lanes.json`) نفس اسطح التلِمتري والحالة لعرض throughput
  الخاص بالlane، وتحذيرات توفر DA، وحجم RBC، وفروقات settlement، وادلة relay.

## قواعد التحقق والاثبات

يجب على merge ring تطبيق ما يلي قبل قبول التزام lane:

1. **صلاحية lane QC.** تحقق من توقيع BLS المجمع على preimage لاصوات التنفيذ (تجزئة الكتلة،
   `parent_state_root`, `post_state_root`, الارتفاع/العرض/الحقبة، `chain_id` وmode tag) مقابل
   roster لجنة dataspace؛ وتاكد من ان طول bitmap للموقعين يطابق اللجنة، وان الموقعين يطابقون
   مؤشرات صالحة، وان ارتفاع الراس يطابق `LaneBlockCommitment.block_height`.
2. **سلامة receipts.** اعادة حساب مجاميع `total_*` من متجه receipts؛ رفض الالتزام اذا اختلفت
   المجاميع او احتوت receipts على `source_id` مكرر.
3. **سلامة metadatas swap.** التاكد من ان `swap_metadata` (ان وجدت) تطابق اعدادات settlement
   وسياسة buffer للlane.
4. **اثبات DA.** التحقق من ان اثباتات RBC/SoraFS المقدمة عبر relay تتطابق مع digest المضمن وان
   مجموعة الـ chunks تغطي كامل حمولة الكتلة (`rbc_bytes_total` في التلِمتري يجب ان يعكس ذلك).
5. **تخفيض merge.** بعد نجاح اثباتات lane، يتم ادراج tip للlane في merge ledger entry واعادة حساب
   تخفيض Poseidon2 (`reduce_merge_hint_roots`). اي عدم تطابق يوقف merge entry.
6. **التلِمتري ومسار التدقيق.** زيادة عدادات التدقيق لكل lane
   (`nexus_audit_outcome_total{lane_id,...}`) وحفظ الـ envelope كي تحتوي حزمة الادلة على البرهان
   ومسار الملاحظة معا.

## اتاحة البيانات والرصد

- **المقاييس:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}` و
  `nexus_audit_outcome_total` موجودة في `crates/iroha_telemetry/src/metrics.rs`. يجب على المشغلين
  التنبيه عند ارتفاع missing-availability (عدادات reschedule قديمة ويجب ان تبقى صفرا)، ويجب ان
  يبقى `lane_relay_invalid_total` صفرا خارج تدريبات الخصم.
- **اسطح Torii:**  
  يتضمن `/v2/sumeragi/status` قيم `lane_commitments` و`lane_settlement_commitments` ولقطات dataspace.
  `/v2/nexus/lane-config` (مخطط) سينشر هندسة `LaneConfig` حتى يتمكن العملاء من مطابقة `lane_id`
  مع تسميات dataspace.
- **لوحات المتابعة:**  
  يعرض `dashboards/grafana/nexus_lanes.json` تراكم lane واشارات توفر DA ومجاميع settlement المذكورة
  اعلاه. يجب ان ترسل التنبيهات عندما:
  - `nexus_scheduler_dataspace_age_slots` يتجاوز السياسة.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` يرتفع باستمرار.
  - `total_xor_variance_micro` ينحرف عن المعدلات التاريخية.
- **حزم الادلة:**  
  يجب على كل اصدار ارفاق صادرات `LaneBlockCommitment` ولقطات Grafana/Alertmanager وmanifests لـ DA
  relay تحت `artifacts/nexus/cross-lane/<date>/`. تصبح الحزمة مجموعة الدليل القياسية عند ارسال
  تقارير جاهزية NX-4.

## قائمة تنفيذ (NX-4)

1. **خدمة LaneRelay**
   - مخطط معرف في `LaneRelayEnvelope`؛ تم تنفيذ broadcaster في
     `crates/iroha_core/src/nexus/lane_relay.rs` وربطه باغلاق الكتل
     (`crates/iroha_core/src/sumeragi/main_loop.rs`)، مع ارسال `NetworkMessage::LaneRelay` وازالة
     التكرار لكل عقدة وحفظ الحالة.
   - حفظ artefacts relay للتدقيق (`artifacts/nexus/relay/...`).
2. **خطافات اثبات DA**
   - دمج اثباتات chunk الخاصة بـ RBC / SoraFS مع relay envelopes وتخزين المقاييس الموجزة في
     `SumeragiStatus`.
   - كشف حالة DA عبر Torii وGrafana للمشغلين.
3. **التحقق من merge ledger**
   - توسيع مدقق merge entry ليتطلب relay envelopes بدلا من raw lane headers.
   - اضافة اختبارات replay (`integration_tests/tests/nexus/*.rs`) تغذي التزامات اصطناعية داخل
     merge ledger وتثبت التخفيض الحتمي.
4. **تحديثات SDK والادوات**
   - توثيق layout Norito لـ `LaneBlockCommitment` لمستهلكي SDK
     (`docs/portal/docs/nexus/lane-model.md` يشير هنا بالفعل؛ اضف مقتطفات API).
   - fixtures الحتمية موجودة تحت `fixtures/nexus/lane_commitments/*.{json,to}`؛ شغل
     `cargo xtask nexus-fixtures` لاعادة التوليد (او `--verify` للتحقق) للعينات
     `default_public_lane_commitment` و`cbdc_private_lane_commitment` عند تغيرات المخطط.
5. **الرصد وrunbooks**
   - ربط حزمة Alertmanager للمقاييس الجديدة وتوثيق سير ادلة الاثبات في
     `docs/source/runbooks/nexus_cross_lane_incident.md` (متابعة).

اكتمال القائمة اعلاه، مع هذه المواصفة، يفي بجزء التوثيق لـ **NX-4** ويفتح بقية العمل التنفيذي.

</div>
