# Taira outage recovery, 2026-09-22

This is incident evidence, not release qualification. At 07:09:52 UTC,
public `/status`, `/v1/accounts/faucet/policy` and `/v1/mcp` still returned
HTTP 502. Funding, deployment and contract execution remain unverified.

The approved MacStadium host's nginx service was running. Its retained Linux
guest accepted TCP connections but closed SSH before authentication; all four
validator API ports refused connections. Restarting that exact guest restored
SSH. The existing VM configuration and disk identity were checked before and
after the restart; neither was replaced. No cause for the guest hang has yet
been established from the retained kernel log.

After the guest restart, validator 1 served its retained height 1969 with no
connected peers. Validators 2 and 4 stopped consensus at retained height 3598:
the mandatory next-height beacon required an active key session absent from
committed state. Validator 3 failed earlier during Kura initialization because
its application-receipt data could not be decoded.

The validator-3 receipt file's first 4,194,304 bytes were zero. Every subsequent
byte matched validators 2 and 4, whose complete receipt files were identical.
All three peers had identical canonical block data, block index, block hashes,
block count, lane incarnation and application-receipt index. Their configured
genesis files also matched. The validated donor receipt file SHA-256 was
`b2edc38ec2fc0467970b0ce8507b97b71326ca31156e842d6b744b3434d9b508`;
the corrupt file SHA-256 was
`c3a62c01446fd92906a133491421a23522a17cb9368a14e9bae3e47ea9a8e097`.

Validator 3 was stopped. After repeating the identity checks, the original
corrupt file was preserved, and the exact donor bytes were installed through a
synced atomic replacement with the original ownership and permissions. The
index and ledger were unchanged. The restarted validator completed Strict Kura
initialization with all 3598 finality artifacts verified at 07:08:04 UTC. It then
reached the same missing-beacon failure as validators 2 and 4. The restored file
still matched the donor hash at 07:09:52 UTC. The cause of the zeroed prefix is
not yet established.

The three height-3598 snapshots select the same generation. Their committed
beacon DKG, key-session, active-session and pulse maps are all empty; the
retained bootstrap-controller beacon directory is absent. This is not evidence
of a missing runtime signing file. No snapshot was edited, no beacon state was
invented, no empty block was produced, and no shared-ledger reset was performed.
The deployed daemon remains source `d085418a382e831875731144436c174d8621cdd6`.

Owner-local evidence is retained under `target/dpn-devex/`:

- `live-vm-restart-20260922.json`
- `live-receipt-index-probe-20260922.json`
- `live-receipt-integrity-20260922.json`
- `live-receipt-restore-preflight-20260922.json`
- `live-receipt-restored-20260922.json`
- `live-beacon-snapshot-projection-20260922.json`
- `live-receipt-post-recovery-20260922.json`

These are ignored diagnostic artifacts, not portable or signed release proof.

At 07:39:49 UTC, fresh JSON GET probes of the same three public routes still
returned HTTP 502. A source review of deployed/current receipt append, rollback
and prune paths found no supported explanation for the zeroed prefix; no
speculative storage correction was applied. Current bootstrap guards already
reject a beacon installation at or beyond its first mandatory pulse deadline.
The retained ledger cannot gain that missing prior committed session merely by
restoring runtime signer files.
