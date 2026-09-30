// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import { Buffer } from "node:buffer";

export const SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024;

/** Own exact, bounded transaction bytes before any asynchronous preflight. */
export function snapshotSorafsOrderbookSubmissionBytes(signedTransaction, context) {
  let view;
  if (ArrayBuffer.isView(signedTransaction)) {
    view = new Uint8Array(
      signedTransaction.buffer,
      signedTransaction.byteOffset,
      signedTransaction.byteLength,
    );
  } else if (signedTransaction instanceof ArrayBuffer) {
    view = new Uint8Array(signedTransaction);
  } else {
    throw new TypeError(`${context}.signedTransaction must be exact bytes`);
  }
  if (view.byteLength === 0 || view.byteLength > SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1) {
    throw new RangeError(
      `${context}.signedTransaction must contain 1..${SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1} bytes`,
    );
  }
  return Buffer.from(view);
}
