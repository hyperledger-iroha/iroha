---
lang: ba
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Миҫалдарға дөйөм ҡараш

Был биттә Kotodama миҫалдары һәм улар IVM syscalls һәм күрһәткес‐АБИ аргументтарына нисек картаға төшөрөүе күрһәтелгән. Шулай уҡ ҡарағыҙ:
- Йүгерергә яраҡлы сығанаҡтар өсөн `examples/`
- `docs/source/ivm_syscalls.md` канонлы сискалл АБИ өсөн
- Тулы тел спецификацияһы өсөн `kotodama_grammar.md`

## Һаумыһығыҙ + иҫәп яҙмаһы ентекле

Сығанаҡ: `examples/hello/hello.ko`

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

Картинг (пункт-АБИ):
- `authority()` → `SCALL 0xA4` (хост `&AccountId` яҙа `r10`)
- `set_account_detail(a, k, v)` → күсеп Kotodama, `r11=&Name`, `r12=&Json`, һуңынан Kotodama.

## Активтар тапшырыу

Сығанаҡ: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
      account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

Картинг (пункт-АБИ):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, Norito, һуңынан Kotodama.

## NFT булдырыу + Трансфер

Сығанаҡ: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let recipient = account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Картинг (пункт-АБИ):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, Kotodama.

## Norito ярҙамсылар

Һылтанмалы ныҡлы хәл өсөн TLV-ларҙы үҙгәртеп ҡороу талап ителә һәм уларҙан .
`NoritoBytes` конверты, унда хужалар һаҡлана. Kotodama хәҙер был ярҙамсыларҙы сымдар
туранан-тура компилятор аша, шулай итеп, төҙөүселәр ҡулланыу мөмкин күрһәткес ғәҙәттәгесә һәм карта
ҡул менән ФФИ йәбештереүсеһе булмаған эҙләүҙәр:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Түбәнге:

- Тапала TLV баҫтырғандан һуң Kotodama сығарылыш сығарылыштары ғәҙәттәгесә, шулай уҡ
  хост һаҡлау өсөн канонлы `NoritoBytes` файҙалы йөк ала.
- Уҡыусылар кире эшләүҙе башҡарыу менән `POINTER_FROM_NORITO`, тәьмин итеү .
  көтөлгән күрһәткес тип id `r11`.
- Ике юл да автоматик рәүештә туранан-тура TLV-ларҙы ИНПУТ төбәгенә баҫтырып сығара, был мөмкинлек бирә.
  ҡыҫҡартыуҙар епле литералдар һәм эшләү ваҡыты күрһәткестәрен үтә күренмәле ҡатыштырып.

Ҡарағыҙ `crates/ivm/tests/kotodama_pointer_args.rs` өсөн йөрөү ваҡытында регрессия, тип
18NI000000057X ҡаршы roup-walls күнекмәләр.

## Детерминистик карта итерацион (дизайн)

Детерминистик карта өсөн һәр береһе бәйләнгән талап итә. Күп яҙма итерацион дәүләт картаһы талап итә; компилятор `.take(n)` йәки иғлан ителгән максималь оҙонлоғон ҡабул итә.

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

Семантика:
- Итерация комплекты — иллюминаторға инеүҙә снимок; заказ лексикографик Norito байт асҡыс.
- структур мутациялар `M` иллюзия тоҙаҡында `E_ITER_MUTATION` менән.
- Компиляторға бәйләнмәйенсә `E_UNBOUNDED_ITERATION` сыға.

## Компилятор/хужа эскесе (Раст, Kotodama сығанаҡ түгел)

Түбәндәге өҙөктәр инструменттар сылбырының Раст яғында йәшәй. Улар компилятор ярҙамсылары һәм виртуаль механиканы иллюстрациялай һәм ** түгел ** дөрөҫ Kotodama `.ko` сығанағы.

## Киң Кодкод рамка Яңыртыуҙар өлөшө

Kotodama’s киң опкод ярҙамсылары маҡсатлы 8-бит операнд макеты ҡулланылған IVM .
киң кодлау. 128-битлы ҡиммәттәрҙе күсергән йөктәр һәм һаҡлау өсөнсө операндты ҡабаттан файҙалана
слот өсөн юғары реестр, шуға күрә база реестры инде финал үткәрергә тейеш
адрес. Йөк/магазин биргәнсе `ADDI` менән базаны көйләү:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```16-байлы аҙымдарҙа базаны алға ебәргән кадрҙарҙы яңыртыу, регистрҙы тәьмин итеү
пар ҡылған `STORE128` ерҙәре кәрәкле тура килтереп сик. Шул уҡ .
өлгөһө `LOAD128`-ҡа ҡағыла; 2012 йылға тиклем теләкле аҙым менән `ADDI` сығарыу.
һәр йөк юғары тәғәйенләнештәге регистр тота өсөнсө операнд слотына бәйле.
Дөрөҫ булмаған адрестар тоҙаҡ менән `VMError::MisalignedAccess`, тап килтереп VM .
тәртибе `crates/ivm/tests/wide_memory128.rs`-та башҡарылған.

Был 128-битлы ярҙамсыларҙы сығарған программалар вектор мөмкинлектәрен рекламаларға тейеш.
Kotodama компиляторы мөмкинлек бирә `VECTOR` режимы автоматик рәүештә ҡасан ғына .
`LOAD128`/`STORE128` барлыҡҡа килә; ВМ тоҙаҡтары менән
`VMError::VectorExtensionDisabled`, әгәр программа уларҙы башҡарырға тырышһа,
тип бит ҡуйылған.

##

Ҡасан Kotodama түбән түбән `if`/`else` йәки өс өлөшлө филиалы киң байткод уны сығара
18NI00000075X эҙмә-эҙлеклелеге нығытылған, унан һуң `JAL` инструкциялары пары:

1. Ҡыҫҡа `BNE` 8-битлы тиҙ арала һыҙат эсендә шартлы тармаҡты һаҡлай.
   һикереп аша үткән `JAL`.
.
   ялған).
3. Икенсе `JAL` һикереп `then` блок (ҡасан ҡабул ителә, ҡасан шарт 1990).
   хәҡиҡәт).

Был ҡалып гарантиялай шарт тикшерергә бер ҡасан да кәрәк кодировка офсет ҙурыраҡ .
±127 һүҙгә ҡарағанда, шул уҡ ваҡытта `then` өсөн үҙ теләге менән ҙур тәндәргә ярҙам иткән.
һәм `else` блоктар аша киң `JAL` ярҙамсыһы. Күрергә
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` өсөн 2019 йыл.
эҙмә-эҙлеклелеккә бикләп торған регрессия һынауы.

### Миҫалдың түбәнәйеүе

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Түбәндәге киң инструкция скелетына компиляциялар (теркәү һандары һәм
абсолют офсеттар ябыу функцияһына бәйле):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Артабанғы күрһәтмәләр константаларҙы матдилаштыра һәм ҡайтарыу ҡиммәтен яҙа.
Сөнки `BNE` һикереп өҫтөндә беренсе `JAL`, шартлы офсет һәр ваҡыт .
`+2` һүҙҙәр, диапазон эсендә тармаҡты һаҡлау хатта блок корпустары киңәйгәндә.