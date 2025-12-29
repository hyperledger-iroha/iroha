package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.client.transport.TransportRequest;

/**
 * Immutable snapshot of a Norito RPC call attempt. Fallback handlers use the context to reissue the
 * request via alternate transports.
 */
public final class NoritoRpcCallContext {
  private final URI baseUri;
  private final URI resolvedUri;
  private final String path;
  private final byte[] payload;
  private final NoritoRpcRequestOptions options;
  private final TransportRequest request;

  NoritoRpcCallContext(
      final URI baseUri,
      final URI resolvedUri,
      final String path,
      final byte[] payload,
      final NoritoRpcRequestOptions options,
      final TransportRequest request) {
    this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
    this.resolvedUri = Objects.requireNonNull(resolvedUri, "resolvedUri");
    this.path = Objects.requireNonNull(path, "path");
    this.payload = payload == null ? null : Arrays.copyOf(payload, payload.length);
    this.options = Objects.requireNonNull(options, "options");
    this.request = Objects.requireNonNull(request, "request");
  }

  /** Base URI supplied by {@link NoritoRpcClient.Builder#setBaseUri(URI)}. */
  public URI baseUri() {
    return baseUri;
  }

  /** Fully resolved URI used for the Norito RPC attempt. */
  public URI resolvedUri() {
    return resolvedUri;
  }

  /** Path argument supplied to {@link NoritoRpcClient#call(String, byte[], NoritoRpcRequestOptions)}. */
  public String path() {
    return path;
  }

  /** Returns a defensive copy of the payload bytes, or {@code null} when the body was empty. */
  public byte[] payload() {
    return payload == null ? null : Arrays.copyOf(payload, payload.length);
  }

  /** Options (headers, query parameters, timeout) used for the call. */
  public NoritoRpcRequestOptions options() {
    return options;
  }

  /** Raw {@link TransportRequest} emitted by the Norito RPC client. */
  public TransportRequest request() {
    return request;
  }
}
