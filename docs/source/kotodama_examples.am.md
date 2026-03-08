---
lang: am
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama ምሳሌዎች አጠቃላይ እይታ

ይህ ገጽ አጭር የKotodama ምሳሌዎችን እና እንዴት ወደ IVM syscals እና pointer-ABI ነጋሪ እሴቶችን እንደሚያሳዩ ያሳያል። በተጨማሪ ይመልከቱ፡
- `examples/` ለሩጫ ምንጮች
- `docs/source/ivm_syscalls.md` ለ ቀኖናዊ syscall ABI
- `kotodama_grammar.md` ለሙሉ የቋንቋ ዝርዝር መግለጫ

## ሰላም + የመለያ ዝርዝር

ምንጭ፡ `examples/hello/hello.ko`

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

ካርታ ስራ (ጠቋሚ-ABI)፡-
- `authority()` → `SCALL 0xA4` (አስተናጋጁ `&AccountId` ወደ `r10` ይጽፋል)
- `set_account_detail(a, k, v)` → ማንቀሳቀስ `r10=&AccountId`፣ `r11=&Name`፣ `r12=&Json`፣ ከዚያ `SCALL 0x1A`

## የንብረት ማስተላለፍ

ምንጭ፡ `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
      account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```

ካርታ ስራ (ጠቋሚ-ABI)፡-
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`፣ `r11=&AccountId(to)`፣ `r12=&AssetDefinitionId(def)`፣ `r13=amount`፣ ከዚያ `SCALL 0x24`

## NFT ይፍጠሩ + ያስተላልፉ

ምንጭ፡ `examples/nft/nft.ko`

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

ካርታ ስራ (ጠቋሚ-ABI)፡-
- `nft_mint_asset(id, owner)` → `r10=&NftId`፣ `r11=&AccountId(owner)`፣ `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`፣ `r11=&NftId`፣ `r12=&AccountId(to)`፣ `SCALL 0x26`

## ጠቋሚ Norito አጋዦች

በጠቋሚ ዋጋ ያለው ዘላቂ ሁኔታ የተተየቡ TLVዎችን ወደ እና ከ መለወጥ ያስፈልገዋል
የሚያስተናግደው `NoritoBytes` ፖስታ እንደቀጠለ ነው። Kotodama አሁን እነዚህን ረዳቶች በሽቦ ያገናኛል።
ግንበኞች የጠቋሚ ነባሪዎችን እና ካርታዎችን መጠቀም እንዲችሉ በቀጥታ በማቀናበሪያው በኩል
ያለ በእጅ የኤፍኤፍአይ ሙጫ ፍለጋዎች

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

ዝቅ ማድረግ፡

- የጠቋሚ ነባሪዎች የተተየበው TLV ካተም በኋላ `POINTER_TO_NORITO` ያወጣል፣ ስለዚህ
  አስተናጋጁ ለማከማቻ ቀኖናዊ `NoritoBytes` ክፍያ ይቀበላል።
- አንብቦ የተገላቢጦሽ ክዋኔውን በ `POINTER_FROM_NORITO` ያከናውናል፣
  የሚጠበቀው የጠቋሚ አይነት መታወቂያ በ`r11`።
- ሁለቱም መንገዶች በቀጥታ በቀጥታ TLVs ወደ INPUT ክልል ያትማሉ፣ ይህም ይፈቅዳሉ
  የሕብረቁምፊ ቃል በቃል እና የሩጫ ጊዜ አመልካቾችን በግልፅ ለማደባለቅ ኮንትራቶች።

ለአሂድ ጊዜ መመለሻ `crates/ivm/tests/kotodama_pointer_args.rs` ይመልከቱ
የዙር ጉዞውን ከ `MockWorldStateView` ጋር ይለማመዳል።

## ቆራጥ የካርታ መደጋገም (ንድፍ)

ለእያንዳንዱ ቆራጥ ካርታ ገደብ ያስፈልገዋል። ባለብዙ-ግቤት ድግግሞሽ የስቴት ካርታ ያስፈልገዋል; አቀናባሪው `.take(n)` ወይም የተገለጸውን ከፍተኛ ርዝመት ይቀበላል።

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

ትርጉም፡
- የድግግሞሽ ስብስብ በ loop መግቢያ ላይ ቅጽበታዊ ገጽ እይታ ነው; ትዕዛዝ በ Norito የቁልፉ ባይት መዝገበ ቃላት ነው።
- መዋቅራዊ ሚውቴሽን ወደ `M` በ loop trap ከ `E_ITER_MUTATION` ጋር።
- ያለ ገደብ ማጠናከሪያው `E_UNBOUNDED_ITERATION` ያወጣል።

## ኮምፕሌተር/አስተናጋጅ የውስጥ (ዝገት እንጂ Kotodama ምንጭ አይደለም)

ከታች ያሉት ቅንጥቦች በመሳሪያው ሰንሰለት ዝገት ጎን ላይ ይኖራሉ። የማጠናከሪያ አጋዥዎችን እና የቪኤም ዝቅ ማድረጊያ መካኒኮችን ይገልጻሉ እና ** አይደሉም ** ልክ ያልሆኑ Kotodama `.ko` ምንጭ።

## ሰፊ የኦፕኮድ የተሰነጠቀ ፍሬም ዝማኔዎች

የKotodama ሰፊ የኦፕኮድ ረዳቶች በIVM ጥቅም ላይ የዋለውን ባለ 8-ቢት ኦፔራ አቀማመጥ ያነጣጥራሉ
ሰፊ ኢንኮዲንግ. 128-ቢት እሴቶችን የሚያንቀሳቅሱ ጭነቶች እና መደብሮች ሶስተኛውን ኦፔራንድ እንደገና ይጠቀማሉ
ማስገቢያ ለከፍተኛ መመዝገቢያ, ስለዚህ የመሠረት መዝገብ ቀድሞውኑ የመጨረሻውን መያዝ አለበት
አድራሻ. ጭነቱን/መደብሩን ከማውጣትዎ በፊት መሰረቱን በ`ADDI` ያስተካክሉት፡

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```የተቆራረጡ የፍሬም ማሻሻያዎች መሰረቱን በ16-ባይት ደረጃዎች ያራምዳሉ፣ ይህም መዝገቡን ያረጋግጣል
በ `STORE128` መሬቶች የተፈፀመ ጥንድ በሚፈለገው የአሰላለፍ ወሰን ላይ። ተመሳሳይ
ስርዓተ ጥለት ለ `LOAD128`; ከዚህ በፊት `ADDI` በሚፈለገው ደረጃ በማውጣት ላይ
እያንዳንዱ ጭነት ከፍተኛውን የመድረሻ መዝገብ ከሶስተኛው ኦፔራንድ ማስገቢያ ጋር በማያያዝ ይይዛል.
የተሳሳቱ አድራሻዎች ከቪኤም ጋር የሚዛመዱ ከ`VMError::MisalignedAccess` ጋር ወጥመድ ይይዛሉ
በ `crates/ivm/tests/wide_memory128.rs` ውስጥ የተለማመዱ ባህሪያት.

እነዚህን ባለ 128 ቢት ረዳቶች የሚለቁ ፕሮግራሞች የቬክተር አቅምን ማስተዋወቅ አለባቸው።
የ Kotodama ማጠናከሪያ የ `VECTOR` ሞድ ቢትን በማንኛውም ጊዜ ያነቃል።
`LOAD128`/`STORE128` ይታያል; የ VM ወጥመዶች ጋር
አንድ ፕሮግራም እነሱን ለማስፈጸም ቢሞክር `VMError::VectorExtensionDisabled`
ያለዚያ ትንሽ ስብስብ።

## ሰፊ ሁኔታዊ ቅርንጫፍ ዝቅ ማድረግ

Kotodama `if`/`else` ወይም ternary ቅርንጫፍ ወደ ሰፊ ባይትኮድ ሲቀንስ
የተስተካከለ `BNE cond, zero, +2` ቅደም ተከተል ተከትሎ ጥንድ `JAL` መመሪያዎች:

1. አጭሩ `BNE` ሁኔታዊ ቅርንጫፉን በ8-ቢት አፋጣኝ መስመር ውስጥ ያስቀምጣል።
   በመውደቅ `JAL` ላይ በመዝለል.
2. የመጀመሪያው `JAL` `else` ብሎክን ያነጣጥራል (ሁኔታው ሲከሰት ይከናወናል)
   ውሸት)።
3. ሁለተኛው `JAL` ወደ `then` ብሎክ (ሁኔታው ሲከሰት ይወሰዳል)
   እውነት)።

ይህ ስርዓተ-ጥለት የሁኔታ ፍተሻው መቼም ቢሆን ትልቅ ማካካሻዎችን መመስጠር እንደማያስፈልገው ዋስትና ይሰጣል
ለ `then` በዘፈቀደ ትላልቅ አካላትን እየደገፈ ከ± 127 ቃላት በላይ
እና `else` ብሎኮች በሰፊው `JAL` ረዳት በኩል። ተመልከት
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` ለ
በቅደም ተከተል ውስጥ የሚቆለፈው የሪግሬሽን ሙከራ.

### ምሳሌ ዝቅ ማድረግ

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

ወደሚከተለው ሰፊ የማስተማሪያ አጽም ያጠናቅራል (ቁጥሮችን መመዝገቢያ እና
ፍጹም ማካካሻዎች በማቀፊያው ተግባር ላይ የተመሰረቱ ናቸው)

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

የሚቀጥሉት መመሪያዎች ቋሚዎችን ተጨባጭነት ያደረጉ እና የመመለሻ ዋጋውን ይፃፉ.
`BNE` በመጀመሪያው `JAL` ላይ ስለሚዘል፣ ሁኔታዊ ማካካሻ ሁል ጊዜ ነው።
`+2` ቃላት፣ የቅርንጫፉን አካል በክልል ውስጥ በማቆየት የማገጃ አካላት ሲሰፋም።