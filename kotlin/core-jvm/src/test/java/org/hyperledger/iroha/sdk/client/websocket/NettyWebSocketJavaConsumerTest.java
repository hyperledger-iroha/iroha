package org.hyperledger.iroha.sdk.client.websocket;

import java.net.URI;
import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import io.netty.channel.MultiThreadIoEventLoopGroup;
import io.netty.channel.nio.NioIoHandler;
import javax.net.ssl.SSLContext;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Java-source consumers of the canonical Kotlin connector, with real wire messages. */
final class NettyWebSocketJavaConsumerTest {
    @Test
    void ownedFactoryAndTwoArgumentConnectorAreJavaCallable() throws Exception {
        try (MockWebServer server = new MockWebServer();
             NettyWebSocketConnector connector = NettyWebSocketConnector.create()) {
            server.enqueue(new MockResponse().withWebSocketUpgrade(new WebSocketListener() {
                @Override public void onMessage(WebSocket socket, String text) { socket.send(text); }
                @Override public void onClosing(WebSocket socket, int code, String reason) { socket.close(code, reason); }
            }));
            CompletableFuture<String> echo = new CompletableFuture<>();
            CompletableFuture<ToriiWebSocketSession> opened = new CompletableFuture<>();
            ToriiWebSocketClient client = ToriiWebSocketClient.builder()
                .setBaseUri(server.url("/").uri())
                .setWebSocketConnector((request, listener) -> connector.connect(request, listener))
                .build();
            client.connect("events", null, new ToriiWebSocketListener() {
                @Override public void onOpen(ToriiWebSocketSession session) {
                    try {
                        assertTrue(session.isOpen());
                        session.sendText("from Java").get(2, TimeUnit.SECONDS);
                        opened.complete(session);
                    } catch (Throwable error) { opened.completeExceptionally(error); }
                }
                @Override public void onText(ToriiWebSocketSession session, String data) {
                    echo.complete(data.toString());
                }
            });
            ToriiWebSocketSession session = opened.get(5, TimeUnit.SECONDS);
            assertEquals("from Java", echo.get(5, TimeUnit.SECONDS));
            session.close(1000, "complete").get(5, TimeUnit.SECONDS);
        }
    }

    @Test
    void borrowedConstructorLeavesApplicationExecutorAvailable() throws Exception {
        MultiThreadIoEventLoopGroup shared = new MultiThreadIoEventLoopGroup(1, NioIoHandler.newFactory());
        try (NettyWebSocketConnector connector = new NettyWebSocketConnector(shared, SSLContext.getDefault(), 1024, 2048L)) {
            connector.close();
            TransportRequest request = TransportRequest.builder()
                .setUri(URI.create("ws://localhost:1/events"))
                .setTimeout(Duration.ofSeconds(1)).build();
            assertTrue(connector.connect(request, new TransportWebSocket.Listener() {}).isCompletedExceptionally());
            assertFalse(shared.isShuttingDown());
            assertEquals("alive", shared.submit(() -> "alive").get());
        } finally {
            shared.shutdownGracefully(0, 2, TimeUnit.SECONDS).syncUninterruptibly();
        }
    }
}
