---
lang: ar
direction: rtl
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T15:34:14.183250+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# نظرة عامة على أمثلة Kotodama

تعرض هذه الصفحة أمثلة موجزة عن Kotodama وكيفية تعيينها لاستدعاءات النظام IVM ووسائط المؤشر-ABI. أنظر أيضا:
- `examples/` للمصادر القابلة للتشغيل
- `docs/source/ivm_syscalls.md` لـ syscall ABI الأساسي
- `kotodama_grammar.md` للحصول على مواصفات اللغة الكاملة

## مرحبا + تفاصيل الحساب

المصدر: `examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

رسم الخرائط (المؤشر-ABI):
- `authority()` → `SCALL 0xA4` (يكتب المضيف `&AccountId` في `r10`)
- `set_account_detail(a, k, v)` → انقل `r10=&AccountId`، `r11=&Name`، `r12=&Json`، ثم `SCALL 0x1A`

## نقل الأصول

المصدر: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ"),
      account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

رسم الخرائط (المؤشر-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`، `r11=&AccountId(to)`، `r12=&AssetDefinitionId(def)`، `r13=amount`، ثم `SCALL 0x24`

## إنشاء + نقل NFT

المصدر: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let recipient = account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

رسم الخرائط (المؤشر-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`، `r11=&AccountId(owner)`، `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`، `r11=&NftId`، `r12=&AccountId(to)`، `SCALL 0x26`

## المؤشر Norito المساعدين

تتطلب الحالة الدائمة ذات قيمة المؤشر تحويل TLVs المكتوبة من وإلى ملف
`NoritoBytes` المغلف الذي يستمر المضيفين. يقوم Kotodama الآن بتوصيل هؤلاء المساعدين
مباشرة من خلال المترجم حتى يتمكن المنشئون من استخدام الإعدادات الافتراضية للمؤشر والخريطة
عمليات البحث بدون غراء FFI اليدوي:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

خفض:

- تصدر الإعدادات الافتراضية للمؤشر `POINTER_TO_NORITO` بعد نشر TLV المكتوب، لذلك
  يتلقى المضيف حمولة `NoritoBytes` الأساسية للتخزين.
- تقوم القراءات بإجراء العملية العكسية باستخدام `POINTER_FROM_NORITO`، مما يوفر
  معرف نوع المؤشر المتوقع في `r11`.
- يقوم كلا المسارين تلقائيًا بنشر TLVs الحرفية في منطقة INPUT، مما يسمح بذلك
  عقود لخلط سلسلة حرفية ومؤشرات وقت التشغيل بشفافية.

راجع `crates/ivm/tests/kotodama_pointer_args.rs` للتعرف على انحدار وقت التشغيل
يمارس رحلة الذهاب والإياب ضد `MockWorldStateView`.

## تكرار الخريطة الحتمية (التصميم)

خريطة حتمية لكل منها تتطلب حدودًا. يتطلب التكرار متعدد الإدخال خريطة الحالة؛ يقبل المترجم `.take(n)` أو الحد الأقصى للطول المعلن.

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

دلالات:
- مجموعة التكرار هي لقطة عند إدخال الحلقة؛ يتم ترتيب المعجم بواسطة Norito بايت من المفتاح.
- الطفرات الهيكلية إلى `M` في مصيدة الحلقة مع `E_ITER_MUTATION`.
- بدون حدود يصدر المترجم `E_UNBOUNDED_ITERATION`.

## الأجزاء الداخلية للمترجم/المضيف (Rust، وليس مصدر Kotodama)

المقتطفات أدناه موجودة على الجانب الصدأ من سلسلة الأدوات. إنها توضح مساعدي برنامج التحويل البرمجي وآليات خفض VM وهي **ليست** صالحة مصدر Kotodama `.ko`.

## تحديثات الإطار المقسم لكود التشغيل على نطاق واسع

تستهدف مساعدات كود التشغيل الواسعة لـ Kotodama تخطيط المعامل 8 بت الذي يستخدمه IVM
ترميز واسع. الأحمال والمخازن التي تنقل قيم 128 بت تعيد استخدام المعامل الثالث
فتحة للسجل العالي، لذلك يجب أن يحتوي السجل الأساسي بالفعل على السجل النهائي
عنوان. اضبط القاعدة باستخدام `ADDI` قبل إصدار التحميل/التخزين:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```تعمل تحديثات الإطار المقسم على تطوير القاعدة بخطوات 16 بايت، مما يضمن التسجيل
الزوج الملتزم به `STORE128` يهبط على حدود المحاذاة المطلوبة. نفس الشيء
ينطبق النمط على `LOAD128`؛ إصدار `ADDI` بالخطوة المطلوبة من قبل
يحافظ كل حمل على سجل الوجهة العالية مرتبطًا بفتحة المعامل الثالثة.
تتناسب العناوين المنحرفة مع `VMError::MisalignedAccess`، مما يتوافق مع الجهاز الظاهري
السلوك الذي تمارسه في `crates/ivm/tests/wide_memory128.rs`.

يجب على البرامج التي تُصدر هذه البرامج المساعدة ذات 128 بت أن تعلن عن قدرة المتجهات.
يقوم المترجم Kotodama بتمكين بت وضع `VECTOR` تلقائيًا كلما
يظهر `LOAD128`/`STORE128`؛ مصائد VM مع
`VMError::VectorExtensionDisabled` إذا حاول أحد البرامج تنفيذها
دون أن مجموعة قليلا.

## خفض فرع مشروط على نطاق واسع

عندما يقوم Kotodama بخفض `if`/`else` أو فرع ثلاثي إلى كود ثانوي عريض فإنه يصدر
تسلسل `BNE cond, zero, +2` ثابت متبوعًا بزوج من تعليمات `JAL`:

1. يحافظ `BNE` القصير على الفرع الشرطي ضمن المسار المباشر 8 بت
   من خلال القفز فوق السقوط `JAL`.
2. يستهدف `JAL` الأول الكتلة `else` (يتم تنفيذها عندما يكون الشرط
   كاذبة).
3. ينتقل `JAL` الثاني إلى الكتلة `then` (يتم التقاطه عندما يكون الشرط
   صحيح).

يضمن هذا النمط أن فحص الحالة لن يحتاج أبدًا إلى تشفير إزاحات أكبر
أكثر من ±127 كلمة مع الاستمرار في دعم الأجسام الكبيرة بشكل تعسفي لـ `then`
وكتل `else` عبر مساعد `JAL` الواسع. انظر
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` ل
اختبار الانحدار الذي يقفل التسلسل.

### مثال للتخفيض

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

يتم تجميعه إلى الهيكل العظمي للتعليمات العريضة التالية (تسجيل الأرقام و
تعتمد الإزاحات المطلقة على الوظيفة المرفقة):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

التعليمات اللاحقة تتحقق من الثوابت وتكتب القيمة المرجعة.
نظرًا لأن `BNE` يقفز فوق `JAL` الأول، تكون الإزاحة الشرطية دائمًا
كلمات `+2`، مما يحافظ على الفرع ضمن النطاق حتى عندما تتوسع أجسام الكتلة.