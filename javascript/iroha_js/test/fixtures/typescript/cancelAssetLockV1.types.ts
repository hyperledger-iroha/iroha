import {
  decodeCancelAssetLockV1,
  encodeCancelAssetLockV1,
  type CancelAssetLockV1,
} from "../../../index.js";

const escrowId =
  "hash:3447B4B2BEA882920F6BA3818A0EB5031305000679612DA058D907FECCEF0A85#035F";
const exact: CancelAssetLockV1 = {
  escrow_id: escrowId,
  expected_remaining_amount: "20",
};

const archive: Buffer = encodeCancelAssetLockV1(exact);
const decoded: CancelAssetLockV1 = decodeCancelAssetLockV1(archive);
encodeCancelAssetLockV1({
  escrow_id: escrowId,
  expected_remaining_amount: "20",
});
decodeCancelAssetLockV1(new Uint8Array(archive));

// @ts-expect-error both canonical fields are mandatory
encodeCancelAssetLockV1({ escrow_id: escrowId });
encodeCancelAssetLockV1({
  // @ts-expect-error camelCase aliases are not part of the hard-cut API
  escrowId,
  expectedRemainingAmount: "20",
});
// @ts-expect-error nested compatibility wrappers are not accepted
encodeCancelAssetLockV1({ CancelAssetLock: exact });
// @ts-expect-error array compatibility representations are not accepted
encodeCancelAssetLockV1([exact]);
encodeCancelAssetLockV1({
  // @ts-expect-error EscrowId is the canonical checksummed string, not raw bytes
  escrow_id: new Uint8Array(32),
  expected_remaining_amount: "20",
});
encodeCancelAssetLockV1({
  escrow_id: escrowId,
  // @ts-expect-error quantities must be canonical decimal strings
  expected_remaining_amount: 20,
});
// @ts-expect-error the bare decoder accepts binary input, not textual hex
decodeCancelAssetLockV1("4e525430");
// @ts-expect-error decoded canonical fields are immutable
decoded.escrow_id = escrowId;

void archive;
void decoded;
