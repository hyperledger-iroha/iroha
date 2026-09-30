`kagemusha_authority_state_v1.json` contains public synthetic data emitted directly
by `iroha_torii_shared::kagemusha_state`'s
`borrowed_binary_and_bounded_json_preserve_the_sole_owned_layout` test. Its raw
SHA-256 is `367fdd6cd34b4ac38dfe2ba58e7039a95a8f6485a7ff062a0994668d7764c5f4`.

The native test builds a synthetic asset and snapshot, certifies a height-two
native fixture decision, verifies the real BLS node signature and certified
snapshot root, then compares owned/borrowed JSON and canonical binary layouts.
Set `KAGEMUSHA_AUTHORITY_STATE_DIAGNOSTIC_JSON` to an absolute, nonexistent local
file to capture the exact owned JSON when reproducing the focused shared DTO test.

The empty governed registry deliberately grants no active release. The fixture
does not establish a production World snapshot, installed release, provisioning,
device qualification or spending authority. Python contract tests use the exact
native output to verify schema compatibility and reject nested substitutions.
