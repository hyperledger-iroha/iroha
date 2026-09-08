# Retry a rolled-back Taira deployment

`scripts/taira_retry.py` retries a completely rolled-back, initially vacant
four-validator deployment using its existing immutable build and transfer
receipts. It creates a fresh deployment identity and authorization nonce, retires
the completed attempt's custody, and runs native assemble, authorize, preflight,
and apply. It then verifies seed continuity and boot persistence. It does not
build or transfer unchanged binaries or source.

After an operator has prepared one owner-only runtime plan, each retry is:

```sh
python3 scripts/taira_retry.py \
  --plan /private/runtime/taira-retry/operator-plan.json \
  --output-root /private/runtime/taira-retry/local-evidence
```

The plan must be an owner-only regular JSON file. The existing output directory
must be owned by the caller with mode `0700`. The command creates a new evidence
directory for each invocation and reports the current phase and elapsed time
every 30 seconds. Native failures report an operation and errno when available;
arbitrary stderr stays in the private log path.

## Operator plan

The plan schema is `taira.same-artifact-retry.v1`. All values describe public
identities, paths and receipt pins. Never put signing keys, peer TOML contents,
passwords, tokens or request headers in the plan.

| Field | Value |
| --- | --- |
| `schema` | `taira.same-artifact-retry.v1` |
| `preparation` | `{ "path": "/absolute/local/result.json", "sha256": "actual raw receipt SHA-256" }` from a successful maintained `taira_release.py prepare` |
| `binary_transfer` | Same reference shape, pointing at the actual completed binary transfer JSON |
| `source_transfer` | Same reference shape, pointing at the actual completed signed source transfer JSON |
| `guest_ssh`, `backing_ssh` | `{ "argv": ["/usr/bin/ssh", "...", "/usr/bin/python3 -I -"], "pins": [{ "path": "/absolute/public/known_hosts", "sha256": "actual SHA-256" }] }` |
| `backing_path` | The approved backing host's absolute directory containing the VM sparse disk |
| `guest` | The runtime fields below, using absolute paths on the approved guest |

Each SSH invocation must use `-F /dev/null`, an explicit `-i` path, strict host-key
checking, batch mode, an explicit public `UserKnownHostsFile`, no global host-key
file, no agent forwarding, no host-key updates and no DNS host-key lookup. The
outer connection also requires `IdentityAgent=none`. A jump route may contain
one fixed `/usr/bin/ssh ... -W destination:22` proxy. The pin list must match
exactly the public host-key files used by those hops. Arbitrary shell proxies,
ambient SSH configuration, extra pins and duplicate options are rejected.

The `guest` object contains exactly these fields:

- Paths: `runtime_root`, `previous_inventory`, `source_manifest`,
  `trusted_public_key`, `signing_key`, `ssh_identity`.
- Public helper references `{ "path": "...", "sha256": "..." }`:
  `guard_support`, `unit_renderer`, `local_node`. These refer to the existing
  admitted owner-guard, service-unit renderer and native Inrou controller helpers.
- `expected_mac`: the actual approved guest's lowercase colon-delimited MAC.

Use the preceding native assembly's actual inventory and the imported source's
actual `verified-manifest.json` path. The command derives the retry directory,
binary manifest, native local arguments, original preparation directory and
native known-hosts path from these receipts. It locates the actual native terminal
by deployment identity. Runtime paths, topology, initial state, source identity,
artifact identity and endpoint guard pins remain exact. Native assemble derives
and validates every new inventory field; cloning JSON alone never admits an
attempt. No per-attempt directory names, nonces or capacity figures need editing.

Read-only admission measures current artifact lengths and the public stage tree.
It reads the small public container/service manifests and bounded bundle archive
metadata. It never reads peer config contents. The three tiny SoraFS manifest
hashes must match the preceding native inventory, which already admitted their
canonical SF1 profiles. Modified manifests cannot reuse the 64 KiB chunk bound.
The shared `derive_capacity` implementation then produces both plans from actual
build sizes and measured geometry: `3A + 2S + 4P + 4R`, plus 2 GiB guest headroom.
Each config is charged at the native 1 MiB materialization limit.

`R` includes each replica's guest hydration, writable root/data lease maxima,
ephemeral storage and bundle cache/block/extraction publication copies. The
physical backing plan charges the full additional guest allocation plus another
2 GiB reserve. See [`taira_disk_capacity.md`](taira_disk_capacity.md) for the
allocation definitions. Unknown stage layouts or additional service artifacts
fail closed instead of silently omitting their capacity.

The command checks the backing host before entering the mutation corridor and
checks the guest again before retirement, assembly, authorization and apply.
It neither reserves space nor credits anticipated cleanup. During native apply,
the 30-second heartbeat includes only the current native phase, step, touched
validator count and edge flag. It never prints journal nonces or failure text,
and progress reporting does not hash artifacts.

## Interrupted attempts

The remote `attempts_root/latest.json` tracks the attempt automatically. Repeating
the command after a failure before native apply resumes that same attempt and
nonce. Existing preapply evidence is retained before assemble and authorize run
again. Completed retirement is reattested under the same custody locks.

An exclusive, durable `apply-started.json` is published before native apply is
spawned. Once it exists, the command requires that exact attempt's real native
rolled-back terminal before allocating another attempt. A process exit, a helper
failure file or an absent current journal never proves rollback. The command
never automatically resubmits a possibly mutating apply.

If native apply completed but a later check failed, repeating the same command
resumes only seed verification, boot persistence and public validation. This path
requires the exact native `completed` and `deployment-proven` records, the same
inventory and authorization, and the original native preflight report bound to
the durable apply marker. Wrapper status or an apply exit code cannot grant it.
It holds the native coordinator lock, keeps the original nonce and prestart
record, and archives incomplete observations before repeating postconditions.
It performs no retirement, assembly, authorization or apply.

Because the deployment's files already exist, the completed route admits only
64 MiB for remaining evidence plus guest/backing reserves, instead of charging
another full rollout. A conflicting or missing native terminal cannot receive
that capacity exception. Public validation uses the exact released CLI doctor
and anonymous requests to verify the returned source revision, committed tip,
NetworkId and curated `iroha.health` result.

The retirement path admits all four completed validator rollbacks and a completed
edge rollback when the edge was touched. Original state roots must be empty,
selectors absent and services inactive. It retains native journal bytes, terminal
history, rollback receipts and private runtime inputs. Earlier partial rollback,
non-vacant topology or pending recovery requires the corresponding native
recovery workflow; the retry command refuses to manufacture completion.

Signing material is opened as a strict inherited read-only descriptor and passed
to native authorization. Python never reads the signing key or peer config
contents. The operation keeps the same source and artifacts; a changed build uses
the release preparation and transfer workflow instead.

A successful retry result proves native apply, seed continuity, boot persistence
and public doctor/source/network/MCP health. Its
`public_application_validation_completed` remains `false`: the application's
own end-to-end acceptance check is separate and must use the released revision
and actual NetworkId.

Run the focused offline tests without Cargo or SSH:

```sh
python3 -B -m unittest discover -s scripts/tests -p 'taira_retry_test.py'
```
