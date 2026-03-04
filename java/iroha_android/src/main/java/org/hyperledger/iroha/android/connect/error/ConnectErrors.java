package org.hyperledger.iroha.android.connect.error;

import java.io.EOFException;
import java.io.UncheckedIOException;
import java.lang.reflect.UndeclaredThrowableException;
import java.net.ConnectException;
import java.net.NoRouteToHostException;
import java.net.PortUnreachableException;
import java.net.SocketException;
import java.net.SocketTimeoutException;
import java.net.UnknownHostException;
import java.security.cert.CertificateException;
import java.text.ParseException;
import java.util.Locale;
import java.util.concurrent.CompletionException;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeoutException;
import javax.net.ssl.SSLException;
import java.nio.channels.ClosedChannelException;
import java.nio.channels.UnresolvedAddressException;

/** Helper functions for translating {@link Throwable} instances into {@link ConnectError}. */
public final class ConnectErrors {
  private ConnectErrors() {}

  public static ConnectError from(final Throwable throwable) {
    return from(throwable, null);
  }

  public static ConnectError from(
      final Throwable throwable, final ConnectErrorOptions options) {
    final ConnectError base = classifyOrFallback(throwable);
    if (options == null) {
      return base;
    }
    final boolean overrideFatal = options.fatal() != null;
    final boolean overrideStatus = options.httpStatus() != null;
    if (!overrideFatal && !overrideStatus) {
      return base;
    }
    return base.toBuilder()
        .fatal(overrideFatal ? options.fatal() : base.fatal())
        .httpStatus(
            overrideStatus ? options.httpStatus() : base.httpStatus().orElse(null))
        .build();
  }

  public static ConnectError fromHttpStatus(final int statusCode, final String message) {
    final String payload =
        (message != null && !message.isEmpty()) ? message : ("HTTP " + statusCode);
    final ConnectError.Builder builder =
        ConnectError.builder().message(payload).httpStatus(statusCode);
    if (statusCode >= 500) {
      builder.category(ConnectErrorCategory.TRANSPORT).code("http.server_error");
    } else if (statusCode == 401 || statusCode == 403 || statusCode == 407) {
      builder.category(ConnectErrorCategory.AUTHORIZATION).code("http.forbidden");
    } else if (statusCode >= 400) {
      builder.category(ConnectErrorCategory.AUTHORIZATION).code("http.client_error");
    } else {
      builder.category(ConnectErrorCategory.TRANSPORT).code("http.other");
    }
    return builder.build();
  }

  private static ConnectError classifyOrFallback(final Throwable input) {
    if (input == null) {
      return unknown(null);
    }
    if (input instanceof ConnectError connectError) {
      return connectError;
    }
    if (input instanceof ConnectErrorConvertible convertible) {
      return convertible.toConnectError();
    }
    final Throwable unwrapped = unwrap(input);
    final ConnectError direct = classify(unwrapped);
    if (direct != null) {
      return direct;
    }
    final Throwable cause = unwrapped != null ? unwrapped.getCause() : null;
    if (cause != null) {
      final ConnectError nested = classify(cause);
      if (nested != null) {
        return nested;
      }
    }
    return unknown(unwrapped);
  }

  private static Throwable unwrap(final Throwable error) {
    Throwable current = error;
    while (true) {
      if (current instanceof CompletionException completion && completion.getCause() != null) {
        current = completion.getCause();
        continue;
      }
      if (current instanceof ExecutionException execution && execution.getCause() != null) {
        current = execution.getCause();
        continue;
      }
      if (current instanceof UncheckedIOException unchecked && unchecked.getCause() != null) {
        current = unchecked.getCause();
        continue;
      }
      if (current instanceof UndeclaredThrowableException undeclared
          && undeclared.getCause() != null) {
        current = undeclared.getCause();
        continue;
      }
      break;
    }
    return current;
  }

  private static ConnectError classify(final Throwable error) {
    if (error == null) {
      return null;
    }
    if (error instanceof ConnectError connectError) {
      return connectError;
    }
    if (error instanceof ConnectErrorConvertible convertible) {
      return convertible.toConnectError();
    }
    if (error instanceof SSLException || error instanceof CertificateException) {
      return newError(
          ConnectErrorCategory.AUTHORIZATION,
          "network.tls_failure",
          error,
          "TLS negotiation failure");
    }
    if (isTimeout(error)) {
      return newError(
          ConnectErrorCategory.TIMEOUT, "network.timeout", error, "Connect timeout");
    }
    if (isTransport(error)) {
      final String code =
          error instanceof SocketException && messageContains(error, "reset")
              ? "client.closed"
              : "network.socket_failure";
      return newError(ConnectErrorCategory.TRANSPORT, code, error, "Transport failure");
    }
    if (isCodec(error)) {
      return newError(
          ConnectErrorCategory.CODEC, "codec.invalid_payload", error, "Codec failure");
    }
    return null;
  }

  private static boolean isTimeout(final Throwable error) {
    return isClass(error, "java.net.http.HttpTimeoutException")
        || isClass(error, "java.net.http.HttpConnectTimeoutException")
        || error instanceof SocketTimeoutException
        || error instanceof TimeoutException;
  }

  private static boolean isTransport(final Throwable error) {
    return error instanceof ConnectException
        || error instanceof NoRouteToHostException
        || error instanceof PortUnreachableException
        || error instanceof UnknownHostException
        || error instanceof ClosedChannelException
        || error instanceof UnresolvedAddressException
        || isClass(error, "java.net.http.WebSocketHandshakeException")
        || error instanceof EOFException
        || error instanceof SocketException
        || messageContains(error, "websocket")
        || messageContains(error, "connection refused")
        || messageContains(error, "connection reset")
        || messageContains(error, "broken pipe");
  }

  private static boolean isCodec(final Throwable error) {
    if (error instanceof ParseException || error instanceof NumberFormatException) {
      return true;
    }
    if (error instanceof IllegalStateException || error instanceof IllegalArgumentException) {
      return messageContains(error, "json")
          || messageContains(error, "codec")
          || messageContains(error, "norito")
          || messageContains(error, "payload");
    }
    return false;
  }

  private static boolean messageContains(final Throwable error, final String needle) {
    if (error == null) {
      return false;
    }
    final String message = error.getMessage();
    if (message == null || message.isEmpty()) {
      return false;
    }
    return message.toLowerCase(Locale.ROOT).contains(needle.toLowerCase(Locale.ROOT));
  }

  private static ConnectError newError(
      final ConnectErrorCategory category,
      final String code,
      final Throwable cause,
      final String fallback) {
    final String message = messageOrDefault(cause, fallback);
    final ConnectError.Builder builder =
        ConnectError.builder()
            .category(category)
            .code(code)
            .message(message)
            .cause(cause);
    if (cause != null) {
      builder.underlying(cause.toString());
    }
    return builder.build();
  }

  private static boolean isClass(final Throwable error, final String className) {
    return error != null && error.getClass().getName().equals(className);
  }

  private static ConnectError unknown(final Throwable cause) {
    return newError(
        ConnectErrorCategory.INTERNAL,
        "unknown_error",
        cause,
        "Unknown Connect error");
  }

  private static String messageOrDefault(final Throwable cause, final String fallback) {
    if (cause == null) {
      return fallback;
    }
    final String message = cause.getMessage();
    if (message == null || message.isEmpty()) {
      return fallback;
    }
    return message;
  }
}
