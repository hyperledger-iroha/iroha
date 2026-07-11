// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.24;

import "../../evm/sccp/TairaXorEvmToken.sol";

/** Concrete BEP20/ERC20 wrapped XOR token for the Taira/BSC SCCP route. */
contract TairaXOR is TairaXorEvmToken {
    constructor(address routeBridge) TairaXorEvmToken(routeBridge) {}
}
