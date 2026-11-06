# Taira SoraFS activation status

The owner authorized the one-time repair on 2026-11-07, with signatures from all
four validators and preservation of all BPNG accounts and data. The canonical
implementation contract is [`taira_repair_v1.md`](../../specs/sorafs/taira_repair_v1.md).

The repair publishes a new immutable kernel, interface and implementation and
selects that exact successor in the same transaction that initializes SoraFS.
It does not extend the existing threshold-key lifecycle or grant general governance
powers. The original lifecycle carrier was rejected and removed.

The SORA Cars package and gateway DNS/TLS are prepared. Implementation and local
checks alone do not establish deployment: the signed release must replay the
retained Taira history, preserve BPNG state, pass native client proof verification,
and ingest and retrieve the published content through all three providers.

Operator tooling lives under `iroha sorafs toolkit repair`: draft the exact change
set, prepare its parent-bound plan, sign separately with each of the four scheduled
validators, and assemble the certificate. Kagami checkpoint evidence supplies the
native parent core/result hashes. Retain exact source, release, authorization,
qualification, payload, signature and finalized transaction evidence together.

Ordinary authenticated transactions then configure reserve, provider credit and
capacity. `sorafs_cli deploy` consumes the verified prepared package. Only a finalized
publication with verified public browser retrieval establishes a working gateway URL.
