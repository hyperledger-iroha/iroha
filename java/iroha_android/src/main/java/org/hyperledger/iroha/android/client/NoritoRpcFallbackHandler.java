package org.hyperledger.iroha.android.client;

/**
 * Handles fallback execution when Norito RPC calls fail (for example, routing through the legacy
 * JSON pipeline).
 */
@FunctionalInterface
public interface NoritoRpcFallbackHandler {

  /**
   * Attempts to handle the failed request.
   *
   * @param context immutable snapshot of the attempted Norito RPC call
   * @param cause failure reported by the primary Norito RPC attempt
   * @return response bytes to return to the caller; {@code null} indicates that the handler could
   *     not recover from the failure and the original exception should be re-thrown
   * @throws Exception when fallback processing fails; callers receive a wrapped
   *     {@link NoritoRpcException}
   */
  byte[] handle(NoritoRpcCallContext context, NoritoRpcException cause) throws Exception;
}
