"""Create and locally verify a disposable proof; no network request or transaction."""

from secrets import token_bytes

from iroha_python import (
    ConfidentialInput,
    ConfidentialProver,
    ConfidentialTree,
    NetworkId,
)
from iroha_python.crypto import (
    AssetDefinitionId,
    compute_confidential_root_v2,
    derive_confidential_diversifier_v2,
    derive_confidential_note_v2,
    derive_confidential_owner_tag_v2,
    hash_blake2b_32,
)


def main() -> None:
    # Real applications obtain network, canonical asset and root from authenticated state.
    network = NetworkId.from_bytes(hash_blake2b_32(b"python-local-redemption-example"))
    asset = str(AssetDefinitionId.from_domain_and_name("example.is", "local-proof"))
    spend_key = token_bytes(32)
    diversifier = derive_confidential_diversifier_v2(token_bytes(32))
    note = ConfidentialInput(amount=7, rho=token_bytes(32), diversifier=diversifier, leaf_index=0)
    owner = derive_confidential_owner_tag_v2(spend_key, diversifier)
    commitment = derive_confidential_note_v2(asset, note.amount, note.rho, owner)
    tree = ConfidentialTree(
        root=compute_confidential_root_v2([commitment]), commitments=[commitment]
    )
    with ConfidentialProver(network, asset, spend_key) as prover:
        result = prover.prove_unshield(tree=tree, inputs=[note], public_amount=7)
    print(
        f"Locally verified {result.relation}: {len(result.proof)} bytes, {len(result.nullifiers)} input note."
    )


if __name__ == "__main__":
    main()
