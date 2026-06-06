"""High-level transaction helpers built on top of the low-level `Instruction` APIs."""

from __future__ import annotations

import json
import time
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from typing import TYPE_CHECKING, Any, Dict, Iterable, List, Mapping, Optional, Sequence, Union

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
PositiveU128Like = Union[str, int]
MetadataLike = Optional[Mapping[str, Any]]
FixedBytesLike = Union[str, bytes, bytearray, memoryview]
VerifyingKeyLike = Union[str, Mapping[str, Any]]
_U128_MAX = (1 << 128) - 1


def _require_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    text = value.strip()
    if not text:
        raise ValueError(f"{context} must be non-empty")
    return text


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


def _normalize_u128_quantity(quantity: NumericLike, context: str) -> str:
    value = Decimal(_normalize_quantity(quantity))
    if value < 0 or value != value.to_integral_value():
        raise ValueError(f"{context} must be a non-negative whole number")
    return str(int(value))


def _normalize_positive_u128_literal(quantity: Any, context: str) -> str:
    if isinstance(quantity, bool):
        raise ValueError(f"{context} must be a positive decimal u128 string")
    if isinstance(quantity, int):
        value = quantity
    elif isinstance(quantity, str):
        text = quantity.strip()
        if not text.isdecimal():
            raise ValueError(f"{context} must be a positive decimal u128 string")
        value = int(text, 10)
    else:
        raise TypeError(f"{context} must be a positive decimal u128 string")
    if value <= 0 or value > _U128_MAX:
        raise ValueError(f"{context} must be a positive decimal u128 string")
    return str(value)


def _normalize_metadata(metadata: MetadataLike) -> Optional[Mapping[str, Any]]:
    if metadata is None:
        return None
    if not isinstance(metadata, Mapping):
        raise TypeError("metadata must be a mapping when provided")
    # Round-trip through JSON to ensure only JSON-serializable values remain (e.g., Decimal -> str).
    serialized = json.dumps(metadata, default=str)
    return json.loads(serialized)


def _normalize_json_value(value: Any, context: str) -> Any:
    """Convert nested payloads to JSON-serializable values, stringifying Decimals when needed."""

    try:
        serialized = json.dumps(value, default=str)
    except TypeError as exc:  # pragma: no cover - exercised by callers
        raise TypeError(f"{context} must be JSON serializable") from exc
    return json.loads(serialized)


def _normalize_mapping_payload(payload: Mapping[str, Any], context: str) -> Dict[str, Any]:
    if not isinstance(payload, Mapping):
        raise TypeError(f"{context} must be a mapping")
    normalized = _normalize_json_value(payload, context)
    if not isinstance(normalized, dict):  # pragma: no cover - json.loads object contract
        raise TypeError(f"{context} must serialize to a JSON object")
    return normalized


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
        verifying_key_name: str,
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
                "verifying_key_name": verifying_key_name,
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

    def register_rwa(
        self,
        rwa: Mapping[str, Any],
    ) -> TransactionDraft:
        """Append a `RegisterRwa` instruction."""

        rwa_payload = _normalize_mapping_payload(rwa, "rwa")
        self.add_instruction(Instruction.register_rwa(rwa_payload))
        return self

    def register_asset_definition_numeric(
        self,
        definition_id: str,
        owner: str,
        *,
        name: Optional[str] = None,
        description: Optional[str] = None,
        alias: Optional[str] = None,
        scale: Optional[Union[int, str]] = None,
        mintable: Optional[str] = None,
        balance_scope_policy: Optional[str] = None,
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
                name=name,
                description=description,
                alias=alias,
                scale=normalized_scale,
                mintable=mintable,
                balance_scope_policy=balance_scope_policy,
                confidential_policy=confidential_policy,
                metadata=metadata_payload,
            )
        )
        return self

    def register_zk_asset(
        self,
        asset_definition_id: str,
        *,
        mode: str = "Hybrid",
        allow_shield: bool = True,
        allow_unshield: bool = True,
        vk_transfer: Optional[VerifyingKeyLike] = None,
        vk_unshield: Optional[VerifyingKeyLike] = None,
        vk_shield: Optional[VerifyingKeyLike] = None,
    ) -> TransactionDraft:
        """Append a `RegisterZkAsset` instruction."""

        definition = _require_non_empty_string(
            asset_definition_id,
            "asset_definition_id",
        )
        self.add_instruction(
            Instruction.register_zk_asset(
                definition,
                mode=mode,
                allow_shield=bool(allow_shield),
                allow_unshield=bool(allow_unshield),
                vk_transfer=vk_transfer,
                vk_unshield=vk_unshield,
                vk_shield=vk_shield,
            )
        )
        return self

    def register_zk_ace_identity_commitment(
        self,
        asset_definition_id: str,
        *,
        identity_commitment: FixedBytesLike,
        policy_hash: FixedBytesLike,
        allowed_accounts: Sequence[str],
        verifier_key: VerifyingKeyLike,
        action_class: Optional[str] = None,
        domain_tag: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `RegisterZkAceIdentityCommitment` instruction."""

        self.add_instruction(
            Instruction.register_zk_ace_identity_commitment(
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                identity_commitment,
                policy_hash,
                allowed_accounts,
                verifier_key=verifier_key,
                action_class=action_class,
                domain_tag=domain_tag,
            )
        )
        return self

    def rotate_zk_ace_identity_commitment(
        self,
        asset_definition_id: str,
        *,
        old_identity_commitment: FixedBytesLike,
        new_identity_commitment: FixedBytesLike,
        policy_hash: FixedBytesLike,
        allowed_accounts: Sequence[str],
        verifier_key: VerifyingKeyLike,
        action_class: Optional[str] = None,
        domain_tag: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `RotateZkAceIdentityCommitment` instruction."""

        self.add_instruction(
            Instruction.rotate_zk_ace_identity_commitment(
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                old_identity_commitment,
                new_identity_commitment,
                policy_hash,
                allowed_accounts,
                verifier_key=verifier_key,
                action_class=action_class,
                domain_tag=domain_tag,
            )
        )
        return self

    def revoke_zk_ace_identity_commitment(
        self,
        asset_definition_id: str,
        *,
        identity_commitment: FixedBytesLike,
        reason_hash: Optional[FixedBytesLike] = None,
    ) -> TransactionDraft:
        """Append a `RevokeZkAceIdentityCommitment` instruction."""

        self.add_instruction(
            Instruction.revoke_zk_ace_identity_commitment(
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                identity_commitment,
                reason_hash=reason_hash,
            )
        )
        return self

    def shield_asset(
        self,
        asset_definition_id: str,
        from_account_id: str,
        amount: NumericLike,
        *,
        note_commitment: FixedBytesLike,
        ephemeral_public_key: FixedBytesLike,
        nonce: FixedBytesLike,
        ciphertext: Optional[Union[str, bytes, bytearray, memoryview]] = None,
        ciphertext_b64: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `Shield` instruction for public-to-shielded movement."""

        if ciphertext is None and ciphertext_b64 is None:
            raise ValueError("provide either ciphertext or ciphertext_b64")
        if ciphertext is not None and ciphertext_b64 is not None:
            raise ValueError("provide only one of ciphertext or ciphertext_b64")
        normalized_ciphertext: Union[bytes, bytearray, memoryview, str]
        if ciphertext_b64 is not None:
            normalized_ciphertext = ciphertext_b64
        elif isinstance(ciphertext, str):
            normalized_ciphertext = ciphertext.encode("utf-8")
        else:
            assert ciphertext is not None
            normalized_ciphertext = ciphertext
        self.add_instruction(
            Instruction.shield_asset(
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                _require_non_empty_string(from_account_id, "from_account_id"),
                _normalize_u128_quantity(amount, "amount"),
                note_commitment,
                ephemeral_public_key,
                nonce,
                normalized_ciphertext,
            )
        )
        return self

    def zk_transfer_prepared(
        self,
        asset_definition_id: str,
        *,
        inputs: Iterable[FixedBytesLike],
        outputs: Iterable[FixedBytesLike],
        proof: Mapping[str, Any],
        root_hint: Optional[FixedBytesLike] = None,
    ) -> TransactionDraft:
        """Append a prepared `ZkTransfer` instruction."""

        if not isinstance(proof, Mapping):
            raise TypeError("proof must be a mapping")
        input_list = list(inputs)
        output_list = list(outputs)
        self.add_instruction(
            Instruction.zk_transfer_prepared(
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                input_list,
                output_list,
                dict(proof),
                root_hint=root_hint,
            )
        )
        return self

    def unshield_prepared(
        self,
        asset_definition_id: str,
        to_account_id: str,
        public_amount: NumericLike,
        *,
        inputs: Iterable[FixedBytesLike],
        proof: Mapping[str, Any],
        outputs: Optional[Iterable[FixedBytesLike]] = None,
        root_hint: Optional[FixedBytesLike] = None,
    ) -> TransactionDraft:
        """Append a prepared `Unshield` instruction."""

        if not isinstance(proof, Mapping):
            raise TypeError("proof must be a mapping")
        self.add_instruction(
            Instruction.unshield_prepared(
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                _require_non_empty_string(to_account_id, "to_account_id"),
                _normalize_u128_quantity(public_amount, "public_amount"),
                list(inputs),
                dict(proof),
                outputs=list(outputs or []),
                root_hint=root_hint,
            )
        )
        return self

    def zk_ace_authorized_transfer(
        self,
        *,
        from_account_id: str,
        to_account_id: str,
        asset_definition_id: str,
        amount: PositiveU128Like,
        identity_commitment: FixedBytesLike,
        tx_digest: FixedBytesLike,
        chain_id: str,
        domain_tag: str,
        action_class: str,
        replay_nullifier: FixedBytesLike,
        policy_hash: FixedBytesLike,
        proof: Mapping[str, Any],
    ) -> TransactionDraft:
        """Append a prepared `SubmitZkAceAuthorizedTransfer` instruction."""

        if not isinstance(proof, Mapping):
            raise TypeError("proof must be a mapping")
        self.add_instruction(
            Instruction.zk_ace_authorized_transfer(
                _require_non_empty_string(from_account_id, "from_account_id"),
                _require_non_empty_string(to_account_id, "to_account_id"),
                _require_non_empty_string(asset_definition_id, "asset_definition_id"),
                _normalize_positive_u128_literal(amount, "amount"),
                identity_commitment,
                tx_digest,
                _require_non_empty_string(chain_id, "chain_id"),
                _require_non_empty_string(domain_tag, "domain_tag"),
                _require_non_empty_string(action_class, "action_class"),
                replay_nullifier,
                policy_hash,
                dict(proof),
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

    def grant_account_permission(
        self,
        destination: str,
        name: str,
        *,
        payload: Any = None,
    ) -> TransactionDraft:
        """Append a `Grant::Permission` instruction for an account."""

        destination = _require_non_empty_string(destination, "destination")
        name = _require_non_empty_string(name, "permission name")
        self.add_instruction(
            Instruction.grant_account_permission(destination, name, payload=payload)
        )
        return self

    def revoke_account_permission(
        self,
        destination: str,
        name: str,
        *,
        payload: Any = None,
    ) -> TransactionDraft:
        """Append a `Revoke::Permission` instruction for an account."""

        destination = _require_non_empty_string(destination, "destination")
        name = _require_non_empty_string(name, "permission name")
        self.add_instruction(
            Instruction.revoke_account_permission(destination, name, payload=payload)
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

    def transfer_rwa(
        self,
        rwa_id: str,
        *,
        quantity: NumericLike,
        destination: str,
        source: Optional[str] = None,
    ) -> TransactionDraft:
        """Append a `TransferRwa` instruction."""

        origin = source or self._config.authority
        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(
            Instruction.transfer_rwa(origin, rwa_id, normalized_quantity, destination)
        )
        return self

    def merge_rwas(
        self,
        merge: Mapping[str, Any],
    ) -> TransactionDraft:
        """Append a `MergeRwas` instruction."""

        merge_payload = _normalize_mapping_payload(merge, "merge")
        self.add_instruction(Instruction.merge_rwas(merge_payload))
        return self

    def redeem_rwa(
        self,
        rwa_id: str,
        *,
        quantity: NumericLike,
    ) -> TransactionDraft:
        """Append a `RedeemRwa` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(Instruction.redeem_rwa(rwa_id, normalized_quantity))
        return self

    def freeze_rwa(self, rwa_id: str) -> TransactionDraft:
        """Append a `FreezeRwa` instruction."""

        self.add_instruction(Instruction.freeze_rwa(rwa_id))
        return self

    def unfreeze_rwa(self, rwa_id: str) -> TransactionDraft:
        """Append an `UnfreezeRwa` instruction."""

        self.add_instruction(Instruction.unfreeze_rwa(rwa_id))
        return self

    def hold_rwa(
        self,
        rwa_id: str,
        *,
        quantity: NumericLike,
    ) -> TransactionDraft:
        """Append a `HoldRwa` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(Instruction.hold_rwa(rwa_id, normalized_quantity))
        return self

    def release_rwa(
        self,
        rwa_id: str,
        *,
        quantity: NumericLike,
    ) -> TransactionDraft:
        """Append a `ReleaseRwa` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(Instruction.release_rwa(rwa_id, normalized_quantity))
        return self

    def force_transfer_rwa(
        self,
        rwa_id: str,
        *,
        quantity: NumericLike,
        destination: str,
    ) -> TransactionDraft:
        """Append a `ForceTransferRwa` instruction."""

        normalized_quantity = _normalize_quantity(quantity)
        self.add_instruction(
            Instruction.force_transfer_rwa(rwa_id, normalized_quantity, destination)
        )
        return self

    def set_rwa_controls(
        self,
        rwa_id: str,
        controls: Mapping[str, Any],
    ) -> TransactionDraft:
        """Append a `SetRwaControls` instruction."""

        controls_payload = _normalize_mapping_payload(controls, "controls")
        self.add_instruction(Instruction.set_rwa_controls(rwa_id, controls_payload))
        return self

    def set_rwa_key_value(
        self,
        rwa_id: str,
        key: str,
        value: Any,
    ) -> TransactionDraft:
        """Append a `SetRwaKeyValue` instruction."""

        normalized_value = _normalize_json_value(value, "value")
        self.add_instruction(Instruction.set_rwa_key_value(rwa_id, key, normalized_value))
        return self

    def remove_rwa_key_value(
        self,
        rwa_id: str,
        key: str,
    ) -> TransactionDraft:
        """Append a `RemoveRwaKeyValue` instruction."""

        self.add_instruction(Instruction.remove_rwa_key_value(rwa_id, key))
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
