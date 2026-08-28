// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;
pragma experimental ABIEncoderV2;

import "../../evm/sccp/TairaXorExactEvmSccpBridge.sol";
/** Concrete exact XOR route bound to one predeployed Ethereum token. */
contract TairaXorEthereumSccpBridge is TairaXorExactEvmSccpBridge {
    constructor(
        address tokenAddress,
        VerifierPolicyV1 memory configuredVerifierPolicy,
        uint8 configuredEthereumProfile,
        uint32 configuredRouteRevision,
        uint256 configuredMaxWrappedSupply
    ) TairaXorExactEvmSccpBridge(
        tokenAddress,
        configuredVerifierPolicy,
        1,
        configuredEthereumProfile,
        configuredRouteRevision,
        configuredMaxWrappedSupply
    ) {}

    /** Return the exact SCCP Ethereum profile tag (`2` mainnet or `3` Sepolia). */
    function ethereumProfile() external view returns (uint8) {
        return networkProfile;
    }
}
