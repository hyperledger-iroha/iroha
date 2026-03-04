---
lang: ar
direction: rtl
source: docs/source/fastpq_transfer_gadget.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 084add6296c5b884a6d6dc07425aeca9966576f0643f6a7cf555da3fc8586466
source_last_modified: "2026-01-08T10:01:27.059307+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% تصميم أداة نقل FastPQ

# نظرة عامة

يسجل مخطط FASTPQ الحالي كل عملية بدائية متضمنة في تعليمات `TransferAsset`، مما يعني أن كل تحويل يدفع مقابل حساب الرصيد وجولات التجزئة وتحديثات SMT بشكل منفصل. لتقليل صفوف التتبع لكل عملية نقل، نقدم أداة مخصصة تتحقق فقط من الحد الأدنى من عمليات التحقق الحسابية/الالتزامات بينما يستمر المضيف في تنفيذ انتقال الحالة الأساسي.

- **النطاق**: عمليات النقل الفردية والدفعات الصغيرة المنبعثة عبر سطح استدعاء النظام Kotodama/IVM `TransferAsset` الموجود.
- **الهدف**: قطع أثر عمود FFT/LDE لعمليات النقل ذات الحجم الكبير من خلال مشاركة جداول البحث وطي العمليات الحسابية لكل عملية نقل إلى كتلة قيود مدمجة.

#الهندسة المعمارية

```
Kotodama builder → IVM syscall (transfer_v1 / transfer_v1_batch)
          │
          ├─ Host (unchanged business logic)
          └─ Transfer transcript (Norito-encoded)
                   │
                   └─ FASTPQ TransferGadget
                        ├─ Balance arithmetic block
                        ├─ Poseidon commitment check
                        ├─ Dual SMT path verifier
                        └─ Authority digest equality
```

## تنسيق النص

يُصدر المضيف `TransferTranscript` لكل استدعاء syscall:

```rust
struct TransferTranscript {
    batch_hash: Hash,
    deltas: Vec<TransferDeltaTranscript>,
    authority_digest: Hash,
    poseidon_preimage_digest: Option<Hash>,
}

struct TransferDeltaTranscript {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    amount: Numeric,
    from_balance_before: Numeric,
    from_balance_after: Numeric,
    to_balance_before: Numeric,
    to_balance_after: Numeric,
    from_merkle_proof: Option<Vec<u8>>,
    to_merkle_proof: Option<Vec<u8>>,
}
```

- يقوم `batch_hash` بربط النص بتجزئة نقطة دخول المعاملة لحماية إعادة التشغيل.
- `authority_digest` هو تجزئة المضيف على بيانات النصاب/الموقعين المصنفة؛ تتحقق الأداة من المساواة ولكنها لا تعيد التحقق من التوقيع. بشكل ملموس، يقوم المضيف Norito بترميز `AccountId` (الذي يتضمن بالفعل وحدة تحكم multisig الأساسية) ويجزئ `b"iroha:fastpq:v1:authority|" || encoded_account` مع Blake2b-256، مما يؤدي إلى تخزين `Hash` الناتج.
- `poseidon_preimage_digest` = Poseidon(account_from || account_to || الأصول || المبلغ || Batch_hash); يضمن أن الأداة تقوم بإعادة حساب نفس الملخص مثل المضيف. يتم إنشاء بايتات preimage كـ `norito(from_account) || norito(to_account) || norito(asset_definition) || norito(amount) || batch_hash` باستخدام ترميز Norito المجرد قبل تمريرها عبر مساعد Poseidon2 المشترك. هذا الملخص موجود لنصوص الدلتا الفردية ويتم حذفه للدفعات متعددة الدلتا.

يتم إجراء تسلسل لجميع الحقول عبر Norito بحيث تظل ضمانات الحتمية الحالية ثابتة.
يتم إصدار كل من `from_path` و`to_path` كـ Norito النقط باستخدام
مخطط `TransferMerkleProofV1`: `{ version: 1, path_bits: Vec<u8>, siblings: Vec<Hash> }`.
يمكن للإصدارات المستقبلية توسيع المخطط بينما يقوم المثبت بفرض علامة الإصدار
قبل فك التشفير. تتضمن البيانات التعريفية `TransitionBatch` النص المشفر بـ Norito
المتجه تحت المفتاح `transfer_transcripts` حتى يتمكن المُثبت من فك تشفير الشاهد
دون إجراء استعلامات خارج النطاق. المدخلات العامة (`dsid`، `slot`، الجذور،
`perm_root`، `tx_set_hash`) يتم حملها في `FastpqTransitionBatch.public_inputs`،
ترك البيانات التعريفية لتسجيل دفاتر عدد التجزئة/النسخ. حتى السباكة المضيفة
الأراضي، يستمد المُثبِّت البراهين بشكل صناعي من أزواج المفاتيح/التوازن وبالتالي الصفوف
قم دائمًا بتضمين مسار SMT محدد حتى عندما يحذف النص الحقول الاختيارية.

## تخطيط الأداة

1. ** موازنة الكتلة الحسابية **
   - المدخلات: `from_balance_before`، `amount`، `to_balance_before`.
   - الشيكات:
     - `from_balance_before >= amount` (أداة النطاق مع تحليل RNS المشترك).
     -`from_balance_after = from_balance_before - amount`.
     -`to_balance_after = to_balance_before + amount`.
   - معبأة في بوابة مخصصة بحيث تستهلك المعادلات الثلاث مجموعة صف واحد.2. ** كتلة التزام بوسيدون **
   - إعادة حساب `poseidon_preimage_digest` باستخدام جدول بحث Poseidon المشترك المستخدم بالفعل في الأدوات الذكية الأخرى. لا توجد جولات بوسيدون لكل عملية نقل في التتبع.

3. ** كتلة مسار ميركل **
   - يعمل على توسيع أداة Kaigi SMT الموجودة من خلال وضع "التحديث المقترن". تشترك ورقتان (المرسل والمستقبل) في نفس العمود للتجزئات الشقيقة، مما يقلل من الصفوف المكررة.

4. **التحقق من ملخص الهيئة**
   - قيد المساواة البسيط بين الملخص المقدم من المضيف وقيمة الشاهد. تظل التوقيعات في أداتهم المخصصة.

5. **حلقة الدفعة**
   - تستدعي البرامج `transfer_v1_batch_begin()` قبل تكرار حلقة منشئي `transfer_asset` و`transfer_v1_batch_end()` بعد ذلك. أثناء نشاط النطاق، يقوم المضيف بتخزين كل عملية نقل مؤقتًا وإعادة تشغيلها كـ `TransferAssetBatch` واحد، مع إعادة استخدام سياق Poseidon/SMT مرة واحدة لكل دفعة. تضيف كل دلتا إضافية فقط العمليات الحسابية والتحقق من ورقتين. يقبل جهاز فك ترميز النصوص الآن دفعات متعددة الدلتا ويظهرها كـ `TransferGadgetInput::deltas` حتى يتمكن المخطط من طي الشهود دون إعادة قراءة Norito. يمكن للعقود التي تحتوي بالفعل على حمولة Norito (على سبيل المثال، CLI/SDKs) تخطي النطاق بالكامل عن طريق الاتصال بـ `transfer_v1_batch_apply(&NoritoBytes<TransferAssetBatch>)`، والذي يسلم المضيف دفعة مشفرة بالكامل في مكالمة نظام واحدة.

# تغييرات المضيف والإثبات| طبقة | التغييرات |
|-------|---------|
| `ivm::syscalls` | أضف `transfer_v1_batch_begin` (`0x29`) / `transfer_v1_batch_end` (`0x2A`) حتى تتمكن البرامج من مكالمات نظام `transfer_v1` المتعددة دون انبعاث ISIs المتوسطة، بالإضافة إلى `transfer_v1_batch_apply` (`0x2B`) للدفعات المشفرة مسبقًا. |
| `ivm::host` والاختبارات | يتعامل المضيفون الأساسيون/الافتراضيون مع `transfer_v1` كملحق دفعة بينما يكون النطاق نشطًا، والسطح `SYSCALL_TRANSFER_V1_BATCH_{BEGIN,END,APPLY}`، ويقوم مضيف WSV الوهمي بتخزين الإدخالات مؤقتًا قبل الالتزام حتى تتمكن اختبارات الانحدار من تأكيد التوازن الحتمي التحديثات.[الصناديق/ivm/src/core_host.rs:1001] 【الصناديق/ivm/src/host.rs:451】الصناديق/ivm/src/mock_wsv.rs :3713】[صناديق/ivm/tests/wsv_host_pointer_tlv.rs:219] 【صناديق/ivm/tests/wsv_host_pointer_tlv.rs:287】
| `iroha_core` | قم بإصدار `TransferTranscript` بعد انتقال الحالة، وقم بإنشاء سجلات `FastpqTransitionBatch` باستخدام `public_inputs` الصريح أثناء `StateBlock::capture_exec_witness`، وقم بتشغيل مسار إثبات FASTPQ بحيث تتلقى كل من أدوات Torii/CLI والواجهة الخلفية Stage6 البيانات الأساسية المدخلات `TransitionBatch`. يقوم `TransferAssetBatch` بتجميع عمليات النقل المتسلسلة في نسخة واحدة، مع حذف ملخص بوسيدون لدفعات متعددة الدلتا حتى يتمكن الجهاز من التكرار عبر الإدخالات بشكل حتمي. |
| `fastpq_prover` | يقوم `gadgets::transfer` الآن بالتحقق من صحة نصوص الدلتا المتعددة (حساب التوازن + ملخص Poseidon) وأسطح الشهود المنظمة (بما في ذلك نقاط SMT المقترنة بالعنصر النائب) للمخطط (`crates/fastpq_prover/src/gadgets/transfer.rs`). يقوم `trace::build_trace` بفك تشفير هذه النصوص من بيانات تعريف الدفعة، ويرفض دفعات النقل التي تفتقد الحمولة `transfer_transcripts`، ويرفق الشهود الذين تم التحقق من صحتهم بـ `Trace::transfer_witnesses`، ويبقي `TracePolynomialData::transfer_plan()` الخطة المجمعة حية حتى يستهلك المخطط الأداة (`crates/fastpq_prover/src/trace.rs`). يتم الآن شحن مجموعة انحدار عدد الصفوف عبر `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)، والتي تغطي سيناريوهات تصل إلى 65536 صفًا مبطنًا، بينما تظل أسلاك SMT المقترنة خلف معلم مساعد الدُفعة TF-3 (العناصر النائبة تحافظ على استقرار تخطيط التتبع حتى تصل هذه المبادلة). |
| Kotodama | يخفض المساعد `transfer_batch((from,to,asset,amount), …)` إلى `transfer_v1_batch_begin`، واستدعاءات `transfer_asset` المتسلسلة، و`transfer_v1_batch_end`. يجب أن تتبع كل وسيطة صف الشكل `(AccountId, AccountId, AssetDefinitionId, int)`؛ عمليات النقل الفردية تحافظ على المنشئ الحالي. |

مثال لاستخدام Kotodama:

```text
fn pay(a: AccountId, b: AccountId, asset: AssetDefinitionId, x: int) {
    transfer_batch((a, b, asset, x), (b, a, asset, 1));
}
```

ينفذ `TransferAssetBatch` نفس الأذونات وعمليات التحقق الحسابية مثل مكالمات `Transfer::asset_numeric` الفردية، ولكنه يسجل جميع الدلتا داخل `TransferTranscript` واحد. تحذف نصوص الدلتا المتعددة ملخص بوسيدون حتى تصل التزامات كل دلتا إلى المتابعة. يقوم منشئ Kotodama الآن بإصدار مكالمات النظام للبداية/النهاية تلقائيًا، لذلك يمكن للعقود نشر عمليات النقل المجمعة دون تشفير حمولات Norito يدويًا.

## أداة انحدار عدد الصفوف

يقوم `fastpq_row_bench` (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`) بتجميع دفعات انتقال FASTPQ مع أعداد المحددات القابلة للتكوين ويبلغ عن ملخص `row_usage` الناتج (`total_rows`، عدد كل محدد، النسبة) إلى جانب الطول/السجل المبطن. احصل على معايير لسقف 65536 صفًا باستخدام:

```bash
cargo run -p fastpq_prover --bin fastpq_row_bench -- \
  --transfer-rows 65536 \
  --mint-rows 256 \
  --burn-rows 128 \
  --pretty \
  --output fastpq_row_usage_max.json
```يعكس JSON المنبعث عناصر مجموعة FASTPQ التي ينبعثها `iroha_cli audit witness` الآن بشكل افتراضي (امرر `--no-fastpq-batches` لمنعها)، لذلك يمكن لـ `scripts/fastpq/check_row_usage.py` وبوابة CI تمييز عمليات التشغيل الاصطناعية مقابل اللقطات السابقة عند التحقق من صحة تغييرات المخطط.

#خطة الطرح

1. **TF-1 (السباكة النصية)**: ✅ يُصدر `StateTransaction::record_transfer_transcripts` الآن نصوص Norito لكل `TransferAsset`/دفعة، ويقوم `sumeragi::witness::record_fastpq_transcript` بتخزينها داخل الشاهد العالمي، ويقوم `StateBlock::capture_exec_witness` ببناء `fastpq_batches` باستخدام `public_inputs` صريح للمشغلين ومسار الإثبات (استخدم `--no-fastpq-batches` إذا كنت بحاجة إلى جهاز أنحف Output).[crates/iroha_core/src/state.rs:8801] 【crates/iroha_core/src/sumeragi/witness.rs:280】【crates/iroha_core/src/fastpq/mod.rs:157】[crates/iroha_cli/src/audit.rs:185]
2. **TF-2 (تنفيذ الأداة)**: ✅ `gadgets::transfer` يتحقق الآن من صحة النصوص متعددة الدلتا (حساب التوازن + ملخص Poseidon)، ويجمع أدلة SMT المقترنة عندما يحذفها المضيفون، ويكشف الشهود المنظمين عبر `TransferGadgetPlan`، ويقوم `trace::build_trace` بإدخال هؤلاء الشهود في `Trace::transfer_witnesses` أثناء تعبئة أعمدة SMT من البروفات. يلتقط `fastpq_row_bench` مجموعة أدوات الانحدار المكونة من 65536 صفًا حتى يتمكن المخططون من تتبع استخدام الصف دون إعادة تشغيل Norito الحمولات.[الصناديق/fastpq_prover/src/gadgets/transfer.rs:1] 【الصناديق/fastpq_prover/src/trace.rs:1】[الصناديق/fastpq_prover/src/bin/fastpq_row_bench.rs:1]
3. **TF-3 (مساعد الدفعة)**: قم بتمكين منشئ syscall + Kotodama، بما في ذلك التطبيق المتسلسل على مستوى المضيف وحلقة الأداة الذكية.
4. **TF-4 (القياس عن بعد والمستندات)**: قم بتحديث مخططات `fastpq_plan.md` و`fastpq_migration_guide.md` ولوحة المعلومات للتخصيص السطحي لصفوف النقل مقابل الأدوات الذكية الأخرى.

# أسئلة مفتوحة

- **حدود النطاق**: ذعر مخطط FFT الحالي للتتبعات التي تتجاوز صفين¹⁴. يجب أن يقوم فريق TF-2 إما برفع حجم النطاق أو توثيق هدف مرجعي مخفض.
- **دفعات متعددة الأصول**: تفترض الأداة الأولية نفس معرف الأصل لكل دلتا. إذا كنا بحاجة إلى دفعات غير متجانسة، فيجب علينا التأكد من أن شاهد Poseidon يتضمن الأصل في كل مرة لمنع إعادة العرض عبر الأصول.
- **إعادة استخدام ملخص السلطة**: يمكننا على المدى الطويل إعادة استخدام نفس الملخص للعمليات الأخرى المسموح بها لتجنب إعادة حساب قوائم الموقعين لكل مكالمة نظام.


تتتبع هذه الوثيقة قرارات التصميم؛ اجعلها متزامنة مع إدخالات خريطة الطريق عندما تصل المعالم.