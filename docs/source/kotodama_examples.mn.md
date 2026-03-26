---
lang: mn
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Жишээнүүдийн тойм

Энэ хуудсанд Kotodama товч жишээнүүд болон тэдгээр нь IVM системийн дуудлагууд болон заагч-ABI аргументуудтай хэрхэн нийцэж байгааг харуулж байна. Мөн үзнэ үү:
- Ажиллаж болох эх үүсвэрийн хувьд `examples/`
- `docs/source/ivm_syscalls.md` канон систем ABI
- Бүрэн хэлний тодорхойлолтын хувьд `kotodama_grammar.md`

## Сайн байна уу + Бүртгэлийн дэлгэрэнгүй

Эх сурвалж: `examples/hello/hello.ko`

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

Газрын зураг (заагч-ABI):
- `authority()` → `SCALL 0xA4` (хост нь `&AccountId`-г `r10` руу бичдэг)
- `set_account_detail(a, k, v)` → шилжих `r10=&AccountId`, `r11=&Name`, `r12=&Json`, дараа нь `SCALL 0x1A`

## Хөрөнгө шилжүүлэх

Эх сурвалж: `examples/transfer/transfer.ko`

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

Газрын зураг (заагч-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, дараа нь `SCALL 0x24`

## NFT үүсгэх + шилжүүлэх

Эх сурвалж: `examples/nft/nft.ko`

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

Газрын зураг (заагч-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Заагч Norito Туслагч

Заагчаар үнэлэгдсэн бат бөх төлөвт бичигдсэн TLV-г дараах руу хөрвүүлэх шаардлагатай
Хостуудын `NoritoBytes` дугтуй хэвээр байна. Kotodama одоо эдгээр туслахуудыг холбодог
шууд хөрвүүлэгчээр дамжуулан бүтээгчид заагч өгөгдмөл болон газрын зургийг ашиглах боломжтой
гарын авлагын FFI цавуугүйгээр хайлтууд:

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

Бууруулах:

- Заагчийн өгөгдмөл нь бичсэн TLV-г нийтлэсний дараа `POINTER_TO_NORITO` ялгаруулдаг тул
  хост хадгалахад зориулж каноник `NoritoBytes` ачааллыг хүлээн авдаг.
- Уншлага нь урвуу үйлдлийг `POINTER_FROM_NORITO`-ээр гүйцэтгэнэ
  `r11` дахь хүлээгдэж буй заагч төрлийн id.
- Хоёр зам нь шууд TLV-г INPUT бүсэд автоматаар нийтлэх боломжийг олгодог
  string literals болон runtime заагчуудыг ил тод холих гэрээнүүд.

Ажиллах цагийн регрессийг `crates/ivm/tests/kotodama_pointer_args.rs` харна уу
`MockWorldStateView`-ийн эсрэг хоёр талдаа дасгал хийдэг.

## Тодорхойлогч газрын зургийн давталт (дизайн)

Тодорхойлогч газрын зураг тус бүрд зааг шаарддаг. Олон оруулгатай давталт нь улсын газрын зургийг шаарддаг; хөрвүүлэгч нь `.take(n)` эсвэл зарласан хамгийн их уртыг хүлээн авдаг.

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

Семантик:
- Давталтын багц нь гогцоонд орох үеийн хормын хувилбар юм; дараалал нь түлхүүрийн Norito байтаар толь бичиг юм.
- `E_ITER_MUTATION`-тай давталтын хавханд `M`-ийн бүтцийн мутаци.
- Хязгаарлалтгүйгээр хөрвүүлэгч нь `E_UNBOUNDED_ITERATION` ялгаруулдаг.

## Хөрвүүлэгч/хостын дотоод (Rust, Kotodama эх сурвалж биш)

Доорх хэсгүүд нь багажны гинжний Rust талд байрладаг. Эдгээр нь хөрвүүлэгчийн туслахууд болон VM бууруулах механикуудыг дүрсэлсэн бөгөөд Kotodama `.ko` эх сурвалж **хүчтэй биш** юм.

## Өргөн opcode хэсэгчилсэн хүрээний шинэчлэлтүүд

Kotodama-ийн өргөн opcode туслахууд нь IVM-ийн ашигладаг 8 битийн операндын байршлыг чиглүүлдэг.
өргөн кодчилол. 128 битийн утгыг зөөж ачаалах ба хадгалалтууд гурав дахь операндыг дахин ашигладаг
өндөр регистрийн үүр, тиймээс үндсэн регистрийн аль хэдийн эцсийн байх ёстой
хаяг. Ачаа/хадгалахаасаа өмнө `ADDI`-ээр суурийг тохируулна уу:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Хуваарилсан хүрээний шинэчлэлтүүд нь 16 байт алхмаар суурийг сайжруулж, бүртгэлийг баталгаажуулдаг
`STORE128`-ийн хийсэн хос нь шаардлагатай тэгшлэх хил дээр бууна. Үүнтэй адил
загвар `LOAD128`-д хамаарна; өмнө нь хүссэн алхамтай `ADDI` гаргах
Ачаалал бүр нь өндөр хүрэх регистрийг гурав дахь операндын үүрэнд холбодог.
VM-тэй таарч байгаа `VMError::MisalignedAccess`-тай буруу тохируулсан хаягууд
`crates/ivm/tests/wide_memory128.rs`-д хэрэгжүүлсэн зан үйл.

Эдгээр 128 битийн туслагчийг ялгаруулдаг программууд нь векторын чадварыг сурталчлах ёстой.
Kotodama хөрвүүлэгч нь `VECTOR` горимын битийг хүссэн үедээ автоматаар идэвхжүүлдэг.
`LOAD128`/`STORE128` гарч ирнэ; нь VM урхи
Хэрэв програм тэдгээрийг ажиллуулахыг оролдвол `VMError::VectorExtensionDisabled`
тэр битийн тохиргоогүйгээр.

## Өргөн нөхцөлт салбарыг буулгах

Kotodama нь `if`/`else` буюу гуравдагч салбарыг өргөн байт код руу буулгахад энэ нь
тогтмол `BNE cond, zero, +2` дараалал, дараа нь `JAL` хос заавар:

1. Богино `BNE` нь нөхцөлт салбарыг 8 битийн шууд эгнээнд байлгадаг.
   `JAL` уналтын дээгүүр харайснаар.
2. Эхний `JAL` нь `else` блокийг онилдог (нөхцөл байгаа үед гүйцэтгэнэ.
   худал).
3. Хоёр дахь `JAL` нь `then` блок руу шилждэг (нөхцөл байгаа үед авсан болно.
   үнэн).

Энэ загвар нь нөхцөл шалгахад илүү том офсет кодлох шаардлагагүй гэдгийг баталгаажуулдаг
`then`-д дур мэдэн том биетүүдийг дэмжсэн хэвээр байхад ±127 үгээс илүү
болон `else` өргөн `JAL` туслагчаар дамжуулан блоклодог. Харна уу
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal`-д зориулагдсан
дарааллыг түгжих регрессийн тест.

### Бууруулах жишээ

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Дараах өргөн зааврын араг ясыг эмхэтгэдэг (регистрийн дугаар ба
үнэмлэхүй офсет нь хаалттай функцээс хамаарна):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Дараагийн заавар нь тогтмолуудыг бодит болгож, буцах утгыг бичнэ.
`BNE` нь эхний `JAL` дээгүүр үсрэх тул нөхцөлт офсет нь үргэлж байдаг.
`+2` үгс, блокийн биетүүд тэлэх үед ч салбарыг хүрээн дотор байлгана.