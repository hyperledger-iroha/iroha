---
lang: ar
direction: rtl
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/sumeragi_randomness_evidence_runbook.md -->

# دليل العشوائية والادلة في Sumeragi

يلبي هذا الدليل بند Milestone A6 في خارطة الطريق والذي يتطلب تحديث اجراءات
المشغلين لعشوائية VRF وادلة slashing. استخدمه مع {doc}`sumeragi` و
{doc}`sumeragi_chaos_performance_runbook` عند تجهيز نسخة مدقق جديدة او جمع
artefacts الجاهزية من اجل governance.


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## النطاق والمتطلبات

- ضبط `iroha_cli` على العنقود المستهدف (انظر `docs/source/cli.md`).
- استخدام `curl`/`jq` لسحب حمولة `/status` من Torii عند تجهيز المدخلات.
- وصول الى Prometheus (او snapshot exports) لمقاييس `sumeragi_vrf_*`.
- معرفة epoch الحالية والroster حتى تطابق مخرجات CLI مع staking snapshot
  او manifesto governance.

## 1. تاكيد اختيار النمط وسياق epoch

1. شغل `iroha sumeragi params --summary` لاثبات ان الباينري حمل
   `sumeragi.consensus_mode="npos"` ولتسجيل `k_aggregators`,
   `redundant_send_r`, طول epoch، و offsets الالتزام/الكشف للـ VRF.
2. افحص منظور runtime:

   ```bash
   iroha sumeragi status --summary
   iroha sumeragi collectors --summary
   iroha sumeragi rbc status --summary
   ```

   سطر `status` يطبع tuple القائد/العرض، backlog الخاص بـ RBC، محاولات DA،
   offsets الخاصة بالepoch، و deferrals للـ pacemaker؛ و `collectors` تربط
   فهارس collectors بهويات peers لتوضيح اي المدققين يحملون مهام العشوائية
   عند الارتفاع المفحوص.
3. التقط رقم epoch الذي تريد تدقيقه:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   احتفظ بالقيمة (decimal او مع بادئة `0x`) لاوامر VRF ادناه.

## 2. التقاط سجلات VRF وعقوبات epoch

استخدم اوامر CLI المخصصة لسحب سجلات VRF المحفوظة من كل مدقق:

```bash
iroha sumeragi vrf-epoch --epoch "$EPOCH" --summary
iroha sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha sumeragi vrf-penalties --epoch "$EPOCH" --summary
iroha sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

توضح الملخصات ما اذا كانت epoch نهائية، وعدد المشاركين الذين قدموا commits/reveals،
وطول roster، والseed المشتق. يسجل JSON قائمة المشاركين، وحالة العقوبات لكل signer،
وقيمة `seed_hex` المستخدمة في pacemaker. قارن عدد المشاركين مع roster staking، وتحقق
من ان مصفوفات العقوبات تعكس التنبيهات التي ظهرت خلال اختبارات chaos (late reveals تظهر
تحت `late_reveals`، والمدققون forfeited تحت `no_participation`).

## 3. مراقبة telemetria VRF والتنبيهات

Prometheus يعرض العدادات المطلوبة في roadmap:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

مثال PromQL للتقرير الاسبوعي:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

خلال تدريبات readiness تاكد من ان:

- `sumeragi_vrf_commits_emitted_total` و `..._reveals_emitted_total` ترتفع لكل بلوك
  داخل نوافذ commit/reveal.
- سيناريوهات late-reveal ترفع `sumeragi_vrf_reveals_late_total` وتمحو الادخال المقابل
  في JSON `vrf_penalties`.
- `sumeragi_vrf_no_participation_total` يرتفع فقط عندما تحجب commits عمدا خلال اختبارات chaos.

لوحة Grafana (`docs/source/grafana_sumeragi_overview.json`) تتضمن لوحات لكل عداد؛ التقط
لقطات شاشة بعد كل تشغيل وارفقها بحزمة artefacts المشار اليها في
{doc}`sumeragi_chaos_performance_runbook`.

## 4. ادخال الادلة وبثها

يجب جمع ادلة slashing في كل مدقق وترحيلها الى Torii. استخدم مساعدات CLI لاثبات
التكافؤ مع HTTP endpoints الموثقة في {doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha sumeragi evidence count --summary
iroha sumeragi evidence list --summary --limit 5

# Show JSON for audits
iroha sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

تحقق من ان `total` المبلغ يطابق ودجت Grafana المغذى بـ `sumeragi_evidence_records_total`،
وتاكد من رفض السجلات الاقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks`
(يطبع CLI سبب الاسقاط). عند اختبار alerting، ارسل payload صالحا معروفا عبر:

```bash
iroha sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex --summary
```

راقب `/v1/events/sse` مع stream مفلتر لاثبات ان SDKs ترى نفس البيانات: استخدم
الـ Python one-liner من {doc}`torii/sumeragi_evidence_app_api` لبناء الفلتر
والتقط اطارات `data:` الخام. يجب ان تعكس حمولات SSE نوع الدليل signer الذي
ظهر في مخرجات CLI.

## 5. تجميع الادلة والتقرير

لكل rehearsal او release candidate:

1. خزّن ملفات JSON من CLI (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) تحت مجلد artefacts الخاص بالتشغيل (نفس الجذر
   المستخدم في سكربتات chaos/performance).
2. سجل نتائج استعلامات Prometheus او snapshot exports للعدادات المذكورة اعلاه.
3. ارفق التقاط SSE واقرارات التنبيه مع README الخاص بالartefacts.
4. حدث `status.md` و
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` بمسارات artefacts
   ورقم epoch الذي تم تدقيقه.

اتباع هذا checklist يجعل اثباتات عشوائية VRF وادلة slashing قابلة للتدقيق خلال
rollout NPoS ويعطي مراجعي governance مسارا حتميا الى المقاييس الملتقطة ولقطات CLI.

## 6. اشارات troubleshooting

- **Mode selection mismatch** — اذا اظهر `iroha sumeragi params --summary`
  `consensus_mode="permissioned"` او اختلف `k_aggregators` عن manifest، احذف
  artefacts الملتقطة، صحح `iroha_config`, اعد تشغيل المدقق، ثم اعد مسار التحقق
  الموصوف في {doc}`sumeragi`.
- **Missing commits or reveals** — سلسلة مسطحة في `sumeragi_vrf_commits_emitted_total`
  او `sumeragi_vrf_reveals_emitted_total` تعني ان Torii لا يمرر اطارات VRF. راجع
  سجلات المدقق لاخطاء `handle_vrf_*`، ثم اعد ارسال payload يدويا عبر مساعدات POST
  الموضحة اعلاه.
- **Unexpected penalties** — عند ارتفاع `sumeragi_vrf_no_participation_total`، افحص
  ملف `vrf_penalties_<epoch>.json` لتاكيد signer ID وقارنه مع roster staking. العقوبات
  التي لا تتطابق مع اختبارات chaos تشير الى انحراف ساعة المدقق او حماية replay في Torii؛
  اصلح النظير المخالف قبل اعادة الاختبار.
- **Evidence ingestion stalls** — عندما يتسطح `sumeragi_evidence_records_total` بينما
  تبعث اختبارات chaos اخطاء، شغل `iroha sumeragi evidence count` على عدة مدققين وتحقق
  من ان `/v1/sumeragi/evidence/count` يطابق مخرجات CLI. اي اختلاف يعني ان مستهلكي
  SSE/webhook قد يكونون stale، لذا اعد ارسال fixture معروف وصعد الامر الى فريق Torii
  اذا لم يرتفع العداد.

</div>
