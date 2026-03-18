package org.hyperledger.iroha.android.client.websocket;

import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

public final class ToriiWebSocketSubscriptionTests {

  private ToriiWebSocketSubscriptionTests() {}

  public static void main(final String[] args) throws Exception {
    reconnectsAfterClosure();
    reconnectsOnceWhenErrorAndClose();
    staleCloseDoesNotReconnect();
    staleMessagesAreIgnored();
    stopsWhenClosed();
    observerReceivesNotifications();
    System.out.println("[IrohaAndroid] ToriiWebSocketSubscriptionTests passed.");
  }

  private static void reconnectsAfterClosure() throws Exception {
    final RecordingSessionOpener opener = new RecordingSessionOpener();
    final CountDownLatch opens = new CountDownLatch(2);
    final List<String> payloads = new ArrayList<>();
    final ToriiWebSocketListener listener =
        new ForwardingListener(opens::countDown, payloads::add, null);
    final ToriiWebSocketSubscription subscription =
        ToriiWebSocketSubscription.builder(opener, listener)
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(10))
            .build()
            .start();
    opener.awaitSessionCount(1);
    opener.emitText(0, "first");
    opener.closeSession(0);
    opener.awaitSessionCount(2);
    opener.emitText(1, "second");
    if (!opens.await(1, TimeUnit.SECONDS)) {
      throw new AssertionError("Expected websocket opens were not observed");
    }
    if (payloads.size() != 2) {
      throw new AssertionError("Expected payloads not delivered: " + payloads);
    }
    subscription.close();
  }

  private static void reconnectsOnceWhenErrorAndClose() throws Exception {
    final RecordingSessionOpener opener = new RecordingSessionOpener();
    final ToriiWebSocketSubscription subscription =
        ToriiWebSocketSubscription.builder(opener, new ForwardingListener(null, null, null))
            .setInitialBackoff(Duration.ofMillis(100))
            .setMaxBackoff(Duration.ofMillis(100))
            .build()
            .start();
    opener.awaitSessionCount(1);
    opener.failSession(0, new RuntimeException("boom"));
    opener.closeSession(0);
    opener.awaitSessionCount(2);
    Thread.sleep(200);
    if (opener.sessionCount.get() != 2) {
      throw new AssertionError(
          "Expected a single reconnection after error+close, saw " + opener.sessionCount.get());
    }
    subscription.close();
  }

  private static void staleCloseDoesNotReconnect() throws Exception {
    final RecordingSessionOpener opener = new RecordingSessionOpener();
    final ToriiWebSocketSubscription subscription =
        ToriiWebSocketSubscription.builder(opener, new ForwardingListener(null, null, null))
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(5))
            .build()
            .start();
    opener.awaitSessionCount(1);
    opener.failSession(0, new RuntimeException("boom"));
    opener.awaitSessionCount(2);
    opener.closeSession(0);
    Thread.sleep(50);
    if (opener.sessionCount.get() != 2) {
      throw new AssertionError(
          "Expected no reconnection after stale close, saw " + opener.sessionCount.get());
    }
    subscription.close();
  }

  private static void staleMessagesAreIgnored() throws Exception {
    final RecordingSessionOpener opener = new RecordingSessionOpener();
    final RecordingListener listener = new RecordingListener();
    final ToriiWebSocketSubscription subscription =
        ToriiWebSocketSubscription.builder(opener, listener)
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(5))
            .build()
            .start();
    opener.awaitSessionCount(1);
    opener.failSession(0, new RuntimeException("boom"));
    opener.awaitSessionCount(2);
    opener.emitText(0, "stale");
    opener.emitBinary(0, ByteBuffer.wrap(new byte[] {1}));
    opener.emitPing(0, ByteBuffer.wrap(new byte[] {2}));
    opener.emitPong(0, ByteBuffer.wrap(new byte[] {3}));
    if (listener.textCount.get() != 0
        || listener.binaryCount.get() != 0
        || listener.pingCount.get() != 0
        || listener.pongCount.get() != 0) {
      throw new AssertionError(
          "Expected stale session messages to be ignored, saw "
              + listener.summary());
    }
    subscription.close();
  }

  private static void stopsWhenClosed() throws Exception {
    final RecordingSessionOpener opener = new RecordingSessionOpener();
    final ToriiWebSocketSubscription subscription =
        ToriiWebSocketSubscription.builder(opener, new ForwardingListener(null, null, null))
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(10))
            .build()
            .start();
    opener.awaitSessionCount(1);
    subscription.close();
    opener.closeSession(0);
    // Give the scheduler a moment; no new sessions should be opened.
    Thread.sleep(50);
    if (opener.sessionCount.get() != 1) {
      throw new AssertionError("Expected no reconnection after close");
    }
  }

  private static void observerReceivesNotifications() throws Exception {
    final RecordingSessionOpener opener = new RecordingSessionOpener();
    final RecordingObserver observer = new RecordingObserver();
    final ToriiWebSocketSubscription subscription =
        ToriiWebSocketSubscription.builder(opener, new ForwardingListener(null, null, null))
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(10))
            .addObserver(observer)
            .build()
            .start();
    observer.expectEvent("reconnect:INITIAL");
    opener.awaitSessionCount(1);
    observer.expectEvent("open");

    opener.closeSession(0);
    observer.expectEvent("closed");
    observer.expectEvent("reconnect:SESSION_CLOSED");
    opener.awaitSessionCount(2);
    observer.expectEvent("open");

    opener.failSession(1, new RuntimeException("boom"));
    observer.expectEvent("failure");
    observer.expectEvent("reconnect:SESSION_FAILURE");
    opener.awaitSessionCount(3);
    observer.expectEvent("open");
    subscription.close();
  }

  private static final class RecordingSessionOpener
      implements ToriiWebSocketSubscription.SessionOpener {
    private final List<FakeSession> sessions = new ArrayList<>();
    private final AtomicInteger sessionCount = new AtomicInteger();

    @Override
    public synchronized ToriiWebSocketSession open(final ToriiWebSocketListener listener) {
      final FakeSession session = new FakeSession(listener);
      sessions.add(session);
      sessionCount.incrementAndGet();
      return session;
    }

    void awaitSessionCount(final int count) throws InterruptedException {
      long deadline = System.currentTimeMillis() + 1_000;
      while (sessionCount.get() < count && System.currentTimeMillis() < deadline) {
        Thread.sleep(5);
      }
      if (sessionCount.get() < count) {
        throw new AssertionError("Timed out waiting for session count " + count);
      }
    }

    void emitText(final int index, final String payload) {
      sessions.get(index).emitText(payload);
    }

    void emitBinary(final int index, final ByteBuffer payload) {
      sessions.get(index).emitBinary(payload);
    }

    void emitPing(final int index, final ByteBuffer payload) {
      sessions.get(index).emitPing(payload);
    }

    void emitPong(final int index, final ByteBuffer payload) {
      sessions.get(index).emitPong(payload);
    }

    void closeSession(final int index) {
      sessions.get(index).close(1000, "test");
    }

    void failSession(final int index, final RuntimeException error) {
      sessions.get(index).fail(error);
    }
  }

  private static final class FakeSession implements ToriiWebSocketSession {
    private final ToriiWebSocketListener listener;
    private final AtomicBoolean closed = new AtomicBoolean(false);

    FakeSession(final ToriiWebSocketListener listener) {
      this.listener = listener;
      listener.onOpen(this);
    }

    void emitText(final String payload) {
      if (!closed.get()) {
        listener.onText(this, payload, true);
      }
    }

    void emitBinary(final ByteBuffer payload) {
      if (!closed.get()) {
        listener.onBinary(this, payload, true);
      }
    }

    void emitPing(final ByteBuffer payload) {
      if (!closed.get()) {
        listener.onPing(this, payload);
      }
    }

    void emitPong(final ByteBuffer payload) {
      if (!closed.get()) {
        listener.onPong(this, payload);
      }
    }

    void fail(final RuntimeException error) {
      if (!closed.get()) {
        listener.onError(this, error);
      }
    }

    @Override
    public CompletableFuture<Void> sendText(final CharSequence data, final boolean last) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> sendBinary(final ByteBuffer data, final boolean last) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> sendPing(final ByteBuffer message) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> sendPong(final ByteBuffer message) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> close(final int statusCode, final String reason) {
      if (closed.compareAndSet(false, true)) {
        listener.onClose(this, statusCode, reason);
      }
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public boolean isOpen() {
      return !closed.get();
    }

    @Override
    public String subprotocol() {
      return "";
    }
  }

  private static final class ForwardingListener implements ToriiWebSocketListener {
    private final Runnable onOpen;
    private final java.util.function.Consumer<String> onText;
    private final java.util.function.Consumer<Throwable> onError;

    ForwardingListener(
        final Runnable onOpen,
        final java.util.function.Consumer<String> onText,
        final java.util.function.Consumer<Throwable> onError) {
      this.onOpen = onOpen;
      this.onText = onText;
      this.onError = onError;
    }

    @Override
    public void onOpen(final ToriiWebSocketSession session) {
      if (onOpen != null) {
        onOpen.run();
      }
    }

    @Override
    public void onText(final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
      if (onText != null) {
        onText.accept(data.toString());
      }
    }

    @Override
    public void onError(final ToriiWebSocketSession session, final Throwable error) {
      if (onError != null) {
        onError.accept(error);
      }
    }
  }

  private static final class RecordingListener implements ToriiWebSocketListener {
    private final AtomicInteger textCount = new AtomicInteger();
    private final AtomicInteger binaryCount = new AtomicInteger();
    private final AtomicInteger pingCount = new AtomicInteger();
    private final AtomicInteger pongCount = new AtomicInteger();

    @Override
    public void onText(
        final ToriiWebSocketSession session,
        final CharSequence data,
        final boolean last) {
      textCount.incrementAndGet();
    }

    @Override
    public void onBinary(
        final ToriiWebSocketSession session,
        final ByteBuffer data,
        final boolean last) {
      binaryCount.incrementAndGet();
    }

    @Override
    public void onPing(final ToriiWebSocketSession session, final ByteBuffer message) {
      pingCount.incrementAndGet();
    }

    @Override
    public void onPong(final ToriiWebSocketSession session, final ByteBuffer message) {
      pongCount.incrementAndGet();
    }

    String summary() {
      return "text="
          + textCount.get()
          + ", binary="
          + binaryCount.get()
          + ", ping="
          + pingCount.get()
          + ", pong="
          + pongCount.get();
    }
  }

  private static final class RecordingObserver implements ToriiWebSocketObserver {
    private final LinkedBlockingQueue<String> events = new LinkedBlockingQueue<>();

    @Override
    public void onSessionOpened() {
      events.add("open");
    }

    @Override
    public void onSessionClosed() {
      events.add("closed");
    }

    @Override
    public void onSessionFailure(final Throwable error) {
      events.add("failure");
    }

    @Override
    public void onReconnectScheduled(
        final Duration delay, final ReconnectReason reason) {
      events.add("reconnect:" + reason.name());
    }

    void expectEvent(final String expectedPrefix) throws InterruptedException {
      final String value = events.poll(1, TimeUnit.SECONDS);
      if (value == null || !value.startsWith(expectedPrefix)) {
        throw new AssertionError("Expected event prefix " + expectedPrefix + " but saw " + value);
      }
    }
  }
}
