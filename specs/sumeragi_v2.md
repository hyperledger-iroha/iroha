# Sumeragi specification location

The first release implements one Sumeragi protocol with wire version `1`.
Its implementation-coupled specification is [sumeragi.md](sumeragi.md), with
outstanding outcomes and qualification evidence in
[sumeragi_goals.md](sumeragi_goals.md). Lane instances and their global merge
are specified in [sumeragi_lanes.md](sumeragi_lanes.md).

The prior v2 design is not a supported wire format, startup path, or parallel
implementation. Current startup uses the original signed genesis and native
certified replay; see [startup_trust_roots.md](startup_trust_roots.md).
