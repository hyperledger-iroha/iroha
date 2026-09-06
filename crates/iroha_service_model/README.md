# Iroha service model

This crate owns state-independent service records and policy semantics below
the aggregate ledger model and runtime implementations. It currently contains
canonical SoraNet transport, anonymity, rollout, and write-mode policies, plus
shared SoraFS alias-cache defaults. Node configuration, SDKs, CAR tools, and
the orchestrator consume those definitions directly.

The architecture guard rejects dependencies on the aggregate model, node
configuration, HTTP DTO composition, the SDK, and node or storage runtimes.
The remaining capability record extraction is tracked in
[the first-release redesign](../../specs/first_release_architecture_redesign.md).

Run `cargo test -p iroha_service_model` for the policy contracts.
