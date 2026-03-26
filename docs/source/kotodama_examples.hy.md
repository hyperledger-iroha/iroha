---
lang: hy
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Օրինակների ակնարկ

Այս էջը ցույց է տալիս հակիրճ Kotodama օրինակներ և ինչպես են դրանք քարտեզագրվում IVM համակարգերի և ցուցիչ-ABI արգումենտների հետ: Տես նաև.
- `examples/` գործարկվող աղբյուրների համար
- `docs/source/ivm_syscalls.md` կանոնական syscall ABI-ի համար
- `kotodama_grammar.md` լեզվի ամբողջական ճշգրտման համար

## Բարև + Հաշվի մանրամասներ

Աղբյուրը` `examples/hello/hello.ko`

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

Քարտեզագրում (ցուցիչ-ABI):
- `authority()` → `SCALL 0xA4` (հյուրընկալողը գրում է `&AccountId` `r10`-ում)
- `set_account_detail(a, k, v)` → տեղափոխել `r10=&AccountId`, `r11=&Name`, `r12=&Json`, ապա `SCALL 0x1A`

## Ակտիվների փոխանցում

Աղբյուրը՝ `examples/transfer/transfer.ko`

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

Քարտեզագրում (ցուցիչ-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, ապա `SCALL 0x24`

## NFT Ստեղծում + փոխանցում

Աղբյուրը` `examples/nft/nft.ko`

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

Քարտեզագրում (ցուցիչ-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Ցուցիչ Norito Օգնականներ

Ցուցանիշի դիմացկուն վիճակը պահանջում է մուտքագրված TLV-ների փոխակերպում դեպի և դրանից
`NoritoBytes` ծրարը, որը տանտերերը պահպանվում են: Kotodama-ն այժմ միացնում է այս օգնականները
ուղղակիորեն կոմպիլյատորի միջոցով, որպեսզի կառուցողները կարողանան օգտագործել ցուցիչի լռելյայն և քարտեզ
որոնումներ առանց ձեռքով FFI սոսինձի.

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

Իջեցում:

- Սլաքի կանխադրվածները թողարկում են `POINTER_TO_NORITO` մուտքագրված TLV-ն հրապարակելուց հետո, ուստի
  հյուրընկալողը պահեստավորման համար ստանում է կանոնական `NoritoBytes` օգտակար բեռ:
- Ընթերցումները կատարում են հակադարձ գործողությունը `POINTER_FROM_NORITO`-ով, մատակարարելով այն
  ակնկալվող ցուցիչի տիպի ID-ն `r11`-ում:
- Երկու ուղիներն էլ ավտոմատ կերպով հրապարակում են բառացի TLV-ներ INPUT տարածաշրջանում՝ թույլ տալով
  պայմանագրեր՝ թափանցիկ կերպով խառնելու լարային տառերը և գործարկման ժամանակի ցուցիչները:

Տե՛ս `crates/ivm/tests/kotodama_pointer_args.rs` գործարկման ժամանակի ռեգրեսիայի համար
իրականացնում է շրջագայությունը `MockWorldStateView`-ի դեմ:

## Դետերմինիստական քարտեզի կրկնություն (դիզայն)

Դետերմինիստական քարտեզը յուրաքանչյուրի համար պահանջում է սահման: Բազմակի մուտքերի կրկնությունը պահանջում է պետական ​​քարտեզ; Կազմողն ընդունում է `.take(n)` կամ հայտարարված առավելագույն երկարությունը:

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

Իմաստաբանություն:
- Կրկնվող հավաքածուն ակնթարթ է հանգույց մուտքագրման ժամանակ; կարգը բառարանագրական է Norito բայթով բանալիով:
- Կառուցվածքային մուտացիաներ `M`-ին `E_ITER_MUTATION`-ով հանգույցի թակարդում:
- Առանց սահմանափակման, կոմպիլյատորը թողարկում է `E_UNBOUNDED_ITERATION`:

## Կազմող/հոսթի ներքին տարրեր (Rust, ոչ թե Kotodama աղբյուր)

Ստորև բերված հատվածներն ապրում են գործիքների շղթայի Rust կողմում: Դրանք պատկերում են կոմպիլյատորների օգնականները և VM-ի իջեցման մեխանիզմները և **չեն** վավեր Kotodama `.ko` աղբյուր:

## Wide Opcode Chunked Frame թարմացումներ

Kotodama-ի լայն opcode օգնականները թիրախավորում են 8-բիթանոց օպերանդի դասավորությունը, որն օգտագործվում է IVM-ի կողմից:
լայն կոդավորում: 128-բիթանոց արժեքներ տեղափոխող բեռները և պահումները կրկին օգտագործում են երրորդ օպերանդը
անցք բարձր ռեգիստրի համար, ուստի բազային ռեգիստրն արդեն պետք է անցնի վերջնականը
հասցեն։ Կարգավորեք բազան `ADDI`-ով նախքան բեռը/պահեստը թողարկելը.

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Շրջանակների մասնատված թարմացումներն առաջ են բերում բազան 16 բայթ քայլերով՝ ապահովելով գրանցումը
`STORE128`-ի կողմից կատարված զույգը վայրէջք է կատարում հավասարեցման պահանջվող սահմանի վրա: Նույնը
օրինակը վերաբերում է `LOAD128`-ին; Նախկինում ցանկալի քայլով թողարկելով `ADDI`
Յուրաքանչյուր բեռնվածություն պահպանում է բարձր նպատակակետ գրանցումը կապված երրորդ օպերանդի բնիկին:
Հասցեների սխալ դասավորվածությունը թակարդում է `VMError::MisalignedAccess`-ով, որը համապատասխանում է VM-ին
վարքագիծ, որն իրականացվում է `crates/ivm/tests/wide_memory128.rs`-ում:

Ծրագրերը, որոնք թողարկում են այս 128-բիթանոց օգնականները, պետք է գովազդեն վեկտորի հնարավորությունները:
Kotodama կոմպիլյատորը հնարավորություն է տալիս `VECTOR` ռեժիմի բիթն ավտոմատ կերպով, երբ
`LOAD128`/`STORE128` հայտնվում; որ VM թակարդներ հետ
`VMError::VectorExtensionDisabled`, եթե ծրագիրը փորձի դրանք կատարել
առանց այդ բիթերի հավաքածուի:

## Լայն պայմանական ճյուղի իջեցում

Երբ Kotodama-ը իջեցնում է `if`/`else` կամ եռյակ ճյուղը լայն բայթ կոդի, այն թողարկում է
ֆիքսված `BNE cond, zero, +2` հաջորդականություն, որին հաջորդում է `JAL` զույգ հրահանգները.

1. Կարճ `BNE`-ը պահում է պայմանական ճյուղը 8-բիթանոց անմիջական գոտում
   ցատկելով `JAL` անկման վրայով:
2. Առաջին `JAL`-ն ուղղված է `else` բլոկին (կատարվում է, երբ պայմանը
   կեղծ):
3. Երկրորդ `JAL`-ը ցատկում է դեպի `then` բլոկ (վերցվում է, երբ պայմանը
   ճիշտ է):

Այս օրինաչափությունը երաշխավորում է, որ վիճակի ստուգումը երբեք կարիք չունի ավելի մեծ օֆսեթների կոդավորման
քան ±127 բառ, մինչդեռ `then`-ի համար կամայականորեն մեծ մարմիններ են աջակցվում
և `else` արգելափակում է լայն `JAL` օգնականի միջոցով: Տես
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` համար
ռեգրեսիայի թեստը, որը կողպվում է հաջորդականությամբ:

### Օրինակ իջեցում

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Կազմում է հետևյալ լայն հրահանգների կմախքը (գրանցման համարները և
բացարձակ օֆսեթները կախված են ընդգրկող ֆունկցիայից).

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Հետագա հրահանգները նյութականացնում են հաստատունները և գրում վերադարձի արժեքը:
Քանի որ `BNE`-ը ցատկում է առաջին `JAL`-ի վրայով, պայմանական օֆսեթը միշտ է
`+2` բառեր՝ ճյուղը պահելով տիրույթում, նույնիսկ երբ բլոկի մարմինները ընդլայնվում են: