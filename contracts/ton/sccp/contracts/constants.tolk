/// TON basechain workchain used by the immutable SCCP deployment.
const SCCP_WORKCHAIN = BASECHAIN

/// TON SCCP protocol-domain identifier.
const SCCP_DOMAIN_TON = 4
/// SORA SCCP protocol-domain identifier.
const SCCP_DOMAIN_SORA = 0
/// Canonical TON account codec identifier.
// Codec ids 3 and 4 belonged to retired presentation codecs and must never be reused.
const SCCP_CODEC_TON_ADDRESS36 = 7
/// Canonical text codec identifier.
const SCCP_CODEC_CANONICAL_TEXT = 1
/// Taira XOR uses nine decimal places; the TON Jetton uses the same base unit.
const SCCP_TAIRA_TO_TON_MULTIPLIER = 1
/// Maximum TEP-74 `VarUInteger 16` value (15 value bytes).
const SCCP_MAX_JETTON_AMOUNT = 0xffffffffffffffffffffffffffffff

const SCCP_PAYLOAD_VERSION = 1
const SCCP_PAYLOAD_DISCRIMINATOR = 2
const SCCP_MESSAGE_SCHEMA_VERSION = 1
const SCCP_STORAGE_VERSION = 1
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

const SCCP_I105_CHECKSUM_DIGITS = 6
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

/// Maximum number of asynchronous records retained by one contract boundary.
/// Replay occupancy is permanent, but live dispatch state is deliberately
/// bounded so an attacker cannot grow contract storage without limit.
const SCCP_PENDING_OPERATION_CAP = 1024
/// The emergency controller is immutable and always requires three of the five
/// route-bound guardian keys.
const SCCP_MINT_BREAKER_THRESHOLD = 3
const SCCP_MINT_BREAKER_GUARDIAN_COUNT = 5

const SCCP_MIN_BRIDGE_RESERVE = 100000000
const SCCP_MIN_MASTER_RESERVE = 100000000
const SCCP_MIN_WALLET_RESERVE = 50000000
const SCCP_OPERATION_VALUE = 120000000
