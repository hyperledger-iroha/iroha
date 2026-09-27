# F12 Python public conviction update boundary

The maintained Python SDK now exposes one native Norito construction path for
`UpdatePlainConviction`. Its exact four arguments are `referendum_id`, `owner`,
`amount`, and `duration_blocks`; neither the native PyO3 classmethod, the
Python facade, nor `TransactionDraft.update_plain_conviction` accepts a choice
or direction argument. The native method constructs the current Rust DataModel
type and its registered `iroha.instruction.v1::governance::UpdatePlainConviction`
wire, with canonical selector, domainless I105 account, and canonical Quantity
checks. The draft helper requires owner to equal transaction authority and
checks the full unsigned-u64 duration bound before appending. No alternate
instruction decoder or cast-as-update path was added.

The native Rust unit test compares the generated `InstructionBox` frame against
the DataModel type's frame and rejects malformed selectors, account aliases,
and quantity text. The Python test covers native roundtrip, transaction
append, exact argument inventory, choice-field rejection, authority mismatch,
and value boundaries. These are local construction tests; they do not establish
the prior on-chain ballot, conviction monotonicity, confidentiality, or final
SDK/release parity. Core remains the authority for those semantics. A loadable
same-source `iroha-native` extension and focused runtime test are required
before marking native runtime parity complete.

Validation on this checkout: the exact native Rust selector
`update_plain_conviction_instruction_has_exact_choice_free_native_wire` passes
1/1. Python syntax, scoped Ruff, Rust formatting, and `git diff --check` pass.
The repo-local Python 3.12 environment used the hash-locked CI dependencies.
The same-source `iroha-native` ABI3 wheel built successfully with SHA-256
`646498e7ca1259b543c94cfd1f305a5edd284c91b807671f8228d9531b60a983`;
the pure Python SDK wheel has SHA-256
`c201c820e0eb8f7eb933ce6a78f3855ba7c053360e5fee5f6803ed19d8b1dbc3`.
Installed-package pytest collected no tests because macOS dyld rejected the
native wheel's extension with `mis-aligned LINKEDIT string pool` at file offset
`0x06CC68EC`. Stripping an ignored test copy did not repair the Mach-O layout.
The wheel is therefore not a qualifying installed native artifact, and the
Python runtime assertions remain unverified against this source. The test
copy was removed; the wheels and virtual environment remain under ignored
`target/` for diagnosis.
