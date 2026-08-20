# Light Client Data Availability Sampling

Sumeragi v2 uses reliable broadcast internally for consensus data availability.
The public first-release Torii API does not expose per-session chunk sampling,
delivery inspection, or global collector-plan endpoints, and there is no
dedicated Torii sampling configuration.

For operator visibility, use authenticated `/v1/sumeragi/status` for compact
consensus state and `/metrics` for node-local transport observations.
These surfaces provide operational diagnostics, not light-client data-
availability proofs.
