---
lang: ka
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama მაგალითების მიმოხილვა

ეს გვერდი გვიჩვენებს Kotodama ლაკონურ მაგალითებს და როგორ ასახავს ისინი IVM სიკალებს და პოინტერ-ABI არგუმენტებს. აგრეთვე იხილეთ:
- `examples/` გაშვებადი წყაროებისთვის
- `docs/source/ivm_syscalls.md` კანონიკური syscall ABI-სთვის
- `kotodama_grammar.md` ენის სრული სპეციფიკაციისთვის

## გამარჯობა + ანგარიშის დეტალები

წყარო: `examples/hello/hello.ko`

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

რუკების შედგენა (pointer-ABI):
- `authority()` → `SCALL 0xA4` (მასპინძელი წერს `&AccountId`-ს `r10`-ში)
- `set_account_detail(a, k, v)` → გადაადგილება `r10=&AccountId`, `r11=&Name`, `r12=&Json`, შემდეგ `SCALL 0x1A`

## აქტივების გადაცემა

წყარო: `examples/transfer/transfer.ko`

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

რუკების შედგენა (pointer-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, შემდეგ `SCALL 0x24`

## NFT შექმნა + გადაცემა

წყარო: `examples/nft/nft.ko`

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

რუკების შედგენა (pointer-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Pointer Norito დამხმარეები

მაჩვენებლის მდგრადი მდგომარეობა მოითხოვს აკრეფილი TLV-ების კონვერტაციას
`NoritoBytes` კონვერტი, რომელიც მასპინძლებს რჩება. Kotodama ახლა აერთებს ამ დამხმარეებს
პირდაპირ შემდგენელის მეშვეობით, რათა შემქმნელებმა გამოიყენონ მაჩვენებლის ნაგულისხმევი პარამეტრები და რუკა
ძიება ხელით FFI წებოს გარეშე:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let first = Owners.ensure(7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    let again = Owners.ensure(7, bob);
    assert(again == alice);
  }
}
```

დაწევა:

- მაჩვენებელი ნაგულისხმევი ასხივებს `POINTER_TO_NORITO` აკრეფილი TLV გამოქვეყნების შემდეგ, ასე რომ
  მასპინძელი იღებს კანონიკურ `NoritoBytes` დატვირთვას შესანახად.
- წაკითხული ასრულებს საპირისპირო ოპერაციას `POINTER_FROM_NORITO`-ით, ამარაგებს
  მოსალოდნელი მაჩვენებელი ტიპის ID-ში `r11`.
- ორივე გზა ავტომატურად აქვეყნებს ლიტერატურულ TLV-ებს INPUT რეგიონში, რაც საშუალებას იძლევა
  კონტრაქტები სიმებიანი ლიტერალების და გაშვების მაჩვენებლების გამჭვირვალედ შერევისთვის.

იხილეთ `crates/ivm/tests/kotodama_pointer_args.rs` გაშვების დროის რეგრესიისთვის
ახორციელებს ორმხრივ მოგზაურობას `MockWorldStateView`-ის წინააღმდეგ.

## დეტერმინისტული რუკის გამეორება (დიზაინი)

განმსაზღვრელი რუკა თითოეულისთვის მოითხოვს ზღვარს. მრავალჯერადი გამეორება მოითხოვს სახელმწიფო რუკას; შემდგენელი იღებს `.take(n)` ან გამოცხადებულ მაქსიმალურ სიგრძეს.

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

სემანტიკა:
- გამეორების ნაკრები არის სნეპშოტი მარყუჟის შესვლისას; შეკვეთა ლექსიკოგრაფიულია გასაღების Norito ბაიტით.
- სტრუქტურული მუტაციები `M`-ში მარყუჟის ხაფანგში `E_ITER_MUTATION`-ით.
- შეზღუდვის გარეშე შემდგენელი გამოსცემს `E_UNBOUNDED_ITERATION`.

## შემდგენელი/ჰოსტის შიდა ნაწილები (Rust, არა Kotodama წყარო)

ქვემოთ მოყვანილი ფრაგმენტები განთავსებულია ხელსაწყოების ჯაჭვის Rust მხარეს. ისინი ასახავს შემდგენელ დამხმარეებს და VM-ის დაწევის მექანიკას და **არ** მოქმედებს Kotodama `.ko` წყარო.

## ფართო ოპკოდის დაქუცმაცებული ჩარჩოს განახლებები

Kotodama-ის ფართო opcode დამხმარეები მიზნად ისახავს 8-ბიტიანი ოპერანდის განლაგებას, რომელსაც იყენებს IVM
ფართო კოდირება. იტვირთება და ინახავს 128-ბიტიან მნიშვნელობებს, ხელახლა იყენებს მესამე ოპერანდს
სლოტი მაღალი რეესტრისთვის, ამიტომ საბაზო რეესტრმა უკვე უნდა დაიკავოს საბოლოო
მისამართი. დაარეგულირეთ ბაზა `ADDI`-ით დატვირთვის/საღაზიის გაცემამდე:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```დაქუცმაცებული ჩარჩოს განახლებები 16 ბაიტიანი ნაბიჯებით აგრძელებს ბაზას, რაც უზრუნველყოფს რეესტრს
წყვილი ჩადენილი `STORE128`-ის მიერ დაჯდება საჭირო გასწორების საზღვარზე. იგივე
ნიმუში ვრცელდება `LOAD128`-ზე; `ADDI`-ის გაცემა სასურველი ნაბიჯით ადრე
თითოეული დატვირთვა ინახავს მაღალი დანიშნულების რეგისტრს მიბმული მესამე ოპერანდის სლოტთან.
არასწორად მორგებული მისამართების ხაფანგში `VMError::MisalignedAccess`, რომელიც შეესაბამება VM-ს
ქცევა განხორციელებული `crates/ivm/tests/wide_memory128.rs`-ში.

პროგრამებმა, რომლებიც ასხივებენ ამ 128-ბიტიან დამხმარეებს, უნდა აჩვენონ ვექტორული შესაძლებლობები.
Kotodama შემდგენელი რთავს `VECTOR` რეჟიმის ბიტს ავტომატურად, როდესაც
გამოჩნდება `LOAD128`/`STORE128`; VM ხაფანგები ერთად
`VMError::VectorExtensionDisabled` თუ პროგრამა შეეცდება მათ შესრულებას
ბიტის ნაკრების გარეშე.

## ფართო პირობითი ტოტის დაწევა

როდესაც Kotodama ამცირებს `if`/`else` ან სამჯერადი განშტოების ფართო ბაიტიკოდს, ის გამოსცემს
ფიქსირებული `BNE cond, zero, +2` თანმიმდევრობა, რასაც მოჰყვება წყვილი `JAL` ინსტრუქცია:

1. მოკლე `BNE` ინარჩუნებს პირობით განშტოებას 8-ბიტიან უშუალო ზოლში
   გადახტომით `JAL`.
2. პირველი `JAL` მიზნად ისახავს `else` ბლოკს (შესრულებულია, როდესაც მდგომარეობა არის
   ყალბი).
3. მეორე `JAL` გადახტება `then` ბლოკზე (მიღებულია, როდესაც მდგომარეობა არის
   მართალია).

ეს ნიმუში იძლევა გარანტიას, რომ მდგომარეობის შემოწმებას არასოდეს სჭირდება უფრო დიდი ოფსეტების კოდირება
ვიდრე ±127 სიტყვა, მიუხედავად იმისა, რომ მხარს უჭერს თვითნებურად დიდ სხეულებს `then`-ისთვის
და `else` ბლოკავს ფართო `JAL` დამხმარე საშუალებით. იხ
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` ამისთვის
რეგრესიის ტესტი, რომელიც იკეტება თანმიმდევრობით.

### მაგალითი დაწევა

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

ადგენს შემდეგ ფართო ინსტრუქციის ჩონჩხს (რეგისტრაციის ნომრები და
აბსოლუტური ოფსეტები დამოკიდებულია დაფარვის ფუნქციაზე):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

შემდგომი ინსტრუქციები ასახავს მუდმივებს და წერს დაბრუნების მნიშვნელობას.
იმის გამო, რომ `BNE` გადახტება პირველ `JAL`-ზე, პირობითი ოფსეტი ყოველთვის არის
`+2` სიტყვები, ინარჩუნებს ტოტს დიაპაზონში მაშინაც კი, როდესაც ბლოკის სხეულები ფართოვდება.