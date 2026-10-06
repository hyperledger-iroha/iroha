# Musubi

Musubi owns Kotodama package resolution, compilation, publication and contract deployment
adapters. The canonical compiler is `kotodama_lang`; signed deployment, fees and finality
belong to `iroha_contract_deploy`.

`deployment_runtime::DeploymentRuntime::new(config, journal_root, cache_root)` requires an
already resolved SDK configuration and explicit private journal/cache roots; deployment selects
its alias scope separately. It compiles `.ko`, verifies `.to`, or builds an explicit Musubi package
without discovering a client configuration, wallet or operating-system user cache. Source,
bytecode and local-only package builds do not open a cache. Managed Kagami and Mochi retain the
original generation's `PreparedLocalnet::build_cache_root()` path for both authenticated resolver
records and immutable archives, including private roots. Cold registry dependencies additionally
require the exact prepared archive transport.

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
DNS origin on its admitted nonzero port and finite token limits for registered-account reads of its immutable objects.
The same finalized projection authenticates the current StreamToken custody record and original
signed enrollment, including expiry and revocation. Every token mint and replacement reobserves
the same authority; a retired key cannot refresh an earlier session. Provider requests carry fresh canonical account
signatures; parent listener tokens and unrelated HTTP credentials are never forwarded. Providers
without this policy remain unavailable to account-mode downloads.

TODO: Qualify the composed cold-cache workflow against an admitted provider, including revocation
during issuance, expiry, cancellation, and adversarial DNS rebinding. Component tests do not establish
whole-service readiness. Existing archive commitments, strict token signatures, bounded CAR
verification, pinned public DNS and immutable cache checks remain authoritative.

`ContractInput::from_path` captures the original native regular `.ko`, `.to`, or explicitly
selected package manifest before a frontend provisions a network. Parent paths resolve at
selection; the selected leaf is never followed through a link. The original native file and its
ancestors stay retained through build, and changed/replaced roots are refused. Descriptor reads
use explicit offsets so repeat builds do not depend on a shared cursor. Declared companions and
package members/dependencies still use their existing fresh, bounded selection owners; this
capture does not claim custody of an entire project before it is read. Selecting a member
manifest or member directory deploys that member by default and refuses `--package` for
another member. Selecting the actual workspace-root manifest or directory keeps the declared
workspace defaults and explicit package selection, including roots which also declare a package.

Generated publication profiles initialize their empty client journal directory before the generation
is published. Begin, Resume and Recover require that original custody before namespace or registry
HTTP; losing either directory refuses the action without recreating history. The shared service
crate owns directory placement, while Musubi remains the sole operation-journal and sidecar owner.

Deployment retains exact bytes and signed transactions before dispatch. A repeated identical
request resumes its pending journal; a completed request returns its receipt only after
authenticated current alias and artifact readback. Changed input cannot replace unresolved
work. Slot and journal locks serialize concurrent writers; recovery binds the journal location
to its authenticated target and exact commit. Both fresh deployment and recovery require the
caller's scope and fee review before execution. Execution failures show their public cause and
exact recovery journal together, while preserving the native error type for callers.

Focused validation: `cargo test -p musubi --lib deployment_runtime`.

`generated_publication::publish_generated` accepts the source-bound generated publication context,
an explicit package manifest, private publication state/cache roots, and the retained generated
archive transport. Generated Begin uses the shared local selection preflight: a member input
binds that member, while a workspace-root input keeps defaults and explicit selection; exactly
one member is required. Kagami and Mochi preflight before managed startup, and the canonical
publication entry validates selection again before namespace or publication effects. Begin
performs the owner-paid namespace binding through the existing wallet parent before resolving
a fresh publication graph. Exact current binding permits a read-only skip
only after the complete original parent census; missing custody is never recreated. Resume and
sidecar recovery retain the original request and do not authorize a new namespace transaction.
Generated TLS selection and fresh native provider discovery remain in the sole archive transport;
this entry never reconstructs that transport from TOML or selects an operating-system user cache.
The ordinary CLI and generated entry share one publication engine and redacting command output.

TODO: Execute the generated three-provider cold publication, detach/restart recovery and subsequent
cold package fetch against the composed native candidate. Source and component controls alone do
not establish publication or service readiness.

Mutable calls require finite aggregate fee authorization. Use the selected network's
configured maxima or supply `musubi call --entrypoint SELECTOR --max-fee-asset ASSET
--max-fee AMOUNT`. The shared native owner quotes, signs and retains the optional
exact self-grant and call before either is sent. Both expire within the original
sixty-second authorization; `--resume JOURNAL` reconciles those exact hashes without
rebuilding, quoting or signing replacements. `DeploymentRuntime::current_deployment`
provides the verified current artifact and receipt for native alias-selected view/call
adapters and never creates a missing slot or repairs its records.
