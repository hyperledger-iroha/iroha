# Kotodama V1 examples

These examples use the branded V1 declaration vocabulary. The normative grammar is
[`kotodama_grammar.md`](./kotodama_grammar.md); the canonical syscall and
pointer-ABI tables are in [`ivm_syscalls.md`](./ivm_syscalls.md).

Every deployable `.ko` file contains exactly one named `seiyaku`/`誓約` unit. Reusable
source contains one `module` and is linked at typed HIR before the deployable
artifact is emitted.

## Check and build

Use the unified Rust compiler driver:

```sh
koto check examples/hello/hello.ko
koto build examples/hello/hello.ko \
  --max-cycles 1000000 \
  --out target/examples/hello.to \
  --manifest-out target/examples/hello.manifest.json
```

ABI v1 is unconditional. Source cannot select an ABI, vector width, or
execution feature. `--max-cycles` selects a positive ceiling, up to node
admission policy, that is embedded in the execution header and covered by the
artifact hash.

ZK contracts are checked, built, and documented with `--zk`. The flag selects a
compiler capability; the source still cannot override execution-header metadata.

## State, errors, entries, and views

`hajimari`/`始まり` establishes scalar state. Mutating public calls use
`kotoage fn`/`言挙げ fn` and an explicit caller permission. Read-only calls use
`view fn`; views are public unless they also declare `authorize`.

```kotodama
seiyaku Counter {
    error enum CounterError {
        NonPositiveDelta = 1,
    }

    state value: i64;

    hajimari() {
        value = 0;
    }

    kotoage fn increment(delta: i64) -> i64 authorize("CanIncrementCounter") {
        require(delta > 0, CounterError::NonPositiveDelta);
        let next = value + delta;
        value = next;
        return next;
    }

    view fn current() -> i64 {
        return value;
    }
}
```

Error variants have explicit, stable, non-zero `u32` codes. Public failures use
`require(condition, Error::Variant)`; free-form strings are not a contract error
protocol. Arithmetic is checked, so `value + delta` deterministically fails and
reverts if it overflows. Use an explicit `math::wrapping_*` operation only when
modular arithmetic is the intended protocol.

## Namespaced ledger operations

Host capabilities are namespaced. Typed constructors produce validated
pointer-ABI values; source cannot allocate host memory, construct raw pointers,
select direct syscalls, or submit opaque instruction bytes.

```kotodama
seiyaku TransferDemo {
    kotoage fn transfer() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
            AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"),
            AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            Amount::from_i64(10),
            DataSpaceId::parse("0")
        );
    }
}
```

The `authorize` permission is caller authorization. It does not replace the
host's operation-specific authorization, and it is separate from the
compiler-derived effect and scheduler-access summaries.

## Triggers

A trigger names its callback in the declaration header. Its event filter and
repeat policy are declarative manifest data; the callback remains an ordinary
typed entrypoint.

```kotodama
seiyaku ScheduledSettlement {
    kotoage fn settle() authorize("CanSettle") {
        debug::info("settlement tick");
    }

    trigger hourly -> settle {
        on time schedule(0, 3600000);
        repeats indefinitely;
        metadata {
            purpose: "settlement";
        }
    }
}
```

Trigger filters also cover explicit trigger execution, supported ledger data
events, and approved transaction or block pipeline events. Trigger authority
and lifecycle policy are enforced by the runtime.

## Durable maps

`StateMap<K, V>` is host-backed durable state. `get` returns `Option<V>` so an
absent key cannot be confused with a zero value. Writes and removals operate on
one key, preserving scheduler precision. Iteration follows canonical Norito key
order and must be compiler-proven bounded; V1 admits at most 64 items.

In-memory `Map`, implicit defaults, unbounded iteration, recursive calls, and
`while` are not V1 language features.

## Modules

A reusable source has this shape:

```kotodama
module AmountRules {
    fn is_positive(amount: i64) -> bool {
        return amount > 0;
    }
}
```

Modules are resolved and type-checked once in the content-addressed build graph,
then linked at HIR. Kotodama has no wildcard imports or textual AST rewriting.

## Boundary arguments and artifacts

Torii and CLI users may supply JSON keyed by parameter name. The boundary
converts that object into one canonical Norito argument record. The public
wrapper decodes the record once and reads typed ABI words; it does not reparse
the payload for every parameter.

`code_hash` is a domain-separated hash of the complete deployable `.to`,
including every execution-header field, CNTR, literals, and code. Source maps
and debug information are hash-keyed sidecars, not unhashed deployable
sections. Nodes independently validate control flow and derive transitive
effects/access instead of trusting compiler claims in CNTR.

For larger checked-in examples, see `examples/`,
`crates/ivm/docs/examples/`, and `crates/kotodama_lang/src/samples/`. The
machine-readable [`kotodama_v1_docs.json`](./kotodama_v1_docs.json) policy
identifies the normative grammar and documentation roots. CI discovers every
tracked `kotodama`/`ko` fence and documented `*.ko` heredoc below those roots.
