// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { snapshotBoundedBytes } from "./boundedByteSnapshot.js";

export const SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024;

/** Own exact, bounded transaction bytes before any asynchronous preflight. */
export function snapshotSorafsOrderbookSubmissionBytes(signedTransaction, context) {
  return snapshotBoundedBytes(
    signedTransaction,
    `${context}.signedTransaction`,
    SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1,
    RangeError,
  );
}
