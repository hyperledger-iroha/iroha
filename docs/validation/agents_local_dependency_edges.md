# Existing local dependency edges

The development and agent workflow sections of [AGENTS.md](../../AGENTS.md)
require coherent manifest and lockfile updates for approved dependency changes.
The [contributor guard](../../ci/check_agents_guardrails.sh) therefore permits a
narrow structural case: adding a plain path dependency from one existing
workspace package to another already present in the lockfile.

[The checker](../../scripts/check_agents_local_dependency_edges.py) compares the
working tree, including staged edits, with the selected base. Every added lock
edge must match a new normal dependency declaration and Cargo's locked, offline
workspace metadata. Cargo metadata uses `--no-deps`; the checker separately
compares the complete lockfile package records and exact added edges. It does
not compile, fetch dependencies, regenerate the lockfile, or claim full feature
resolution. Cycles introduced by a new normal dependency are checked through
normal and build dependencies; development dependency cycles remain permitted. Aliases,
optional dependencies, feature changes and escaping paths
are outside this exception.

All package identities, versions, sources, checksums and existing dependency
edges must remain unchanged. New packages, external edges, dependency removals,
unmatched manifest changes, unavailable metadata or helper failures reject the
exception. Existing new-crate, workspace-member, external-dependency and SoraNet
privacy checks remain in place. Changes outside this structural case need their
own reviewed dependency-policy update; there is no environment-variable bypass.

Run `python3 -m pytest -q scripts/tests/check_agents_local_dependency_edges_test.py`
for the positive and mutation controls, and run
`AGENTS_BASE_REF=<base> bash ci/check_agents_guardrails.sh` for the complete guard.
