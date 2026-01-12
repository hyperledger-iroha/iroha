"""Compose and optionally submit a simple transaction using high-level helpers."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from decimal import Decimal, InvalidOperation
from typing import Any, Optional

from iroha_python import (
    Ed25519KeyPair,
    SignedTransactionEnvelope,
    TransactionConfig,
    TransactionDraft,
    create_torii_client,
)


def build_sample_transaction(
    config: TransactionConfig,
    keypair: Ed25519KeyPair,
    *,
    domain_id: str,
    account_id: str,
    asset_definition_id: str,
    asset_id: str,
    quantity: Decimal,
    burn_quantity: Optional[Decimal] = None,
    transfer_destination: Optional[str] = None,
) -> tuple[TransactionDraft, SignedTransactionEnvelope]:
    """Return the draft and signed envelope for a sample asset mint flow."""

    draft = TransactionDraft(config)
    draft.register_domain(domain_id) \
         .register_account(account_id) \
         .register_asset_definition_numeric(
            asset_definition_id,
            owner=account_id,
            mintable="Infinitely",
         )
    draft.mint_asset_numeric(asset_id, quantity)
    if burn_quantity is not None:
        draft.burn_asset_numeric(asset_id, burn_quantity)
    if transfer_destination is not None:
        draft.transfer_asset_numeric(asset_id, quantity, transfer_destination)
    envelope = draft.sign_with_keypair(keypair)
    return draft, envelope


def main() -> None:
    parser = argparse.ArgumentParser(description="Build and optionally submit a sample transaction")
    parser.add_argument("--base-url", default="http://127.0.0.1:8080", help="Torii base URL")
    parser.add_argument("--chain-id", default="dev-chain", help="Target chain id")
    parser.add_argument("--authority", required=True, help="Signing account id (e.g. alice@test)")
    parser.add_argument("--private-key-hex", required=True, help="Hex-encoded Ed25519 private key")
    parser.add_argument("--domain-id", default="playground", help="Domain id to register")
    parser.add_argument("--account-id", default="alice@playground", help="Account id to register")
    parser.add_argument(
        "--asset-definition-id",
        default="demo#playground",
        help="Asset definition id to register",
    )
    parser.add_argument(
        "--asset-id",
        default="demo#playground#alice",
        help="Asset id to mint into",
    )
    parser.add_argument("--quantity", default="10", help="Quantity for the mint instruction")
    parser.add_argument("--burn-quantity", help="Optional quantity to burn after minting")
    parser.add_argument(
        "--transfer-destination",
        help="Optional account id to transfer the minted asset to after burn (uses original quantity)",
    )
    parser.add_argument("--ttl-ms", type=int, default=120_000, help="Transaction TTL in milliseconds")
    parser.add_argument("--submit", action="store_true", help="Submit the signed transaction to Torii")
    parser.add_argument("--wait", action="store_true", help="Submit and wait for a terminal status")
    parser.add_argument(
        "--wait-timeout",
        type=float,
        default=30.0,
        help="Timeout in seconds when --wait is enabled (default: %(default)s)",
    )
    parser.add_argument(
        "--wait-interval",
        type=float,
        default=1.0,
        help="Polling interval in seconds when --wait is enabled (default: %(default)s)",
    )
    parser.add_argument("--auth-token", help="Optional Torii auth token")
    parser.add_argument("--api-token", help="Optional Torii API token")
    parser.add_argument(
        "--json-output",
        help="Optional path to write the signed envelope JSON payload (pretty-printed).",
    )
    args = parser.parse_args()

    config = TransactionConfig(
        chain_id=args.chain_id,
        authority=args.authority,
        ttl_ms=args.ttl_ms,
    )
    keypair = Ed25519KeyPair.from_private_key(bytes.fromhex(args.private_key_hex))

    if args.wait:
        args.submit = True
        if args.wait_timeout is not None and args.wait_timeout <= 0:
            parser.error("--wait-timeout must be greater than zero")
        if args.wait_interval is not None and args.wait_interval <= 0:
            parser.error("--wait-interval must be greater than zero")

    try:
        quantity_dec = _parse_quantity(args.quantity, "--quantity")
        burn_dec = _parse_quantity(args.burn_quantity, "--burn-quantity") if args.burn_quantity else None
        if burn_dec is not None and burn_dec > quantity_dec:
            parser.error("--burn-quantity must be less than or equal to --quantity")
    except ValueError as exc:
        parser.error(str(exc))

    draft, envelope = build_sample_transaction(
        config,
        keypair,
        domain_id=args.domain_id,
        account_id=args.account_id,
        asset_definition_id=args.asset_definition_id,
        asset_id=args.asset_id,
        quantity=quantity_dec,
        burn_quantity=burn_dec,
        transfer_destination=args.transfer_destination,
    )

    payload = json.loads(envelope.to_json())
    print("Signed transaction envelope:")
    print(json.dumps(payload, indent=2))

    if args.json_output:
        destination = Path(args.json_output)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"Envelope JSON written to {destination}")

    if args.submit:
        client = create_torii_client(
            args.base_url,
            auth_token=args.auth_token,
            api_token=args.api_token,
        )
        if args.wait:
            status = client.submit_transaction_envelope_and_wait(
                envelope,
                interval=args.wait_interval,
                timeout=args.wait_timeout,
            )
            print("Transaction final status:", status)
        else:
            status = client.submit_transaction_envelope(envelope)
            print("Transaction submission status:", status)
    else:
        print("Draft contains", len(list(draft.instructions)), "instructions. Use --submit to relay.")


if __name__ == "__main__":
    main()


def _parse_quantity(value: Optional[str], flag: str) -> Decimal:
    if value is None:
        raise ValueError(f"{flag} requires a numeric value")
    try:
        parsed = Decimal(value)
    except InvalidOperation as exc:
        raise ValueError(f"{flag} must be a numeric value") from exc
    if parsed <= 0:
        raise ValueError(f"{flag} must be greater than zero")
    return parsed
