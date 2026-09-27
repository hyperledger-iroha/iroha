// Value, fees and storage rent of the SCCP v1 TON contracts (§5.3.5).
//
// Required values are computed at entry from the current price config with
// `GETGASFEE`, `GETFORWARDFEE` and `GETSTORAGEFEE`; there is no fixed TON
// amount anywhere, so price-config changes cannot break a flow. The gas
// limits are the measured per-step maxima (n = 31, t = 21, maximal paths and
// payloads) with headroom, and the size constants cover the largest message
// and state of each kind. `tests/unit/sccp-fees.test.tolk` pins every
// constant against measured gas and actual cell sizes.

import "@stdlib/gas-payments"
import "sccp-constants"

/// 100 years (§5.3.5 minter and bucket floors).
const SCCP_FLOOR_SECONDS = 3153600000
/// 5 years: TEP-74 reference wallet minimum storage.
const SCCP_WALLET_MIN_STORAGE_SECONDS = 157680000

// Per-step gas limits: measured maxima (Acton 1.2.0 emulator, n = 31,
// t = 21, block path 9, history path 32, 1024-byte sender or recipient) plus
// about 20% headroom. `tests/unit/sccp-fees.test.tolk` prints the measured
// values and fails when one exceeds its limit.
const SCCP_GAS_MINTER_FINALIZE = 205000
const SCCP_GAS_MINTER_VOID = 205000
const SCCP_GAS_MINTER_VOID_FROZEN = 16000
const SCCP_GAS_MINTER_DEPLOY_BUCKETS = 60000
const SCCP_GAS_MINTER_CONSUMED = 14000
const SCCP_GAS_MINTER_RANGE_REPLY = 8000
const SCCP_GAS_MINTER_BURN = 48000
const SCCP_GAS_BUCKET_CONSUME = 8000
const SCCP_GAS_BUCKET_CONSUME_RANGE = 9000
const SCCP_GAS_BUCKET_TOP_UP = 2000
const SCCP_GAS_BUCKET_UNCONSUME = 5000
const SCCP_GAS_WALLET_RECEIVE = 8000
const SCCP_GAS_WALLET_TRANSFER = 8000
const SCCP_GAS_WALLET_BURN = 7000

// Account state sizes (code + data), upper bounds of the compiled contracts
// and of the maximal minter data (n = 31 current and previous rosters).
const SCCP_BUCKET_STATE_BITS = 8000
const SCCP_BUCKET_STATE_CELLS = 24
const SCCP_WALLET_STATE_BITS = 7500
const SCCP_WALLET_STATE_CELLS = 16
const SCCP_MINTER_STATE_BITS = 92000
const SCCP_MINTER_STATE_CELLS = 185

// StateInit sizes attached to deployments (code + data + root), upper bounds.
const SCCP_BUCKET_STATE_INIT_BITS = 8000
const SCCP_BUCKET_STATE_INIT_CELLS = 24
const SCCP_WALLET_STATE_INIT_BITS = 7500
const SCCP_WALLET_STATE_INIT_CELLS = 17

// Message sizes (header and body cells), upper bounds.
const SCCP_MSG_CONSUME_BITS = 1800
const SCCP_MSG_CONSUME_CELLS = 3
const SCCP_MSG_SMALL_BITS = 1000
const SCCP_MSG_SMALL_CELLS = 2
const SCCP_MSG_INTERNAL_TRANSFER_BITS = 1600
const SCCP_MSG_INTERNAL_TRANSFER_CELLS = 2
const SCCP_MSG_EXT_EVENT_BITS = 1200
const SCCP_MSG_EXT_EVENT_CELLS = 2
/// `sccp_transfer_to_taira` with a 1138-byte payload in 10 snake cells.
const SCCP_MSG_EXT_TRANSFER_BITS = 11000
const SCCP_MSG_EXT_TRANSFER_CELLS = 13
/// `burn_notification` with a 1024-byte recipient in 9 snake cells.
const SCCP_MSG_BURN_NOTIFICATION_BITS = 10000
const SCCP_MSG_BURN_NOTIFICATION_CELLS = 13

@inline
fun sccpGasFee(gas: int): coins {
    return calculateGasFee(SCCP_WORKCHAIN, gas);
}

@inline
fun sccpForwardFee(bits: int, cells: int): coins {
    return calculateForwardFee(SCCP_WORKCHAIN, bits, cells);
}

/// `BUCKET_FLOOR`: 100 years of a bucket's storage at current prices.
fun sccpBucketFloor(): coins {
    return calculateStorageFee(
        SCCP_WORKCHAIN,
        SCCP_FLOOR_SECONDS,
        SCCP_BUCKET_STATE_BITS,
        SCCP_BUCKET_STATE_CELLS,
    );
}

/// `MINTER_FLOOR`: 100 years of the minter's maximal storage at current prices.
fun sccpMinterFloor(): coins {
    return calculateStorageFee(
        SCCP_WORKCHAIN,
        SCCP_FLOOR_SECONDS,
        SCCP_MINTER_STATE_BITS,
        SCCP_MINTER_STATE_CELLS,
    );
}

/// Storage a freshly deployed wallet keeps (TEP-74 reference: 5 years).
fun sccpWalletMinStorage(): coins {
    return calculateStorageFee(
        SCCP_WORKCHAIN,
        SCCP_WALLET_MIN_STORAGE_SECONDS,
        SCCP_WALLET_STATE_BITS,
        SCCP_WALLET_STATE_CELLS,
    );
}

/// Value attached to a bucket deployment that carries no consume.
fun sccpBucketDeployValue(): coins {
    return sccpGasFee(SCCP_GAS_BUCKET_TOP_UP) + sccpBucketFloor();
}

/// Cost of one extra bucket deployment sent by the minter (value plus
/// forwarding, paid separately).
fun sccpExtraBucketDeployCost(): coins {
    return sccpBucketDeployValue() +
        sccpForwardFee(
            SCCP_MSG_SMALL_BITS + SCCP_BUCKET_STATE_INIT_BITS,
            SCCP_MSG_SMALL_CELLS + SCCP_BUCKET_STATE_INIT_CELLS,
        );
}

/// Forwarding of `sccp_consume`, with the bucket StateInit when it deploys.
fun sccpConsumeForwardFee(deploys: bool): coins {
    if (deploys) {
        return sccpForwardFee(
            SCCP_MSG_CONSUME_BITS + SCCP_BUCKET_STATE_INIT_BITS,
            SCCP_MSG_CONSUME_CELLS + SCCP_BUCKET_STATE_INIT_CELLS,
        );
    }
    return sccpForwardFee(SCCP_MSG_CONSUME_BITS, SCCP_MSG_CONSUME_CELLS);
}

/// Bucket round trip of one consume: forwarding there and back, bucket gas
/// and the bucket floor top-up.
fun sccpConsumeRoundTrip(deploys: bool): coins {
    return sccpConsumeForwardFee(deploys) + sccpGasFee(SCCP_GAS_BUCKET_CONSUME) +
        sccpBucketFloor() +
        sccpForwardFee(SCCP_MSG_CONSUME_BITS, SCCP_MSG_CONSUME_CELLS);
}

/// Excess return to `response`.
fun sccpExcessForwardFee(): coins {
    return sccpForwardFee(SCCP_MSG_SMALL_BITS, SCCP_MSG_SMALL_CELLS);
}

/// `finalize_required_value()` for a finalize that deploys `deployCount`
/// buckets (0 when the nonce's bucket exists).
fun sccpFinalizeRequiredValue(deployCount: int): coins {
    val extra = deployCount > 1 ? deployCount - 1 : 0;
    return sccpGasFee(SCCP_GAS_MINTER_FINALIZE) + sccpConsumeRoundTrip(deployCount > 0) +
        sccpGasFee(SCCP_GAS_MINTER_CONSUMED) +
        sccpForwardFee(SCCP_MSG_EXT_EVENT_BITS, SCCP_MSG_EXT_EVENT_CELLS) +
        sccpForwardFee(
            SCCP_MSG_INTERNAL_TRANSFER_BITS + SCCP_WALLET_STATE_INIT_BITS,
            SCCP_MSG_INTERNAL_TRANSFER_CELLS + SCCP_WALLET_STATE_INIT_CELLS,
        ) +
        sccpGasFee(SCCP_GAS_WALLET_RECEIVE) +
        sccpWalletMinStorage() +
        sccpExcessForwardFee() +
        extra * sccpExtraBucketDeployCost();
}

/// `void_required_value()` for a void that deploys `deployCount` buckets.
fun sccpVoidRequiredValue(deployCount: int): coins {
    val extra = deployCount > 1 ? deployCount - 1 : 0;
    return sccpGasFee(SCCP_GAS_MINTER_VOID) + sccpConsumeRoundTrip(deployCount > 0) +
        sccpGasFee(SCCP_GAS_MINTER_CONSUMED) +
        sccpForwardFee(SCCP_MSG_EXT_EVENT_BITS, SCCP_MSG_EXT_EVENT_CELLS) +
        sccpExcessForwardFee() +
        extra * sccpExtraBucketDeployCost();
}

/// Value the minter attaches to `sccp_consume_range`: bucket gas and floor,
/// the reply and the minter's handling of it.
fun sccpRangeConsumeValue(): coins {
    return sccpGasFee(SCCP_GAS_BUCKET_CONSUME_RANGE) + sccpBucketFloor() +
        sccpForwardFee(SCCP_MSG_SMALL_BITS, SCCP_MSG_SMALL_CELLS) +
        sccpGasFee(SCCP_GAS_MINTER_RANGE_REPLY) +
        sccpForwardFee(SCCP_MSG_EXT_EVENT_BITS, SCCP_MSG_EXT_EVENT_CELLS);
}

/// Required value of `sccp_void_frozen` deploying `deployCount` buckets.
fun sccpVoidFrozenRequiredValue(deployCount: int): coins {
    val extra = deployCount > 1 ? deployCount - 1 : 0;
    val rangeForward = deployCount > 0
        ? sccpForwardFee(
            SCCP_MSG_SMALL_BITS + SCCP_BUCKET_STATE_INIT_BITS,
            SCCP_MSG_SMALL_CELLS + SCCP_BUCKET_STATE_INIT_CELLS,
        )
        : sccpForwardFee(SCCP_MSG_SMALL_BITS, SCCP_MSG_SMALL_CELLS);
    return sccpGasFee(SCCP_GAS_MINTER_VOID_FROZEN) + rangeForward + sccpRangeConsumeValue() +
        sccpExcessForwardFee() +
        extra * sccpExtraBucketDeployCost();
}

/// Required value of `sccp_deploy_buckets` for `count` buckets.
fun sccpDeployBucketsRequiredValue(count: int): coins {
    return sccpGasFee(SCCP_GAS_MINTER_DEPLOY_BUCKETS) + count * sccpExtraBucketDeployCost() +
        sccpExcessForwardFee();
}

/// Value the minter attaches to `sccp_unconsume` (forwarding paid separately).
fun sccpUnconsumeValue(): coins {
    return sccpGasFee(SCCP_GAS_BUCKET_UNCONSUME);
}

/// Minimal value of an SCCP burn sent to a wallet: the wallet step, the
/// `burn_notification` with a maximal recipient, the minter step, the
/// `sccp_transfer_to_taira` external message and the excess return.
fun sccpBurnRequiredValue(): coins {
    return sccpGasFee(SCCP_GAS_WALLET_BURN) +
        sccpForwardFee(SCCP_MSG_BURN_NOTIFICATION_BITS, SCCP_MSG_BURN_NOTIFICATION_CELLS) +
        sccpGasFee(SCCP_GAS_MINTER_BURN) +
        sccpForwardFee(SCCP_MSG_EXT_TRANSFER_BITS, SCCP_MSG_EXT_TRANSFER_CELLS) +
        sccpExcessForwardFee();
}

/// Minimal value of a wallet-to-wallet transfer (TEP-74 reference formula).
fun sccpTransferRequiredValue(forwardTonAmount: int, fwdFee: int): coins {
    val forwardCount = forwardTonAmount != 0 ? 2 : 1;
    return forwardTonAmount + forwardCount * fwdFee +
        calculateForwardFeeWithoutLumpPrice(
            SCCP_WORKCHAIN,
            SCCP_WALLET_STATE_INIT_BITS,
            SCCP_WALLET_STATE_INIT_CELLS,
        ) +
        sccpGasFee(SCCP_GAS_WALLET_TRANSFER) +
        sccpGasFee(SCCP_GAS_WALLET_RECEIVE) +
        sccpWalletMinStorage();
}

/// Reserves `max(floor, balance before this message)` (mode 2): the account
/// never drops below its floor or its prior balance, and every later
/// "carry all balance" send forwards only the incoming value.
fun sccpReserveFloor(floor: coins, incomingValue: coins) {
    val before = contract.getOriginalBalance() - incomingValue;
    reserveGramsOnBalance(max(floor, before), RESERVE_MODE_AT_MOST);
}

/// Part of `MINTER_FLOOR` that the incoming value must cover because the
/// minter's balance before this message is below it.
fun sccpMinterFloorDeficit(incomingValue: coins): coins {
    val before = contract.getOriginalBalance() - incomingValue;
    val floor = sccpMinterFloor();
    return before >= floor ? 0 : floor - before;
}
