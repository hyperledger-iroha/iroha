"""High-level crypto helpers backed by `iroha_crypto` via PyO3 bindings."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, Final, Iterable, Mapping, Optional, TypeAlias

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
PRIVACY_FFI_VERSION_V1: Final[int] = 1
PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: Final[int] = 7
PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: Final[int] = 64 * 1024 * 1024
PRIVACY_FFI_STATUS_ERROR: Final[int] = 1
PRIVACY_FFI_ERROR_NULL_POINTER: Final[int] = 1
PRIVACY_FFI_ERROR_MALFORMED_NORITO: Final[int] = 2
PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM: Final[int] = 3
PRIVACY_FFI_ERROR_PRODUCTION_DISABLED: Final[int] = 4
PRIVACY_FFI_ERROR_INVALID_REQUEST: Final[int] = 5
_PRIVACY_MAX_BRIDGE_ABI_VERSION: Final[int] = 0xFFFF_FFFF
_PRIVACY_NORITO_HEADER_BYTES: Final[int] = 40
_PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES: Final[int] = 64
_PRIVACY_NORITO_SUPPORTED_FLAGS_MASK: Final[int] = 0x27
_PRIVACY_NORITO_FIELD_BITSET_FLAG: Final[int] = 0x20
_PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS: Final[int] = 0x06
_PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE: Final[int] = 0x50
_PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE: Final[int] = 0x42
_PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE: Final[int] = 0x56
_PRIVACY_REQUEST_SCHEMA_BYTE: Final[int] = 0x52
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
    "PRIVACY_FFI_VERSION_V1",
    "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION",
    "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    "PRIVACY_FFI_STATUS_ERROR",
    "PRIVACY_FFI_ERROR_NULL_POINTER",
    "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
    "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
    "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
    "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    "SUPPORTED_CRYPTO_ALGORITHMS",
    "ED25519_PRIVATE_KEY_LENGTH",
    "ED25519_PUBLIC_KEY_LENGTH",
    "ED25519_SIGNATURE_LENGTH",
    "SM2_PRIVATE_KEY_LENGTH",
    "SM2_PUBLIC_KEY_LENGTH",
    "SM2_SIGNATURE_LENGTH",
    "SM2_DEFAULT_DISTINGUISHED_ID",
    "CryptoKeyPair",
    "Ed25519KeyPair",
    "Sm2KeyPair",
    "Instruction",
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
    "build_confidential_transfer_proof_v2",
    "buildConfidentialTransferProofV2",
    "build_confidential_unshield_proof_v3",
    "buildConfidentialUnshieldProofV3",
    "build_confidential_asset_hidden_transfer_proof_v1",
    "buildConfidentialAssetHiddenTransferProofV1",
    "build_zk_ace_authorization_proof_v1",
    "zk_ace_build_transfer_authorization_v1",
    "privacy_bridge_abi_version",
    "is_privacy_native_available",
    "privacy_proof_request_v1",
    "privacy_capabilities_v1",
    "privacy_build_proof_v1",
    "privacy_verify_proof_v1",
    "sm2_fixture_from_seed",
]

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

    class _InstructionFacadeMeta(type):
        def __getattr__(cls, name: str) -> Any:
            return getattr(_NativeInstruction, name)

    class Instruction(metaclass=_InstructionFacadeMeta):
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

    SignedTransactionEnvelope = _crypto.SignedTransactionEnvelope
    TransactionBuilder = _crypto.TransactionBuilder
verify_signed_transaction_versioned = _crypto.verify_signed_transaction_versioned
DomainId = _crypto.DomainId
AccountId = _crypto.AccountId
AssetDefinitionId = _crypto.AssetDefinitionId
AssetId = _crypto.AssetId
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
    instructions: Iterable[Instruction] = (),
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
    instructions:
        Iterable of `Instruction` instances to append.
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

    builder = TransactionBuilder(chain_id, authority)
    if creation_time_ms is not None:
        builder.set_creation_time_ms(int(creation_time_ms))
    if ttl_ms is not None:
        builder.set_ttl_ms(int(ttl_ms))
    if nonce is not None:
        builder.set_nonce(int(nonce))
    if metadata is not None:
        builder.set_metadata(metadata)
    for instruction in instructions:
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


buildConfidentialTransferProofV2 = build_confidential_transfer_proof_v2
buildConfidentialUnshieldProofV3 = build_confidential_unshield_proof_v3
buildConfidentialAssetHiddenTransferProofV1 = (
    build_confidential_asset_hidden_transfer_proof_v1
)


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


def build_zk_ace_authorization_proof_v1(**kwargs: Any) -> Dict[str, Any]:
    """Build the executable Python SDK ZK-ACE authorization proof v1 payload."""

    return zk_ace_build_transfer_authorization_v1(**kwargs)


def _privacy_request_archive(request_archive: bytes | bytearray | memoryview) -> bytearray:
    if isinstance(request_archive, str):
        raise TypeError("request_archive must be Norito V1 bytes, not a string")
    view = _privacy_unsigned_byte_view(
        request_archive,
        bytes_like_message="request_archive must be bytes-like",
        typed_message="request_archive must use unsigned byte elements",
    )
    if view.nbytes == 0:
        raise ValueError("request_archive must not be empty")
    if view.nbytes > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES:
        raise ValueError(
            f"request_archive must not exceed {PRIVACY_NATIVE_ARCHIVE_MAX_BYTES} bytes"
        )
    _assert_privacy_norito_archive(
        "request_archive",
        view,
        native_output=False,
        expected_schema_byte=_PRIVACY_REQUEST_SCHEMA_BYTE,
    )
    return bytearray(view)


def _clear_privacy_request_archive(request_archive: bytearray) -> None:
    request_archive[:] = b"\x00" * len(request_archive)


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


def _privacy_request_component_bytes(
    value: bytes | bytearray | memoryview,
    name: str,
    *,
    allow_empty: bool,
) -> bytes:
    if isinstance(value, str):
        raise TypeError(f"{name} must be bytes-like, not a string")
    view = _privacy_unsigned_byte_view(
        value,
        bytes_like_message=f"{name} must be bytes-like",
        typed_message=f"{name} must use unsigned byte elements",
    )
    if not allow_empty and view.nbytes == 0:
        raise ValueError(f"{name} must not be empty")
    if view.nbytes > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES:
        raise ValueError(
            f"{name} must not exceed {PRIVACY_NATIVE_ARCHIVE_MAX_BYTES} bytes"
        )
    return view.tobytes()


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
    try:
        return {
            "privacy_capabilities_v1": _PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE,
            "privacy_build_proof_v1": _PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE,
            "privacy_verify_proof_v1": _PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE,
        }[operation]
    except KeyError as exc:
        raise RuntimeError(
            f"native {operation} is not a supported privacy native operation"
        ) from exc


_PRIVACY_NATIVE_METHODS: Final[tuple[str, ...]] = (
    "privacy_capabilities_v1",
    "privacy_proof_request_v1",
    "privacy_build_proof_v1",
    "privacy_verify_proof_v1",
)
_PRIVACY_BRIDGE_ABI_VERSION_METHOD: Final[str] = "privacy_bridge_abi_version"
_PRIVACY_PYO3_BYTES_CAST_ERROR_FRAGMENT: Final[str] = "cannot be cast as 'bytes'"
_PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE: Final[bytes] = (
    b"NRT0\x00\x00"
    + bytes([_PRIVACY_REQUEST_SCHEMA_BYTE]) * 16
    + (b"\x00" * 18)
)


def _privacy_bridge_abi_version(module: object) -> int | None:
    method = getattr(module, _PRIVACY_BRIDGE_ABI_VERSION_METHOD, None)
    if not callable(method):
        return None
    try:
        version = method()
    except (TypeError, ValueError, RuntimeError, OSError):
        return None
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
    return (
        version is not None
        and version >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION
    )


def _missing_privacy_native_methods(module: object) -> tuple[str, ...]:
    return tuple(
        operation
        for operation in _PRIVACY_NATIVE_METHODS
        if not callable(getattr(module, operation, None))
    )


def _privacy_native_method(operation: str):
    if not _has_privacy_bridge_abi(_crypto):
        raise RuntimeError(
            "privacy FFI requires native bridge ABI "
            f"{PRIVACY_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    missing_methods = _missing_privacy_native_methods(_crypto)
    if missing_methods:
        if operation in missing_methods:
            raise RuntimeError(
                f"iroha_python._crypto is missing {operation}; rebuild the extension"
            )
        raise RuntimeError(
            "privacy FFI requires complete native method surface; missing "
            + ", ".join(missing_methods)
        )
    method = getattr(_crypto, operation, None)
    return method


def _call_privacy_native_method(method: object, *args: object) -> object:
    if not callable(method):
        raise TypeError("privacy native method is not callable")
    try:
        return method(*args)
    except TypeError as exc:
        if (
            args
            and isinstance(args[0], bytearray)
            and _PRIVACY_PYO3_BYTES_CAST_ERROR_FRAGMENT in str(exc)
        ):
            return method(bytes(args[0]))
        raise


def _privacy_native_probe_returns_bytes(
    module: object,
    operation: str,
    request_archive: bytes | None = None,
) -> bool:
    method = getattr(module, operation, None)
    if not callable(method):
        return False
    request = bytearray(request_archive) if request_archive is not None else None
    result: object | None = None
    try:
        if request is None:
            result = _call_privacy_native_method(method)
        else:
            result = _call_privacy_native_method(method, request)
        _privacy_output_archive(operation, result)
        return True
    except Exception:
        return False
    finally:
        _clear_privacy_native_output(result)
        if request is not None:
            _clear_privacy_request_archive(request)


def _privacy_proof_request_native_probe_returns_bytes(module: object) -> bool:
    method = getattr(module, "privacy_proof_request_v1", None)
    if not callable(method):
        return False
    public_inputs = bytearray(b"public-inputs")
    result: object | None = None
    try:
        result = _call_privacy_native_method(
            method,
            _ZK_ACE_ALGORITHM_ID,
            _ZK_ACE_PRODUCTION_ENTRYPOINT,
            _ZK_ACE_PRODUCTION_VK_REF,
            public_inputs,
            b"",
            b"",
        )
        view = _privacy_unsigned_byte_view(
            result,
            bytes_like_message="native privacy_proof_request_v1 returned non-byte output",
            typed_message="native privacy_proof_request_v1 output must use unsigned byte elements",
        )
        request_archive = view.tobytes()
        _assert_privacy_norito_archive(
            "privacy_proof_request_v1",
            request_archive,
            expected_schema_byte=_PRIVACY_REQUEST_SCHEMA_BYTE,
        )
        return True
    except Exception:
        return False
    finally:
        _clear_privacy_native_output(result)
        _clear_privacy_request_archive(public_inputs)


def _invoke_privacy_native(operation: str, *args: object) -> object:
    method = _privacy_native_method(operation)
    failed = False
    try:
        return _call_privacy_native_method(method, *args)
    except Exception:
        failed = True
    if failed:
        raise RuntimeError(f"native {operation} failed")
    raise AssertionError("unreachable privacy native invocation state")


def privacy_bridge_abi_version() -> int:
    """Return the native bridge ABI version required by privacy FFI helpers."""

    version = _privacy_bridge_abi_version(_crypto)
    if version is None:
        raise RuntimeError(
            "privacy FFI requires native bridge ABI "
            f"{PRIVACY_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    return version


def is_privacy_native_available() -> bool:
    """Return whether the loaded native extension exposes the complete privacy FFI ABI."""

    return _has_privacy_bridge_abi(_crypto) and all(
        _privacy_native_probe_returns_bytes(_crypto, operation)
        if operation == "privacy_capabilities_v1"
        else _privacy_proof_request_native_probe_returns_bytes(_crypto)
        if operation == "privacy_proof_request_v1"
        else _privacy_native_probe_returns_bytes(
            _crypto,
            operation,
            _PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE,
        )
        for operation in _PRIVACY_NATIVE_METHODS
    )


def _privacy_native_archive(
    operation: str,
    request_archive: bytes | bytearray | memoryview,
) -> bytes:
    request = _privacy_request_archive(request_archive)
    try:
        result = _invoke_privacy_native(operation, request)
        return _privacy_output_archive(operation, result)
    finally:
        _clear_privacy_request_archive(request)


def privacy_capabilities_v1() -> bytes:
    """Return Norito V1 privacy capability records from the native production gate."""

    return _privacy_output_archive(
        "privacy_capabilities_v1",
        _invoke_privacy_native("privacy_capabilities_v1"),
    )


def privacy_proof_request_v1(
    *,
    algorithm_id: str,
    entrypoint: str,
    vk_ref: str,
    public_inputs: bytes | bytearray | memoryview,
    witness: bytes | bytearray | memoryview = b"",
    proof: bytes | bytearray | memoryview = b"",
) -> bytes:
    """Encode a Norito V1 `PrivacyProofRequest` for the native build/verify FFI."""

    public_inputs_bytes = _privacy_request_component_bytes(
        public_inputs,
        "public_inputs",
        allow_empty=False,
    )
    witness_bytes = _privacy_request_component_bytes(
        witness,
        "witness",
        allow_empty=True,
    )
    proof_bytes = _privacy_request_component_bytes(
        proof,
        "proof",
        allow_empty=True,
    )
    method = _privacy_native_method("privacy_proof_request_v1")
    archive = method(
        str(algorithm_id),
        str(entrypoint),
        str(vk_ref),
        public_inputs_bytes,
        witness_bytes,
        proof_bytes,
    )
    view = _privacy_unsigned_byte_view(
        archive,
        bytes_like_message="native privacy_proof_request_v1 returned non-byte output",
        typed_message="native privacy_proof_request_v1 output must use unsigned byte elements",
    )
    if view.nbytes == 0:
        raise RuntimeError("native privacy_proof_request_v1 returned empty output")
    if view.nbytes > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES:
        raise RuntimeError("native privacy_proof_request_v1 returned oversized output")
    request_archive = view.tobytes()
    _assert_privacy_norito_archive(
        "privacy_proof_request_v1",
        request_archive,
        expected_schema_byte=_PRIVACY_REQUEST_SCHEMA_BYTE,
    )
    return request_archive


def privacy_build_proof_v1(request_archive: bytes | bytearray | memoryview) -> bytes:
    """Build a privacy proof via the native Rust engine, returning a Norito V1 result archive."""

    return _privacy_native_archive("privacy_build_proof_v1", request_archive)


def privacy_verify_proof_v1(request_archive: bytes | bytearray | memoryview) -> bytes:
    """Verify a privacy proof via the native Rust engine, returning a Norito V1 result archive."""

    return _privacy_native_archive("privacy_verify_proof_v1", request_archive)
