// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;
pragma experimental ABIEncoderV2;

import "../../evm/sccp/TairaXorExactEvmSccpBridge.sol";
/** Concrete exact XOR route bound to one predeployed BSC token. */
contract TairaXorBscSccpBridge is TairaXorExactEvmSccpBridge {
    constructor(
        address tokenAddress,
        VerifierPolicyV1 memory configuredVerifierPolicy,
        uint8 configuredBscProfile,
        uint32 configuredRouteRevision
    ) TairaXorExactEvmSccpBridge(
        tokenAddress,
        configuredVerifierPolicy,
        2,
        configuredBscProfile,
        configuredRouteRevision
    ) {}

    /** Return the exact SCCP BSC profile tag (`4` mainnet or `5` testnet). */
    function bscProfile() external view returns (uint8) {
        return networkProfile;
    }
}
