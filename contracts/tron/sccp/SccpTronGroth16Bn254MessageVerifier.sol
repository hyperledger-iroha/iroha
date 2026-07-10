// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/SccpGroth16Bn254MessageVerifier.sol";

/**
 * @title SccpTronGroth16Bn254MessageVerifier
 * @dev TRON/TVM deployment entrypoint for the SCCP BN254 Groth16 verifier.
 *
 * TVM is EVM-compatible for Solidity contracts and supports the altbn128
 * operations used by the inherited verifier. This wrapper keeps the deployed
 * contract name and artifact path TRON-specific while pinning the exact
 * SCCP public-signal, verifying-key, and proof-validation metadata shared with
 * EVM-compatible lanes. Replay protection and the route-specific destination
 * binding belong to the token route contract, because one verifier deployment
 * may safely serve several independently bound routes.
 */
contract SccpTronGroth16Bn254MessageVerifier is SccpGroth16Bn254MessageVerifier {
    uint32 private constant SCCP_DOMAIN_SORA = 0;
    uint32 private constant SCCP_DOMAIN_TRON = 5;

    bytes32 private constant TRON_GROTH16_BACKEND_HASH =
        keccak256("tron-groth16-bn254-v1");
    bytes32 private constant STARK_FRI_PROOF_FAMILY_HASH =
        keccak256("stark-fri-v1");

    bytes32 public immutable verifierKeyHash;
    bytes32 public immutable verifierBackendHash;
    bytes32 public immutable proofFamilyHash;
    bytes32 public immutable networkId;
    uint32 public immutable expectedSourceDomain;
    uint32 public immutable expectedTargetDomain;

    event VerifierBound(
        address indexed verifier,
        bytes32 verifierKeyHash,
        bytes32 verifierBackendHash,
        bytes32 proofFamilyHash,
        bytes32 semanticProofProfileHash,
        bytes32 soraFinalityAnchorHash
    );

    constructor(
        uint256[2] memory configuredAlpha1,
        uint256[4] memory configuredBeta2,
        uint256[4] memory configuredGamma2,
        uint256[4] memory configuredDelta2,
        uint256[] memory configuredIc,
        bytes32 configuredSemanticProofProfileHash,
        bytes32 configuredSoraFinalityAnchorHash,
        bytes32 expectedVerifierKeyHash,
        string memory proofFamily,
        bytes32 configuredNetworkId,
        uint32 configuredSourceDomain,
        uint32 configuredTargetDomain
    )
        SccpGroth16Bn254MessageVerifier(
            configuredAlpha1,
            configuredBeta2,
            configuredGamma2,
            configuredDelta2,
            configuredIc,
            configuredSemanticProofProfileHash,
            configuredSoraFinalityAnchorHash
        )
    {
        require(
            expectedVerifierKeyHash != bytes32(0),
            "Verifier key hash is required"
        );
        require(
            verifyingKeyHash() == expectedVerifierKeyHash,
            "Verifier key hash mismatch"
        );
        require(bytes(proofFamily).length != 0, "Proof family is required");
        require(configuredNetworkId != bytes32(0), "Network id is required");
        require(
            configuredTargetDomain == SCCP_DOMAIN_TRON,
            "Target domain must be TRON"
        );
        require(
            configuredSourceDomain == SCCP_DOMAIN_SORA,
            "Source domain must be SORA"
        );
        require(
            configuredSourceDomain != configuredTargetDomain,
            "Source and target domains must differ"
        );

        bytes32 configuredProofFamilyHash = keccak256(bytes(proofFamily));
        require(
            configuredProofFamilyHash == STARK_FRI_PROOF_FAMILY_HASH,
            "Proof family must be stark-fri-v1"
        );

        verifierKeyHash = expectedVerifierKeyHash;
        verifierBackendHash = TRON_GROTH16_BACKEND_HASH;
        proofFamilyHash = configuredProofFamilyHash;
        networkId = configuredNetworkId;
        expectedSourceDomain = configuredSourceDomain;
        expectedTargetDomain = configuredTargetDomain;

        emit VerifierBound(
            address(this),
            expectedVerifierKeyHash,
            TRON_GROTH16_BACKEND_HASH,
            configuredProofFamilyHash,
            configuredSemanticProofProfileHash,
            configuredSoraFinalityAnchorHash
        );
    }

    /** Return the immutable verifier runtime code hash used by deployment tooling. */
    function verifierCodeHash() public view returns (bytes32) {
        return _runtimeCodeHash();
    }

    function _runtimeCodeHash() internal view returns (bytes32 codeHash) {
        address account = address(this);
        assembly {
            codeHash := extcodehash(account)
        }
    }

}
