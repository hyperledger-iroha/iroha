// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Default bindings to the current Iroha JavaScript SDK.
 *
 * Loaded lazily the first time a page starts pairing, so the button and the
 * sakura layer render without pulling in the Connect cryptography.
 */

import {
  ConnectApprovalRejectedError,
  ConnectSessionClosedError,
  ConnectSignRequestError,
  NetworkId,
  TORII_CANONICAL_REQUEST_DOMAIN_TAG,
  createConnectAppSession,
  createConnectCanonicalRequestAuth,
  createConnectSessionPreview,
  deleteConnectSession,
  registerConnectSession,
  resolveConnectLaunchUri,
  resolveConnectLaunchUriForProtocol,
} from "@iroha/iroha-js/connect-browser";
import { ToriiBrowserClient } from "@iroha/iroha-js/torii-browser";

/**
 * Browser wallet approval is verified against Torii's native account
 * inspection on an unsigned client bound to the exact Torii origin and
 * NetworkId; the browser never substitutes its own I105 codec.
 */
export function createAccountInspector({ baseUrl, networkId }) {
  const client = new ToriiBrowserClient(baseUrl, { networkId });
  return (accountId, options) => client.inspectAccountIdentity(accountId, options);
}

export default Object.freeze({
  ConnectApprovalRejectedError,
  ConnectSessionClosedError,
  ConnectSignRequestError,
  NetworkId,
  TORII_CANONICAL_REQUEST_DOMAIN_TAG,
  createAccountInspector,
  createConnectAppSession,
  createConnectCanonicalRequestAuth,
  createConnectSessionPreview,
  deleteConnectSession,
  registerConnectSession,
  resolveConnectLaunchUri,
  resolveConnectLaunchUriForProtocol,
});
