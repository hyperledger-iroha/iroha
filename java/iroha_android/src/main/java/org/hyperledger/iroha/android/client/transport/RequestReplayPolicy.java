package org.hyperledger.iroha.android.client.transport;

/**
 * Closed transport replay policy derived from the complete HTTP request.
 *
 * <p>{@link #ONE_SHOT} requests may carry a signature, nonce, credential, or mutating body. An
 * executor must make exactly one underlying network dispatch and must not follow redirects,
 * authenticate again, or retry after any response or transport failure. {@link #RETRY_SAFE} is
 * reserved for bodyless, unsigned {@code GET}, {@code HEAD}, and {@code OPTIONS} requests.
 */
public enum RequestReplayPolicy {
  /** The exact request bytes and headers may be dispatched at most once. */
  ONE_SHOT,

  /** The request is a bodyless, unsigned read and may use an explicit retry policy. */
  RETRY_SAFE
}
