---
lang: kk
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Мысалдар шолу

Бұл бетте қысқаша Kotodama мысалдары және олардың IVM жүйелік қоңыраулары мен көрсеткіш-ABI аргументтерімен салыстыру жолы көрсетілген. Сондай-ақ қараңыз:
- іске қосылатын көздер үшін `examples/`
- ABI канондық жүйесі үшін `docs/source/ivm_syscalls.md`
- Толық тіл сипаттамасы үшін `kotodama_grammar.md`

## Сәлем + Есептік жазба мәліметтері

Дереккөз: `examples/hello/hello.ko`

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

Карталау (көрсеткіш-ABI):
- `authority()` → `SCALL 0xA4` (хост `&AccountId` файлын `r10` ішіне жазады)
- `set_account_detail(a, k, v)` → жылжыту `r10=&AccountId`, `r11=&Name`, `r12=&Json`, содан кейін `SCALL 0x1A`

## Активтерді тасымалдау

Дереккөз: `examples/transfer/transfer.ko`

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

Карталау (көрсеткіш-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, содан кейін `SCALL 0x24`

## NFT Жасау + Тасымалдау

Дереккөз: `examples/nft/nft.ko`

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

Карталау (көрсеткіш-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Көрсеткіш Norito Көмекшілер

Көрсеткіштің тұрақты күйі терілген TLV мәндерін келесіге және одан түрлендіруді талап етеді
Хосттар сақталатын `NoritoBytes` конверт. Kotodama енді осы көмекшілерді өткізеді
құрастырушылар көрсеткіштің әдепкі мәндерін және картаны пайдалана алатындай етіп тікелей компилятор арқылы
қолмен FFI желімсіз іздеулер:

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

Төмендету:

- Меңзердің әдепкі мәндері терілген TLV жарияланғаннан кейін `POINTER_TO_NORITO` шығарады, сондықтан
  хост сақтау үшін канондық `NoritoBytes` пайдалы жүктемесін алады.
- Оқулар `POINTER_FROM_NORITO` көмегімен кері әрекетті орындайды, оны қамтамасыз етеді
  `r11` ішіндегі күтілетін көрсеткіш түрінің идентификаторы.
- Екі жол да INPUT аймағына тікелей TLV мәндерін автоматты түрде жариялайды, бұл мүмкіндік береді
  жол литералдары мен орындалу уақыты көрсеткіштерін мөлдір араластыруға келісім-шарттар жасайды.

Орындалу уақыты регрессиясын `crates/ivm/tests/kotodama_pointer_args.rs` қараңыз
`MockWorldStateView` қарсы бару-қайтуды жүзеге асырады.

## Детерминистік карта итерациясы (дизайн)

Әрқайсысы үшін детерминистік карта шекті қажет етеді. Көп жазбалы итерация күй картасын қажет етеді; компилятор `.take(n)` немесе жарияланған максималды ұзындықты қабылдайды.

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
- Итерациялар жиыны – циклды енгізу кезіндегі сурет; реті кілттің Norito байты бойынша лексикографиялық.
- `E_ITER_MUTATION` контурлық тұзақтағы `M` құрылымдық мутациялары.
- Шектеусіз компилятор `E_UNBOUNDED_ITERATION` шығарады.

## Компилятор/хосттың ішкі бөліктері (Rust, Kotodama көзі емес)

Төмендегі үзінділер құралдар тізбегінің Rust жағында орналасқан. Олар компилятор көмекшілерін және VM төмендету механикасын суреттейді және **жарамсыз** Kotodama `.ko` көзі болып табылады.

## Кең операциялық кодты біріктірілген жақтау жаңартулары

Kotodama кең операциялық код көмекшілері IVM пайдаланатын 8 биттік операнд орналасуына бағытталған.
кең кодтау. 128 биттік мәндерді жылжытатын жүктер мен қоймалар үшінші операндты қайта пайдаланады
жоғары тізілімге арналған слот, сондықтан базалық тізілім финалды ұстауы керек
мекенжайы. Жүктемені/сақтауды бермес бұрын негізді `ADDI` көмегімен реттеңіз:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Бөлшектелген кадр жаңартулары тізілімді қамтамасыз ете отырып, базаны 16 байт қадамдармен алға жылжытады
`STORE128` арқылы жасалған жұп қажетті туралау шекарасына түседі. Дәл солай
үлгі `LOAD128` үшін қолданылады; бұрын қажетті қадаммен `ADDI` шығару
әрбір жүктеме үшінші операнд ұяшығына байланысты жоғары тағайындалған регистрді сақтайды.
VM-ге сәйкес келетін `VMError::MisalignedAccess` қате тураланбаған мекенжайлар тұзағы
`crates/ivm/tests/wide_memory128.rs` ішінде орындалған мінез-құлық.

Осы 128-биттік көмекшілерді шығаратын бағдарламалар вектор мүмкіндігін жарнамалауы керек.
Kotodama компиляторы `VECTOR` режимінің битін кез келген уақытта автоматты түрде қосады
`LOAD128`/`STORE128` пайда болады; VM тұзақтарды
`VMError::VectorExtensionDisabled`, егер бағдарлама оларды орындауға әрекет жасаса
бұл бит орнатылмаған.

## Кең шартты тармақты төмендету

Kotodama `if`/`else` немесе үштік тармақты кең байт кодқа түсіргенде, ол
бекітілген `BNE cond, zero, +2` тізбегі, одан кейін `JAL` жұбы нұсқаулары:

1. Қысқа `BNE` шартты тармақты 8 биттік тікелей жолақ ішінде сақтайды
   `JAL` құлау арқылы секіру арқылы.
2. Бірінші `JAL` `else` блогына бағытталған (шарт орындалғанда орындалады.
   жалған).
3. Екінші `JAL` `then` блогына өтеді (шарт орындалғанда алынады
   шын).

Бұл үлгі шартты тексеруге ешқашан үлкенірек офсеттерді кодтаудың қажеті жоқ екеніне кепілдік береді
`then` үшін ерікті үлкен денелерді әлі де қолдаған кезде ±127 сөзден артық
және `else` блоктары кең `JAL` көмекшісі арқылы. Қараңыз
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` үшін
реттілігін бекітетін регрессия сынағы.

### Төмендету мысалы

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Келесі кең нұсқау скелетіне құрастырады (тізілім нөмірлері және
абсолютті ығысулар қоршау функциясына байланысты):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Келесі нұсқаулар тұрақты мәндерді материалдандырады және қайтарылатын мәнді жазады.
`BNE` бірінші `JAL` үстінен өтетіндіктен, шартты ығысу әрқашан болады
`+2` сөздер, тіпті блок денелері кеңейген кезде де тармақты ауқымда сақтайды.