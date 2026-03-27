---
lang: uz
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Misollar Ko'rib chiqish

Bu sahifada qisqa Kotodama misollari va ular IVM tizim qoʻngʻiroqlari va koʻrsatkich-ABI argumentlariga qanday mos kelishi koʻrsatilgan. Shuningdek qarang:
- Ishlaydigan manbalar uchun `examples/`
- ABI kanonik tizimi uchun `docs/source/ivm_syscalls.md`
- To'liq til spetsifikatsiyasi uchun `kotodama_grammar.md`

## Salom + Hisob tafsilotlari

Manba: `examples/hello/hello.ko`

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

Xaritalash (ko‘rsatkich-ABI):
- `authority()` → `SCALL 0xA4` (host `&AccountId` ni `r10` ga yozadi)
- `set_account_detail(a, k, v)` → harakatlantiring `r10=&AccountId`, `r11=&Name`, `r12=&Json`, keyin `SCALL 0x1A`

## Aktivlarni uzatish

Manba: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

Xaritalash (ko‘rsatkich-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, keyin `SCALL 0x24`

## NFT yaratish + uzatish

Manba: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let recipient = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Xaritalash (ko‘rsatkich-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Pointer Norito yordamchilari

Pointer-qiymatli chidamlilik holati yozilgan TLV-larni ga va undan konvertatsiya qilishni talab qiladi
Xostlar saqlanadigan `NoritoBytes` konvert. Kotodama endi bu yordamchilarni o'rnatadi
to'g'ridan-to'g'ri kompilyator orqali, shuning uchun quruvchilar ko'rsatgich standartlaridan va xaritadan foydalanishlari mumkin
qo'lda FFI elimsiz qidiruvlar:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Pastga tushirish:

- Ko'rsatgich standart qiymatlari kiritilgan TLVni nashr etgandan so'ng `POINTER_TO_NORITO` chiqaradi, shuning uchun
  xost saqlash uchun kanonik `NoritoBytes` foydali yukni oladi.
- O'qishlar `POINTER_FROM_NORITO` bilan teskari operatsiyani amalga oshiradi, ta'minlaydi
  `r11` da kutilgan ko'rsatkich turi identifikatori.
- Ikkala yo'l ham avtomatik ravishda INPUT hududiga so'zma-so'z TLVlarni nashr etadi, bu esa imkon beradi
  string literallari va ish vaqti ko'rsatkichlarini shaffof aralashtirish uchun shartnomalar.

Ish vaqti regressiyasi uchun `crates/ivm/tests/kotodama_pointer_args.rs` ga qarang
`MockWorldStateView` ga qarshi safarni amalga oshiradi.

## Deterministik xarita iteratsiyasi (dizayn)

Deterministik xarita har biri uchun chegarani talab qiladi. Ko'p kirishli iteratsiya davlat xaritasini talab qiladi; kompilyator `.take(n)` yoki e'lon qilingan maksimal uzunlikni qabul qiladi.

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

Semantika:
- Takrorlash to'plami - tsiklga kirishda oniy tasvir; tartib kalitning Norito baytlari bo'yicha leksikografik.
- `E_ITER_MUTATION` bilan halqa tuzog'ida `M` ga strukturali mutatsiyalar.
- Cheksiz kompilyator `E_UNBOUNDED_ITERATION` ni chiqaradi.

## Kompilyator/host ichki qurilmalari (Rust, Kotodama manbasi emas)

Quyidagi parchalar asboblar zanjirining Rust tomonida joylashgan. Ular kompilyator yordamchilari va VMni pasaytirish mexanikasini tasvirlaydi va **yaroqli emas** Kotodama `.ko` manbasi.

## Keng Opcode Chunked Frame yangilanishlari

Kotodama ning keng opcode yordamchilari IVM tomonidan qo'llaniladigan 8 bitli operand tartibiga qaratilgan.
keng kodlash. 128-bit qiymatlarni ko'chiruvchi yuklar va saqlashlar uchinchi operandni qayta ishlatadi
yuqori registr uchun slot, shuning uchun bazaviy registr allaqachon finalni ushlab turishi kerak
manzil. Yukni/do'konni berishdan oldin bazani `ADDI` bilan sozlang:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Bo'laklangan ramka yangilanishlari bazani 16 baytlik bosqichlarda oldinga siljitib, registrni ta'minlaydi
`STORE128` tomonidan amalga oshirilgan juftlik kerakli tekislash chegarasiga tushadi. Xuddi shunday
naqsh `LOAD128` uchun amal qiladi; oldin kerakli qadam bilan `ADDI` chiqarish
har bir yuk yuqori maqsad registrini uchinchi operand uyasiga bog'lab turadi.
`VMError::MisalignedAccess` bilan noto‘g‘ri joylangan manzillar VM ga mos keladi
`crates/ivm/tests/wide_memory128.rs` da amalga oshirilgan xatti-harakatlar.

Ushbu 128-bitli yordamchilarni chiqaradigan dasturlar vektor qobiliyatini reklama qilishlari kerak.
Kotodama kompilyatori istalgan vaqtda `VECTOR` rejimi bitini avtomatik ravishda yoqadi.
`LOAD128`/`STORE128` paydo bo'ladi; bilan VM tuzoqqa tushadi
Agar dastur ularni bajarishga harakat qilsa, `VMError::VectorExtensionDisabled`
bu bit o'rnatilgan holda.

## Keng shartli novdani pasaytirish

Kotodama `if`/`else` yoki uchlik tarmoqni keng bayt kodiga tushirganda, u
sobit `BNE cond, zero, +2` ketma-ketligi va undan keyin bir juft `JAL` ko'rsatmalari:

1. Qisqa `BNE` shartli novdani 8 bitli darhol chiziq ichida saqlaydi
   `JAL` to'sig'idan sakrab o'tib.
2. Birinchi `JAL` `else` blokiga mo‘ljallangan (shart bajarilganda bajariladi.
   yolg'on).
3. Ikkinchi `JAL` `then` blokiga o'tadi (shart bajarilganda olinadi
   rost).

Ushbu naqsh shartni tekshirish hech qachon kattaroq ofsetlarni kodlamasligini kafolatlaydi
`then` uchun o'zboshimchalik bilan katta jismlarni qo'llab-quvvatlagan holda ±127 so'zdan ortiq
va keng `JAL` yordamchisi orqali `else` bloklari. Qarang
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` uchun
ketma-ketlikni bloklaydigan regressiya testi.

### Pastga tushirish misoli

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Quyidagi keng ko'rsatma skeletini tuzadi (ro'yxatga olish raqamlari va
mutlaq ofsetlar yopish funktsiyasiga bog'liq):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Keyingi ko'rsatmalar konstantalarni materiallashtiradi va qaytariladigan qiymatni yozadi.
`BNE` birinchi `JAL` ustidan o'tganligi sababli, shartli ofset har doim bo'ladi.
`+2` so'zlari, blok jismlari kengayganida ham filialni diapazonda saqlaydi.