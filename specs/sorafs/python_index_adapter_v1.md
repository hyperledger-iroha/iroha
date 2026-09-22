# SoraFS Python original-index adapter V1

`scripts/sorafs_sdk_python_artifact_verifier.py::verify_python_consumer` consumes
the Python row of the closed six-consumer package index through that index's
original `OpenedIndexFiles`. It requires a clean, matching trusted source tree
and independently supplied runtime/dependency manifest digests. Those selections
remain the signed aggregate's responsibility; an execution manifest cannot
approve its own inputs.

The package is the original SDK wheel. Execution is the distinct canonical ZIP.
Exactly twenty indexed inputs supply the native wheel, native ABI-23 manifest,
runtime manifest, dependency manifest, runtime byte bundle and fifteen dependency
wheels. Every role has a unique original digest/size join; missing, ambiguous,
reused or unused inputs reject. The same original descriptors and paths are
rechecked before observations return.

The adapter consumes the sole wheel, installed-content, dependency, runtime
bundle, ZIP and command verifiers. The explicit native profile is POSIX ABI3;
there is no interpreter-host suffix fallback. Original extension bytes must match
the candidate's native manifest. All package source/data and build recipes join
the independently selected source tree. Both source and package censuses are
repeated before return, including build inputs that Git might ignore.

Historical producer paths are logical labels. The shared child input parser
separates canonical byte checks from the live child's current-path checks. The
adapter reconstructs the exact child input from original wheel observations,
trusted sources and runtime executable bytes, without reopening those labels.
Its nine command observations and complete stdout/stderr inventories must match
the one production catalog. Runtime probes, offline hash requirements and the
installed distribution inventory join the same inputs and private layout.

The complete captured environment is exactly the fixed bootstrap plus native,
SDK and dependency content. Its interpreter copies and configuration join the
runtime; generated activation/console programs remain bounded unexecuted output.
Dependency file rows are compared through canonical JSON so Boolean values cannot
masquerade as integer sizes. Installed native/SDK RECORD and direct URL bytes are
verified by the sole byte relation and compared with the retained metadata.

The complete execute stdout is replayed through the fixed report parser. Its
report bytes, all 77 ordered cases and 231 successful phases, exact trusted source
inventory, Python/pytest identities, installed files and loaded modules must
agree with the original captured bytes. `sorafs_python_report_origins.py` owns
the shared producer/adapter origin relation. Reported device/inode/time/mode
values remain observations; the adapter never manufactures live physical owners.

The return type is an immutable set of scoped observations with no `passed` or
`qualified` authority. A fabricated but internally consistent transcript can
exercise component tests; it cannot authenticate who executed it. TODO: connect
independent producer/operator approvals and the exact selected pins to the
signed aggregate, ReleaseManifest custody/completion and SF11 derivation. Complete
the four remaining consumer adapters and run the actual matching-candidate
native/SDK/platform matrix. This POSIX host contract does not qualify Windows,
devices, hardware parity or the release.
