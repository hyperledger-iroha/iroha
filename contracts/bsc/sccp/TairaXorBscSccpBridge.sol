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
        uint32 configuredRouteRevision,
        address[5] memory configuredMintGuardians,
        uint256 configuredMaxWrappedSupply
    ) TairaXorExactEvmSccpBridge(
        tokenAddress,
        configuredVerifierPolicy,
        2,
        configuredBscProfile,
        configuredRouteRevision,
        configuredMintGuardians,
        configuredMaxWrappedSupply
    ) {}

    /** Return the exact SCCP BSC mainnet profile tag (`0x42`). */
    function bscProfile() external view returns (uint8) {
        return networkProfile;
    }
}
