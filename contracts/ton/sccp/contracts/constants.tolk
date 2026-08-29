/// TON basechain workchain used by the immutable SCCP deployment.
const SCCP_WORKCHAIN = BASECHAIN

/// TON SCCP protocol-domain identifier.
const SCCP_DOMAIN_TON = 4
/// SORA SCCP protocol-domain identifier.
const SCCP_DOMAIN_SORA = 0
/// Canonical TON account codec identifier.
const SCCP_CODEC_TON_ADDRESS36 = 3
/// Canonical text codec identifier.
const SCCP_CODEC_CANONICAL_TEXT = 0
/// Taira XOR uses nine decimal places; the TON Jetton uses the same base unit.
const SCCP_TAIRA_TO_TON_MULTIPLIER = 1
/// Maximum TEP-74 `VarUInteger 16` value (15 value bytes).
const SCCP_MAX_JETTON_AMOUNT = 0xffffffffffffffffffffffffffffff

const SCCP_PAYLOAD_VERSION = 1
const SCCP_PAYLOAD_DISCRIMINATOR = 0
const SCCP_MESSAGE_SCHEMA_VERSION = 1
const SCCP_STORAGE_VERSION = 1
const SCCP_TON_MAINNET_GLOBAL_ID = -239
/// Final-V1 network-profile tags. These are not transfer-domain identifiers.
const SCCP_NETWORK_PROFILE_SORA_V1 = 0x40
const SCCP_NETWORK_PROFILE_TON_V1 = 0x44
const SCCP_SORA_TAIRA_CHAIN_ID_V1 = 0xfc56984b2be7431d840e21514d1883f0
const SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1 = 0x17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569
const SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1 = 0x5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e
const SCCP_SORA_NETWORK_BYTES_V1 = 22
const SCCP_TON_MAINNET_NETWORK_BYTES_V1 = 90
const SCCP_TON_TO_SORA_LANE_HASH_V1 = 0x53c771f37951401d26513e2519cf46b93c582f75529d3949a6c548217ec87b55
const SCCP_SORA_TO_TON_LANE_HASH_V1 = 0x4876fd8bf67c75bbe3e96c884e82a8e794a2bd277f2f206cb19b96bedbf19bc0
const SCCP_DESTINATION_PROOF_BACKEND = 3
const SCCP_GROTH16_PUBLIC_SIGNAL_COUNT = 11
const SCCP_GROTH16_VK_IC_COUNT = 12
/// BLS12-381 scalar field used by the destination Groth16 verifier.
const SCCP_BLS12_381_SCALAR_FIELD = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001
/// The canonical 36-byte single-key Taira account has 42 base-105 payload digits and six
/// checksum digits. All 48 digits may be three-byte I105 kana, following the four-byte sentinel.
const SCCP_MAX_TAIRA_RECIPIENT_BYTES = 148
/// Canonical recipient input snakes split after this many bytes. This also keeps every payload
/// chunk below the TVM ordinary-cell bit limit after framing.
const SCCP_TAIRA_RECIPIENT_HEAD_BYTES = 80
/// The fixed canonical transfer header occupies the first external payload cell.
const SCCP_CANONICAL_HEADER_BYTES = 50
/// Remaining canonical bytes are split deterministically into two 100-byte chunks and a tail.
const SCCP_CANONICAL_CHUNK_BYTES = 100
/// Four cells can carry the 50-byte header and at most 324 remaining bytes.
const SCCP_MAX_CANONICAL_PAYLOAD_BYTES = 374
/// A valid Taira-to-TON transfer plus the payload-hash domain prefix occupies
/// at most three BLAKE2b blocks after strict destination framing is parsed.
const SCCP_MAX_TAIRA_TO_TON_HASH_BLOCKS = 3
const SCCP_MAX_TON_TO_TAIRA_HASH_BLOCKS = 2
const SCCP_ED25519_SUBGROUP_BITS_PER_STEP = 96
const SCCP_ED25519_SUBGROUP_MAX_STEPS = 3

const SCCP_I105_CHECKSUM_DIGITS = 6
const SCCP_I105_SINGLE_KEY_PAYLOAD_DIGITS = 42
const SCCP_I105_BASE = 105
const SCCP_I105_BECH32M_CONSTANT = 0x2bc830a3
const SCCP_ED25519_FIELD = 0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed
const SCCP_ED25519_D = 0x52036cee2b6ffe738cc740797779e89800700a4d4141d8ab75eb4dca135978a3
const SCCP_ED25519_SQRT_M1 = 0x2b8324804fc1df0b2b4d00993dfbd7a72f431806ad2fe478c4ee1b274a0ea0b0
const SCCP_ED25519_SQRT_EXPONENT = 0x0ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd
const SCCP_ED25519_SUBGROUP_ORDER = 0x1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed
const SCCP_ED25519_TWO_D = 0x2406d9dc56dffce7198e80f2eef3d13000e0149a8283b156ebd69b9426b2f159
const SCCP_ED25519_Y_MASK = 0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
const SCCP_ED25519_TORSION_Y_1 = 0x7a03ac9277fdc74ec6cc392cfa53202a0f67100d760b3cba4fd84d3d706a17c7
const SCCP_ED25519_TORSION_Y_2 = 0x05fc536d880238b13933c6d305acdfd5f098eff289f4c345b027b2c28f95e826

const SCCP_PENDING_MINT = 1
const SCCP_PENDING_BURN = 2
const SCCP_COMPLETED = 3

const SCCP_REPLAY_TON_BRIDGE_INBOUND_MINT = 0x30
const SCCP_REPLAY_TON_BRIDGE_OUTBOUND_BURN = 0x31
const SCCP_REPLAY_TON_MASTER_MINT = 0x32
const SCCP_REPLAY_TON_MASTER_BURN = 0x33
const SCCP_REPLAY_TON_WALLET_MINT_CREDIT = 0x34
/// Bridge-authorized irreversible wallet burn transition.
const SCCP_REPLAY_TON_WALLET_BURN_AUTHORIZATION = 0x35
/// Owner-authenticated transient burn-lock debit.
const SCCP_REPLAY_TON_WALLET_BURN_LOCK = 0x36
/// Recredit of a rejected lock that never reached bridge operation 0x31.
const SCCP_REPLAY_TON_WALLET_BURN_REFUND = 0x37

/// Maximum number of asynchronous records retained by one contract boundary.
/// Replay occupancy is permanent, but live dispatch state is deliberately
/// bounded so an attacker cannot grow contract storage without limit.
const SCCP_PENDING_OPERATION_CAP = 1024
/// The emergency controller is immutable and always requires three of the five
/// route-bound guardian keys.
const SCCP_MINT_BREAKER_THRESHOLD = 3
const SCCP_MINT_BREAKER_GUARDIAN_COUNT = 5

/// Mainnet TVM transaction gas ceiling used with `GETGASFEE`. No SCCP call
/// assumes a cheaper witness or execution path.
const SCCP_MAX_TRANSACTION_GAS = 1000000
/// Five years of rent is kept on every boundary. Bounds include code, all 256
/// replay roots, and every live map filled to its 1024-entry protocol cap.
const SCCP_STORAGE_RESERVE_SECONDS = 5 * 365 * 24 * 60 * 60
const SCCP_MAX_BRIDGE_ACCOUNT_CELLS = 32768
const SCCP_MAX_BRIDGE_ACCOUNT_BITS = SCCP_MAX_BRIDGE_ACCOUNT_CELLS * 1023
const SCCP_MAX_MASTER_ACCOUNT_CELLS = 8192
const SCCP_MAX_MASTER_ACCOUNT_BITS = SCCP_MAX_MASTER_ACCOUNT_CELLS * 1023
const SCCP_MAX_WALLET_ACCOUNT_CELLS = 24576
const SCCP_MAX_WALLET_ACCOUNT_BITS = SCCP_MAX_WALLET_ACCOUNT_CELLS * 1023

/// Conservative full-message size ceilings. One maximally dense depth-248
/// witness is exactly 84 cells/64,256 bits; envelopes and semantic fields are
/// included in these larger limits and checked by focused tests.
const SCCP_MAX_SMALL_MESSAGE_CELLS = 24
const SCCP_MAX_SMALL_MESSAGE_BITS = 8192
const SCCP_MAX_ONE_WITNESS_MESSAGE_CELLS = 128
const SCCP_MAX_ONE_WITNESS_MESSAGE_BITS = 80000
const SCCP_MAX_TWO_WITNESS_MESSAGE_CELLS = 220
const SCCP_MAX_TWO_WITNESS_MESSAGE_BITS = 145000
const SCCP_MAX_THREE_WITNESS_MESSAGE_CELLS = 300
const SCCP_MAX_THREE_WITNESS_MESSAGE_BITS = 200000
const SCCP_MAX_FOUR_WITNESS_MESSAGE_CELLS = 350
const SCCP_MAX_FOUR_WITNESS_MESSAGE_BITS = 265000
/// Master-to-wallet mint may also carry the canonical wallet StateInit.
const SCCP_MAX_WALLET_DEPLOY_MESSAGE_CELLS = 500
const SCCP_MAX_WALLET_DEPLOY_MESSAGE_BITS = 300000

/// Generic TEP-74 calls retain a small non-SCCP operation allowance. SCCP
/// handlers use the network-derived recursive budgets in `jetton-utils`.
const SCCP_OPERATION_VALUE = 120000000
