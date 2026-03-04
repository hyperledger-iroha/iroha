---
lang: ar
direction: rtl
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/finance/repo_governance_packet_template.md -->

# قالب حزمة حوكمة Repo (خارطة الطريق F1)

استخدم هذا القالب عند تجهيز حزمة artefacts المطلوبة لبند خارطة الطريق F1 (توثيق دورة حياة repo والادوات). الهدف هو تسليم المراجعين ملف Markdown واحدا يسرد كل ادخال وhash وحزمة ادلة حتى يتمكن مجلس الحوكمة من اعادة تشغيل البايتات المشار اليها في المقترح.

> انسخ القالب الى دليل الادلة الخاص بك (مثال
> `artifacts/finance/repo/2026-03-15/packet.md`)، استبدل القوالب، وقم بعمل commit/رفع له بجوار artefacts ذات الهاش المشار اليها ادناه.

## 1. البيانات الوصفية

| الحقل | القيمة |
|-------|-------|
| معرف الاتفاق/التغيير | `<repo-yyMMdd-XX>` |
| اعد بواسطة / التاريخ | `<desk lead> - 2026-03-15T10:00Z` |
| تمت المراجعة بواسطة | `<dual-control reviewer(s)>` |
| نوع التغيير | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| امين/امناء الحفظ | `<custodian id(s)>` |
| المقترح/الاستفتاء المرتبط | `<governance ticket id or GAR link>` |
| دليل الادلة | ``artifacts/finance/repo/<slug>/`` |

## 2. حمولة التعليمات

سجل تعليمات Norito المرحلية التي وافقت عليها المكاتب عبر
`iroha app repo ... --output`. يجب ان يتضمن كل ادخال هاش الملف الناتج ووصفا موجزا
للاجراء الذي سيتم تقديمه بعد نجاح التصويت.

| الاجراء | الملف | SHA-256 | ملاحظات |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | يحتوي على سيقان cash/collateral المعتمدة من desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | يلتقط cadence + participant id الذي اثار النداء. |
| Unwind | `instructions/unwind.json` | `<sha256>` | دليل للساق العكسية بعد تحقق الشروط. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 اقرارات امين الحفظ (tri-party فقط)

اكمل هذا القسم كلما استخدم repo `--custodian`. يجب ان تتضمن حزمة الحوكمة اقرارا موقعا من كل امين حفظ مع هاش الملف المشار اليه في قسم 2.8 من `docs/source/finance/repo_ops.md`.

| امين الحفظ | الملف | SHA-256 | ملاحظات |
|-----------|------|---------|-------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` | SLA موقع يغطي نافذة الحفظ وحساب التوجيه وجهة اتصال drill. |

> خزّن الاقرار بجوار الادلة الاخرى (`artifacts/finance/repo/<slug>/`) حتى يسجل
> `scripts/repo_evidence_manifest.py` الملف في نفس الشجرة مع التعليمات المرحلية
> ومقاطع الاعدادات. راجع
> `docs/examples/finance/repo_custodian_ack_template.md` للقالب الجاهز الذي يطابق عقد ادلة الحوكمة.

## 3. مقطع الاعدادات

الصق كتلة TOML `[settlement.repo]` التي ستطبق على العنقود (بما في ذلك
`collateral_substitution_matrix`). خزّن الهاش بجوار المقطع حتى يتمكن المدققون من
تأكيد سياسة التشغيل وقت اعتماد حجز repo.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 لقطات الاعدادات بعد الموافقة

بعد اكتمال الاستفتاء او تصويت الحوكمة وتطبيق تغيير `[settlement.repo]`، التقط لقطات
`/v1/configuration` من كل peer حتى يتمكن المدققون من اثبات ان السياسة المعتمدة حية عبر
العنقود (راجع `docs/source/finance/repo_ops.md` القسم 2.9 لتدفق الادلة).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / المصدر | الملف | SHA-256 | ارتفاع الكتلة | ملاحظات |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | لقطة مباشرة بعد نشر الاعدادات. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | تؤكد تطابق `[settlement.repo]` مع TOML المرحلي. |

سجل digests بجوار معرفات peer في `hashes.txt` (او الملخص المكافئ) حتى يتمكن المراجعون من تتبع اي عقد ابتلعت التغيير. تعيش اللقطات تحت `config/peers/` بجوار مقطع TOML وستلتقط تلقائيا بواسطة `scripts/repo_evidence_manifest.py`.

## 4. artefacts اختبار حتمية

ارفق احدث المخرجات من:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

سجل مسارات الملفات + الهاشات لحزم السجلات او JUnit XML التي ينتجها نظام CI.

| الاثر | الملف | SHA-256 | ملاحظات |
|----------|------|---------|-------|
| سجل اثبات دورة الحياة | `tests/repo_lifecycle.log` | `<sha256>` | تم التقاطه مع `--nocapture`. |
| سجل اختبار التكامل | `tests/repo_integration.log` | `<sha256>` | يتضمن تغطية substitution + cadence للهامش. |

## 5. لقطة اثبات دورة الحياة

يجب ان تتضمن كل حزمة لقطة دورة الحياة الحتمية المصدرة من
`repo_deterministic_lifecycle_proof_matches_fixture`. شغّل harness مع مفاتيح التصدير
مفعلة حتى يتمكن المراجعون من مقارنة اطار JSON والهاش مع fixture الموجودة في
`crates/iroha_core/tests/fixtures/` (راجع `docs/source/finance/repo_ops.md` القسم 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

او استخدم الاداة المثبتة لاعادة توليد fixtures ونسخها في حزمة الادلة بخطوة واحدة:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| الاثر | الملف | SHA-256 | ملاحظات |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | اطار دورة حياة قياسي صادر عن harness الاثبات. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | هاش hex باحرف كبيرة مطابق لـ `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; ارفقه حتى عند عدم التغيير. |

## 6. manifesto الادلة

انشئ manifesto لكل دليل الادلة حتى يتمكن المدققون من التحقق من الهاشات دون فك الارشيف. الاداة تعكس سير العمل الموصوف في `docs/source/finance/repo_ops.md` القسم 3.2.

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| الاثر | الملف | SHA-256 | ملاحظات |
|----------|------|---------|-------|
| manifesto الادلة | `manifest.json` | `<sha256>` | ادخل checksum في تذكرة الحوكمة / ملاحظات الاستفتاء. |

## 7. لقطة التليمترى والاحداث

صدّر ادخالات `AccountEvent::Repo(*)` ذات الصلة واي لوحات او صادرات CSV مشار اليها في `docs/source/finance/repo_ops.md`. سجل الملفات والهاشات هنا ليتمكن المراجعون من الوصول مباشرة الى الادلة.

| التصدير | الملف | SHA-256 | ملاحظات |
|--------|------|---------|-------|
| احداث repo JSON | `evidence/repo_events.ndjson` | `<sha256>` | تدفق Torii خام مصفى لحسابات desk. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | صادر من Grafana باستخدام لوحة Repo Margin. |

## 8. الموافقات والتواقيع

- **موقعو التحكم المزدوج:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` لملف GAR PDF الموقع او رفع minutes.
- **موقع التخزين:** `governance://finance/repo/<slug>/packet/`

## 9. قائمة التحقق

ضع علامة على كل عنصر عند الاكتمال.

- [ ] حمولة التعليمات مرحلية، وتمت تجزئتها وارفاقها.
- [ ] تم تسجيل هاش مقطع الاعدادات.
- [ ] تم التقاط سجلات الاختبارات الحتمية وتجزيئها.
- [ ] تم تصدير لقطة دورة الحياة + digest.
- [ ] تم توليد manifesto الادلة وتسجيل هاشه.
- [ ] تم التقاط صادرات الاحداث/التليمترى وتجزيئها.
- [ ] تم ارشفة اقرارات التحكم المزدوج.
- [ ] تم رفع GAR/minutes وتسجيل digest اعلاه.

الحفاظ على هذا القالب بجوار كل حزمة يبقي DAG الحوكمة حتميا ويوفر للمدققين manifesto قابلا للنقل لقرارات دورة حياة repo.

</div>
