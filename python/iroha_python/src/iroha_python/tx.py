"""High-level transaction helpers built on top of the low-level `Instruction` APIs."""

from __future__ import annotations

import json
import time
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from typing import TYPE_CHECKING, Any, Dict, Iterable, List, Mapping, Optional, Union

from .crypto import (
    Ed25519KeyPair,
    Instruction,
    SignedTransactionEnvelope,
    TransactionBuilder,
    _normalize_lane_privacy_attachment,
    build_signed_transaction,
)
from .settlement import SettlementLeg, SettlementPlan

if TYPE_CHECKING:  # pragma: no cover - typing only
    from .client import ToriiClient
    from .repo import RepoCashLeg, RepoCollateralLeg, RepoGovernance

__all__ = [
    "TransactionConfig",
    "TransactionDraft",
]


NumericLike = Union[str, int, float, Decimal]
MetadataLike = Optional[Mapping[str, Any]]


@dataclass(frozen=True)
class TransactionConfig:
    """Configuration shared across transactions signed by :class:`TransactionDraft`."""

    chain_id: str
    authority: str
    creation_time_ms: Optional[int] = None
    ttl_ms: Optional[int] = None
    nonce: Optional[int] = None
    metadata: Optional[Mapping[str, Any]] = None


def _ensure_creation_time_ms(config: TransactionConfig) -> int:
    return int(config.creation_time_ms or int(time.time() * 1000))


def _normalize_quantity(quantity: NumericLike) -> str:
    """Convert numeric inputs to the canonical string representation expected by Norito."""

    if isinstance(quantity, Decimal):
        value = quantity
    elif isinstance(quantity, int):
        value = Decimal(quantity)
    elif isinstance(quantity, float):
        # Convert through repr to preserve precision expectations.
        value = Decimal(str(quantity))
    elif isinstance(quantity, str):
        try:
            value = Decimal(quantity)
        except InvalidOperation as exc:  # pragma: no cover - handled in tests
            raise ValueError(f"quantity '{quantity}' is not a valid decimal string") from exc
    else:
        raise TypeError(f"unsupported quantity type: {type(quantity)!r}")

    if value.is_nan() or value.is_infinite():
        raise ValueError("quantity must be a finite decimal value")

    # `normalize()` removes trailing zeros but may produce scientific notation; format with `f`.
    normalized = value.normalize()
    return format(normalized, "f")


def _normalize_metadata(metadata: MetadataLike) -> Optional[Mapping[str, Any]]:
    if metadata is None:
        return None
    if not isinstance(metadata, Mapping):
        raise TypeError("metadata must be a mapping when provided")
    # Round-trip through JSON to ensure only JSON-compatible values remain (e.g., Decimal -> str).
    serialized = json.dumps(metadata, default=str)
    return json.loads(serialized)


class TransactionDraft:
    """Collect instructions and sign transactions with ergonomic helpers."""

    def __init__(self, config: TransactionConfig):
        self._config = config
        self._instructions: List[Instruction] = []
        self._lane_privacy_attachments: List[Mapping[str, Any]] = []

    @property
    def config(self) -> TransactionConfig:
        """Return the configuration used by this draft."""

        return self._config

    @property
    def instructions(self) -> Iterable[Instruction]:
        """Iterator over appended instructions."""

        return tuple(self._instructions)

    def __iter__(self):
        return iter(self._instructions)

    def __len__(self) -> int:
        return len(self._instructions)

    def add_instruction(self, instruction: Instruction) -> Instruction:
        """Append an existing :class:`Instruction` to the draft."""

        self._instructions.append(instruction)
        return instruction

    def extend_instructions(self, instructions: Iterable[Instruction]) -> None:
        """Append multiple instructions in order."""

        for instruction in instructions:
            self.add_instruction(instruction)

    def clear_instructions(self) -> None:
        """Remove all instructions from the draft."""

        self._instructions.clear()
        self._lane_privacy_attachments.clear()

    def add_lane_privacy_merkle_proof(
        self,
        *,
        commitment_id: int,
        leaf: bytes,
        leaf_index: int,
        audit_path: Iterable[Optional[bytes]],
        proof_backend: str,
        proof_bytes: bytes,
        verifying_key_bytes: bytes,
    ) -> TransactionDraft:
        """Attach a lane privacy Merkle proof used by Nexus commitment-only lanes.

        Parameters mirror :func:`iroha_python.crypto.build_signed_transaction` and are validated
        using the shared normalization helper.
        """

        attachment = _normalize_lane_privacy_attachment(
            {
                "commitment_id": commitment_id,
                "leaf": leaf,
                "leaf_index": leaf_index,
                "audit_path": list(audit_path),
                "proof_backend": proof_backend,
                "proof_bytes": proof_bytes,
                "verifying_key_bytes": verifying_key_bytes,
            }
        )
        self._lane_privacy_attachments.append(attachment)
        return self

    def clear_lane_privacy_attachments(self) -> None:
        """Remove any staged lane privacy attachments."""

        self._lane_privacy_attachments.clear()

    # ------------------------------------------------------------------
    # High-level helpers for common instruction families
    # ------------------------------------------------------------------
    def register_domain(
        self,
        domain_id: str,
        *,
        metadata: MetadataLike = None,
    ) -> TransactionDraft:
        """Append a `RegisterDomain` instruction and return the draft for fluent chaining."""

        metadata_payload = _normalize_metadata(metadata)
        self.add_instruction(Instruction.register_domain(domain_id, metadata_payload))
        return self

    def register_account(
        self,
        account_id: str,
        *,
        metadata: MetadataLike = None,
    ) -> TransactionDraft:
        """Append a `RegisterAccount` instruction."""

        metadata_payload = _normalize_metadata(metadata)
        self.add_instruction(Instruction.register_account(account_id, metadata_payload))
        return self

    def register_asset_definition_numeric(
        self,
        definition_id: str,
        owner: str,
        *,
        scale: Optional[Union[int, str]] = None,
        mintable: Optional[str] = None,
        confidential_policy: Optional[str] = None,
        metadata: MetadataLike = None,
    ) -> TransactionDraft:
        """Append a `RegisterAssetDefinition` instruction for numeric assets."""

        normalized_scale: Optional[int]
        if scale is None:
            normalized_scale = None
        elif isinstance(scale, int):
            normalized_scale = scale
        elif isinstance(scale, str):
            try:
                normalized_scale = int(scale)
            except ValueError as exc:  # pragma: no cover - defensive
                raise ValueError(f"scale '{scale}' must be an integer value") from exc
        else:
            raise TypeError("scale must be an integer or string when provided")

        metadata_payload = _normalize_metadata(metadata)

        self.add_instruction(
            Instruction.register_asset_definition_numeric(
                definition_id,
                owner,
                scale=normalized_scale,
                mintable=mintable,
                confidential_policy=confidential_policy,
                metadata=metadata_payload,
            )
        )
        return self

    def mint_asset_numeric(self, asset_id: str, quantity: NumericLike) -> TransactionDraft:
        """Append a numeric `MintAsset` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(Instruction.mint_asset_numeric(asset_id, normalized_quantity))
        return self

    def burn_asset_numeric(self, asset_id: str, quantity: NumericLike) -> TransactionDraft:
        """Append a numeric `BurnAsset` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(Instruction.burn_asset_numeric(asset_id, normalized_quantity))
        return self

    def transfer_asset_numeric(
        self,
        asset_id: str,
        quantity: NumericLike,
        destination: str,
    ) -> TransactionDraft:
        """Append a numeric `TransferAsset` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(
            Instruction.transfer_asset_numeric(
                asset_id,
                normalized_quantity,
                destination,
            )
        )
        return self

    def repo_initiate(
        self,
        agreement_id: str,
        initiator: str,
        counterparty: str,
        cash_leg: "RepoCashLeg",
        collateral_leg: "RepoCollateralLeg",
        *,
        custodian: Optional[str] = None,
        rate_bps: int,
        maturity_timestamp_ms: int,
        governance: Optional["RepoGovernance"] = None,
    ) -> TransactionDraft:
        """Append a `RepoIsi` instruction for repo initiation or rolling."""

        governance = governance or RepoGovernance(haircut_bps=0, margin_frequency_secs=0)
        cash_payload: Dict[str, Any] = {
            "asset_definition_id": cash_leg.asset_definition_id,
            "quantity": _normalize_quantity(cash_leg.quantity),
        }
        collateral_payload: Dict[str, Any] = {
            "asset_definition_id": collateral_leg.asset_definition_id,
            "quantity": _normalize_quantity(collateral_leg.quantity),
        }
        if collateral_leg.metadata:
            collateral_payload["metadata"] = _normalize_metadata(collateral_leg.metadata)

        governance_payload = {
            "haircut_bps": int(governance.haircut_bps),
            "margin_frequency_secs": int(governance.margin_frequency_secs),
        }

        instruction = Instruction.repo_initiate(
            agreement_id,
            initiator,
            counterparty,
            custodian,
            cash_payload,
            collateral_payload,
            int(rate_bps),
            int(maturity_timestamp_ms),
            governance_payload,
        )
        self.add_instruction(instruction)
        return self

    def repo_unwind(
        self,
        agreement_id: str,
        initiator: str,
        counterparty: str,
        cash_leg: "RepoCashLeg",
        collateral_leg: "RepoCollateralLeg",
        *,
        settlement_timestamp_ms: int,
    ) -> TransactionDraft:
        """Append a `ReverseRepoIsi` instruction to unwind a repo agreement."""

        cash_payload: Dict[str, Any] = {
            "asset_definition_id": cash_leg.asset_definition_id,
            "quantity": _normalize_quantity(cash_leg.quantity),
        }
        collateral_payload: Dict[str, Any] = {
            "asset_definition_id": collateral_leg.asset_definition_id,
            "quantity": _normalize_quantity(collateral_leg.quantity),
        }
        if collateral_leg.metadata:
            collateral_payload["metadata"] = _normalize_metadata(collateral_leg.metadata)

        instruction = Instruction.repo_unwind(
            agreement_id,
            initiator,
            counterparty,
            cash_payload,
            collateral_payload,
            int(settlement_timestamp_ms),
        )
        self.add_instruction(instruction)
        return self

    def repo_margin_call(self, agreement_id: str) -> TransactionDraft:
        """Append a `RepoMarginCallIsi` instruction to record a margin check."""

        instruction = Instruction.repo_margin_call(agreement_id)
        self.add_instruction(instruction)
        return self

    def set_account_key_value(
        self,
        key: str,
        value: Any,
        *,
        account_id: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `SetKeyValue` instruction targeting an account.

        When ``account_id`` is omitted, the draft authority is used.
        """

        target = account_id or self._config.authority
        self.add_instruction(Instruction.set_account_key_value(target, key, value))
        return self

    def remove_account_key_value(
        self,
        key: str,
        *,
        account_id: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `RemoveKeyValue` instruction targeting an account.

        When ``account_id`` is omitted, the draft authority is used.
        """

        target = account_id or self._config.authority
        self.add_instruction(Instruction.remove_account_key_value(target, key))
        return self

    def transfer_domain(
        self,
        domain_id: str,
        *,
        destination: str,
        source: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `TransferDomain` instruction."""

        origin = source or self._config.authority
        self.add_instruction(Instruction.transfer_domain(origin, domain_id, destination))
        return self

    def transfer_asset_definition(
        self,
        definition_id: str,
        *,
        destination: str,
        source: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `TransferAssetDefinition` instruction."""

        origin = source or self._config.authority
        self.add_instruction(
            Instruction.transfer_asset_definition(origin, definition_id, destination)
        )
        return self

    def transfer_nft(
        self,
        nft_id: str,
        *,
        destination: str,
        source: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `TransferNft` instruction."""

        origin = source or self._config.authority
        self.add_instruction(Instruction.transfer_nft(origin, nft_id, destination))
        return self

    def settlement_dvp(
        self,
        settlement_id: str,
        delivery_leg: SettlementLeg,
        payment_leg: SettlementLeg,
        *,
        plan: Optional[SettlementPlan] = None,
        metadata: MetadataLike = None,
    ) -> TransactionDraft:
        """Append a delivery-versus-payment settlement instruction."""

        effective_plan = plan or SettlementPlan()
        metadata_payload = (
            _normalize_metadata(metadata) if metadata is not None else None
        )
        instruction = Instruction.settlement_dvp(
            settlement_id,
            delivery_leg.to_payload(),
            payment_leg.to_payload(),
            order=effective_plan.order.value,
            atomicity=effective_plan.atomicity.value,
            metadata=metadata_payload,
        )
        self.add_instruction(instruction)
        return self

    def settlement_pvp(
        self,
        settlement_id: str,
        primary_leg: SettlementLeg,
        counter_leg: SettlementLeg,
        *,
        plan: Optional[SettlementPlan] = None,
        metadata: MetadataLike = None,
    ) -> TransactionDraft:
        """Append a payment-versus-payment settlement instruction."""

        effective_plan = plan or SettlementPlan()
        metadata_payload = (
            _normalize_metadata(metadata) if metadata is not None else None
        )
        instruction = Instruction.settlement_pvp(
            settlement_id,
            primary_leg.to_payload(),
            counter_leg.to_payload(),
            order=effective_plan.order.value,
            atomicity=effective_plan.atomicity.value,
            metadata=metadata_payload,
        )
        self.add_instruction(instruction)
        return self

    # ------------------------------------------------------------------
    # Signing helpers
    # ------------------------------------------------------------------
    def sign(
        self,
        private_key: bytes,
        *,
        instructions: Optional[Iterable[Instruction]] = None,
        creation_time_ms: Optional[int] = None,
        ttl_ms: Optional[int] = None,
        nonce: Optional[int] = None,
        metadata: Optional[Mapping[str, Any]] = None,
        chain_id: Optional[str] = None,
        authority: Optional[str] = None,
    ) -> SignedTransactionEnvelope:
        """Sign the draft with ``private_key`` and return a :class:`SignedTransactionEnvelope`."""

        payload_instructions = list(instructions or self._instructions)
        effective_chain = chain_id or self._config.chain_id
        effective_authority = authority or self._config.authority
        effective_creation = (
            int(creation_time_ms)
            if creation_time_ms is not None
            else _ensure_creation_time_ms(self._config)
        )
        effective_ttl = ttl_ms if ttl_ms is not None else self._config.ttl_ms
        effective_nonce = nonce if nonce is not None else self._config.nonce
        effective_metadata = metadata if metadata is not None else self._config.metadata
        return build_signed_transaction(
            effective_chain,
            effective_authority,
            private_key,
            instructions=payload_instructions,
            creation_time_ms=effective_creation,
            ttl_ms=effective_ttl,
            nonce=effective_nonce,
            metadata=effective_metadata,
            lane_privacy_attachments=self._lane_privacy_attachments,
        )

    def sign_with_keypair(
        self,
        keypair: Ed25519KeyPair,
        *,
        instructions: Optional[Iterable[Instruction]] = None,
        **overrides: Any,
    ) -> SignedTransactionEnvelope:
        """Sign using an :class:`Ed25519KeyPair`."""

        return self.sign(keypair.private_key, instructions=instructions, **overrides)

    def sign_hex_private_key(
        self,
        private_key_hex: str,
        *,
        instructions: Optional[Iterable[Instruction]] = None,
        **overrides: Any,
    ) -> SignedTransactionEnvelope:
        """Sign using a hex-encoded private key string."""

        return self.sign(bytes.fromhex(private_key_hex), instructions=instructions, **overrides)

    def sign_and_submit(
        self,
        client: "ToriiClient",
        private_key: bytes,
        *,
        instructions: Optional[Iterable[Instruction]] = None,
        **overrides: Any,
    ) -> tuple[SignedTransactionEnvelope, Any]:
        """Sign the draft and submit it via the provided :class:`ToriiClient`."""

        envelope = self.sign(private_key, instructions=instructions, **overrides)
        status = client.submit_transaction_envelope(envelope)
        return envelope, status

    def sign_hex_and_submit(
        self,
        client: "ToriiClient",
        private_key_hex: str,
        *,
        instructions: Optional[Iterable[Instruction]] = None,
        **overrides: Any,
    ) -> tuple[SignedTransactionEnvelope, Any]:
        """Sign using a hex-encoded key and submit the transaction."""

        return self.sign_and_submit(
            client,
            bytes.fromhex(private_key_hex),
            instructions=instructions,
            **overrides,
        )

    def to_builder(self) -> TransactionBuilder:
        """Return a :class:`TransactionBuilder` populated with the draft state."""

        builder = TransactionBuilder(self._config.chain_id, self._config.authority)
        builder.set_creation_time_ms(_ensure_creation_time_ms(self._config))
        if self._config.ttl_ms is not None:
            builder.set_ttl_ms(int(self._config.ttl_ms))
        if self._config.nonce is not None:
            builder.set_nonce(int(self._config.nonce))
        if self._config.metadata is not None:
            builder.set_metadata(self._config.metadata)
        for instruction in self._instructions:
            builder.add_instruction(instruction)
        return builder

    # ------------------------------------------------------------------
    # Manifest helpers
    # ------------------------------------------------------------------
    def to_manifest_dict(
        self,
        *,
        include_creation_time: bool = False,
        include_metadata: bool = True,
    ) -> Mapping[str, Any]:
        """Return a JSON-serialisable manifest describing the draft.

        Parameters
        ----------
        include_creation_time:
            When ``True`` the manifest includes ``creation_time_ms`` using the deterministic
            timestamp derived from :class:`TransactionConfig`. By default the field is omitted
            unless the configuration already provides an explicit value.
        include_metadata:
            When ``True`` (default) the manifest embeds the configured metadata. Disable when
            callers intend to redact metadata before serialising the manifest.
        """

        manifest: dict[str, Any] = {
            "chain_id": self._config.chain_id,
            "authority": self._config.authority,
            "instructions": [
                json.loads(instruction.to_json()) for instruction in self._instructions
            ],
        }

        if include_metadata and self._config.metadata is not None:
            manifest["metadata"] = _normalize_metadata(self._config.metadata)
        if self._config.ttl_ms is not None:
            manifest["ttl_ms"] = int(self._config.ttl_ms)
        if self._config.nonce is not None:
            manifest["nonce"] = int(self._config.nonce)

        creation_time_ms = self._config.creation_time_ms
        if include_creation_time and creation_time_ms is None:
            creation_time_ms = _ensure_creation_time_ms(self._config)
        if creation_time_ms is not None:
            manifest["creation_time_ms"] = int(creation_time_ms)

        return manifest

    def to_manifest_json(
        self,
        *,
        include_creation_time: bool = False,
        include_metadata: bool = True,
        indent: Optional[int] = None,
    ) -> str:
        """Serialise :meth:`to_manifest_dict` as a JSON string."""

        manifest = self.to_manifest_dict(
            include_creation_time=include_creation_time,
            include_metadata=include_metadata,
        )
        return json.dumps(manifest, indent=indent, separators=None if indent else (",", ":"))

    # ------------------------------------------------------------------
    # Convenience factory
    # ------------------------------------------------------------------
