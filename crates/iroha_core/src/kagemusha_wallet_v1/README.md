# KAGEMUSHA ledger implementation

`KagemushaWalletLedgerV1` is the closed instruction boundary. Registration requires an
exact `CanManageKagemushaWallet { asset_definition }` grant and the reserve account's
own transaction authority. The asset-definition capability owner grants that scoped
permission through the normal permission instruction; an exact holder may delegate it.
An asset owner cannot capture another account merely by registering a scheme.

Registration permanently binds the scheme, asset incarnation, scale, reserve account,
and balance scope. It records authenticated constant-size reserve/account/definition
indexes in the canonical World table. Exact registration retry does not increment them.
No unregister or reserve-withdrawal instruction exists. Ordinary transfer, burn, account
retirement, asset retirement, and domain teardown cannot spend or remove that reserve.
Current and rollback snapshot restoration verify every reserve owner/index and the exact
positive registration reference counts. Missing, mixed or inflated indexes are rejected.

Load deposits and Unload/fee payouts use the existing atomic numeric movement owner,
including transfer controls, source custody, exact balances, admission, and execution
transcripts. Both resolved balance buckets must equal the registered scope. The offline
blacklist is not consulted for Unload. This implements the distinction in design §6:
full claim face value and historical fees remain fixed; ordinary online asset controls
can delay execution but do not rewrite a retained claim or create a haircut.

The native verifier is a mandatory Rust dependency. The public handler currently returns
`ArtifactsUnavailable` for package verification until the authenticated production A/Ω
artifact loader is installed. Fixture verifiers exist only in tests. Registration or
structural G1 checks cannot substitute for a completed Bootstrap proof.

Finalized issuance reads require a source-verified global CertifiedChain tip under finite
NativeFinalityLimits, plus exact original transaction membership in that same World cut.
An issuance body is not a signed voucher. `LoadAuthorizer` only prepares from that concrete
capability. Its closed PublishVoucher command requires
`CanPublishKagemushaLoadVoucher { asset_definition, scheme, authorizer_certificate }` and
the exact historical LoadAuthorization signature/body. Asset governance delegates submission
authority separately from management. The first bytes are immutable; exact retries succeed
and another valid signature encoding conflicts. Publication moves no reserve funds.

RotateLoadAuthorizer requires the management permission and a root-certified LoadAuthorization
key. It changes a separate active pointer for future issuance; original registration, historical
certificates, old issuance and published bytes remain fixed. The authorizer prefers retained
bytes on retry. The Torii query remains read-only and releases only verified finalized records.

Unsigned issuances atomically retain a pending-publication index under their original signer
certificate. First publication removes that index in the same World transaction; snapshot and
undo validation require both directions of the index/source binding. The worker reads bounded
certificate-prefix pages and never scans or copies permanent issuance history. Its disposable
cursors provide fair scheduling across historical certificates. Restart or an unknown queue
result rereads finalized issuance and reconstructs the same deterministic voucher bytes; a
completed first publication is obtained only from finalized state.

The optional daemon section `[kagemusha_load_authorizer]` defaults off. `enabled = true`
requires owner-admitted `keyring_file` and `submitter_key_file` references. The former is the
canonical Norito `LoadAuthorizerKeyringV1` (version 1, at most 32 role-certified keys, at most
65,536 bytes); the latter is a canonical ordinary ledger private key. The submitter needs the
exact historical certificate submission grant. Keyring scalars and input buffers are wiped on
drop; diagnostics redact them. No GET route signs, and no client supplies a validity verdict.

`poll_interval_ms`, `page_size`, `block_bytes`, `journal_bytes`, `block_count`,
`allocated_bytes` and `transaction_ttl_ms` set finite local capacities. `charge_limits` is the
canonical ordered list of exact online fee kinds, assets and maximum amounts authorized by the
submitter; the empty default authorizes no fee. These never relax
finality, signatures or monetary validity. The supervised worker uses ordinary transaction
admission and queue permissions/fees. Unavailable history, a stopped worker, revoked submission
permission, or queue rejection retains the reserve liability and its pending work. Submission
does not provide a durable-completion response.

TODO(G6): qualify complete restart/network operation and the authenticated native A/Ω artifacts.
No publication fee exemption or reserve debit is used.
