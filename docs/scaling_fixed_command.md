# Fixed scaling command implementation contract

`scripts/nexus/run_multilane_scaling_gate.py` has one execution interface:
archived Python with `-I -B -S`, `--launch-input-fd`,
`--launch-input-sha256` and `--seed-fd`. The protected parent uses
`scaling_cli_bootstrap.fixed_scaling_argv` and passes exactly those two original
descriptors through its existing bounded process owner. Repeated, abbreviated,
unknown and retired command options are errors. Help needs no runtime input.

The launch input is an owner-only, single-link, read-only regular file, at most
64 KiB, with schema `iroha.sumeragi_v2.multilane_scaling.launch.v1`. Its exact
fields name the admitted release runtime, private Python dependency bundle,
plan and budget paths/digests, fresh evidence/runtime outputs, worker pack and
three declared lab/build labels. `scaling_experiment_cli_inputs` owns this
schema. Duplicate keys, unexpected values and aliased inputs fail before work.
Whitespace and key order have no semantic effect.

The seed descriptor is a ready, read-only nonblocking anonymous pipe containing
exactly 64 lowercase hexadecimal bytes followed by EOF. The command consumes
the seed after runtime admission. It does not place the seed in argv, environment,
public evidence or diagnostic output. The fixed native generator's descriptor
integration must preserve that property for descendant commands too.

Before importing the collector's dependency-bearing modules, bootstrap verifies
the exact private Python source closure and the admitted BLAKE3 package.
`InvocationResources` then owns the original plan/budget files, output parents,
four executable images, worker pack and `RuntimeAdmission`. The admission reuses
the source manifest, prebuilt bundle and archived framework-runtime verifiers.
Declared machine/storage labels do not substitute for directly observed host
facts or the release coordinator's build identity.

The execution service runs the ten original trials, publishes their manifest,
replays their evidence and publishes the canonical observed-measurement report.
Exit 0 means all three measurement criteria passed; exit 1 means a completed
experiment failed a criterion. Invalid execution returns 2 and interruption
returns 130. Printed output is diagnostic; a release receipt must bind the
original parent's actual process observation and exact archived artifacts.

On failed or interrupted cleanup, the command retains the original experiment,
runtime and dependencies while polling the original child owners. It does not
renew their deadlines, signal processes or close borrowed inputs early. The
dependency owner closes last, after original child cleanup finishes.

The protected bootstrap now owns the original preflight, launch handoff and
independent archive/native replay. Its canonical `scaling-execution.json` record
is consumed by the release receipt. See the [scaling gate contract](../specs/sumeragi_v2_multilane_scaling_gate.md).

TODO: migrate remaining receipt/approval fixtures and declarations to this
interface, retire unused tooling after its assertions move, and qualify the
complete path with actual pinned native processes. Component tests with explicit
process/runtime seams do not establish G-SCALE performance.
