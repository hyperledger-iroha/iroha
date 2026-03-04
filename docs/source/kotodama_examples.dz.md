---
lang: dz
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T13:08:23.284550+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama དཔེར་བརྗོད།

ཤོག་ལེབ་འདི་གིས་ དཔེ་ཚུ་ བསྡུ་སྒྲིག་འབད་དེ་ Kotodama དང་ དེ་ཚུ་གིས་ IVM syscalls དང་ དཔག་བྱེད་-ABI སྒྲུབ་རྟགས་ཚུ་ལུ་ ག་དེ་སྦེ་ སབ་ཁྲ་བཟོཝ་ཨིན། ད་དུང་གཟིགས།
- གཡོག་བཀོལ་བཏུབ་པའི་འབྱུང་ཁུངས་ཚུ་གི་དོན་ལུ་ `examples/`
- Kotodama འདི་ ཀེ་ནོ་ནིག་སི་ཀཱལ་ ABI གི་དོན་ལུ་ཨིན།
- སྐད་ཡིག་ཆ་ཚང་གསལ་བཀོད་ཀྱི་དོན་ལུ་ `kotodama_grammar.md`

## ཧེ་ལོ་ + རྩིས་ཁྲའི་ཁ་གསལ།

ཡོང་ཁུངས།： 18NI0000024X

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

ས་ཁྲ་ (pointer‑ABI):
- `authority()` → `SCALL 0xA4` (ཧོསཊི་ཧོསཊི་ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༢༧ཨེགསི་འདི་ ཨའི་༡༨ཨེན་ཨའི་༠༠༠༠༠༠༠༢༨ ཨེགསི་ནང་བྲིས།)
- `set_account_detail(a, k, v)` → `r10=&AccountId`, `r11=&Name`, Kotodama, དེ་ལས་ `SCALL 0x1A`,

## བརྗེ་སོར།

ཡོང་ཁུངས།： 18NI0000034X

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"),
      account!("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland"),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```

ས་ཁྲ་ (pointer‑ABI):
→ `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, Kotodama, དེ་ལས་ `SCALL 0x24`, དེ་ལས་ `SCALL 0x24`, དེ་ལས་ `SCALL 0x24`,

## NFT གསར་བསྐྲུན་ + སྤོ་བཤུད།

ཡོང་ཁུངས།： `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland");
    let recipient = account!("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

ས་ཁྲ་ (pointer‑ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, Kotodama, Kotodama, Norito, Kotodama,

## ས་ཚིགས་ Norito རོགས་སྐྱོར།

དཔག་བྱེད་-གནས་གོང་ཐུབ་པའི་གནས་ལུགས་འདི་གིས་ ཡིག་དཔར་རྐྱབས་ཡོད་པའི་ཊི་ཨེལ་ཝི་ཚུ་ དང་ལས་ གཞི་བསྒྱུར་འབད་དགོཔ་ཨིན།
`NoritoBytes` ཧོསིཊི་ཚུ་ རྟག་བརྟན་བཟོ་མི་ ཡིག་ཤུབས་ཁ་ཕྱེ་ཡོདཔ། Kotodama ད་ལྟ་རོགས་སྐྱོར་འདི་དག་ལ་གློག་ཐག་གཏོང་བ།
ཐད་ཀར་དུ་ བསྡུ་སྒྲིག་འབད་མི་བརྒྱུད་དེ་ བཟོ་བསྐྲུན་པ་ཚུ་གིས་ དཔག་བྱེད་སྔོན་སྒྲིག་དང་ སབ་ཁྲ་ཚུ་ལག་ལེན་འཐབ་ཚུགས།
ལག་དེབ་མེད་པའི་ FFI འབྱར་རྫས་མེད་པའི་བལྟ་སྟངས།

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

དམའ་བ།

- ས་ཚིགས་སྔོན་སྒྲིག་ཚུ་གིས་ Kotodama འདི་ཡིག་དཔར་རྐྱབས་ཞིནམ་ལས་ ཡིག་དཔར་རྐྱབས་ཡོད་པའི་ཊི་ཨེལ་ཝི་དཔར་བསྐྲུན་འབད་བའི་ཤུལ་ལས་ བཏོན་གཏང་།
  ཧོསིཊི་གིས་ གསོག་འཇོག་གི་དོན་ལུ་ ཀེ་ནོ་ནིག་ `NoritoBytes` པེ་ལོཌི་ཐོབ་ཨིན།
- ལྷག་མི་ཚུ་གིས་ `POINTER_FROM_NORITO` དང་གཅིག་ཁར་ ལོག་སྤྱོད་འབད་ཞིནམ་ལས་ བཀྲམ་སྤེལ་འབད་ཡོདཔ་ཨིན།
  Kotodama ནང་ རེ་བ་སྐྱེད་པའི་ དཔག་བྱེད་ id .
- འགྲུལ་ལམ་གཉིས་ཆ་རང་ རང་བཞིན་གྱིས་ ཊི་ཨེལ་ཝི་ཚུ་ ཨའི་ཨེན་པི་ཡུ་ཊི་ ལུང་ཕྱོགས་ནང་ དཔར་བསྐྲུན་འབད་དེ་ འབད་བཅུགཔ་ཨིན།
  ཡིག་རྒྱུན་ཡིག་འབྲུ་ཚུ་དང་ རན་ཊའིམ་དཔག་ཚད་ཚུ་ དྭངས་གསལ་སྦེ་ སླ་བསྲེ་རྐྱབ་ནི་ལུ་ གན་རྒྱ་བཟོཝ་ཨིན།

རན་ཊའིམ་རི་གེ་རེ་ཤཱན་གྱི་དོན་ལུ་ `ADDI` ལུ་བལྟ།
སྦྱོང་བརྡར་ཚུ་ `MockWorldStateView` ལུ་འགོག་པའི་སྐོར་རིམ་ཅིག་ཨིན།

## གཏན་འབེབས་སབ་ཁྲ་བསྐྱར་ཟློས་ (བཟོ་བཀོད།)

རེ་རེ་བཞིན་གྱི་ གཏན་འབེབས་སབ་ཁྲ་ལུ་ མཐའ་མཚམས་དགོཔ་ཨིན། སྣ་མང་ཐོ་བཀོད་ལུ་ མངའ་སྡེའི་སབ་ཁྲ་དགོཔ་ཨིན། བསྡུ་སྒྲིག་འབད་མི་འདི་གིས་ `.take(n)` དང་ ཡང་ན་ གསལ་བསྒྲགས་འབད་ཡོད་པའི་རིང་ཚད་མཐོ་ཤོས་ཅིག་ ངོས་ལེན་འབདཝ་ཨིན།

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

ཡིག་བརྡ།
- བསྐྱར་ལོག་ཆ་ཚན་འདི་ བསྐྱར་ལོག་ཐོ་བཀོད་ནང་ པར་ཆས་ཨིན། གོ་རིམ་འདི་ ལྡེ་མིག་གི་ Norito བཱའིཊི་གིས་ ཚིག་མཛོད་ཨིན།
- `M` ལུ་ བཀོད་རིས་འགྱུར་བཅོས་འདི་ `E_ITER_MUTATION` དང་ཅིག་ཁར་ ལུབ་ཀྱི་ལྕགས་ཐག་ནང་།
- མ་བསྡམས་མི་གིས་ བསྡུ་སྒྲིག་འབད་མི་འདི་གིས་ `E_UNBOUNDED_ITERATION` བཏོནམ་ཨིན།

## བསྡུ་སྒྲིག་འབད་མི་/ཧོསིཊི་ནང་ཁུངས།

འོག་ལུ་ཡོད་པའི་ བརྡ་རྟགས་ཚུ་ ལག་ཆས་གཙོ་བོ་གི་ རཱསི་ཕྱོགས་ལུ་ སྡོདཔ་ཨིན། དེ་ཚུ་གིས་ བསྡུ་སྒྲིག་འབད་མི་གྲོགས་རམ་པ་དང་ ཝི་ཨེམ་གྱི་འཕྲུལ་རིག་ཚུ་མར་ཕབ་འབད་མི་ པར་རིས་ཚུ་ པར་རིས་བཀོད་ཡོདཔ་དང་ **not** ནུས་ཅན་ Kotodama `.ko` འབྱུང་ཁུངས་ཨིན།

## རྒྱ་ཚད་ཀྱི་ཨོ་པི་ཀོཌ་ ཆུང་བའི་གཞི་ཁྲམ་དུས་མཐུན།

Kotodama གི་རྒྱ་ཆེ་བའི་ཨོ་པི་ཀོཌི་གྲོགས་རམ་པ་ཚུ་ IVM གིས་ལག་ལེན་འཐབ་མི་ 8-bit operand བཀོད་སྒྲིག་འདི་དམིགས་གཏད་བསྐྱེདཔ་ཨིན།
རྒྱ་ཆེ་བའི་ཨེན་ཀོ་ཌིང་། ༡༢༨-བིཊི་གནས་གོང་ཚུ་སྤོ་བཤུད་འབད་མི་ མངོན་གསལ་དང་གསོག་འཇོག་ཚུ་ བཀོལ་སྤྱོད་གསུམ་པ་དེ་ ལོག་ལག་ལེན་འཐབ།
ཐོ་བཀོད་མཐོ་བའི་དོན་ལུ་ བཤུད་བརྙན་འབདཝ་ལས་ གཞི་རྟེན་ཐོ་བཀོད་འདི་གིས་ ཧེ་མ་ལས་རང་ མཐའ་མཇུག་འདི་ བཟུང་དགོ།
ཁ༌འབྱང། མངོན་གསལ་/ཚོང་ཁང་འདི་མ་བྱིན་པའི་ཧེ་མ་ `ADDI` དང་གཅིག་ཁར་གཞི་རྟེན་འདི་བདེ་སྒྲིག་འབད།

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```ཆ་སྒྲིག་འབད་ཡོད་པའི་གཞི་ཁྲམ་དུས་མཐུན་ཚུ་གིས་ གཞི་རྟེན་འདི་ བཱའིཊི་༡༦ གི་རིམ་པ་ནང་ གོང་འཕེལ་བཏང་སྟེ་ ཐོ་བཀོད་འབད་ནི་ལུ་ ངེས་གཏན་བཟོ་ཡི།
ཆ་གཅིག་ `STORE128` གིས་ དགོས་མཁོའི་ ཕྲང་སྒྲིག་མཚམས་ཐིག་གུ་ ས་གཞི་ཚུ་ བཟོ་ཡོདཔ་ཨིན། དེ་བཟུམ་སྦེ་
དཔེ་རིས་འདི་ `LOAD128` ལུ་འཇུག་སྤྱོད་འབདཝ་ཨིན། རེ་བ་ཡོད་པའི་ stride དང་མཉམ་དུ་ `ADDI` འདི་ ཧེ་མ་ལས་ ཧེ་མ་ལས་ བཏོནམ་ཨིན།
མངོན་གསལ་རེ་རེ་གིས་ འགྲོ་ཡུལ་མཐོ་བའི་ཐོ་བཀོད་འདི་ ཨོ་པི་རན་ཌི་གསུམ་པ་ལུ་ མཐུད་དེ་བཞགཔ་ཨིན།
ཝི་ཨེམ་དང་མཐུན་སྒྲིག་འབད་དེ་ `VMError::MisalignedAccess` དང་ཅིག་ཁར་ མགུ་ཐོམ་སི་སི་གི་ཁ་བྱང་ཚུ།
སྤྱོད་ལམ་འདི་ `crates/ivm/tests/wide_memory128.rs` ནང་ལུ་ལག་ལེན་འཐབ་ཡོདཔ་ཨིན།

འ་ནི་ ༡༢༨ བིཊ་གི་གྲོགས་རམ་པ་ཚུ་ བཏོན་ཚུགས་པའི་ ལས་རིམ་ཚུ་ ཝེག་ཊར་གྱི་ ལྕོགས་གྲུབ་ཁྱབ་བསྒྲགས་འབད་དགོ།
Kotodama བསྡུ་སྒྲིག་འབད་མི་འདི་གིས་ `VECTOR` ཐབས་ལམ་འདི་ རང་བཞིན་གྱིས་ ལྕོགས་ཅན་བཟོཝ་ཨིན།
`LOAD128`/`STORE128` དང་། the VM དང་མཉམ་དུ།
`VMError::VectorExtensionDisabled` ལས་རིམ་ཅིག་གིས་ དེ་ཚུ་ལག་ལེན་འཐབ་ནི་གི་དཔའ་བཅམ་པ་ཅིན།
བིཊི་ཆ་ཚན་མེད་པར་།

## རྒྱ་ཚད་གནས་སྟངས་ཀྱི་ཡན་ལག་མཐོ་བ།

Kotodama གིས་ `if`/`else` ཡང་ན་ བཱའིཊི་ཀོཌི་རྒྱ་ཆེཝ་སྦེ་ བཏོནམ་ཨིན།
defy `BNE cond, zero, +2` རིམ་པ་དེ་གི་ཤུལ་ལས་ `JAL` གི་བཀོད་རྒྱ་ཆ་གཅིག་ཡོདཔ་ཨིན།

1. ཐུང་ངུ་ `BNE` གིས་ ༨-བིཊི་འཕྲལ་འཕྲལ་གྱི་ལམ་ཁར་ ཆ་རྐྱེན་ཅན་གྱི་ཡན་ལག་འདི་བཞགཔ་ཨིན།
   མཆོང་ལྡིང་ཐོག་ལས་ `JAL`.
2. `JAL` གིས་ Kotodama བཀག་ཆ་ལུ་དམིགས་གཏད་བསྐྱེདཔ་ཨིན།
   རྫུས་མ)།
3. `JAL` འདི་ `then` སྡེབ་ཚན་ལུ་མཆོང་ (གནས་སྟངས་དེ་ཡོད་པའི་སྐབས་ལེན་ཡོད།
   ངོ་མ)།

དཔེ་རིས་འདི་གིས་ གནས་སྟངས་ཞིབ་དཔྱད་འདི་ ནམ་ཡང་ ཨོཕ་སེཊི་སྦོམ་ཚུ་ ཨིན་ཀོཌི་དགོཔ་མི་དགོ།
bay ±127 མིང་ཚིག་ཚུ་ ད་ལྟོ་ཡང་ `then` གི་དོན་ལུ་ གཟུགས་སྦོམ་ལུ་རྒྱབ་སྐྱོར་འབད་བའི་སྐབས་ཨིན།
དང་ `else` རྒྱ་ཆེ་བའི་`JAL` རོགས་རམ་བརྒྱུད་དེ་བཀག་ཡོད། མཐོང
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` 2022
གོ་རིམ་ནང་བསྡམ་བཞག་མི་ འགྱུར་ལྡོག་བརྟག་དཔྱད།

###དཔེར་བརྗོད།

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

གཤམ་གསལ་གྱི་རྒྱ་ཆེ་བའི་བཀོད་རྒྱ་ཀེང་རུས་ལུ་བསྡུ་སྒྲིག་འབད། (ཐོ་བཀོད་ཨང་གྲངས་དང་ཨང་ཡིག་བཀོད་ནི།
ཆ་ཚང་ཨོཕ་སེཊི་ཚུ་ ཨེན་ཀ་སི་ལས་འགན་ལུ་རག་ལསཔ་ཨིན།

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

ཤུལ་མམ་གྱི་བཀོད་རྒྱ་ཚུ་གིས་ རྟག་བརྟན་ཚུ་ དངོས་གྲུབ་འབད་དེ་ ལོག་གནས་གོང་འདི་བྲིས།
ག་ཅི་འབད་ཟེར་བ་ཅིན་ `BNE` འདི་ `JAL` དང་པ་གུ་ལས་ མཆོངས་འགྱོཝ་ལས་ གནས་སྟངས་ཀྱི་ཨོཕ་སེཊི་འདི་ ཨ་རྟག་ར་ཨིན།
`+2` ཚིག་ཚུ་ བཀག་ཆ་གཟུགས་ཚུ་རྒྱ་སྐྱེད་འགྱོ་བའི་སྐབས་ལུ་ཡང་ ཡན་ལག་འདི་ ཁྱབ་ཚད་ནང་འཁོད་ལུ་བཞག་དགོ།