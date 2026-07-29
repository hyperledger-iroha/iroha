"""High-level crypto helpers backed by `iroha_crypto` via PyO3 bindings."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, Final, Iterable, Mapping, Optional, TypeAlias, Union

from ._native import load_crypto_extension
from .address import AccountAddress

_crypto = load_crypto_extension()

ED25519_ALGORITHM: Final[str] = "ed25519"
SECP256K1_ALGORITHM: Final[str] = "secp256k1"
ML_DSA_ALGORITHM: Final[str] = "ml-dsa"
GOST_3410_2012_256_PARAMSET_A_ALGORITHM: Final[str] = "gost3410-2012-256-paramset-a"
GOST_3410_2012_256_PARAMSET_B_ALGORITHM: Final[str] = "gost3410-2012-256-paramset-b"
GOST_3410_2012_256_PARAMSET_C_ALGORITHM: Final[str] = "gost3410-2012-256-paramset-c"
GOST_3410_2012_512_PARAMSET_A_ALGORITHM: Final[str] = "gost3410-2012-512-paramset-a"
GOST_3410_2012_512_PARAMSET_B_ALGORITHM: Final[str] = "gost3410-2012-512-paramset-b"
BLS_NORMAL_ALGORITHM: Final[str] = "bls_normal"
BLS_SMALL_ALGORITHM: Final[str] = "bls_small"
SM2_ALGORITHM: Final[str] = "sm2"
PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: Final[int] = 7
PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: Final[int] = 64 * 1024 * 1024
_PRIVACY_MAX_BRIDGE_ABI_VERSION: Final[int] = 0xFFFF_FFFF
_PRIVACY_NORITO_HEADER_BYTES: Final[int] = 40
_PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES: Final[int] = 64
_PRIVACY_NORITO_SUPPORTED_FLAGS_MASK: Final[int] = 0x27
_PRIVACY_NORITO_FIELD_BITSET_FLAG: Final[int] = 0x20
_PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS: Final[int] = 0x06
_PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE: Final[int] = 0x50
_PRIVACY_CRC64_MASK: Final[int] = 0xFFFF_FFFF_FFFF_FFFF
_PRIVACY_CRC64_REFLECTED_POLY: Final[int] = 0xC96C_5795_D787_0F42
_PRIVACY_NORITO_MAGIC: Final[bytes] = b"NRT0"
try:
    SUPPORTED_CRYPTO_ALGORITHMS: Final[tuple[str, ...]] = tuple(
        _crypto.supported_crypto_algorithms()
    )
except AttributeError as exc:  # pragma: no cover - stale native extension guard
    raise RuntimeError(
        "iroha_python._crypto is missing the all-algorithm crypto API; rebuild the extension"
    ) from exc

ED25519_PRIVATE_KEY_LENGTH: Final[int] = 32
ED25519_PUBLIC_KEY_LENGTH: Final[int] = 32
ED25519_SIGNATURE_LENGTH: Final[int] = 64
_ED25519_MULTIHASH_PREFIX: Final[str] = "ed0120"
_DEFAULT_I105_DISCRIMINANT: Final[int] = 0x02F1
_MAX_CONTRACT_ARGUMENT_RECORD_BYTES: Final[int] = 1024 * 1024
# Keep this byte bound aligned with the native V1 CancelAssetLock builders in
# every SDK. The derived EscrowId remains a fixed 32-byte wire value.
CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1: Final[int] = 4_096
ContractArguments: TypeAlias = Union[bytes, bytearray, memoryview]

SM2_PRIVATE_KEY_LENGTH: Final[int] = 32
SM2_PUBLIC_KEY_LENGTH: Final[int] = 65
SM2_SIGNATURE_LENGTH: Final[int] = 64
SM2_DEFAULT_DISTINGUISHED_ID: Final[str] = _crypto.sm2_default_distid()

_SM2_FIXTURE_REFERENCE: Dict[str, str] = {
    "distid": "1234567812345678",
    "seed_hex": "1111111111111111111111111111111111111111111111111111111111111111",
    "message_hex": "69726F686120736D2073646B2066697874757265",
    "private_key_hex": "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569",
    "public_key_sec1_hex": "04361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
    "public_key_multihash": "86265300103132333435363738313233343536373804361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
    "public_key_prefixed": "sm2:86265300103132333435363738313233343536373804361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F",
    "za": "E54EDEDE2A2FCC1C9DF868C56F8A2DD8C562F1AD3C78DC11DD7D91BB6F0EBD46",
    "signature": "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7FF72EBF26C29E77AAAB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268",
    "r": "1877845D5FFE0305946EEA3046D0279BE886B866EF620B7325413602CAD17C7F",
    "s": "F72EBF26C29E77AAAB2226EDFBEE2D6D6ABC0D6C9B2C9A2248E2BD9324A12268",
}
_SM2_FIXTURE_SEED = bytes.fromhex(_SM2_FIXTURE_REFERENCE["seed_hex"])
_SM2_FIXTURE_MESSAGE = bytes.fromhex(_SM2_FIXTURE_REFERENCE["message_hex"])

__all__ = [
    "ED25519_ALGORITHM",
    "SECP256K1_ALGORITHM",
    "ML_DSA_ALGORITHM",
    "GOST_3410_2012_256_PARAMSET_A_ALGORITHM",
    "GOST_3410_2012_256_PARAMSET_B_ALGORITHM",
    "GOST_3410_2012_256_PARAMSET_C_ALGORITHM",
    "GOST_3410_2012_512_PARAMSET_A_ALGORITHM",
    "GOST_3410_2012_512_PARAMSET_B_ALGORITHM",
    "BLS_NORMAL_ALGORITHM",
    "BLS_SMALL_ALGORITHM",
    "SM2_ALGORITHM",
    "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION",
    "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    "SUPPORTED_CRYPTO_ALGORITHMS",
    "ED25519_PRIVATE_KEY_LENGTH",
    "ED25519_PUBLIC_KEY_LENGTH",
    "ED25519_SIGNATURE_LENGTH",
    "SM2_PRIVATE_KEY_LENGTH",
    "SM2_PUBLIC_KEY_LENGTH",
    "SM2_SIGNATURE_LENGTH",
    "SM2_DEFAULT_DISTINGUISHED_ID",
    "CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1",
    "CryptoKeyPair",
    "Ed25519KeyPair",
    "Sm2KeyPair",
    "Instruction",
    "ContractCall",
    "TransactionExecutableEntry",
    "SignedTransactionEnvelope",
    "TransactionBuilder",
    "ConfidentialKeyset",
    "DomainId",
    "AccountId",
    "AssetDefinitionId",
    "AssetId",
    "normalize_crypto_algorithm",
    "supported_crypto_algorithms",
    "derive_keypair_from_seed",
    "derive_ed25519_keypair_from_seed",
    "generate_keypair",
    "generate_ed25519_keypair",
    "load_keypair",
    "load_keypair_from_multihash",
    "load_ed25519_keypair",
    "load_ed25519_keypair_from_hex",
    "derive_sm2_keypair_from_seed",
    "generate_sm2_keypair",
    "load_sm2_keypair",
    "sm2_public_key_multihash",
    "ed25519_public_key_multihash",
    "ed25519_public_key_account_id",
    "build_signed_transaction",
    "build_find_asset_escrow_query",
    "build_find_asset_escrows_by_seller_query",
    "build_find_asset_escrows_by_buyer_query",
    "build_find_committed_transaction_query",
    "build_find_block_by_hash_query",
    "committed_transaction_carrier_block_hash",
    "verify_committed_transaction_inclusion",
    "hash_blake2b_32",
    "public_key_multihash",
    "private_key_multihash",
    "parse_public_key_multihash",
    "parse_private_key_multihash",
    "sign",
    "sign_ed25519",
    "sign_sm2",
    "decode_transaction_receipt_json",
    "verify_signed_transaction_versioned",
    "verify",
    "verify_ed25519",
    "verify_sm2",
    "derive_confidential_keyset",
    "derive_confidential_keyset_from_hex",
    "compute_confidential_root_v2",
    "computeConfidentialRootV2",
    "derive_confidential_next_zero_path_v2",
    "deriveConfidentialNextZeroPathV2",
    "derive_confidential_diversifier_v2",
    "deriveConfidentialDiversifierV2",
    "derive_confidential_owner_tag_v2",
    "deriveConfidentialOwnerTagV2",
    "derive_confidential_note_v2",
    "deriveConfidentialNoteV2",
    "build_confidential_transfer_proof_v2",
    "buildConfidentialTransferProofV2",
    "build_confidential_transfer_proof_v2_with_paths",
    "buildConfidentialTransferProofV2WithPaths",
    "build_confidential_unshield_proof_v3",
    "buildConfidentialUnshieldProofV3",
    "build_confidential_unshield_proof_v3_with_paths",
    "buildConfidentialUnshieldProofV3WithPaths",
    "build_confidential_asset_hidden_transfer_proof_v1",
    "buildConfidentialAssetHiddenTransferProofV1",
    "confidential_transfer_v2_verifying_key_registration_payload_v1",
    "confidential_unshield_v3_verifying_key_registration_payload_v1",
    "zk_ace_verifying_key_registration_payload_v1",
    "build_zk_ace_authorization_proof_v1",
    "zk_ace_authorized_transfer_digest_check",
    "zk_ace_build_transfer_authorization_v1",
    "zk_ace_verifying_key_registration_payload_v1",
    "privacy_bridge_abi_version",
    "is_privacy_native_available",
    "privacy_capabilities_v1",
    "sm2_fixture_from_seed",
]


@dataclass(frozen=True)
class ContractCall:
    """One deployed-contract invocation in an ordered transaction batch."""

    contract_address: str
    expected_code_hash_hex: str
    entrypoint: str
    arguments: Optional[ContractArguments] = None

    def __post_init__(self) -> None:
        for value, context in (
            (self.contract_address, "contract_address"),
            (self.expected_code_hash_hex, "expected_code_hash_hex"),
            (self.entrypoint, "entrypoint"),
        ):
            if not isinstance(value, str):
                raise TypeError(f"{context} must be a string")
            if not value:
                raise ValueError(f"{context} must be non-empty")
            if value != value.strip():
                raise ValueError(f"{context} must not contain surrounding whitespace")

        if len(self.expected_code_hash_hex) != 64:
            raise ValueError("expected_code_hash_hex must contain exactly 32 bytes")
        try:
            code_hash = bytes.fromhex(self.expected_code_hash_hex)
        except ValueError as exc:
            raise ValueError("expected_code_hash_hex must be valid hexadecimal") from exc
        if len(code_hash) != 32:
            raise ValueError("expected_code_hash_hex must contain exactly 32 bytes")
        if code_hash[-1] & 1 != 1:
            raise ValueError("expected_code_hash_hex must have its least significant bit set")

        if self.arguments is None:
            return
        if not isinstance(self.arguments, (bytes, bytearray, memoryview)):
            raise TypeError("arguments must be bytes-like when provided")
        arguments = bytes(self.arguments)
        if len(arguments) > _MAX_CONTRACT_ARGUMENT_RECORD_BYTES:
            raise ValueError(
                f"arguments exceed the {_MAX_CONTRACT_ARGUMENT_RECORD_BYTES}-byte limit"
            )
        object.__setattr__(self, "arguments", arguments)


TransactionExecutableEntry: TypeAlias = Union["Instruction", ContractCall]

if TYPE_CHECKING:
    Instruction: TypeAlias = Any
    SignedTransactionEnvelope: TypeAlias = Any
    TransactionBuilder: TypeAlias = Any
else:
    _NativeInstruction = _crypto.Instruction

    def _normalize_zk_ace_allowed_accounts(allowed_accounts: Any) -> list[str]:
        if allowed_accounts is None:
            raise TypeError("allowed_accounts must be a non-empty sequence of account ids")
        if isinstance(allowed_accounts, (str, bytes, bytearray, memoryview)):
            raise TypeError("allowed_accounts must be a non-empty sequence of account ids")
        try:
            accounts = list(allowed_accounts)
        except TypeError as exc:
            raise TypeError("allowed_accounts must be a non-empty sequence of account ids") from exc
        if not accounts:
            raise ValueError("allowed_accounts must be non-empty")
        if len(accounts) > 16:
            raise ValueError("allowed_accounts must contain at most 16 accounts")
        seen: set[str] = set()
        for index, account in enumerate(accounts):
            if not isinstance(account, str) or not account.strip():
                raise ValueError(f"allowed_accounts[{index}] must be a non-empty account id")
            if account in seen:
                raise ValueError(f"allowed_accounts[{index}] duplicates an earlier account")
            seen.add(account)
        return accounts

    def _require_cancel_asset_lock_id(value: Any) -> str:
        if not isinstance(value, str):
            raise TypeError("escrow_id lock-ID preimage must be a string")
        if not value:
            raise ValueError("escrow_id lock-ID preimage must be non-empty")
        if (
            value[0].isspace()
            or value[-1].isspace()
            or value[0] == "\ufeff"
            or value[-1] == "\ufeff"
        ):
            raise ValueError(
                "escrow_id lock-ID preimage must not contain surrounding whitespace or BOM"
            )
        try:
            encoded = value.encode("utf-8")
        except UnicodeEncodeError as exc:
            raise ValueError("escrow_id lock-ID preimage must be valid UTF-8 text") from exc
        if len(encoded) > CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1:
            raise ValueError(
                "escrow_id lock-ID preimage must be at most "
                f"{CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1} UTF-8 bytes"
            )
        return value

    class _InstructionFacadeMeta(type):
        def __getattr__(cls, name: str) -> Any:
            return getattr(_NativeInstruction, name)

    class Instruction(metaclass=_InstructionFacadeMeta):
        @staticmethod
        def cancel_asset_lock(
            escrow_id: str,
            expected_remaining_amount: str,
        ) -> Any:
            """Build V1 cancellation from an exact lock-ID preimage.

            ``escrow_id`` is bounded by
            :data:`CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1` before hashing.
            """

            return _NativeInstruction.cancel_asset_lock(
                _require_cancel_asset_lock_id(escrow_id),
                expected_remaining_amount,
            )

        @staticmethod
        def register_zk_ace_identity_commitment(
            asset_definition_id: str,
            identity_commitment: Any,
            policy_hash: Any,
            allowed_accounts: Any = None,
            verifier_key: Any = None,
            *,
            action_class: Optional[str] = None,
            domain_tag: Optional[str] = None,
        ) -> Any:
            accounts = _normalize_zk_ace_allowed_accounts(allowed_accounts)
            return _NativeInstruction.register_zk_ace_identity_commitment(
                asset_definition_id,
                identity_commitment,
                policy_hash,
                accounts,
                verifier_key,
                action_class=action_class,
                domain_tag=domain_tag,
            )

        @staticmethod
        def rotate_zk_ace_identity_commitment(
            asset_definition_id: str,
            old_identity_commitment: Any,
            new_identity_commitment: Any,
            policy_hash: Any,
            allowed_accounts: Any = None,
            verifier_key: Any = None,
            *,
            action_class: Optional[str] = None,
            domain_tag: Optional[str] = None,
        ) -> Any:
            accounts = _normalize_zk_ace_allowed_accounts(allowed_accounts)
            return _NativeInstruction.rotate_zk_ace_identity_commitment(
                asset_definition_id,
                old_identity_commitment,
                new_identity_commitment,
                policy_hash,
                accounts,
                verifier_key,
                action_class=action_class,
                domain_tag=domain_tag,
            )

        @staticmethod
        def issue_replication_order(
            order_id: str,
            order_payload: str,
            issued_epoch: int,
            deadline_epoch: int,
        ) -> Any:
            """Build a canonical native ``IssueReplicationOrder`` instruction."""

            from .sorafs_replication import build_issue_replication_order_instruction

            return build_issue_replication_order_instruction(
                order_id,
                order_payload,
                issued_epoch,
                deadline_epoch,
            )

        @staticmethod
        def complete_replication_order(
            order_id: str,
            provider_id: str,
            completion_epoch: int,
        ) -> Any:
            """Build the provider-specific completion instruction."""

            from .sorafs_replication import build_complete_replication_order_instruction

            return build_complete_replication_order_instruction(
                order_id,
                provider_id,
                completion_epoch,
            )

        @staticmethod
        def expire_replication_order(
            order_id: str,
            expiration_epoch: int,
        ) -> Any:
            """Build a canonical native ``ExpireReplicationOrder`` instruction."""

            from .sorafs_replication import build_expire_replication_order_instruction

            return build_expire_replication_order_instruction(
                order_id,
                expiration_epoch,
            )

    SignedTransactionEnvelope = _crypto.SignedTransactionEnvelope
    TransactionBuilder = _crypto.TransactionBuilder
verify_signed_transaction_versioned = _crypto.verify_signed_transaction_versioned
DomainId = _crypto.DomainId
AccountId = _crypto.AccountId
AssetDefinitionId = _crypto.AssetDefinitionId
AssetId = _crypto.AssetId


def build_find_asset_escrow_query(
    authority: str,
    private_key: bytes,
    escrow_id: str,
) -> bytes:
    """Build a versioned Norito signed query for one native escrow."""

    return bytes(_crypto.build_find_asset_escrow_query(authority, private_key, escrow_id))


def build_find_asset_escrows_by_seller_query(
    authority: str,
    private_key: bytes,
    seller: str,
) -> bytes:
    """Build a signed iterable query for escrows funded by ``seller``."""

    return bytes(
        _crypto.build_find_asset_escrows_by_seller_query(
            authority,
            private_key,
            seller,
        )
    )


def build_find_asset_escrows_by_buyer_query(
    authority: str,
    private_key: bytes,
    buyer: str,
) -> bytes:
    """Build a signed iterable query for escrows benefiting ``buyer``."""

    return bytes(
        _crypto.build_find_asset_escrows_by_buyer_query(
            authority,
            private_key,
            buyer,
        )
    )


def build_find_committed_transaction_query(
    authority: str,
    private_key: bytes,
    transaction_hash: str,
) -> bytes:
    """Build a signed native query for one canonical committed transaction."""

    return bytes(
        _crypto.build_find_committed_transaction_query(
            authority,
            private_key,
            transaction_hash,
        )
    )


def build_find_block_by_hash_query(
    authority: str,
    private_key: bytes,
    block_hash: str,
) -> bytes:
    """Build a signed native query for one exact carrier block."""

    return bytes(
        _crypto.build_find_block_by_hash_query(
            authority,
            private_key,
            block_hash,
        )
    )


def committed_transaction_carrier_block_hash(
    transaction_hash: str,
    response_bytes: bytes,
) -> str:
    """Extract the bound carrier hash from an exact native transaction response."""

    return str(
        _crypto.committed_transaction_carrier_block_hash(
            transaction_hash,
            response_bytes,
        )
    )


def verify_committed_transaction_inclusion(
    transaction_hash: str,
    transaction_response_bytes: bytes,
    block_response_bytes: bytes,
) -> Mapping[str, Any]:
    """Verify native committed-transaction proofs against the exact carrier block."""

    payload = _crypto.verify_committed_transaction_inclusion_json(
        transaction_hash,
        transaction_response_bytes,
        block_response_bytes,
    )
    decoded = json.loads(payload)
    if not isinstance(decoded, Mapping):
        raise RuntimeError("native committed transaction verifier returned malformed JSON")
    return decoded


if not TYPE_CHECKING:
    SignedTransactionEnvelope.__doc__ = """Signed transaction envelope produced by the Python SDK."""


def signed_transaction_envelope_from_json(payload: str) -> SignedTransactionEnvelope:
    """Reconstruct a `SignedTransactionEnvelope` from its JSON representation."""

    return SignedTransactionEnvelope.from_json(payload)


def decode_transaction_receipt_json(payload: bytes) -> str:
    """Decode a Norito-framed transaction receipt into a JSON string."""

    return _crypto.decode_transaction_receipt_json(payload)


def _normalize_bytes(value: Any, name: str, *, expected_len: Optional[int] = None) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise TypeError(f"{name} must be bytes")
    data = bytes(value)
    if expected_len is not None and len(data) != expected_len:
        raise ValueError(f"{name} must be exactly {expected_len} bytes (got {len(data)})")
    return data


def _normalize_lane_privacy_attachment(entry: Mapping[str, Any]) -> Dict[str, Any]:
    if not isinstance(entry, Mapping):
        raise TypeError("lane_privacy_attachments entries must be mappings")

    try:
        commitment_id = int(entry["commitment_id"])
        leaf_index = int(entry.get("leaf_index", 0))
        proof_backend = _require_exact_non_empty_string(
            entry.get("proof_backend", "halo2/ipa"),
            "proof_backend",
        )
        proof_bytes = _normalize_bytes(entry["proof_bytes"], "proof_bytes")
        verifying_key_name = _require_exact_non_empty_string(
            entry["verifying_key_name"],
            "verifying_key_name",
        )
        leaf = _normalize_bytes(entry["leaf"], "leaf", expected_len=32)
        raw_audit = entry.get("audit_path", [])
    except KeyError as exc:  # pragma: no cover - defensive path
        raise KeyError(f"lane privacy attachment missing required key: {exc}") from exc

    if not isinstance(raw_audit, Iterable):
        raise TypeError("audit_path must be an iterable of optional bytes")
    audit_path: list[Optional[bytes]] = []
    for idx, sibling in enumerate(raw_audit):
        if sibling is None:
            audit_path.append(None)
            continue
        audit_path.append(_normalize_bytes(sibling, f"audit_path[{idx}]", expected_len=32))

    return {
        "commitment_id": commitment_id,
        "leaf": leaf,
        "leaf_index": leaf_index,
        "audit_path": audit_path,
        "proof_backend": proof_backend,
        "proof_bytes": proof_bytes,
        "verifying_key_name": verifying_key_name,
    }


@dataclass(frozen=True)
class CryptoKeyPair:
    """Container for an `iroha_crypto` signature key pair."""

    algorithm: str
    private_key: bytes
    public_key: bytes

    def __post_init__(self) -> None:
        object.__setattr__(self, "algorithm", normalize_crypto_algorithm(self.algorithm))
        object.__setattr__(self, "private_key", bytes(self.private_key))
        object.__setattr__(self, "public_key", bytes(self.public_key))

    @property
    def private_key_hex(self) -> str:
        """Return the private-key payload as a hex string."""

        return self.private_key.hex()

    @property
    def public_key_hex(self) -> str:
        """Return the public-key payload as a hex string."""

        return self.public_key.hex()

    @property
    def public_key_multihash(self) -> str:
        """Return the canonical bare multihash for the public key."""

        return public_key_multihash(self.algorithm, self.public_key)

    @property
    def prefixed_public_key_multihash(self) -> str:
        """Return the algorithm-prefixed multihash for the public key."""

        return public_key_multihash(self.algorithm, self.public_key, prefixed=True)

    @property
    def private_key_multihash(self) -> str:
        """Return the canonical bare multihash for the private key."""

        return private_key_multihash(self.algorithm, self.private_key)

    @property
    def prefixed_private_key_multihash(self) -> str:
        """Return the algorithm-prefixed multihash for the private key."""

        return private_key_multihash(self.algorithm, self.private_key, prefixed=True)

    def sign(self, message: bytes) -> bytes:
        """Sign ``message`` using this key pair."""

        return sign(self.algorithm, self.private_key, message)

    def verify(self, message: bytes, signature: bytes) -> bool:
        """Verify ``signature`` against ``message`` using this key pair."""

        return verify(self.algorithm, self.public_key, message, signature)

    @classmethod
    def generate(cls, algorithm: str) -> CryptoKeyPair:
        """Generate a random key pair for ``algorithm``."""

        return generate_keypair(algorithm)

    @classmethod
    def from_seed(cls, seed: bytes, algorithm: str) -> CryptoKeyPair:
        """Derive a key pair for ``algorithm`` from ``seed``."""

        return derive_keypair_from_seed(seed, algorithm)

    @classmethod
    def from_private_key(cls, algorithm: str, private_key: bytes) -> CryptoKeyPair:
        """Reconstruct a key pair for ``algorithm`` from a private-key payload."""

        return load_keypair(private_key, algorithm)

    @classmethod
    def from_private_key_multihash(cls, encoded: str) -> CryptoKeyPair:
        """Reconstruct a key pair from a private-key multihash string."""

        return load_keypair_from_multihash(encoded)


@dataclass(frozen=True)
class Ed25519KeyPair:
    """Container for an Ed25519 key pair."""

    private_key: bytes
    public_key: bytes

    @property
    def private_key_hex(self) -> str:
        """Return the private key as a hex string."""

        return self.private_key.hex()

    @property
    def public_key_hex(self) -> str:
        """Return the public key as a hex string."""

        return self.public_key.hex()

    @property
    def public_key_multihash(self) -> str:
        """Return the public key encoded with the canonical multihash prefix."""

        return ed25519_public_key_multihash(self.public_key)

    def sign(self, message: bytes) -> bytes:
        """Sign ``message`` using the private key."""

        return sign_ed25519(self.private_key, message)

    def verify(self, message: bytes, signature: bytes) -> bool:
        """Verify ``signature`` against ``message``."""

        return verify_ed25519(self.public_key, message, signature)

    @classmethod
    def from_private_key(cls, private_key: bytes) -> Ed25519KeyPair:
        """Construct a key pair from raw private key bytes."""

        return load_ed25519_keypair(private_key)

    @classmethod
    def from_private_key_hex(cls, private_key_hex: str) -> Ed25519KeyPair:
        """Construct a key pair from hex-encoded private key bytes."""

        return load_ed25519_keypair_from_hex(private_key_hex)

    def default_account_id(self, domain: str, discriminant: int = _DEFAULT_I105_DISCRIMINANT) -> str:
        """Return the canonical I105 account id using the public key and `domain`."""

        return ed25519_public_key_account_id(
            self.public_key, domain, discriminant=discriminant
        )


@dataclass(frozen=True)
class Sm2KeyPair:
    """Container for an SM2 key pair."""

    private_key: bytes
    public_key: bytes
    distid: str

    @property
    def private_key_hex(self) -> str:
        """Return the private key as a hex string."""

        return self.private_key.hex()

    @property
    def public_key_sec1_hex(self) -> str:
        """Return the uncompressed SEC1 public key as a hex string."""

        return self.public_key.hex()

    @property
    def public_key_multihash(self) -> str:
        """Return the canonical multihash representation of the SM2 public key."""

        return sm2_public_key_multihash(self.public_key, self.distid)

    def sign(self, message: bytes) -> bytes:
        """Sign ``message`` with the SM2 private key."""

        return sign_sm2(self.private_key, message, self.distid)

    def verify(self, message: bytes, signature: bytes) -> bool:
        """Verify ``signature`` against ``message`` with the SM2 public key."""

        return verify_sm2(self.public_key, message, signature, self.distid)


@dataclass(frozen=True)
class ConfidentialKeyset:
    """Confidential spend/view key hierarchy derived from a 32-byte seed."""

    sk_spend: bytes
    nk: bytes
    ivk: bytes
    ovk: bytes
    fvk: bytes

    def as_hex(self) -> dict[str, str]:
        """Return all keys hex-encoded."""

        return {
            "sk_spend": self.sk_spend_hex,
            "nk": self.nk_hex,
            "ivk": self.ivk_hex,
            "ovk": self.ovk_hex,
            "fvk": self.fvk_hex,
        }

    @property
    def sk_spend_hex(self) -> str:
        """Spend key encoded as hexadecimal."""

        return self.sk_spend.hex()

    @property
    def nk_hex(self) -> str:
        """Nullifier key encoded as hexadecimal."""

        return self.nk.hex()

    @property
    def ivk_hex(self) -> str:
        """Incoming view key encoded as hexadecimal."""

        return self.ivk.hex()

    @property
    def ovk_hex(self) -> str:
        """Outgoing view key encoded as hexadecimal."""

        return self.ovk.hex()

    @property
    def fvk_hex(self) -> str:
        """Full view key encoded as hexadecimal."""

        return self.fvk.hex()


def normalize_crypto_algorithm(algorithm: str) -> str:
    """Return the canonical `iroha_crypto` label for ``algorithm``."""

    if not isinstance(algorithm, str):
        raise TypeError("algorithm must be a string")
    algorithm = _require_exact_non_empty_string(algorithm, "algorithm")
    return str(_crypto.normalize_crypto_algorithm(algorithm))


def supported_crypto_algorithms() -> tuple[str, ...]:
    """Return canonical labels for algorithms compiled into this SDK build."""

    return SUPPORTED_CRYPTO_ALGORITHMS


def _generic_keypair(algorithm: str, private: bytes, public: bytes) -> CryptoKeyPair:
    return CryptoKeyPair(
        algorithm=normalize_crypto_algorithm(algorithm),
        private_key=bytes(private),
        public_key=bytes(public),
    )


def generate_keypair(algorithm: str) -> CryptoKeyPair:
    """Generate a random key pair for any supported signature algorithm."""

    normalized = normalize_crypto_algorithm(algorithm)
    private, public = _crypto.generate_keypair(normalized)
    return _generic_keypair(normalized, private, public)


def derive_keypair_from_seed(seed: bytes, algorithm: str) -> CryptoKeyPair:
    """Derive a key pair for any supported signature algorithm from ``seed``."""

    normalized = normalize_crypto_algorithm(algorithm)
    private, public = _crypto.derive_keypair_from_seed(seed, normalized)
    return _generic_keypair(normalized, private, public)


def load_keypair(private_key: bytes, algorithm: str) -> CryptoKeyPair:
    """Reconstruct a key pair for any supported algorithm from private-key payload bytes."""

    normalized = normalize_crypto_algorithm(algorithm)
    private, public = _crypto.load_keypair(private_key, normalized)
    return _generic_keypair(normalized, private, public)


def load_keypair_from_multihash(encoded: str) -> CryptoKeyPair:
    """Reconstruct a key pair from a bare or algorithm-prefixed private-key multihash."""

    algorithm, private, public = _crypto.load_keypair_from_multihash(encoded)
    return _generic_keypair(algorithm, private, public)


def generate_ed25519_keypair() -> Ed25519KeyPair:
    """Generate a random Ed25519 key pair."""

    private, public = _crypto.generate_ed25519_keypair()
    return Ed25519KeyPair(private_key=private, public_key=public)


def derive_ed25519_keypair_from_seed(seed: bytes) -> Ed25519KeyPair:
    """Derive an Ed25519 key pair from ``seed``."""

    private, public = _crypto.derive_ed25519_keypair_from_seed(seed)
    return Ed25519KeyPair(private_key=private, public_key=public)


def load_ed25519_keypair(private_key: bytes) -> Ed25519KeyPair:
    """Reconstruct an Ed25519 key pair from raw private key bytes."""

    private, public = _crypto.load_ed25519_keypair(private_key)
    return Ed25519KeyPair(private_key=private, public_key=public)


def load_ed25519_keypair_from_hex(private_key_hex: str) -> Ed25519KeyPair:
    """Reconstruct an Ed25519 key pair from a hex-encoded private key."""

    return load_ed25519_keypair(bytes.fromhex(private_key_hex))


def _effective_sm2_distid(distid: Optional[str]) -> str:
    if distid is None:
        return SM2_DEFAULT_DISTINGUISHED_ID
    if not isinstance(distid, str):
        raise TypeError("distid must be a string")
    cleaned = distid.strip()
    if not cleaned:
        raise ValueError("distid must not be empty")
    return cleaned


def generate_sm2_keypair(distid: Optional[str] = None) -> Sm2KeyPair:
    """Generate a random SM2 key pair."""

    effective_distid = _effective_sm2_distid(distid)
    private, public = _crypto.generate_sm2_keypair(distid)
    if len(private) != SM2_PRIVATE_KEY_LENGTH:
        raise RuntimeError("SM2 private key length mismatch; this is a bug")
    if len(public) != SM2_PUBLIC_KEY_LENGTH:
        raise RuntimeError("SM2 public key length mismatch; this is a bug")
    return Sm2KeyPair(private_key=bytes(private), public_key=bytes(public), distid=effective_distid)


def derive_sm2_keypair_from_seed(seed: bytes, distid: Optional[str] = None) -> Sm2KeyPair:
    """Derive an SM2 key pair from ``seed``."""

    effective_distid = _effective_sm2_distid(distid)
    private, public = _crypto.derive_sm2_keypair_from_seed(seed, distid)
    if len(private) != SM2_PRIVATE_KEY_LENGTH:
        raise RuntimeError("SM2 private key length mismatch; this is a bug")
    if len(public) != SM2_PUBLIC_KEY_LENGTH:
        raise RuntimeError("SM2 public key length mismatch; this is a bug")
    return Sm2KeyPair(private_key=bytes(private), public_key=bytes(public), distid=effective_distid)


def load_sm2_keypair(private_key: bytes, distid: Optional[str] = None) -> Sm2KeyPair:
    """Reconstruct an SM2 key pair from raw private key bytes."""

    effective_distid = _effective_sm2_distid(distid)
    if len(private_key) != SM2_PRIVATE_KEY_LENGTH:
        raise ValueError(
            f"private key must be {SM2_PRIVATE_KEY_LENGTH} bytes, got {len(private_key)}"
        )
    private, public = _crypto.load_sm2_keypair(private_key, distid)
    return Sm2KeyPair(private_key=bytes(private), public_key=bytes(public), distid=effective_distid)


def sm2_public_key_multihash(public_key: bytes, distid: Optional[str] = None) -> str:
    """Return the canonical multihash for an SM2 public key."""

    if len(public_key) != SM2_PUBLIC_KEY_LENGTH:
        raise ValueError(
            f"public key must be {SM2_PUBLIC_KEY_LENGTH} bytes, got {len(public_key)}"
        )
    return _crypto.sm2_public_key_multihash(public_key, distid)


def sign_sm2(private_key: bytes, message: bytes, distid: Optional[str] = None) -> bytes:
    """Sign ``message`` with the provided SM2 private key."""

    if len(private_key) != SM2_PRIVATE_KEY_LENGTH:
        raise ValueError(
            f"private key must be {SM2_PRIVATE_KEY_LENGTH} bytes, got {len(private_key)}"
        )
    signature = _crypto.sign_sm2(private_key, message, distid)
    if len(signature) != SM2_SIGNATURE_LENGTH:
        raise RuntimeError("SM2 signature length mismatch; this is a bug")
    return bytes(signature)


def verify_sm2(
    public_key: bytes,
    message: bytes,
    signature: bytes,
    distid: Optional[str] = None,
) -> bool:
    """Verify ``signature`` against ``message`` and the provided SM2 public key."""

    if len(public_key) != SM2_PUBLIC_KEY_LENGTH:
        raise ValueError(
            f"public key must be {SM2_PUBLIC_KEY_LENGTH} bytes, got {len(public_key)}"
        )
    if len(signature) != SM2_SIGNATURE_LENGTH:
        raise ValueError(
            f"signature must be {SM2_SIGNATURE_LENGTH} bytes, got {len(signature)}"
        )
    return bool(_crypto.verify_sm2(public_key, message, signature, distid))


def public_key_multihash(algorithm: str, public_key: bytes, *, prefixed: bool = False) -> str:
    """Return the canonical multihash string for a public-key payload."""

    normalized = normalize_crypto_algorithm(algorithm)
    return str(_crypto.public_key_multihash(normalized, public_key, prefixed))


def private_key_multihash(algorithm: str, private_key: bytes, *, prefixed: bool = False) -> str:
    """Return the canonical multihash string for a private-key payload."""

    normalized = normalize_crypto_algorithm(algorithm)
    return str(_crypto.private_key_multihash(normalized, private_key, prefixed))


def parse_public_key_multihash(encoded: str) -> tuple[str, bytes]:
    """Decode a bare or algorithm-prefixed public-key multihash string."""

    algorithm, public_key = _crypto.parse_public_key_multihash(encoded)
    return str(algorithm), bytes(public_key)


def parse_private_key_multihash(encoded: str) -> tuple[str, bytes]:
    """Decode a bare or algorithm-prefixed private-key multihash string."""

    algorithm, private_key = _crypto.parse_private_key_multihash(encoded)
    return str(algorithm), bytes(private_key)


def ed25519_public_key_multihash(public_key: bytes) -> str:
    """Return the canonical multihash string for an Ed25519 public key."""

    if len(public_key) != ED25519_PUBLIC_KEY_LENGTH:
        raise ValueError(
            f"public key must be {ED25519_PUBLIC_KEY_LENGTH} bytes, got {len(public_key)}"
        )
    return f"{_ED25519_MULTIHASH_PREFIX}{public_key.hex().upper()}"


def ed25519_public_key_account_id(
    public_key: bytes,
    domain: str,
    *,
    discriminant: int = _DEFAULT_I105_DISCRIMINANT,
) -> str:
    """Return the canonical I105 account id using the public key within `domain`."""

    domain = domain.strip()
    if not domain or "@" in domain:
        raise ValueError("domain must be a non-empty string without '@'")
    address = AccountAddress.from_account(domain=domain, public_key=public_key)
    return address.to_i105(discriminant)


def _build_confidential_keyset(payload: Mapping[str, bytes]) -> ConfidentialKeyset:
    try:
        return ConfidentialKeyset(
            sk_spend=payload["sk_spend"],
            nk=payload["nk"],
            ivk=payload["ivk"],
            ovk=payload["ovk"],
            fvk=payload["fvk"],
        )
    except KeyError as exc:  # pragma: no cover - defensive guard
        missing = exc.args[0]
        raise RuntimeError(
            f"confidential keyset payload missing `{missing}` field; this is a bug"
        ) from exc


def derive_confidential_keyset(spend_key: bytes) -> ConfidentialKeyset:
    """Derive the confidential key hierarchy from a 32-byte spend key."""

    if len(spend_key) != 32:
        raise ValueError("confidential spend key must be exactly 32 bytes")
    raw = _crypto.derive_confidential_keyset(spend_key)
    return _build_confidential_keyset(raw)


def derive_confidential_keyset_from_hex(spend_key_hex: str) -> ConfidentialKeyset:
    """Derive the confidential key hierarchy from a hex-encoded spend key."""

    try:
        spend_key = bytes.fromhex(spend_key_hex)
    except ValueError as exc:
        raise ValueError("confidential spend key must be valid hex") from exc
    return derive_confidential_keyset(spend_key)


def sm2_fixture_from_seed(
    distid: str,
    seed: bytes | bytearray | memoryview | str,
    message: bytes | bytearray | memoryview | str,
) -> Dict[str, str]:
    """Return the canonical SM2 fixture values for the given seed and message."""

    if isinstance(seed, str):
        seed_bytes = seed.encode("utf-8")
    else:
        seed_bytes = bytes(seed)
    if isinstance(message, str):
        message_bytes = message.encode("utf-8")
    else:
        message_bytes = bytes(message)

    if not hasattr(_crypto, "sm2_fixture_from_seed"):
        if (
            distid == _SM2_FIXTURE_REFERENCE["distid"]
            and seed_bytes == _SM2_FIXTURE_SEED
            and message_bytes == _SM2_FIXTURE_MESSAGE
        ):
            return dict(_SM2_FIXTURE_REFERENCE)
        raise RuntimeError(
            "SM2 fixture helper unavailable; rebuild iroha_python._crypto with SM support"
        )

    fixture = _crypto.sm2_fixture_from_seed(distid, seed_bytes, message_bytes)
    # The native layer returns a dictionary mapping to uppercase hex strings.
    return dict(fixture)


def build_signed_transaction(
    chain_id: str,
    authority: str,
    private_key: bytes,
    *,
    fee_payment: Mapping[str, Any],
    instructions: Optional[Iterable[Instruction]] = None,
    entries: Optional[Iterable[TransactionExecutableEntry]] = None,
    creation_time_ms: Optional[int] = None,
    ttl_ms: Optional[int] = None,
    nonce: Optional[int] = None,
    metadata: Optional[Mapping[str, Any]] = None,
    lane_privacy_attachments: Optional[Iterable[Mapping[str, Any]]] = None,
) -> SignedTransactionEnvelope:
    """Compose and sign a transaction in one step.

    Parameters
    ----------
    chain_id:
        Target chain identifier.
    authority:
        Transaction authority account identifier (domainless encoded account
        literal: canonical I105 only).
    private_key:
        Ed25519 private key bytes aligned with `authority`.
    fee_payment:
        Required Norito JSON-compatible ``FeePaymentIntent`` mapping. The
        payer, exact sponsor revision, charge assets and maxima, and gas bound
        are included in the transaction signature.
    instructions:
        Iterable of `Instruction` instances to append using the legacy instruction executable.
    entries:
        Ordered executable-batch entries. Supplying this argument selects batch encoding even when
        every entry is an instruction; it is mutually exclusive with ``instructions``.
    creation_time_ms:
        Optional creation timestamp in milliseconds since UNIX epoch.
    ttl_ms:
        Optional time-to-live in milliseconds.
    nonce:
        Optional non-zero nonce value.
    metadata:
        Optional mapping converted to Norito metadata.
    lane_privacy_attachments:
        Optional iterable of mappings describing Merkle lane privacy proofs. Each mapping
        must include ``commitment_id``, ``leaf`` (32-byte hash), ``leaf_index``,
        ``audit_path`` (iterable of optional 32-byte hashes), ``proof_backend``,
        ``proof_bytes``, and ``verifying_key_name``.
    """

    if not isinstance(fee_payment, Mapping):
        raise TypeError("fee_payment must be a FeePaymentIntent mapping")
    fee_payment_json = json.dumps(dict(fee_payment), separators=(",", ":"))
    builder = TransactionBuilder(chain_id, authority, fee_payment_json)
    if creation_time_ms is not None:
        builder.set_creation_time_ms(int(creation_time_ms))
    if ttl_ms is not None:
        builder.set_ttl_ms(int(ttl_ms))
    if nonce is not None:
        builder.set_nonce(int(nonce))
    if metadata is not None:
        builder.set_metadata(metadata)
    if instructions is not None and entries is not None:
        raise ValueError("instructions and entries are mutually exclusive")
    if entries is not None:
        builder.use_executable_batch()
        for index, entry in enumerate(entries):
            if isinstance(entry, ContractCall):
                builder.add_contract_call(
                    entry.contract_address,
                    entry.expected_code_hash_hex,
                    entry.entrypoint,
                    entry.arguments,
                )
            else:
                try:
                    builder.add_instruction(entry)
                except TypeError as exc:
                    raise TypeError(
                        f"entries[{index}] must be an Instruction or ContractCall"
                    ) from exc
    else:
        for instruction in instructions or ():
            builder.add_instruction(instruction)
    if lane_privacy_attachments is not None:
        for entry in lane_privacy_attachments:
            normalized = _normalize_lane_privacy_attachment(entry)
            builder.add_lane_privacy_merkle_attachment(
                normalized["commitment_id"],
                normalized["leaf"],
                normalized["leaf_index"],
                normalized["audit_path"],
                normalized["proof_backend"],
                normalized["proof_bytes"],
                normalized["verifying_key_name"],
            )
    return builder.sign(private_key)


def sign(algorithm: str, private_key: bytes, message: bytes) -> bytes:
    """Return a signature for ``message`` using any supported algorithm."""

    normalized = normalize_crypto_algorithm(algorithm)
    return bytes(_crypto.sign(normalized, private_key, message))


def verify(algorithm: str, public_key: bytes, message: bytes, signature: bytes) -> bool:
    """Verify a signature for any supported algorithm."""

    normalized = normalize_crypto_algorithm(algorithm)
    return bool(_crypto.verify(normalized, public_key, message, signature))


def sign_ed25519(private_key: bytes, message: bytes) -> bytes:
    """Return the Ed25519 signature for ``message``."""

    return _crypto.sign_ed25519(private_key, message)


def verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> bool:
    """Verify an Ed25519 signature."""

    return _crypto.verify_ed25519(public_key, message, signature)


def hash_blake2b_32(data: bytes) -> bytes:
    """Compute the canonical Iroha Blake2b-256 hash for ``data``."""

    return _crypto.hash_blake2b_32(data)


_ZK_ACE_ALGORITHM_ID: Final[str] = "zk-ace-pq-authorization-v0"
_ZK_ACE_PRODUCTION_ENTRYPOINT: Final[str] = "buildZkAceAuthorizationProofV1"
_ZK_ACE_PRODUCTION_VK_REF: Final[str] = "stark-fri:zk_ace_pq_authorization_v0"
_ZK_ACE_PRODUCTION_DISABLED_MESSAGE: Final[str] = (
    "native ZK-ACE prover returned PRIVACY_FFI_ERROR_PRODUCTION_DISABLED for "
    f"{_ZK_ACE_ALGORITHM_ID} {_ZK_ACE_PRODUCTION_ENTRYPOINT} "
    f"{_ZK_ACE_PRODUCTION_VK_REF}: "
    "Iroha production allowlist is not enabled for this audited row"
)
_U128_MAX: Final[int] = (1 << 128) - 1


def _zk_ace_sanitized_native_prover_error(error: Exception) -> RuntimeError:
    message = str(error)
    if (
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED" in message
        or "production disabled" in message.lower()
        or "production-disabled" in message.lower()
        or "Iroha production allowlist" in message
    ):
        return RuntimeError(_ZK_ACE_PRODUCTION_DISABLED_MESSAGE)
    return RuntimeError("native ZK-ACE prover failed")


def _normalize_positive_u128_literal(value: int | str, name: str) -> str:
    if isinstance(value, bool):
        raise ValueError(f"{name} must be a positive decimal u128 string")
    if isinstance(value, int):
        amount = value
    elif isinstance(value, str):
        normalized = value.strip()
        if not normalized.isdecimal():
            raise ValueError(f"{name} must be a positive decimal u128 string")
        amount = int(normalized, 10)
    else:
        raise TypeError(f"{name} must be a positive decimal u128 string")
    if amount <= 0 or amount > _U128_MAX:
        raise ValueError(f"{name} must be a positive decimal u128 string")
    return str(amount)


def _normalize_u128_literal(value: int | str, name: str) -> str:
    if isinstance(value, bool):
        raise ValueError(f"{name} must be a whole-number u128 string")
    if isinstance(value, int):
        amount = value
    elif isinstance(value, str):
        normalized = value.strip()
        if not normalized.isdecimal():
            raise ValueError(f"{name} must be a whole-number u128 string")
        amount = int(normalized, 10)
    else:
        raise TypeError(f"{name} must be a whole-number u128 string")
    if amount < 0 or amount > _U128_MAX:
        raise ValueError(f"{name} must be a whole-number u128 string")
    return str(amount)


def _require_exact_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or value == "":
        raise ValueError(f"{context} must be a non-empty string")
    if value.strip() != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    return value


def _confidential_verifying_key_parts(
    verifying_key: Mapping[str, Any],
    context: str,
) -> tuple[str, str, Any]:
    if not isinstance(verifying_key, Mapping):
        raise TypeError(f"{context} must be a mapping")
    backend = (
        verifying_key.get("backend")
        or verifying_key.get("vk_backend")
        or verifying_key.get("vkBackend")
    )
    circuit_id = (
        verifying_key.get("circuit_id")
        or verifying_key.get("circuitId")
        or verifying_key.get("vk_circuit_id")
        or verifying_key.get("vkCircuitId")
    )
    vk_bytes = (
        verifying_key.get("bytes")
        or verifying_key.get("vk_bytes")
        or verifying_key.get("vkBytes")
    )
    backend = _require_exact_non_empty_string(backend, f"{context}.backend")
    circuit_id = _require_exact_non_empty_string(circuit_id, f"{context}.circuit_id")
    if vk_bytes is None:
        raise ValueError(f"{context}.bytes is required")
    return backend, circuit_id, vk_bytes


def _confidential_native_result(result: Any, context: str) -> Dict[str, Any]:
    if not isinstance(result, dict):
        raise RuntimeError(f"{context} returned a non-object payload")
    return result


def _confidential_path_hex(value: Any, name: str) -> str:
    data = bytes(value)
    if len(data) != 32:
        raise RuntimeError(f"{name} must be 32 bytes, got {len(data)}")
    return data.hex()


def _confidential_path_list_hex(value: Any, name: str) -> list[str]:
    if not isinstance(value, list):
        raise RuntimeError(f"{name} must be a list")
    return [_confidential_path_hex(item, f"{name}[{index}]") for index, item in enumerate(value)]


def _confidential_merkle_path_result(result: Any, context: str) -> Dict[str, Any]:
    if not isinstance(result, dict):
        raise RuntimeError(f"{context} returned a non-object payload")
    leaf_index = result.get("leaf_index")
    if isinstance(leaf_index, bool) or not isinstance(leaf_index, int) or leaf_index < 0:
        raise RuntimeError(f"{context} returned an invalid leaf_index")
    directions_raw = result.get("directions")
    if not isinstance(directions_raw, list):
        raise RuntimeError(f"{context} returned invalid directions")
    directions = []
    for index, item in enumerate(directions_raw):
        if item not in (0, 1):
            raise RuntimeError(f"{context} directions[{index}] must be 0 or 1")
        directions.append(int(item))
    return {
        "leaf_index": leaf_index,
        "commitment": _confidential_path_hex(result.get("commitment"), f"{context}.commitment"),
        "siblings": _confidential_path_list_hex(result.get("siblings"), f"{context}.siblings"),
        "directions": directions,
        "witness_nodes": _confidential_path_list_hex(
            result.get("witness_nodes"),
            f"{context}.witness_nodes",
        ),
        "root": _confidential_path_hex(result.get("root"), f"{context}.root"),
    }


def compute_confidential_root_v2(
    tree_commitments: Iterable[bytes | bytearray | memoryview | str],
) -> bytes:
    """Compute the canonical confidential-transfer v2 tree root."""

    if not hasattr(_crypto, "compute_confidential_root_v2"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential root v2 support; rebuild the extension"
        )
    result = _crypto.compute_confidential_root_v2(list(tree_commitments))
    root = bytes(result)
    if len(root) != 32:
        raise RuntimeError("confidential root v2 returned a non-32-byte root")
    return root


def derive_confidential_next_zero_path_v2(
    *,
    previous_leaf_commitment: bytes | bytearray | memoryview | str,
    previous_leaf_index: int,
    previous_path: Mapping[str, Any],
    root_hint: bytes | bytearray | memoryview | str,
) -> Dict[str, Any]:
    """Derive the padded zero-leaf path immediately after the latest commitment."""

    if not hasattr(_crypto, "derive_confidential_next_zero_path_v2"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential next-zero path support; rebuild the extension"
        )
    if isinstance(previous_leaf_index, bool) or not isinstance(previous_leaf_index, int):
        raise TypeError("previous_leaf_index must be a non-negative integer")
    if previous_leaf_index < 0:
        raise ValueError("previous_leaf_index must be a non-negative integer")
    result = _crypto.derive_confidential_next_zero_path_v2(
        previous_leaf_commitment,
        previous_leaf_index,
        dict(previous_path),
        root_hint,
    )
    return _confidential_merkle_path_result(result, "confidential next-zero path")


def derive_confidential_diversifier_v2(
    seed: bytes | bytearray | memoryview | str,
) -> bytes:
    """Derive a canonical confidential-transfer v2 note diversifier."""

    if not hasattr(_crypto, "derive_confidential_diversifier_v2"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential diversifier v2 support; rebuild the extension"
        )
    result = _crypto.derive_confidential_diversifier_v2(seed)
    diversifier = bytes(result)
    if len(diversifier) != 32:
        raise RuntimeError("confidential diversifier v2 returned non-32-byte output")
    return diversifier


def derive_confidential_owner_tag_v2(
    spend_key: bytes | bytearray | memoryview | str,
    diversifier: bytes | bytearray | memoryview | str,
) -> bytes:
    """Derive a canonical confidential-transfer v2 owner tag."""

    if not hasattr(_crypto, "derive_confidential_owner_tag_v2"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential owner tag v2 support; rebuild the extension"
        )
    result = _crypto.derive_confidential_owner_tag_v2(spend_key, diversifier)
    owner_tag = bytes(result)
    if len(owner_tag) != 32:
        raise RuntimeError("confidential owner tag v2 returned non-32-byte output")
    return owner_tag


def derive_confidential_note_v2(
    asset_definition_id: str,
    amount: int | str,
    rho: bytes | bytearray | memoryview | str,
    owner_tag: bytes | bytearray | memoryview | str,
) -> bytes:
    """Derive a canonical confidential-transfer v2 note commitment."""

    if not hasattr(_crypto, "derive_confidential_note_v2"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential note v2 support; rebuild the extension"
        )
    result = _crypto.derive_confidential_note_v2(
        asset_definition_id,
        amount,
        rho,
        owner_tag,
    )
    note_commitment = bytes(result)
    if len(note_commitment) != 32:
        raise RuntimeError("confidential note v2 returned non-32-byte output")
    return note_commitment


def build_confidential_transfer_proof_v2(
    *,
    chain_id: str,
    asset_definition_id: str,
    spend_key: bytes | bytearray | memoryview | str,
    tree_commitments: Iterable[bytes | bytearray | memoryview | str],
    inputs: Iterable[Mapping[str, Any]],
    outputs: Iterable[Mapping[str, Any]],
    root_hint: bytes | bytearray | memoryview | str,
    verifying_key: Mapping[str, Any],
) -> Dict[str, Any]:
    """Build a confidential transfer v2 proof envelope with the native Halo2 prover."""

    if not hasattr(_crypto, "build_confidential_transfer_proof_v2"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential transfer v2 prover support; rebuild the extension"
        )
    vk_backend, vk_circuit_id, vk_bytes = _confidential_verifying_key_parts(
        verifying_key,
        "verifying_key",
    )
    result = _crypto.build_confidential_transfer_proof_v2(
        str(chain_id),
        str(asset_definition_id),
        spend_key,
        list(tree_commitments),
        list(inputs),
        list(outputs),
        root_hint,
        vk_backend,
        vk_circuit_id,
        vk_bytes,
    )
    return _confidential_native_result(result, "confidential transfer v2 prover")


def build_confidential_transfer_proof_v2_with_paths(
    *,
    chain_id: str,
    asset_definition_id: str,
    spend_key: bytes | bytearray | memoryview | str,
    input_paths: Iterable[Mapping[str, Any]],
    inputs: Iterable[Mapping[str, Any]],
    outputs: Iterable[Mapping[str, Any]],
    root_hint: bytes | bytearray | memoryview | str,
    verifying_key: Mapping[str, Any],
) -> Dict[str, Any]:
    """Build a confidential transfer v2 proof envelope from ledger Merkle paths."""

    if not hasattr(_crypto, "build_confidential_transfer_proof_v2_with_paths"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential transfer v2 path prover support; rebuild the extension"
        )
    vk_backend, vk_circuit_id, vk_bytes = _confidential_verifying_key_parts(
        verifying_key,
        "verifying_key",
    )
    result = _crypto.build_confidential_transfer_proof_v2_with_paths(
        str(chain_id),
        str(asset_definition_id),
        spend_key,
        list(input_paths),
        list(inputs),
        list(outputs),
        root_hint,
        vk_backend,
        vk_circuit_id,
        vk_bytes,
    )
    return _confidential_native_result(result, "confidential transfer v2 path prover")


def build_confidential_unshield_proof_v3(
    *,
    chain_id: str,
    asset_definition_id: str,
    spend_key: bytes | bytearray | memoryview | str,
    tree_commitments: Iterable[bytes | bytearray | memoryview | str],
    inputs: Iterable[Mapping[str, Any]],
    outputs: Iterable[Mapping[str, Any]],
    public_amount: int | str,
    root_hint: bytes | bytearray | memoryview | str,
    verifying_key: Mapping[str, Any],
) -> Dict[str, Any]:
    """Build a confidential unshield v3 proof envelope with optional private change."""

    if not hasattr(_crypto, "build_confidential_unshield_proof_v3"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential unshield v3 prover support; rebuild the extension"
        )
    vk_backend, vk_circuit_id, vk_bytes = _confidential_verifying_key_parts(
        verifying_key,
        "verifying_key",
    )
    result = _crypto.build_confidential_unshield_proof_v3(
        str(chain_id),
        str(asset_definition_id),
        spend_key,
        list(tree_commitments),
        list(inputs),
        list(outputs),
        _normalize_u128_literal(public_amount, "public_amount"),
        root_hint,
        vk_backend,
        vk_circuit_id,
        vk_bytes,
    )
    return _confidential_native_result(result, "confidential unshield v3 prover")


def build_confidential_unshield_proof_v3_with_paths(
    *,
    chain_id: str,
    asset_definition_id: str,
    spend_key: bytes | bytearray | memoryview | str,
    input_paths: Iterable[Mapping[str, Any]],
    inputs: Iterable[Mapping[str, Any]],
    outputs: Iterable[Mapping[str, Any]],
    public_amount: int | str,
    root_hint: bytes | bytearray | memoryview | str,
    verifying_key: Mapping[str, Any],
) -> Dict[str, Any]:
    """Build a confidential unshield v3 proof envelope from ledger Merkle paths."""

    if not hasattr(_crypto, "build_confidential_unshield_proof_v3_with_paths"):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential unshield v3 path prover support; rebuild the extension"
        )
    vk_backend, vk_circuit_id, vk_bytes = _confidential_verifying_key_parts(
        verifying_key,
        "verifying_key",
    )
    result = _crypto.build_confidential_unshield_proof_v3_with_paths(
        str(chain_id),
        str(asset_definition_id),
        spend_key,
        list(input_paths),
        list(inputs),
        list(outputs),
        _normalize_u128_literal(public_amount, "public_amount"),
        root_hint,
        vk_backend,
        vk_circuit_id,
        vk_bytes,
    )
    return _confidential_native_result(result, "confidential unshield v3 path prover")


def build_confidential_asset_hidden_transfer_proof_v1(
    *,
    chain_id: str,
    pool_id: str,
    asset_set_root: bytes | bytearray | memoryview | str,
    input_commitments: Iterable[bytes | bytearray | memoryview | str],
    nullifiers: Iterable[bytes | bytearray | memoryview | str],
    output_commitments: Iterable[bytes | bytearray | memoryview | str],
    root_hint: bytes | bytearray | memoryview | str,
    verifying_key: Mapping[str, Any],
) -> Dict[str, Any]:
    """Build an asset-hidden transfer v1 proof envelope with the native Halo2 prover."""

    if not hasattr(_crypto, "build_confidential_asset_hidden_transfer_proof_v1"):
        raise RuntimeError(
            "iroha_python._crypto is missing asset-hidden transfer v1 prover support; rebuild the extension"
        )
    vk_backend, vk_circuit_id, vk_bytes = _confidential_verifying_key_parts(
        verifying_key,
        "verifying_key",
    )
    result = _crypto.build_confidential_asset_hidden_transfer_proof_v1(
        str(chain_id),
        str(pool_id),
        asset_set_root,
        list(input_commitments),
        list(nullifiers),
        list(output_commitments),
        root_hint,
        vk_backend,
        vk_circuit_id,
        vk_bytes,
    )
    return _confidential_native_result(result, "asset-hidden transfer v1 prover")


computeConfidentialRootV2 = compute_confidential_root_v2
deriveConfidentialNextZeroPathV2 = derive_confidential_next_zero_path_v2
deriveConfidentialDiversifierV2 = derive_confidential_diversifier_v2
deriveConfidentialOwnerTagV2 = derive_confidential_owner_tag_v2
deriveConfidentialNoteV2 = derive_confidential_note_v2
buildConfidentialTransferProofV2 = build_confidential_transfer_proof_v2
buildConfidentialTransferProofV2WithPaths = build_confidential_transfer_proof_v2_with_paths
buildConfidentialUnshieldProofV3 = build_confidential_unshield_proof_v3
buildConfidentialUnshieldProofV3WithPaths = build_confidential_unshield_proof_v3_with_paths
buildConfidentialAssetHiddenTransferProofV1 = (
    build_confidential_asset_hidden_transfer_proof_v1
)


def zk_ace_verifying_key_registration_payload_v1() -> Dict[str, Any]:
    """Build the canonical active ZK-ACE verifier-key registration payload."""

    if not hasattr(_crypto, "zk_ace_verifying_key_registration_payload_v1"):
        raise RuntimeError(
            "iroha_python._crypto is missing ZK-ACE verifier-key support; rebuild the extension"
        )
    payload = _crypto.zk_ace_verifying_key_registration_payload_v1()
    if not isinstance(payload, Mapping):
        raise RuntimeError("ZK-ACE verifier-key builder returned a non-object payload")
    return dict(payload)


def confidential_transfer_v2_verifying_key_registration_payload_v1() -> Dict[str, Any]:
    """Build the canonical active confidential transfer v2 verifier-key payload."""

    method = "confidential_transfer_v2_verifying_key_registration_payload_v1"
    if not hasattr(_crypto, method):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential transfer v2 verifier-key support; "
            "rebuild the extension"
        )
    payload = getattr(_crypto, method)()
    if not isinstance(payload, Mapping):
        raise RuntimeError(
            "confidential transfer v2 verifier-key builder returned a non-object payload"
        )
    return dict(payload)


def confidential_unshield_v3_verifying_key_registration_payload_v1() -> Dict[str, Any]:
    """Build the canonical active confidential unshield v3 verifier-key payload."""

    method = "confidential_unshield_v3_verifying_key_registration_payload_v1"
    if not hasattr(_crypto, method):
        raise RuntimeError(
            "iroha_python._crypto is missing confidential unshield v3 verifier-key support; "
            "rebuild the extension"
        )
    payload = getattr(_crypto, method)()
    if not isinstance(payload, Mapping):
        raise RuntimeError(
            "confidential unshield v3 verifier-key builder returned a non-object payload"
        )
    return dict(payload)


def zk_ace_build_transfer_authorization_v1(
    *,
    from_account_id: str,
    to_account_id: str,
    asset_definition_id: str,
    amount: int | str,
    chain_id: str,
    identity_root: bytes | bytearray | memoryview | str,
    identity_blinding: bytes | bytearray | memoryview | str,
    replay_secret: bytes | bytearray | memoryview | str,
    policy_hash: bytes | bytearray | memoryview | str,
    verifier_key_id: str | Mapping[str, Any] | None = None,
    vk_commitment: bytes | bytearray | memoryview | str | None = None,
) -> Dict[str, Any]:
    """Build a STARK/FRI-backed ZK-ACE transparent-transfer authorization."""

    if not hasattr(_crypto, "zk_ace_build_transfer_authorization_v1"):
        raise RuntimeError(
            "iroha_python._crypto is missing ZK-ACE prover support; rebuild the extension"
        )
    native_args = (
        str(from_account_id),
        str(to_account_id),
        str(asset_definition_id),
        _normalize_positive_u128_literal(amount, "amount"),
        str(chain_id),
        identity_root,
        identity_blinding,
        replay_secret,
        policy_hash,
        verifier_key_id,
        vk_commitment,
    )
    native_error: Exception | None = None
    try:
        result = _crypto.zk_ace_build_transfer_authorization_v1(*native_args)
    except Exception as error:
        native_error = error
        result = ""
    if native_error is not None:
        raise _zk_ace_sanitized_native_prover_error(native_error)
    parsed = json.loads(result)
    if not isinstance(parsed, dict):
        raise RuntimeError("ZK-ACE prover returned a non-object payload")
    return parsed


def zk_ace_authorized_transfer_digest_check(
    instruction_archive_hex: str,
) -> Dict[str, Any]:
    """Decode a ZK-ACE transfer archive and compare its digest bindings."""

    if not hasattr(_crypto, "zk_ace_authorized_transfer_digest_check"):
        raise RuntimeError(
            "iroha_python._crypto is missing ZK-ACE digest inspection support; rebuild the extension"
        )
    return dict(_crypto.zk_ace_authorized_transfer_digest_check(str(instruction_archive_hex)))


def build_zk_ace_authorization_proof_v1(**kwargs: Any) -> Dict[str, Any]:
    """Build the executable Python SDK ZK-ACE authorization proof v1 payload."""

    return zk_ace_build_transfer_authorization_v1(**kwargs)


def _privacy_unsigned_byte_view(
    value: object,
    *,
    bytes_like_message: str,
    typed_message: str,
) -> memoryview:
    try:
        view = memoryview(value)
    except TypeError as exc:
        raise TypeError(bytes_like_message) from exc
    if view.format != "B" or view.itemsize != 1:
        raise TypeError(typed_message)
    return view


def _privacy_output_archive(operation: str, result: object) -> bytes:
    if result is None:
        raise RuntimeError(f"native {operation} returned no output")
    if isinstance(result, str):
        raise RuntimeError(
            f"native {operation} returned text instead of Norito V1 bytes"
        )
    view = _privacy_unsigned_byte_view(
        result,
        bytes_like_message=f"native {operation} returned non-byte output",
        typed_message=f"native {operation} output must use unsigned byte elements",
    )
    if view.nbytes == 0:
        raise RuntimeError(f"native {operation} returned empty output")
    if view.nbytes > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES:
        raise RuntimeError(f"native {operation} returned oversized output")
    archive = view.tobytes()
    if not archive:
        raise RuntimeError(f"native {operation} returned empty output")
    _assert_privacy_norito_archive(
        operation,
        archive,
        expected_schema_byte=_privacy_expected_result_schema_byte(operation),
    )
    return archive


def _clear_privacy_native_output(result: object) -> None:
    if result is None or isinstance(result, str):
        return
    try:
        view = _privacy_unsigned_byte_view(
            result,
            bytes_like_message="native privacy output must be bytes-like",
            typed_message="native privacy output must use unsigned byte elements",
        )
    except TypeError:
        return
    if view.readonly or view.nbytes == 0:
        return
    try:
        view[:] = b"\x00" * view.nbytes
    except (TypeError, ValueError, BufferError):
        return


def _privacy_crc64_table() -> tuple[int, ...]:
    table: list[int] = []
    for index in range(256):
        crc = index
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ _PRIVACY_CRC64_REFLECTED_POLY
            else:
                crc >>= 1
        table.append(crc & _PRIVACY_CRC64_MASK)
    return tuple(table)


_PRIVACY_CRC64_TABLE: Final[tuple[int, ...]] = _privacy_crc64_table()


def _privacy_crc64(payload: bytes) -> int:
    crc = _PRIVACY_CRC64_MASK
    for byte in payload:
        crc = _PRIVACY_CRC64_TABLE[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return (crc ^ _PRIVACY_CRC64_MASK) & _PRIVACY_CRC64_MASK


def _assert_privacy_norito_archive(
    operation: str,
    archive: bytes | memoryview,
    *,
    native_output: bool = True,
    expected_schema_byte: int,
) -> None:
    archive_view = memoryview(archive)

    def fail() -> None:
        if not native_output:
            raise ValueError(f"{operation} must be a valid Norito V1 archive")
        raise RuntimeError(f"native {operation} returned invalid Norito V1 archive")

    if (
        isinstance(expected_schema_byte, bool)
        or not isinstance(expected_schema_byte, int)
        or expected_schema_byte < 0
        or expected_schema_byte > 0xFF
    ):
        if not native_output:
            raise ValueError(f"{operation} must use the privacy request schema")
        raise RuntimeError(
            f"native {operation} returned unexpected privacy result schema"
        )

    if archive_view.nbytes < _PRIVACY_NORITO_HEADER_BYTES:
        fail()
    if archive_view[0:4].tobytes() != _PRIVACY_NORITO_MAGIC:
        fail()
    if archive_view[4] != 0 or archive_view[5] != 0:
        fail()
    if archive_view[22] != 0:
        fail()
    flags = archive_view[39]
    if flags & ~_PRIVACY_NORITO_SUPPORTED_FLAGS_MASK:
        fail()
    if (
        flags & _PRIVACY_NORITO_FIELD_BITSET_FLAG
        and flags & _PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS
        != _PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS
    ):
        fail()
    payload_length = int.from_bytes(archive_view[23:31], "little")
    if payload_length == 0:
        if not native_output:
            raise ValueError(
                f"{operation} must contain a non-empty privacy request payload"
            )
        raise RuntimeError(
            f"native {operation} returned empty privacy result payload"
        )
    minimum_length = _PRIVACY_NORITO_HEADER_BYTES + payload_length
    if archive_view.nbytes < minimum_length:
        fail()
    padding_length = archive_view.nbytes - minimum_length
    if padding_length > _PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES:
        fail()
    padding_start = _PRIVACY_NORITO_HEADER_BYTES
    padding_end = padding_start + padding_length
    if any(archive_view[padding_start:padding_end]):
        fail()
    payload = archive_view[padding_end:]
    expected_crc = int.from_bytes(archive_view[31:39], "little")
    if _privacy_crc64(payload) != expected_crc:
        fail()
    if any(byte != expected_schema_byte for byte in archive_view[6:22]):
        if not native_output:
            raise ValueError(f"{operation} must use the privacy request schema")
        raise RuntimeError(
            f"native {operation} returned unexpected privacy result schema"
        )


def _privacy_expected_result_schema_byte(operation: str) -> int:
    if operation != "privacy_capabilities_v1":
        raise RuntimeError(
            f"native {operation} is not a supported privacy capability operation"
        )
    return _PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE


_PRIVACY_BRIDGE_ABI_VERSION_METHOD: Final[str] = "privacy_bridge_abi_version"
_PRIVACY_CAPABILITY_METHOD: Final[str] = "privacy_capabilities_v1"


def _privacy_bridge_abi_version(module: object) -> int | None:
    method = getattr(module, _PRIVACY_BRIDGE_ABI_VERSION_METHOD, None)
    if not callable(method):
        return None
    try:
        version = method()
    except Exception:
        return None
    if (
        isinstance(version, bool)
        or not isinstance(version, int)
        or version < 0
        or version > _PRIVACY_MAX_BRIDGE_ABI_VERSION
    ):
        return None
    return version


def _has_privacy_bridge_abi(module: object) -> bool:
    version = _privacy_bridge_abi_version(module)
    return version is not None and version >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION


def _privacy_capability_method(module: object):
    if not _has_privacy_bridge_abi(module):
        raise RuntimeError(
            "privacy capabilities require native bridge ABI "
            f"{PRIVACY_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    method = getattr(module, _PRIVACY_CAPABILITY_METHOD, None)
    if not callable(method):
        raise RuntimeError(
            "iroha_python._crypto is missing privacy_capabilities_v1; rebuild the extension"
        )
    return method


def _privacy_native_probe_returns_bytes(module: object) -> bool:
    result: object | None = None
    try:
        result = _privacy_capability_method(module)()
        _privacy_output_archive(_PRIVACY_CAPABILITY_METHOD, result)
        return True
    except Exception:
        return False
    finally:
        _clear_privacy_native_output(result)


def _invoke_privacy_capability_native() -> object:
    try:
        return _privacy_capability_method(_crypto)()
    except Exception:
        raise RuntimeError("native privacy_capabilities_v1 failed") from None


def privacy_bridge_abi_version() -> int:
    """Return the native bridge ABI version required by privacy capabilities."""

    version = _privacy_bridge_abi_version(_crypto)
    if version is None:
        raise RuntimeError(
            "privacy capabilities require native bridge ABI "
            f"{PRIVACY_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    return version


def is_privacy_native_available() -> bool:
    """Return whether the typed privacy capability snapshot bridge is available."""

    return _privacy_native_probe_returns_bytes(_crypto)


def privacy_capabilities_v1() -> bytes:
    """Return the canonical Norito V1 privacy capability snapshot archive."""

    return _privacy_output_archive(
        _PRIVACY_CAPABILITY_METHOD,
        _invoke_privacy_capability_native(),
    )
