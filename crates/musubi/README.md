# Musubi

Musubi owns Kotodama package resolution, compilation, publication and contract deployment
adapters. The canonical compiler is `kotodama_lang`; signed deployment, fees and finality
belong to `iroha_contract_deploy`.

`deployment_runtime::DeploymentRuntime` accepts an already resolved SDK configuration,
private journal root and alias scope. It compiles `.ko`, verifies `.to`, or builds an explicit
Musubi package without discovering a client configuration or wallet. Source and bytecode
inputs require no project manifest. Cold registry dependencies additionally require an
explicit prepared archive transport.

The native SDK's `get_provider_discovery` verifies current provider authority and its exact
signed advert against an independently selected finality decision and qualified native World
schema. The bounded projection includes the complete current World hash preimages and exact
admission heads/owner, so a historical admission cannot conceal a later revocation.

`with_build_registry_resolver` supplies an explicitly selected registry only after local-only
resolution fails. A private child uses its retained parent registry identity and parent account;
absence never selects the child's registry. An unchanged locked graph whose sources authenticate
in the immutable cache requires no registry or provider HTTP once that explicit binding is resolved.

Account-mode archive fetching uses the existing storage transport and stream-token issuer.
The provider must explicitly admit the `registered_account_read` capability: an exact HTTPS
DNS origin on port 443 and finite token limits for registered-account reads of its immutable objects.
The same finalized projection authenticates the current StreamToken custody record and original
signed enrollment, including expiry and revocation. Every token mint and replacement reobserves
the same authority; a retired key cannot refresh an earlier session. Provider requests carry fresh canonical account
signatures; parent listener tokens and unrelated HTTP credentials are never forwarded. Providers
without this policy remain unavailable to account-mode downloads.

TODO: Qualify the composed cold-cache workflow against an admitted provider, including revocation
during issuance, expiry, cancellation, and adversarial DNS rebinding. Component tests do not establish
whole-service readiness. Existing archive commitments, strict token signatures, bounded CAR
verification, pinned public DNS and immutable cache checks remain authoritative.

`ContractInput::from_path` checks input names and existing regular files before a frontend
provisions a network. The build then validates the source graph, bytecode or package contents.

Deployment retains exact bytes and signed transactions before dispatch. A repeated identical
request resumes its pending journal; a completed request returns its receipt only after
authenticated current alias and artifact readback. Changed input cannot replace unresolved
work. Slot and journal locks serialize concurrent writers; recovery binds the journal location
to its authenticated target and exact commit. Both fresh deployment and recovery require the
caller's scope and fee review before execution.

Focused validation: `cargo test -p musubi --lib deployment_runtime`.

For a complete authenticated `.to` artifact, `musubi --format json artifact --config
<explicit-client.toml> --journal <fresh-owner-private-directory> prepare --artifact
<artifact.to> --alias <exact-alias> --fee-payment <native-intent.json>` prepares and
persists the native signed lifecycle without dispatch. Its output includes native-decoded
`intended_transactions` for an independent execution-output owner to retain before
submission. `inspect` authenticates those exact existing intents without signing or
dispatch; `resume` executes or recovers only that retained journal. Recovery cannot
take replacement artifact, alias or fee inputs. Entrypoint grants and activation calls
are independently selected operations after artifact deployment.
