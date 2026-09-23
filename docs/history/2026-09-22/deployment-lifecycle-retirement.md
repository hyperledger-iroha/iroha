# Canonical deployment ownership after epoch-worker retirement

The retired epoch-maintenance CLI and supervisor renderer are removed. Reset
assembly, update and retry inputs use one closed first-release shape and reject
retired administrator, worker-plan and seed-source fields. Consensus authority
retention belongs to the authenticated node protocol.

Removing the worker does not remove deployment exclusion. Native reset actions,
routine updates, dispatcher transitions and retained-release retirement share
`/var/lib/taira-deployment/.deployment.lock`. Existing-file operations reject a
missing lock instead of creating replacement authority. Custody, no-follow,
single-link and path-rebinding checks remain. A durable `.reset-owner.json`
blocks Python update and retirement paths without reading, clearing or reclaiming
it. Native ownership is released only after all local targets have verified
terminal state; retained intent and terminal receipts persist separately.

Focused Python validation passes 184 updater and retained-release tests and
128 retry tests, including real lock exclusion, missing-lock rejection,
custody, rollback and failed-start recovery. The synthetic smoke evidence-reader
suite separately passes 47 tests and rejects the removed authority-context keys;
it does not establish cryptographic finality. Initial failed runs are retained.
Logs and script hashes live in the ignored local evidence directory
`target/architecture-redesign/da-manifest-async-2026-09-22/`.

The Rust CLI adds rejection tests for retired commands. Four public-input
fixtures now run directly on the ordinary test stack instead of spawning
16-MiB workers. TODO: Qualify these Rust tests and deployment lifecycle controls
from the completed merged-source build. Python passes and source review do not
qualify native deployment, stack usage, live hosts or release readiness.
