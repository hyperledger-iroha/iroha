---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/reports/sf6-security-review.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1e13cdcf18cfe376b01b299735021db28ab8060baf8e0da20e30139050f46602
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# مراجعة أمان SF-6

**نافذة التقييم:** 2026-02-10 → 2026-02-18  
**قادة المراجعة:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**النطاق:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`)، واجهات proof streaming، معالجة manifests في Torii، تكامل Sigstore/OIDC، وخطافات release في CI.  
**القطع الفنية:**  
- مصدر CLI والاختبارات (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- معالجات manifest/proof في Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- أتمتة release (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harness تساوي حتمي (`crates/sorafs_car/tests/sorafs_cli.rs`, [تقرير تكافؤ GA لمشغل SoraFS](./orchestrator-ga-parity.md))

## المنهجية

1. **ورش threat modelling** قامت بمواءمة قدرات المهاجمين لمحطات عمل المطورين وأنظمة CI وعقد Torii.  
2. **مراجعة الشيفرة** ركزت على أسطح الاعتمادات (تبادل رموز OIDC، keyless signing)، تحقق manifests من Norito، وback-pressure في proof streaming.  
3. **اختبارات ديناميكية** أعادت تشغيل fixture manifests ومحاكت أوضاع فشل (token replay، manifest tampering، proof streams مقطوعة) باستخدام parity harness وfuzz drives مخصصة.  
4. **فحص الإعدادات** تحقق من defaults في `iroha_config`، معالجة أعلام CLI، وسكربتات release لضمان تشغيلات حتمية وقابلة للتدقيق.  
5. **مقابلة عملية** أكدت تدفق remediation ومسارات التصعيد وجمع evidence التدقيق مع owners الإطلاق في Tooling WG.

## ملخص النتائج

| ID | الشدة | المجال | النتيجة | المعالجة |
|----|----------|------|---------|------------|
| SF6-SR-01 | عالي | Keyless signing | كانت audience الافتراضية لرموز OIDC ضمنية في قوالب CI، ما يعرض لخطر replay بين المستأجرين. | تمت إضافة فرض صريح لـ `--identity-token-audience` في hooks الإطلاق وقوالب CI ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). أصبح CI يفشل عند غياب audience. |
| SF6-SR-02 | متوسط | Proof streaming | مسارات back-pressure قبلت buffers غير محدودة للمشتركين، ما يتيح استنزاف الذاكرة. | يفرض `sorafs_cli proof stream` أحجام قنوات محدودة مع truncation حتمي، ويسجل Norito summaries ويجهض التدفق؛ وتم تحديث Torii mirror لتقييد response chunks (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | متوسط | إرسال manifests | قبل CLI manifests دون التحقق من chunk plans المضمنة عندما يكون `--plan` غائبا. | أصبح `sorafs_cli manifest submit` يعيد حساب ويقارن CAR digests ما لم يتم تقديم `--expect-plan-digest`، ويرفض mismatches ويعرض تلميحات remediation. تغطي الاختبارات حالات النجاح/الفشل (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | منخفض | Audit trail | كانت قائمة التحقق للإطلاق تفتقر إلى سجل موافقة موقع لمراجعة الأمان. | تمت إضافة قسم في [release process](../developer-releases.md) يطلب إرفاق hashes لمذكرة المراجعة ورابط تذكرة sign-off قبل GA. |

تم إصلاح جميع نتائج high/medium خلال نافذة المراجعة والتحقق منها عبر parity harness الحالي. لا توجد مشكلات حرجة كامنة.

## التحقق من الضوابط

- **نطاق الاعتمادات:** تفرض قوالب CI الآن audience وissuer صريحين؛ يفشل CLI وrelease helper بسرعة ما لم يرافق `--identity-token-audience` الخيار `--identity-token-provider`.  
- **إعادة التشغيل الحتمية:** تغطي الاختبارات المحدثة تدفقات إرسال manifests الإيجابية والسلبية، وتضمن أن mismatched digests تبقى أخطاء غير حتمية وتُكتشف قبل لمس الشبكة.  
- **Back-pressure في proof streaming:** يقوم Torii ببث عناصر PoR/PoTR عبر قنوات محدودة، ويحتفظ CLI فقط بعينات latency مقطوعة + خمسة أمثلة فشل، مانعا نمو المشتركين غير المحدود مع الحفاظ على summaries حتمية.  
- **الرصد:** تلتقط عدادات proof streaming (`torii_sorafs_proof_stream_*`) وCLI summaries أسباب الإجهاض، مما يوفر breadcrumbs تدقيق للمشغلين.  
- **التوثيق:** تشير أدلة المطورين ([developer index](../developer-index.md)، [CLI reference](../developer-cli.md)) إلى الأعلام الحساسة للأمان ومسارات التصعيد.

## إضافات إلى قائمة التحقق للإطلاق

**يجب** على مديري الإطلاق إرفاق الأدلة التالية عند ترقية مرشح GA:

1. Hash لآخر مذكرة مراجعة أمان (هذا المستند).  
2. رابط لتذكرة remediation المتابعة (مثل `governance/tickets/SF6-SR-2026.md`).  
3. مخرجات `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` التي تظهر معاملات audience/issuer صريحة.  
4. سجلات parity harness الملتقطة (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. تأكيد أن ملاحظات إصدار Torii تتضمن counters تليمترية لبث الإثباتات المقيد.

عدم جمع artefacts أعلاه يمنع sign-off لـ GA.

**Hashes مرجعية للقطع الفنية (sign-off بتاريخ 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## متابعات معلقة

- **تحديث نموذج التهديد:** إعادة هذه المراجعة كل ربع أو قبل إضافات كبيرة لأعلام CLI.  
- **تغطية fuzzing:** يتم fuzzing لتشفيرات نقل proof streaming عبر `fuzz/proof_stream_transport`، بما يشمل payloads: identity, gzip, deflate, zstd.  
- **تمرين الحوادث:** جدولة تمرين للمشغلين لمحاكاة اختراق token وrollback للـ manifest، مع ضمان أن التوثيق يعكس الإجراءات المطبقة.

## الاعتماد

- ممثل Security Engineering Guild: @sec-eng (2026-02-20)  
- ممثل Tooling Working Group: @tooling-wg (2026-02-20)

احفظ الموافقات الموقعة بجانب bundle القطع الفنية للإطلاق.
