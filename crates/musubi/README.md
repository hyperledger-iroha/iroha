# Musubi

Musubi owns Kotodama package resolution, compilation, publication and contract deployment
adapters. The canonical compiler is `kotodama_lang`; signed deployment, fees and finality
belong to `iroha_contract_deploy`.

`deployment_runtime::DeploymentRuntime` accepts an already resolved SDK configuration,
private journal root and alias scope. It compiles `.ko`, verifies `.to`, or builds an explicit
Musubi package without discovering a client configuration or wallet. Source and bytecode
inputs require no project manifest. Cold registry dependencies additionally require an
explicit prepared archive transport.

Deployment retains exact bytes and signed transactions before dispatch. A repeated identical
request resumes its pending journal; a completed request returns its receipt only after
authenticated current alias and artifact readback. Changed input cannot replace unresolved
work. Slot and journal locks serialize concurrent writers.

Focused validation: `cargo test -p musubi --lib deployment_runtime`.
