# Account off-chain signing keys

An account's controller (its single transaction key, or the members of its multisig policy)
signs only transactions. Everything else the account signs off chain is signed by a separate
**signing key** that the account registers on chain. A dApp that asks a wallet to sign
arbitrary bytes therefore never obtains a signature that can authorize a transaction.

## Why

Iroha Connect `SignRequestRaw` asks a wallet to sign dApp-chosen bytes. Wallets signed those
bytes with the account's transaction key, and a transaction signature is a signature over the
32-byte `HashOf<TransactionPayload>`. A dApp could send that hash, or grind one that passes a
wallet's "readable text" check, and receive a valid transaction signature. Torii canonical-request
authentication used the same key over the same path.

## State and instruction

The release addition `iroha.account.signing_key.v1`
(`crates/iroha_core/src/runtime/catalog/additions.rs`) adds:

- `world.account_signing_keys`: `AccountId -> AccountSigningKeyRecordV1 { generation, public_key }`;
- `world.account_signing_key_reservations`: `AccountId -> AccountSigningKeyReservationV1
  { account, generation, active }`, every key that is or was a signing key, keyed by the
  single-key `AccountId` the key would control;
- the instruction `SetAccountSigningKeyV1 { account, signing_key }`
  (`iroha.account.signing_key.set.v1`).

Until a governed change set activates the addition, the instruction resolves as
`definition_inactive`, both tables stay empty and out of snapshots, and every account
authenticates off chain with its controller exactly as before.

`SetAccountSigningKeyV1` (`crates/iroha_core/src/smartcontracts/isi/account_signing_key.rs`):

- only the account itself (the transaction authority) changes its signing key, and only while it
  is registered;
- `Some(key)` installs or rotates, `None` revokes; every change raises `generation` by one;
- a key that is a member of the account's controller, controls a registered account, or is
  reserved (active or retired, for any account) is refused, so a key is a signing key once.

A controller change (rekey or recovery) gives the account a new `AccountId`, which has no
signing key until it installs one; the old key stays reserved.

Verifiers read the key from `GET /v1/accounts/{account_id}/signing-key` (Torii,
`crates/iroha_torii/src/account_signing_keys.rs`): `{ "public_key": "ed0120…", "generation": n }`,
or `404 account_signing_key_not_found`.

## The off-chain envelope

Every off-chain signature covers (`iroha_data_model::account::signing_key`):

```text
"iroha:offchain:v1\0"            18 bytes
purpose                          u8: 1 ConnectRaw, 2 ToriiCanonicalRequest
network_id                       [u8; 32], the genesis hash
account_len, account             u32 LE, canonical account-address bytes
generation                       u32 LE, the signing key's generation
tag_len, domain_tag              u32 LE, UTF-8
bytes_len, bytes                 u64 LE, the signed bytes
```

The envelope is at least 71 bytes and starts with a fixed prefix, so it never equals a 32-byte
transaction or query hash, and a signature made for one purpose, network, account, key
generation or tag never verifies for another. The signer chooses the purpose; a requester never
does. Shared vectors: `fixtures/account/offchain_signing_vectors.json` (checked by the Rust data
model and the JavaScript SDK).

## Torii canonical requests

For an account with a signing key, `X-Iroha-Signature` is the signing key's signature over the
envelope with purpose `ToriiCanonicalRequest`, tag `iroha:torii:canonical-request:v1` and bytes =
the canonical request message (`crates/iroha_torii/src/app_auth.rs`). Signatures and witnesses
of its controller are refused, including for multisig accounts, which then authenticate with the
signing key alone. Accounts without a signing key authenticate with their controller as before.
DA ingest still requires controller signatures on the DA authorization itself; a request
authenticated with the signing key carries them from the account's controller members
(`crates/iroha_torii/src/da/ingest.rs`).

## Iroha Connect

- A wallet signs `SignRequestRaw { domain_tag, bytes }` with the signing key over the envelope:
  purpose `ToriiCanonicalRequest` when `domain_tag` is `iroha:torii:canonical-request:v1` (after
  checking that the bytes are a canonical request and showing it), otherwise `ConnectRaw`
  (`iroha_torii_shared::connect_sdk::sign_raw_request`).
- A wallet signs `SignRequestTx { tx_bytes }` only after decoding the payload, with the transaction
  key over its hash (`crates/iroha_torii_shared/examples/connect_wallet.rs`).
- A dApp verifies a raw reply against the key it read from committed state
  (`connect_sdk::verify_raw_request_signature`; JavaScript `createConnectAppSession`,
  `createConnectCanonicalRequestAuth`).
- The approval signature stays with the controller: its preimage is a wallet-built, tagged
  structure (`connect_sdk::build_approve_preimage`), not requester-chosen bytes.

## Wallet recipe

1. Derive the signing key from the wallet secret on its own hardened path (or generate it), never
   reusing the transaction key: for example `m/44'/<coin>'/<account>'/1'` when the transaction key
   is `m/44'/<coin>'/<account>'/0'`.
2. Submit `SetAccountSigningKeyV1::install(account, signing_public_key)` signed by the account's
   controller, and record the resulting generation (`1` for the first key).
3. For each Connect raw request, build the envelope with the account, network, generation, tag and
   bytes, and sign it with the signing key. Sign canonical requests the same way with purpose
   `ToriiCanonicalRequest`. Never sign raw bytes with the transaction key.
4. To rotate, install a fresh key (generation + 1); to stop off-chain signing, revoke. A replaced or
   revoked key is reserved forever.

## Follow-up

- Registering an account, explicitly or implicitly, whose controller is a reserved signing key is
  not refused yet; that needs a rebind of the baseline registration handlers, which this release
  cannot dispatch. Such an account gains nothing, because no envelope signature is a valid
  transaction signature.
- No governed parameter yet requires a signing key for off-chain authentication; accounts without
  one keep controller authentication.
- Connect approvals, signed queries and the structured controller authorizations (private
  settlement, DA, SoraFS, Musubi, SoraCloud, SoraNet, oracles, KAGEMUSHA) stay with the controller.
- The Swift, Kotlin/Java and Python canonical-request signers and Connect wallets still sign with the
  controller key.
