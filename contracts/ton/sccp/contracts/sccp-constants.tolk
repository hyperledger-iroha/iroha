// SCCP v1 TON constants (specs/sccp.md revision 4, §3 and §5.3).
//
// Every value here is pinned by a golden test in
// `tests/unit/sccp-vectors.test.tolk`.

/// All SCCP TON contracts live in the basechain.
const SCCP_WORKCHAIN = BASECHAIN

/// `GLOBALID` of TON mainnet (§5.1.3 step 1).
const SCCP_TON_GLOBAL_ID = -239

/// SCCP domains (§3.2).
const SCCP_DOMAIN_TAIRA = 0
const SCCP_DOMAIN_TON = 4

/// Network profile tags (§2.1).
const SCCP_TAG_TAIRA = 0x40
const SCCP_TAG_TON = 0x44

/// `identity_word(ton-mainnet)`: two's-complement `int256(-239)` (§2.1).
const SCCP_TON_IDENTITY_WORD = 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff11

/// Transfer payload constants (§3.2).
const SCCP_PAYLOAD_KIND_TRANSFER = 0x02
const SCCP_PAYLOAD_VERSION = 0x01
const SCCP_CODEC_CANONICAL_TEXT = 1
const SCCP_CODEC_TAIRA_ACCOUNT = 3
const SCCP_CODEC_TON_ACCOUNT36 = 7
/// `asset_id = "xor"` as a 24-bit big-endian integer.
const SCCP_ASSET_ID_XOR = 0x786f72
const SCCP_ASSET_ID_XOR_LEN = 3
/// `route_id = "taira_ton_xor"` as a 104-bit big-endian integer.
const SCCP_ROUTE_ID_TON = 0x74616972615f746f6e5f786f72
const SCCP_ROUTE_ID_TON_LEN = 13
const SCCP_TON_ACCOUNT36_LEN = 36
const SCCP_MAX_TAIRA_ACCOUNT_BYTES = 1024
const SCCP_MAX_PAYLOAD_BYTES = 4096
/// Amounts crossing a TON endpoint are below 2^96 (§3.2).
const SCCP_TON_AMOUNT_LIMIT = 0x1000000000000000000000000

/// Standard snake cells: 127 data bytes per non-final chunk (§5.3.2).
const SCCP_SNAKE_CHUNK_BYTES = 127
const SCCP_SNAKE_CHUNK_BITS = 1016

/// Commitment trees and history (§3.4, §3.5).
const SCCP_MAX_BLOCK_LEAVES = 512
const SCCP_MAX_BLOCK_PATH = 9
const SCCP_MAX_HISTORY_PATH = 32
const SCCP_MAX_HISTORY_SIZE = 0x100000000

/// Rosters (§3.7).
const SCCP_MIN_ROSTER_SIZE = 4
const SCCP_MAX_ROSTER_SIZE = 31
const SCCP_MEMBERS_PER_CHUNK = 6
const SCCP_HASHES_PER_CHUNK = 3
const SCCP_ADDRESS_MASK = 0xffffffffffffffffffffffffffffffffffffffff

/// Contract constants of §3.9.
const SCCP_PREVIOUS_ROSTER_GRACE_MS = 86400000
const SCCP_MAX_ROSTER_VALIDITY_MS = 2592000000
const SCCP_MAX_CLOCK_SKEW_MS = 3600000

/// secp256k1 group order and half order (§0).
const SECP256K1_N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
const SECP256K1_HALF_N = 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0

/// EIP-712 constants (§3.6).
const SCCP_DOMAIN_TYPEHASH = 0x599a80fcaa47b95e2323ab4d34d34e0cc9feda4b843edafcc30c7bdf60ea15bf
const SCCP_NAME_HASH = 0xd7bacbdfe367013f66397ff7242325c31126ab14c95620817c994f22f1d123ba
const SCCP_VERSION_HASH = 0xc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc6
const SCCP_ATTESTATION_TYPEHASH = 0x6ea54d1f320a2e5892d6a362adc4b569967facc991d3a327cbc84b746f520c66

/// Domain tags as big-endian integers (§3.3, §3.4, §3.5, §3.7).
const SCCP_TAG_ROSTER_V1 = 0x534343502f524f535445522f5631
const SCCP_TAG_ROSTER_V1_BITS = 112
const SCCP_TAG_LEAF_V1 = 0x534343502f4c4541462f5631
const SCCP_TAG_LEAF_V1_BITS = 96
const SCCP_TAG_CONTROL_V1 = 0x534343502f434f4e54524f4c2f5631
const SCCP_TAG_CONTROL_V1_BITS = 120
const SCCP_TAG_NODE_V1 = 0x534343502f4e4f44452f5631
const SCCP_TAG_NODE_V1_BITS = 96
const SCCP_TAG_HISTORY_V1 = 0x534343502f484953544f52592f5631
const SCCP_TAG_HISTORY_V1_BITS = 120
const SCCP_TAG_PAYLOAD_V1 = 0x534343502f5041594c4f41442f5631
const SCCP_TAG_PAYLOAD_V1_BITS = 120
const SCCP_TAG_MESSAGE_V1 = 0x534343502f4d4553534147452f5631
const SCCP_TAG_MESSAGE_V1_BITS = 120

/// Consumption buckets hold 512 nonce flags each (§5.3.3).
const SCCP_BUCKET_BITS = 512
const SCCP_BUCKET_SHIFT = 9
const SCCP_BUCKET_MASK = 511
/// Finalize and void deploy at most this many buckets ahead of `deployed_buckets` (§5.3.4).
const SCCP_MAX_DEPLOY_AHEAD = 4
/// Upper bound of one `sccp_deploy_buckets` request.
const SCCP_MAX_DEPLOY_BUCKETS_PER_CALL = 16

/// Consume kinds carried by `ConsumeTail.kind` (§5.3.3).
const SCCP_CONSUME_KIND_MINT = 0
const SCCP_CONSUME_KIND_VOID = 1

/// Jetton metadata (§5.3.1): 9 decimals everywhere.
const SCCP_JETTON_DECIMALS = 9

/// Opcodes that appear in bounced bodies (first 32 bits after `0xFFFFFFFF`).
const SCCP_OP_CONSUME = 0x53434331
const SCCP_OP_CONSUMED = 0x53434332
const SCCP_OP_CONSUME_RANGE = 0x53434333
const SCCP_OP_RANGE_CONSUMED = 0x53434334
const SCCP_OP_INTERNAL_TRANSFER = 0x178d4519
const SCCP_OP_BURN_NOTIFICATION = 0x7bdd97de
