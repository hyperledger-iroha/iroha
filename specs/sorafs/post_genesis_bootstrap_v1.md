# Post-genesis SoraFS initialization

The original proposal to append a bootstrap action to the selected threshold-key
lifecycle was rejected and is not implemented. Its three original lifecycle actions,
wire layout and catalog identity remain unchanged.

The owner-authorized, Taira-only replacement is specified in
[`taira_repair_v1.md`](taira_repair_v1.md). It uses a new typed instruction and a
separately identified kernel, an exact parent-bound plan signed by all four validators,
and one rollback-owned transition. No generic bootstrap mechanism is introduced.

The shared `SorafsBootstrapPayloadV1` names the council, three provider admissions,
existing owner accounts, allowlisted SoraFS grants and compliance-role members.
The full certificate and once-only receipt are retained in native state and history.
