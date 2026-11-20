# Certified scoped-read permission on Taira

Release R (815f5b46a0) activates the keyed State profile and the exact-row permission.
Its final selection is `fdda49f61f74f9fe50344a952ccfe7c965beaa446c9c727ab20b349a05ca6b29`.
The successor recognizes one additional certificate at this selection. Its change set publishes
the compiled `CanReadStateScope` definition
`58922c8ba977ae786c440bb3df20df04c66fed1e94f8bb970931560064cb506d` and executes one
`Activate` transition. The certificate and operator tool require the exact resulting selection
`108d5cfd34150bd0502911564429fbd942f8048db63059fe61a2e24f89e2b1b5`.
The chain label and signed-genesis network identity must equal the compiled Taira identities.

`iroha taira read-scope draft-change-set` takes the independently pinned selection JSON, the
network ID, a nonzero qualification digest and an effective height. Each of the four scheduled
validators reviews the resulting predecessor, successor, descriptor and change-set digest.
`prepare` binds that exact draft to the chain, the ordered frozen roster, the expiry, the notice,
the operation ID and the owner authorization, release and source-closure digests. Each seat
runs `sign` with its own runtime-only private key file, the reviewed plan and its independent
pins. `assemble` verifies one share for every distinct seat and writes a single signed-certificate
instruction for transaction stdin. The operator submits it as the sole instruction of an external
transaction. The canonical Norito plan identity and signing preimage each have a dedicated
`iroha:runtime:taira-read-scope` domain.

Scheduling retains the full certificate at `runtime/taira_read_scope/certificate` and leaves
the predecessor selected. Every validator must separately attest to support for the scheduled
change set through the ordinary node operator route. At the start of its effective height,
the ordinary activation checks the attestations, notice, selection, obligations and permission
inventory. A preexisting account or role grant makes activation lapse. A lapsed attempt can be
retried with a new certificate. Each prior attempt's change-set record remains in State.

After every seat reports this change set as activated at that height, the existing holder of
`CanReadAllLedgerData` signs the individual direct account grants. The scheduling transaction
confers no reader authority. The certificate cannot be replayed on a later selection. Ordinary
governance owns subsequent restrictions and retirement.

Supply each [permission object](state_scope_grant.schema.json) as stdin to
`iroha --config <client.toml> --emit-instructions account permission grant --id <reader-I105>`.
Keep the embedded payload in its canonical compact JSON spelling. Each invocation emits a
JSON array containing one Norito instruction frame. Combine the frames in the reviewed
grant order for the genesis operator’s single signed transaction.

The successor's ignored `read_scope::four_seats_attest_and_activate_before_the_scoped_grant`
campaign starts from four stopped release-R localhost stores. It expects `IROHA_READ_SCOPE_R_FIXTURE`
to name their test-network directory and `IROHA_READ_SCOPE_FIXTURE_DAEMON` to name a fixture
build whose deployment constants pin that exact test chain and genesis hash. Use a fresh copy for
each run and supply its source manifest in `IROHA_READ_SCOPE_FIXTURE_SOURCE_MANIFEST`. Both plans
bind SHA-256 of the executable and source manifest. The first schedule lapses with three
support attestations. A fresh four-signature schedule with all four attestations activates at
its declared height. The campaign verifies grants, revocation and scoped membership against
the certified prefix and retains each canonical public certificate in its scratch directory.
Its `public-read-scope` directory contains the signed genesis, the effective selection JSON
from the signed runtime route, and canonical finality and keyed-row proofs at the activation
and granted-reader heights. The activation proof authenticates the original
`world.runtime_selection` cell and checks its bytes against the exported JSON. At each height,
`kagami advanced kura checkpoint` can verify the stopped journal from the pinned genesis and
export the native finality checkpoint for an offline consumer.

The ignored `unpatched_successor_refuses_the_same_four_signature_fixture_certificate` control
uses a fresh copy of the same R stores. Set `IROHA_READ_SCOPE_PRODUCTION_DAEMON` to the
unmodified successor and `IROHA_READ_SCOPE_CERTIFICATE` to the positive run's
`activated-certificate.nrt`. Run it before the test certificate expires. It checks signatures,
roster, deployment, predecessor and notice before confirming that the production daemon
refuses the isolated fixture certificate. Release-R restoration and replay also use the
unmodified successor binary.
