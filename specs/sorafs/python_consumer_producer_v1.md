# SoraFS Python consumer producer V1

`scripts/build_sorafs_python_consumer_artifact.py` owns execution of the fixed
[77-case child](python_reference_child_v1.md). It consumes prebuilt native and SDK
wheels, the original native ABI-23 manifest, independently pinned
[runtime/dependency manifests](python_runtime_inputs_v1.md), and an independently
selected clean source commit and source-manifest digest. It creates a fresh private
directory beneath this checkout's `target/`. It accepts no caller command, pytest
options, online resolver, source-dirty override or asserted passing result.

The CPython 3.12 executable, actual shared Python runtime and complete configured
stdlib/extension tree retain original byte and physical identity checks. The
actual runtime probe joins interpreter paths to those inputs. Base site-packages
is excluded from execution. Stock toolchain file links are explicitly modeled;
the private Linux venv's optional `lib64 -> lib` directory link is the sole
modeled environment alias. Normal OS/native-library trust remains part of host
toolchain qualification, not a claim of protection against a malicious host.

The producer creates a `--without-pip --copies` environment, checks its empty
site directory and disabled system packages, and bootstraps the independently
pinned pip wheel. Installation uses only local hash-required wheel inputs with
`--no-index`, `--no-deps`, `--no-compile` and no cache. Startup hooks, cached
bytecode and the complete case-insensitive sitecustomize/usercustomize module
family reject before an installed interpreter runs. The sole native/SDK wheel
verifier continues to own their archive and installed-file contracts.

Native verification runs through the unchanged ABI-23 checker using that private
interpreter. Its actual extension must match the original native wheel and
manifest; the candidate must remain clean and exact. Captured fixed tools, test
source and complete fixture/Norito/Torii source trees feed the child. Original
primary input descriptors and ancestry survive execution; bounded source/runtime
trees use complete before/after scans rather than thousands of retained handles.

Every owned operation has a fixed deadline and independently bounded stdout and
stderr. Logs retain real process output and exit status. The parent requires a
successful child, empty child stderr, the exact final report frame and all 77
ordered cases/231 successful phases. Complete installed bytes, source copies,
native/SDK installed RECORD/direct_url metadata and process observations belong
to the execution archive. Original wheels remain separate inputs. A separate
bounded runtime bundle retains executable, shared-runtime and stdlib bytes for
the future adapter without reopening live host paths.

The two exact reviewed package recipes select the complete candidate source/data
inventory, and each original wheel must match it before installation. The same
original source owner is checked again before publication. The [dependency
contract](python_dependency_install_v1.md) joins every installed dependency byte
and RECORD to the original pinned archives before any installed interpreter
starts. Native/SDK files retain their sole verifier's typed ownership. Generated
console scripts are retained observations and are never executed.

The runtime bundle and execution ZIP are staged with retained original file
handles. The bundle is read back and parsed against the pinned manifest. Final
source, runtime, input and environment checks run while their owners remain live,
before either completed artifact name exists. Publication uses same-directory
no-replace links, with the ZIP last; failure removes only this attempt's owned
links. Cleanup only closes handles after publication. This does not claim atomic
power-loss durability or signed producer authority. The nested native Python
probe explicitly uses `-I -B`, keeping missing bytecode from changing its inputs.

TODO: complete the original-index Python adapter and signed aggregate integration.
The unsigned producer is not a promotion authority. Component fixtures, process
controls and fresh-venv mechanics do not execute the canonical native assertions
or qualify any supported platform. Matching-candidate native execution, all SDK
and hardware runs, independent review and authenticated producer/operator
approval remain required; SF11 remains open.

`sorafs_python_archive.py` owns the sole execution ZIP writer and captured-byte
reader. It requires sorted regular stored members, exact metadata and complete
canonical records within 20,000 files, 256 MiB per member, 768 MiB payload and a
1 GiB encoded envelope. The reader extracts nothing and supplies no execution
authority. There is no alternate archive layout or import alias for the retired
writer location.

`sorafs_python_commands.py` owns all nine fixed operations, their exact argv,
log limits, deadlines and stderr policy. The producer executes only the next
operation from that catalog. Captured replay joins each command and log byte to
the same definitions without accessing historical producer paths. It rejects
extra/omitted/reordered commands, nonzero results, added arguments, unaccounted
output and exceeded bounds; matching a transcript still grants no producer
approval.

The shared native checker also bounds nested ABI stdout/stderr to 4 KiB each and
symbol-inspection stdout to 16 MiB, with 4 KiB stderr and a 30-second deadline.
Those limits apply during reads, before complete output can accumulate. Nested
probes inherit the producer's process group/session, preserving outer timeout
and cancellation cleanup. The generic checker reaps its direct child and makes
no standalone arbitrary-descendant containment claim. Actual Windows execution,
separate Git/source-manifest collectors and in-process C probes require their
own qualification; Windows pipe API controls alone do not provide that evidence.
