# Native lane relay instruction retirement

The first release uses native lane admission and global execution (`specs/sumeragi_lanes.md` §7).
`RegisterVerifiedLaneRelay` and `SetLaneRelayEmergencyValidators` have no registered wire identifiers,
visitor hooks, execution handlers or fee exemptions. The emergency permission token is retired.
No compatibility decoder or replacement receipt authority is installed.

Exact captured retired instruction frames are retained only in
`crates/iroha_data_model/tests/fixtures/retired_lane_instruction_frames.json` as negative inputs.
Every other captured record is unchanged. The Nexus confidential-header specimen keeps its
explicit field assertions; its removed relay transport is not an identity or privacy qualification.
The separate privacy-record capture mismatch remains an independent gate.

Retired protocol tests and their source-pinning Python guard no longer describe a production owner.
Their removal is not a passed test gate. Current native lane authentication, bounded admission,
original execution custody, replay and fee tests must pass before release.

Emergency configuration and World storage have also been removed from the active graph.
TODO: remove the remaining read-only relay DTO/query surfaces and migrate their consumers;
complete native cross-dataspace atomic execution (S6). Retirement alone does not
qualify either outcome.