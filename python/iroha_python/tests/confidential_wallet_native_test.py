"""Installed public wallet integration; missing native support fails instead of skipping.

Run with the installed interpreter in isolated mode, for example
``python -I /path/to/confidential_wallet_native_test.py -v``.
"""

import unittest
from secrets import token_bytes
from threading import Event, Thread

import iroha_python
from iroha_python import (
    ConfidentialChange,
    ConfidentialInput,
    ConfidentialProver,
    ConfidentialProverError,
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


class InstalledConfidentialWalletTests(unittest.TestCase):
    """Exercise one actual proof through the normal package and extension loader."""

    def test_wallet_public_exports(self):
        for name in (
            "ConfidentialChange", "ConfidentialInput", "ConfidentialOutput", "ConfidentialProof",
            "ConfidentialProver", "ConfidentialProverError", "ConfidentialTree",
        ):
            self.assertIn(name, iroha_python.__all__)
            self.assertIsNotNone(getattr(iroha_python, name))

    def test_private_change_can_be_spent_with_its_default_owner(self):
        network = NetworkId.from_bytes(hash_blake2b_32(b"installed-change-wallet-test"))
        asset = str(AssetDefinitionId.from_domain_and_name("example.is", "change-test"))
        key = token_bytes(32)
        note = ConfidentialInput(7, token_bytes(32), derive_confidential_diversifier_v2(token_bytes(32)), 0)
        owner = derive_confidential_owner_tag_v2(key, note.diversifier)
        commitment = derive_confidential_note_v2(asset, note.amount, note.rho, owner)
        tree = ConfidentialTree(compute_confidential_root_v2([commitment]), commitments=[commitment])
        change = ConfidentialChange(3, token_bytes(32))
        with ConfidentialProver(network, asset, key) as prover:
            proof = prover.prove_unshield(tree=tree, inputs=[note], public_amount=4, change=change)
            self.assertEqual(proof.relation, "redemption_with_change")
            # This is a local proof fixture. Real wallets authenticate the new root and index.
            change_input = change.to_input(0)
            self.assertNotEqual(change_input.diversifier, note.diversifier)
            expected = derive_confidential_note_v2(
                asset, change_input.amount, change_input.rho,
                derive_confidential_owner_tag_v2(key, change_input.diversifier),
            )
            self.assertEqual(proof.output_commitments, (expected,))
            change_tree = ConfidentialTree(compute_confidential_root_v2([expected]), commitments=[expected])
            redemption = prover.prove_unshield(tree=change_tree, inputs=[change_input], public_amount=3)
            self.assertEqual(redemption.relation, "full_redemption")
            self.assertTrue(redemption.proof)

    def test_real_proof_releases_gil_and_closes_native_owner(self):
        network = NetworkId.from_bytes(hash_blake2b_32(b"installed-python-wallet-test"))
        asset = str(AssetDefinitionId.from_domain_and_name("example.is", "native-test"))
        key = token_bytes(32)
        diversifier = derive_confidential_diversifier_v2(token_bytes(32))
        note = ConfidentialInput(7, token_bytes(32), diversifier, 0)
        owner = derive_confidential_owner_tag_v2(key, diversifier)
        commitment = derive_confidential_note_v2(asset, note.amount, note.rho, owner)
        tree = ConfidentialTree(compute_confidential_root_v2([commitment]), commitments=[commitment])
        ready = Event()
        stop = Event()
        progress = []

        def other_python_thread():
            ready.set()
            while not stop.wait(0.01):
                progress.append(None)

        thread = Thread(target=other_python_thread, daemon=True)
        thread.start()
        self.assertTrue(ready.wait(5), "Python progress worker did not start")
        try:
            with ConfidentialProver(network, asset, key) as prover:
                with self.assertRaises(ConfidentialProverError) as failure:
                    prover.prove_unshield(tree=tree, inputs=[note], public_amount=8)
                self.assertEqual(failure.exception.code, "invalid_public_amount")
                before = len(progress)
                proof = prover.prove_unshield(tree=tree, inputs=[note], public_amount=7)
                self.assertGreater(len(progress) - before, 1, "proof blocked Python thread progress")
                self.assertEqual(proof.relation, "full_redemption")
                self.assertEqual(proof.backend, "halo2/ipa")
                self.assertTrue(proof.proof)
                self.assertEqual(proof.root, tree.root)
                self.assertEqual(len(proof.nullifiers), 1)
                self.assertEqual(proof.output_commitments, ())
                print(f"Installed native proof: {len(proof.proof)} bytes; Python progress observed.")
            prover.close()
            with self.assertRaises(ConfidentialProverError) as failure:
                prover.prove_unshield(tree=tree, inputs=[note], public_amount=7)
            self.assertEqual(failure.exception.code, "closed")
        finally:
            stop.set()
            thread.join(5)
            self.assertFalse(thread.is_alive())


if __name__ == "__main__":
    unittest.main()
