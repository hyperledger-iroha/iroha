# Kotodama Examples Overview

This page shows concise Kotodama examples and how they map to IVM syscalls and pointer‑ABI arguments. See also:
- `examples/` for runnable sources
- `docs/source/ivm_syscalls.md` for the canonical syscall ABI
- `kotodama_grammar.md` for the full language specification

## Hello + Account Detail

Source: `examples/hello/hello.ko`

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

Mapping (pointer‑ABI):
- `authority()` → `SCALL 0xA4` (host writes `&AccountId` into `r10`)
- `set_account_detail(a, k, v)` → move `r10=&AccountId`, `r11=&Name`, `r12=&Json`, then `SCALL 0x1A`

## Asset Transfer

Source: `examples/transfer/transfer.ko`

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

Mapping (pointer‑ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, then `SCALL 0x24`

## NFT Create + Transfer

Source: `examples/nft/nft.ko`

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

Mapping (pointer‑ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Pointer Norito Helpers

Pointer-valued durable state requires converting typed TLVs to and from the
`NoritoBytes` envelope that hosts persist. Kotodama now wires these helpers
directly through the compiler so builders can use pointer defaults and map
lookups without manual FFI glue:

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

Lowering:

- Pointer defaults emit `POINTER_TO_NORITO` after publishing the typed TLV, so
  the host receives a canonical `NoritoBytes` payload for storage.
- Reads perform the reverse operation with `POINTER_FROM_NORITO`, supplying the
  expected pointer type id in `r11`.
- Both paths automatically publish literal TLVs into the INPUT region, allowing
  contracts to mix string literals and runtime pointers transparently.

See `crates/ivm/tests/kotodama_pointer_args.rs` for a runtime regression that
exercises the round-trip against the `MockWorldStateView`.

## Deterministic Map Iteration (design)

Deterministic map for‑each requires a bound. Multi-entry iteration requires a state map; the compiler accepts `.take(n)` or a declared maximum length.

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
- Iteration set is a snapshot at loop entry; order is lexicographic by Norito bytes of the key.
- Structural mutations to `M` in the loop trap with `E_ITER_MUTATION`.
- Without a bound the compiler emits `E_UNBOUNDED_ITERATION`.

## Compiler/host internals (Rust, not Kotodama source)

The snippets below live on the Rust side of the toolchain. They illustrate compiler helpers and VM lowering mechanics and are **not** valid Kotodama `.ko` source.

## Wide Opcode Chunked Frame Updates

Kotodama’s wide opcode helpers target the 8-bit operand layout used by the IVM
wide encoding. Loads and stores that move 128-bit values reuse the third operand
slot for the high register, so the base register must already hold the final
address. Adjust the base with an `ADDI` before issuing the load/store:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```

Chunked frame updates advance the base in 16-byte steps, ensuring the register
pair committed by `STORE128` lands on the required alignment boundary. The same
pattern applies to `LOAD128`; issuing an `ADDI` with the desired stride before
each load keeps the high destination register bound to the third operand slot.
Misaligned addresses trap with `VMError::MisalignedAccess`, matching the VM
behaviour exercised in `crates/ivm/tests/wide_memory128.rs`.

Programs that emit these 128-bit helpers must advertise vector capability.
The Kotodama compiler enables the `VECTOR` mode bit automatically whenever
`LOAD128`/`STORE128` appear; the VM traps with
`VMError::VectorExtensionDisabled` if a program attempts to execute them
without that bit set.

## Wide Conditional Branch Lowering

When Kotodama lowers an `if`/`else` or ternary branch to wide bytecode it emits a
fixed `BNE cond, zero, +2` sequence followed by a pair of `JAL` instructions:

1. The short `BNE` keeps the conditional branch within the 8-bit immediate lane
   by jumping over the fallthrough `JAL`.
2. The first `JAL` targets the `else` block (executed when the condition is
   false).
3. The second `JAL` jumps to the `then` block (taken when the condition is
   true).

This pattern guarantees the condition check never needs to encode offsets larger
than ±127 words while still supporting arbitrarily large bodies for the `then`
and `else` blocks via the wide `JAL` helper. See
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` for
the regression test that locks in the sequence.

### Example Lowering

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Compiles to the following wide instruction skeleton (register numbers and
absolute offsets depend on the enclosing function):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Subsequent instructions materialise the constants and write the return value.
Because the `BNE` jumps over the first `JAL`, the conditional offset is always
`+2` words, keeping the branch within range even when the block bodies expand.
