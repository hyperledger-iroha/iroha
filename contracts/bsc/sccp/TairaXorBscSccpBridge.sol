// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "../../evm/sccp/TairaXorExactEvmSccpBridge.sol";

/** Concrete exact XOR transfer route for BSC mainnet or BSC testnet. */
contract TairaXorBscSccpBridge is TairaXorExactEvmSccpBridge {
    constructor(
        address tokenAddress,
        address verifierAddress,
        bytes32 expectedVerifierCodeHash,
        bytes32 expectedVerifierKeyHash,
        bytes32 expectedSemanticProofProfileHash,
        bytes32 expectedSoraFinalityAnchorHash,
        uint8 configuredBscProfile,
        uint32 configuredRouteRevision
    ) TairaXorExactEvmSccpBridge(
        tokenAddress,
        verifierAddress,
        expectedVerifierCodeHash,
        expectedVerifierKeyHash,
        expectedSemanticProofProfileHash,
        expectedSoraFinalityAnchorHash,
        2,
        configuredBscProfile,
        configuredRouteRevision
    ) {}

    /** Return the exact SCCP BSC profile tag (`4` mainnet or `5` testnet). */
    function bscProfile() external view returns (uint8) {
        return networkProfile;
    }
}
