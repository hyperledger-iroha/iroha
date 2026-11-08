// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/** Wait for queued microtasks and one macrotask. */
export function settle(ms = 0) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Resolve once `predicate()` holds, polling every few milliseconds. */
export async function waitFor(predicate, { timeout = 2000, label = "condition" } = {}) {
  const started = Date.now();
  while (!predicate()) {
    if (Date.now() - started > timeout) {
      throw new Error(`timed out waiting for ${label}`);
    }
    await settle(5);
  }
}
