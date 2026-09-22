# Direct State publication custody

The direct State/World candidate on `optimizations` now retains each original
executing field through borrowed preparation, publication, refusal and unwind.
The enclosing owner releases all physical writers before payload destruction or
wake delivery. This record does not qualify the retained production publisher.

`BlockField` consumes the original sealed MV block into its publication slot.
`TransactionsBlockField` retains the original membership action and displaced
values. `BlockHashField` installs the actual acquired writer and native commit
slot before fallible preparation. World and TriggerSet use their existing field
inventories for preparation, publication and release. A terminal owner cannot be
reused as an executing block or preparation authority.

Direct State commit retains its State fields and the checked native application
proof through the visibility cut. Native publication precedes derived work that
reads the same native maps. State-view notifications, budget refunds and the
source-bound commit/write/lifecycle fence notifications remain deferred until
physical retirement of the original State. Both component-to-fence and
fence-to-component callback regressions exercise that ordering.

World owns one boxed field aggregate, retained across every phase. This keeps
large field values out of enclosing State/runtime stack moves. The exact test
artifact's runtime acquisition frame is 17,280 bytes plus 32 bytes of callee-save
space, down from the earlier 333,680-byte frame. The World materializer still has
its separate 240,048-byte frame. No test stack override is used. Both ordinary and
replacement World publication/abort/unwind run in explicit 2 MiB threads.

## Scoped evidence

`dist/sumeragi-main-work/generation156-core/state-prior-consumer-verification.json`
joins the completed `state-build3`, `state-runtime3`, `prior-runtime3` and
`consumer-check` receipts:

- 633 distinct Core runtime tests pass on one captured test binary, including
  every one of the preceding 513 controls and all 13 previous stack-overflow
  cases. Each selected case has its own successful one-test result.
- Core, Torii, daemon, CLI, client, test-network and integration-test targets pass
  `cargo check --tests`.
- All 7,523 captured Rust/build inputs match exactly across build, runtime and
  consumer checks; branch, HEAD and index metadata also remain unchanged.
- Test binary SHA-256:
  `b2e6bccef94313c3df80b845481ad7143a80b38217171762dcb2e49214fc0a74`.
- Formatting, the no-legacy-codec guard and diff whitespace checks pass for that
  candidate. Earlier failed builds and runtime runs remain recorded separately.

Formal source bindings and negative controls are being migrated with their
original test identities preserved. Their ongoing runs are not a passing formal
qualification. A selector escaping failure is recorded separately from executed
controls. The later retained-hash change requires fresh evidence.

## Open boundaries

The retained hash preflight/abort path still needs one caller-owned preparation
owner to retain actual reader notifications and enclosing State/Queue/Kura
fences through callee unwind. Complete retained component acquisition and
resource admission, including the World aggregate allocation and membership map
storage, remain required before production activation. The unchanged production
Validate-to-Apply cutover and real four-/seven-validator fault, restart and final
transaction qualification remain open. All L1–L6 remain open.
