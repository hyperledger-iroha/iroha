---
lang: ur
direction: rtl
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T15:34:14.183250+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama مثالوں کا جائزہ

اس صفحے میں جامع Kotodama مثالوں کو دکھایا گیا ہے اور وہ IVM Syscalls اور پوائنٹر - ABI دلائل پر کس طرح نقشہ بناتے ہیں۔ یہ بھی دیکھیں:
- چلانے کے قابل ذرائع کے لئے `examples/`
- `docs/source/ivm_syscalls.md` کیننیکل سیسکل ABI کے لئے
- `kotodama_grammar.md` زبان کی مکمل تصریح کے لئے

## ہیلو + اکاؤنٹ کی تفصیل

ماخذ: `examples/hello/hello.ko`

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

نقشہ سازی (پوائنٹر - آبائی):
- `authority()` → `SCALL 0xA4` (میزبان `&AccountId` کو `r10` میں لکھتا ہے)
- `set_account_detail(a, k, v)` → اقدام `r10=&AccountId` ، `r11=&Name` ، `r12=&Json` ، پھر `SCALL 0x1A`

## اثاثہ کی منتقلی

ماخذ: `examples/transfer/transfer.ko`

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

نقشہ سازی (پوائنٹر - آبائی):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)` ، `r11=&AccountId(to)` ، `r12=&AssetDefinitionId(def)` ، `r13=amount` ، پھر `SCALL 0x24`

## NFT تخلیق + ٹرانسفر

ماخذ: `examples/nft/nft.ko`

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

نقشہ سازی (پوائنٹر - آبائی):
- `nft_mint_asset(id, owner)` → `r10=&NftId` ، `r11=&AccountId(owner)` ، `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)` ، `r11=&NftId` ، `r12=&AccountId(to)` ، `SCALL 0x26`

## پوائنٹر Norito مددگار

پوائنٹر کی قدر کی پائیدار ریاست کے لئے ٹائپڈ TLVs کو اور سے تبدیل کرنے کی ضرورت ہے
`NoritoBytes` لفافہ جو میزبان برقرار رہتا ہے۔ Kotodama اب ان مددگاروں کو تاروں سے دوچار کرتا ہے
براہ راست مرتب کرنے والے کے ذریعہ تاکہ بلڈر پوائنٹر ڈیفالٹس اور نقشہ استعمال کرسکیں
دستی FFI گلو کے بغیر تلاش:

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

کم کرنا:

- ٹائپڈ TLV شائع کرنے کے بعد پوائنٹر ڈیفالٹس `POINTER_TO_NORITO` خارج کریں
  میزبان اسٹوریج کے لئے ایک کیننیکل `NoritoBytes` پے لوڈ وصول کرتا ہے۔
- پڑھتا ہے `POINTER_FROM_NORITO` کے ساتھ ریورس آپریشن انجام دیں ،
  `r11` میں متوقع پوائنٹر ٹائپ ID۔
- دونوں راستے خود بخود ان پٹ خطے میں لفظی TLVs کو شائع کرتے ہیں ، اجازت دیتے ہیں
  سٹرنگ لٹریلز اور رن ٹائم پوائنٹرز کو شفاف طریقے سے ملانے کے معاہدے۔

رن ٹائم رجعت کے ل I `crates/ivm/tests/kotodama_pointer_args.rs` دیکھیں
`MockWorldStateView` کے خلاف راؤنڈ ٹرپ کی مشق کرتا ہے۔

## تعی .ن نقشہ کی تکرار (ڈیزائن)

- ہر ایک کے لئے عین مطابق نقشہ کے لئے پابند کی ضرورت ہوتی ہے۔ کثیر انٹری تکرار کے لئے ریاستی نقشہ کی ضرورت ہوتی ہے۔ مرتب `.take(n)` یا ایک اعلان کردہ زیادہ سے زیادہ لمبائی قبول کرتا ہے۔

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

Semantics:
- تکرار سیٹ لوپ انٹری میں ایک سنیپ شاٹ ہے۔ آرڈر کلید کے Norito بائٹس کے ذریعہ لغت ہے۔
- `E_ITER_MUTATION` کے ساتھ لوپ ٹریپ میں `M` میں ساختی تغیرات۔
- پابند کے بغیر مرتب `E_UNBOUNDED_ITERATION`۔

## مرتب/میزبان انٹرنلز (زنگ ، Kotodama ماخذ نہیں)

نیچے دیئے گئے ٹکڑے ٹولچین کے زنگ کی طرف رہتے ہیں۔ وہ مرتب کرنے والے مددگاروں اور VM کو کم کرنے والے میکانکس کی وضاحت کرتے ہیں اور ** نہیں ** درست Kotodama `.ko` ماخذ۔

## وسیع اوپکوڈ کو chunked فریم اپ ڈیٹ

Kotodama کے وسیع آپ کوڈ مددگار IVM کے ذریعہ استعمال ہونے والے 8 بٹ اوپریینڈ لے آؤٹ کو نشانہ بناتے ہیں
وسیع انکوڈنگ۔ بوجھ اور اسٹورز جو 128 بٹ اقدار کو منتقل کرتے ہیں تیسرا اوپریینڈ کو دوبارہ استعمال کرتے ہیں
اعلی رجسٹر کے لئے سلاٹ ، لہذا بیس رجسٹر کو پہلے ہی فائنل کا انعقاد کرنا ہوگا
پتہ بوجھ/اسٹور جاری کرنے سے پہلے `ADDI` کے ساتھ بیس کو ایڈجسٹ کریں:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```منقطع فریم کی تازہ کاریوں نے رجسٹر کو یقینی بناتے ہوئے ، 16 بائٹ قدموں میں اڈے کو آگے بڑھایا
مطلوبہ سیدھ کی حدود پر `STORE128` اراضی کے ذریعہ جوڑی کا ارتکاب کیا گیا ہے۔ ایک ہی
پیٹرن کا اطلاق `LOAD128` پر ہوتا ہے۔ اس سے پہلے مطلوبہ پیش قدمی کے ساتھ `ADDI` جاری کرنا
ہر بوجھ اعلی منزل کے رجسٹر کو تیسرے اوپریینڈ سلاٹ کا پابند رکھتا ہے۔
`VMError::MisalignedAccess` کے ساتھ غلط پتے کے جال ، VM سے ملتے ہیں
`crates/ivm/tests/wide_memory128.rs` میں سلوک کا استعمال۔

ان 128 بٹ مددگاروں کو خارج کرنے والے پروگراموں میں ویکٹر کی صلاحیت کی تشہیر کرنی ہوگی۔
Kotodama مرتب `VECTOR` موڈ بٹ کو خود بخود قابل بناتا ہے
`LOAD128`/`STORE128` ظاہر ؛ VM کے ساتھ پھنس گیا ہے
`VMError::VectorExtensionDisabled` اگر کوئی پروگرام ان پر عمل درآمد کرنے کی کوشش کرتا ہے
اس بٹ سیٹ کے بغیر

## وسیع مشروط شاخ کم

جب Kotodama ایک `if`/`else` یا ternary برانچ کو وسیع بائیک کوڈ میں کم کرتا ہے تو یہ خارج ہوتا ہے
فکسڈ `BNE cond, zero, +2` تسلسل کے بعد `JAL` ہدایات کا ایک جوڑا:

1. مختصر `BNE` مشروط برانچ کو 8 بٹ فوری لین کے اندر رکھتا ہے
   فال تھرو `JAL` پر کود کر۔
2. پہلا `JAL` `else` بلاک کو نشانہ بناتا ہے (جب حالت ہوتی ہے تو عمل میں لایا جاتا ہے
   جھوٹا)۔
3. دوسرا `JAL` `then` بلاک پر چھلانگ لگاتا ہے (جب حالت ہوتی ہے تو لیا جاتا ہے
   سچ)۔

یہ نمونہ اس شرط کی ضمانت دیتا ہے کہ کبھی بھی آفسیٹس کو بڑے انکوڈ کرنے کی ضرورت نہیں ہے
`then` کے لئے اب بھی من مانی طور پر بڑی لاشوں کی حمایت کرتے ہوئے ± 127 الفاظ سے زیادہ
اور `else` وسیع `JAL` مددگار کے ذریعے بلاکس۔ دیکھو
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` کے لئے
رجعت کا امتحان جو ترتیب میں لاک ہوجاتا ہے۔

### مثال کم کرنا

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

مندرجہ ذیل وسیع انسٹرکشن کنکال پر مرتب کریں (نمبر اور رجسٹر نمبر اور
مطلق آفسیٹس منسلک فنکشن پر منحصر ہے):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

اس کے بعد کی ہدایات مستقل کو مرتب کریں اور واپسی کی قیمت لکھیں۔
چونکہ `BNE` پہلے `JAL` پر چھلانگ لگاتا ہے ، لہذا مشروط آفسیٹ ہمیشہ رہتا ہے
`+2` الفاظ ، برانچ کو حدود میں رکھنا یہاں تک کہ جب بلاک لاشوں میں توسیع ہوتی ہے۔