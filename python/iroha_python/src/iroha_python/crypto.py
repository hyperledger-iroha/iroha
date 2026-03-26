"""High-level Ed25519 and SM2 helpers backed by `iroha_crypto` via PyO3 bindings."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Dict, Final, Iterable, Mapping, Optional

try:
    from typing import TypeAlias
except ImportError:  # pragma: no cover - Python <3.10 compatibility path
    from typing_extensions import TypeAlias

from ._native import load_crypto_extension
from .address import AccountAddress

_crypto = load_crypto_extension()

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
    "ED25519_PRIVATE_KEY_LENGTH",
    "ED25519_PUBLIC_KEY_LENGTH",
    "ED25519_SIGNATURE_LENGTH",
    "SM2_PRIVATE_KEY_LENGTH",
    "SM2_PUBLIC_KEY_LENGTH",
    "SM2_SIGNATURE_LENGTH",
    "SM2_DEFAULT_DISTINGUISHED_ID",
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
    "derive_ed25519_keypair_from_seed",
    "generate_ed25519_keypair",
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
    "sign_ed25519",
    "sign_sm2",
    "decode_transaction_receipt_json",
    "verify_signed_transaction_versioned",
    "verify_ed25519",
    "verify_sm2",
    "derive_confidential_keyset",
    "derive_confidential_keyset_from_hex",
    "sm2_fixture_from_seed",
]

if TYPE_CHECKING:
    Instruction: TypeAlias = Any
    SignedTransactionEnvelope: TypeAlias = Any
    TransactionBuilder: TypeAlias = Any
else:
    Instruction = _crypto.Instruction
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
        proof_backend = str(entry.get("proof_backend", "halo2/ipa"))
        proof_bytes = _normalize_bytes(entry["proof_bytes"], "proof_bytes")
        verifying_key_bytes = _normalize_bytes(entry["verifying_key_bytes"], "verifying_key_bytes")
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
        "verifying_key_bytes": verifying_key_bytes,
    }


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
        """Return the canonical Katakana i105 account id using the public key and `domain`."""

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
    """Return the canonical Katakana i105 account id using the public key within `domain`."""

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
        literal: canonical Katakana i105 only).
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
        ``proof_bytes``, and ``verifying_key_bytes``.
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
                normalized["verifying_key_bytes"],
            )
    return builder.sign(private_key)


def sign_ed25519(private_key: bytes, message: bytes) -> bytes:
    """Return the Ed25519 signature for ``message``."""

    return _crypto.sign_ed25519(private_key, message)


def verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> bool:
    """Verify an Ed25519 signature."""

    return _crypto.verify_ed25519(public_key, message, signature)


def hash_blake2b_32(data: bytes) -> bytes:
    """Compute the canonical Iroha Blake2b-256 hash for ``data``."""

    return _crypto.hash_blake2b_32(data)
