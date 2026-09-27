// SccpConsumedBucket: 512 replay flags of the SCCP v1 TON minter (§5.3.3,
// §5.3.4). Bucket `index` holds the flags of outbound nonces
// `index·512 .. index·512 + 511`. Only its minter may set or clear flags; the
// minter deploys each bucket exactly once and never re-attaches a StateInit,
// so flags can never be reset by redeployment.

import "sccp-constants"
import "sccp-errors"
import "sccp-fees"
import "sccp-messages"
import "sccp-storage"

contract SccpConsumedBucket {
    author: "Hyperledger Iroha"
    version: "1.0.0"
    description: "SCCP v1 consumption bucket: 512 outbound-nonce flags of one Taira XOR minter"
    storage: BucketStorage
    incomingMessages: BucketMessage
}

type BucketMessage = SccpConsume | SccpConsumeRange | SccpUnconsume | TopUpTons

/// Replies of the bucket that the minter may bounce back.
type BucketBounce = SccpConsumed | SccpRangeConsumed

/// `2^width − 1` for `0 ≤ width ≤ 256` (2^256 itself overflows int257).
@pure
fun bucketOnes(width: int): int {
    if (width == 256) {
        return ((1 << 255) - 1) * 2 + 1;
    }
    return (1 << width) - 1;
}

/// Mask of flags `[offset, offset + count)` of a 512-bit bitmap as (hi, lo),
/// for `0 ≤ offset` and `offset + count ≤ 512`.
@pure
fun bucketRangeMask(offset: int, count: int): (int, int) {
    val end = offset + count;
    var lo = 0;
    var hi = 0;
    if (offset < 256) {
        val loEnd = min(end, 256);
        lo = bucketOnes(loEnd - offset) << offset;
    }
    if (end > 256) {
        val hiStart = max(offset, 256) - 256;
        hi = bucketOnes(end - 256 - hiStart) << hiStart;
    }
    return (hi, lo);
}

/// Flag of `nonce` within this bucket as (hi, lo) single-bit masks.
fun BucketStorage.flagMask(self, nonce: int): (int, int) {
    assert ((nonce >> SCCP_BUCKET_SHIFT) == self.index) throw SccpErrors.BadNonce;
    return bucketRangeMask(nonce & SCCP_BUCKET_MASK, 1);
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy BucketMessage.fromSlice(in.body);

    match (msg) {
        SccpConsume => {
            var storage = lazy BucketStorage.load();
            assert (in.senderAddress == storage.minterAddress) throw SccpErrors.NotMinter;
            val (hi, lo) = storage.flagMask(msg.nonce);
            sccpReserveFloor(sccpBucketFloor(), in.valueCoins);
            if (((storage.bitsHi & hi) | (storage.bitsLo & lo)) == 0) {
                storage.bitsHi |= hi;
                storage.bitsLo |= lo;
                storage.save();
                val reply = createMessage({
                    bounce: BounceMode.Only256BitsOfBody,
                    dest: storage.minterAddress,
                    value: 0,
                    body: SccpConsumed {
                        queryId: msg.queryId,
                        nonce: msg.nonce,
                        amount: msg.amount,
                        tail: msg.tail,
                    },
                });
                reply.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
            } else {
                val reply = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: storage.minterAddress,
                    value: 0,
                    body: SccpAlreadyConsumed {
                        queryId: msg.queryId,
                        nonce: msg.nonce,
                        amount: msg.amount,
                        tail: msg.tail,
                    },
                });
                reply.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
            }
        }

        SccpConsumeRange => {
            var storage = lazy BucketStorage.load();
            assert (in.senderAddress == storage.minterAddress) throw SccpErrors.NotMinter;
            val first = msg.first;
            val count = msg.count;
            assert ((count >= 1) & (count <= SCCP_BUCKET_BITS)) throw SccpErrors.BadRange;
            assert ((first >> SCCP_BUCKET_SHIFT) == storage.index) throw SccpErrors.BadRange;
            assert (
                ((first + count - 1) >> SCCP_BUCKET_SHIFT) == storage.index
            ) throw SccpErrors.BadRange;
            val (hi, lo) = bucketRangeMask(first & SCCP_BUCKET_MASK, count);
            sccpReserveFloor(sccpBucketFloor(), in.valueCoins);
            if (((storage.bitsHi & hi) | (storage.bitsLo & lo)) == 0) {
                storage.bitsHi |= hi;
                storage.bitsLo |= lo;
                storage.save();
                val reply = createMessage({
                    bounce: BounceMode.Only256BitsOfBody,
                    dest: storage.minterAddress,
                    value: 0,
                    body: SccpRangeConsumed {
                        queryId: msg.queryId,
                        first,
                        count,
                    },
                });
                reply.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
            } else {
                val reply = createMessage({
                    bounce: BounceMode.NoBounce,
                    dest: storage.minterAddress,
                    value: 0,
                    body: SccpRangeRejected {
                        queryId: msg.queryId,
                        first,
                        count,
                    },
                });
                reply.send(SEND_MODE_CARRY_ALL_BALANCE | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
            }
        }

        SccpUnconsume => {
            var storage = lazy BucketStorage.load();
            assert (in.senderAddress == storage.minterAddress) throw SccpErrors.NotMinter;
            val (hi, lo) = storage.flagMask(msg.nonce);
            storage.bitsHi &= ~hi;
            storage.bitsLo &= ~lo;
            storage.save();
        }

        TopUpTons => {
            // anyone may extend the bucket's storage floor
        }

        else => {
            assert (in.body.isEmpty()) throw SccpErrors.InvalidMessage;
        }
    }
}

/// A bounced `sccp_consumed` / `sccp_range_consumed` means the minter failed
/// to handle it: clear the flags again so the nonces stay finalizable or
/// voidable (§5.3.4 step 4).
fun onBouncedMessage(in: InMessageBounced) {
    var storage = lazy BucketStorage.load();
    if (in.senderAddress != storage.minterAddress) {
        return;
    }
    in.bouncedBody.skipBouncedPrefix();
    val msg = lazy BucketBounce.fromSlice(in.bouncedBody);
    match (msg) {
        SccpConsumed => {
            val (hi, lo) = storage.flagMask(msg.nonce);
            storage.bitsHi &= ~hi;
            storage.bitsLo &= ~lo;
            storage.save();
        }
        SccpRangeConsumed => {
            val first = msg.first;
            val count = msg.count;
            if ((first >> SCCP_BUCKET_SHIFT) != storage.index) {
                return;
            }
            val (hi, lo) = bucketRangeMask(first & SCCP_BUCKET_MASK, count);
            storage.bitsHi &= ~hi;
            storage.bitsLo &= ~lo;
            storage.save();
        }
    }
}

/// `(minter, index, bits_hi, bits_lo)`; flag `i` is bit `i` of `hi ‖ lo`.
get fun get_bucket_data(): BucketDataReply {
    val storage = lazy BucketStorage.load();
    return {
        minterAddress: storage.minterAddress,
        index: storage.index,
        bitsHi: storage.bitsHi,
        bitsLo: storage.bitsLo,
    };
}

/// The 512 flags as `(bits_hi, bits_lo)` (§5.3.6).
get fun get_bits(): (int, int) {
    val storage = lazy BucketStorage.load();
    return (storage.bitsHi, storage.bitsLo);
}

/// Whether `nonce` (which must belong to this bucket) is consumed.
get fun is_consumed(nonce: int): bool {
    val storage = lazy BucketStorage.load();
    val (hi, lo) = storage.flagMask(nonce);
    return ((storage.bitsHi & hi) | (storage.bitsLo & lo)) != 0;
}
