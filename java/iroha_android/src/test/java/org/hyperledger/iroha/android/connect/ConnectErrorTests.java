package org.hyperledger.iroha.android.connect;

import java.net.ConnectException;
import java.net.SocketTimeoutException;
import javax.net.ssl.SSLException;
import org.hyperledger.iroha.android.connect.error.ConnectError;
import org.hyperledger.iroha.android.connect.error.ConnectErrorCategory;
import org.hyperledger.iroha.android.connect.error.ConnectErrors;
import org.hyperledger.iroha.android.connect.error.ConnectQueueError;

public final class ConnectErrorTests {
    private ConnectErrorTests() {}

    public static void main(String[] args) {
        testQueueOverflow();
        testTimeoutClassification();
        testTlsClassification();
        testCodecClassification();
        testTransportClassification();
        testHttpStatusMapping();
    }

    private static void testQueueOverflow() {
        ConnectQueueError overflow = ConnectQueueError.overflow(256);
        ConnectError error = overflow.toConnectError();
        assert error.category() == ConnectErrorCategory.QUEUE_OVERFLOW : "queue category";
        assert "queue.overflow".equals(error.code()) : "queue code";
        assert error.underlying().isPresent() : "queue metadata";
    }

    private static void testTimeoutClassification() {
        ConnectError error = ConnectErrors.from(new SocketTimeoutException("timed out"));
        assert error.category() == ConnectErrorCategory.TIMEOUT : "timeout category";
        assert "network.timeout".equals(error.code()) : "timeout code";
    }

    private static void testTlsClassification() {
        ConnectError error = ConnectErrors.from(new SSLException("certificate expired"));
        assert error.category() == ConnectErrorCategory.AUTHORIZATION : "tls category";
        assert "network.tls_failure".equals(error.code()) : "tls code";
    }

    private static void testCodecClassification() {
        IllegalStateException ex =
                new IllegalStateException("Trailing characters after JSON payload");
        ConnectError error = ConnectErrors.from(ex);
        assert error.category() == ConnectErrorCategory.CODEC : "codec category";
        assert "codec.invalid_payload".equals(error.code()) : "codec code";
    }

    private static void testTransportClassification() {
        ConnectError error =
                ConnectErrors.from(new ConnectException("Connection refused"));
        assert error.category() == ConnectErrorCategory.TRANSPORT : "transport category";
        assert "network.socket_failure".equals(error.code()) : "transport code";
    }

    private static void testHttpStatusMapping() {
        ConnectError error = ConnectErrors.fromHttpStatus(403, "Forbidden");
        assert error.category() == ConnectErrorCategory.AUTHORIZATION : "http category";
        assert "http.forbidden".equals(error.code()) : "http code";
        assert error.httpStatus().orElse(-1) == 403 : "http status";
    }
}
