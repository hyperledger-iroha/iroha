# SoraFS reference SDK package index

The sole V1 inventory is `sorafs.reference_sdk.package_index.v1`, implemented in
`scripts/sorafs_sdk_artifact_index.py`. It owns actual input files; parsing an
inventory does not establish execution, signer authority or release readiness.
The adapters are `scripts/sorafs_sdk_java_artifact_verifier.py` and
`scripts/sorafs_sdk_python_artifact_verifier.py`.

## Exact structure

The canonical UTF-8 JSON has four fields, sorted keys, compact separators and a
final newline. Duplicate/unknown fields and retired consumer names reject.

| Field | Contract |
| --- | --- |
| `schema` | `sorafs.reference_sdk.package_index.v1` |
| `candidate` | Exact `source_commit` and `workspace_source_manifest_sha256`, independently supplied to the parser |
| `files` | Sorted canonical relative paths, each mapping to exact positive `size` and lowercase nonzero `sha256` |
| `consumers` | Six ordered rows, each containing `consumer`, `version`, `artifact`, `execution`, and sorted distinct `inputs` |

The six consumers, in order, are `javascript`, `python`, `kotlin_jvm`,
`java_source_kotlin`, `swift`, and `csharp`. Their primary formats are npm tarball,
SDK wheel, Kotlin mobile ZIP, Java qualification ZIP, Apple ZIP, and NuGet package.
The Java archive owns its execution observations. Each other consumer needs a
separate execution ZIP. Java and Kotlin share the Kotlin version; Java must name
the Kotlin distribution among its inputs. Distinct consumers cannot substitute
the same artifact bytes. Every referenced file exists in the inventory, and
every inventoried file has a consumer.

The parser bounds metadata to 2 MiB, 256 files, 1 GiB per file and 8 GiB total.
These are evidence limits; they do not increase proof or runtime budgets.
`OpenedIndexFiles` opens ordinary single-link files through retained no-follow
directory descriptors. It refuses physical aliases, links and special files;
nonblocking leaf opens prevent FIFO hangs. It checks exact bytes before reads
and rechecks original descriptors, path ownership and ancestor identities before
completion. Consumers must finish the context manager before publishing a result.

## Java input and execution relation

The producer `build_sorafs_java_consumer_artifact.py` accepts a fresh output
directory. Within the source checkout, output is permitted only under `target/`.
It continues to require the original clean-source native ABI-23 verification,
JDK 21 with Java 8 compilation, and independently pinned dependencies. Its archive
contains actual sources, fixtures, compiled consumers and observed logs/reports;
packages, dependencies, JDK inputs and the native library are separate indexed
original files, not digest-only substitutes or ZIP extraction destinations.

The Java adapter rederives the deterministic archive, exact retained inventory,
two execution lanes and all 25 non-skipped Java assertion groups per lane. It
opens the actual JAR/AAR, joins them byte-for-byte to the Kotlin distribution,
and parses compiled source ownership and the native manifest from captured bytes.
It reconstructs report/class/library streams from the original bounded stdout
and checks exact SDK, assertion, runner and Android probe class origins. Missing,
foreign, repeated or altered observations reject even if an attacker recomputes
the surrounding index and archive hashes. Source, fixture and producer copies
must match the independently selected source root.

The mandatory `dependency_classes` execution field separately binds each JVM
process's loaded dependency classes to the numbered private JAR copies. JDK 21
multi-release lookup selects the highest active class version through 21;
shadowed SDK, qualification or JDK classes reject. Both the JUnit engine and
launcher must be observed during execution. Generated dependency lambdas retain
their actual previously loaded bootstrap bytes. Annotation-only dependencies
need not load, and JDK origins use the checked JDK 21 namespace/origin forms.
Version selection follows the Java 21
[JAR specification](https://docs.oracle.com/en/java/javase/21/docs/specs/jar/jar.html#multi-release-jar-files).

The returned immutable observations do not authenticate an untrusted producer.
The native manifest's identity join does not rerun the native ABI/export probe,
and matching selected source/tool bytes does not independently verify the whole
clean Git/source manifest. Those remain separate native verification and signed
candidate responsibilities. JDK input identities are the producer's recorded
subset, not complete toolchain provenance. Host-JNI execution does not qualify
Android physical devices.

## Remaining integration

The existing `ci/verify_privacy_python_wheel.py` now provides the sole bounded
`parse_wheel_bytes` archive/RECORD parser for immutable captured inputs. Its
`WheelArchive` result carries no file or execution authority; the existing
`preflight_wheel` owner retains stable-file and expected-seal checks before
delegating to it. The [Python adapter](python_index_adapter_v1.md) consumes the
original indexed wheel bytes and joins executed/loaded-member observations to
their complete captured installed content. Structural parsing alone is not an
execution result.

The [fixed Python child contract](python_reference_child_v1.md) now owns
same-process installed/loaded-module rechecks and the exact 77 reference cases
with all 231 setup/call/teardown phases. Its complete copied snapshot rejects
unexpected package initializers, sibling sources and substituted directories.
`scripts/sorafs_python_consumer_artifact.py` parses immutable observations using
the sole canonical wheel seal parser, checks loaded-member/source joins and
authenticates the recorded digest against every actual log byte preceding the
final report frame. It does not authenticate the process that supplied those
observations. The [parent producer](python_consumer_producer_v1.md) now joins
original native/SDK and dependency bytes, complete CPython/stdlib custody, actual
process logs and final publication checks. The original-index Python adapter
consumes all twenty input roles with independently selected runtime/dependency
pins. Signed producer/aggregate joins remain required before its execution can
participate in the aggregate. Its pytest 9.0.3 requirement matches
`scripts/requirements.txt`; the separate pytest 8.4.2 CI lock is not that input's
authority. Existing reference assertions and wheel verification are unchanged.

TODO: implement and review the other four concrete executed-package adapters,
then connect this inventory to the existing signed aggregate, ReleaseManifest
custody/completion verification and SF11 consumer derivation in one cutover.
The current SF11 Boolean/digest assertions cannot close these requirements.
Do not add a generic successful-report substitute or a fallback for retired
layouts. Preserve all five CLI native targets, the 17 readiness summaries,
independent signer authority, and matching-candidate release qualification.
