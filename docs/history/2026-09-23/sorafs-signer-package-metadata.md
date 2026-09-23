# SoraFS signer package metadata and topology trust

The first-release SoraFS release workflow no longer asserts that a topology
signer used a particular backend. Its exact signed topology envelope has no
backend-origin field. Authenticated software custody is permitted without an
HSM, and optional hardware custody gains no authority from its device type.

The CLI candidate, release bundle and release image manifests now report only
whether the external signer binary is packaged. That bit is derived from the
checked signer/broker artifact inventory, not from the target operating system
or an assumed deployed signer qualification. A packaged binary does not prove
current signer authorization, finalized operation completion, revocation,
recovery, or a production-ready service.

The source changes are in
`scripts/package_sorafs_cli_candidate.py`, `scripts/build_release_bundle.sh`,
`scripts/build_release_image.sh`, `.github/workflows/sorafs-cli-release.yml`
and `scripts/check_sorafs_release_automation.py`. The focused package/profile
tests passed 102/102; the topology backend-claim and repository-contract
automation selection passed 5/5. The automation checker and shell syntax check
passed. The full bundle/image release builders remain blocked by their strict
trusted-source seal because this shared checkout contains uncommitted changes;
that seal was neither refreshed nor bypassed.

TODO: Build and verify all five native target packages from the one frozen
candidate, then supply genuine operator signer authorization and completed
operation evidence before promotion. No release gate closes from this metadata
correction.
