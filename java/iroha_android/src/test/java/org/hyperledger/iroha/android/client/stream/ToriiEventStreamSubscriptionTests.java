package org.hyperledger.iroha.android.client.stream;

import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

public final class ToriiEventStreamSubscriptionTests {

  private ToriiEventStreamSubscriptionTests() {}

  public static void main(final String[] args) throws Exception {
    reconnectsAfterClosure();
    reconnectsOnceWhenErrorAndClose();
    staleCloseDoesNotReconnect();
    staleEventsAreIgnored();
    stopsWhenClosed();
    observerReceivesNotifications();
    System.out.println("[IrohaAndroid] ToriiEventStreamSubscriptionTests passed.");
  }

  private static void reconnectsAfterClosure() throws Exception {
    final RecordingStreamOpener opener = new RecordingStreamOpener();
    final CountDownLatch events = new CountDownLatch(2);
    final ToriiEventStreamListener listener =
        new ForwardingListener(event -> events.countDown());
    final ToriiEventStreamSubscription subscription =
        ToriiEventStreamSubscription.builder(opener, listener)
            .setInitialBackoff(Duration.ofMillis(10))
            .setMaxBackoff(Duration.ofMillis(20))
            .build()
            .start();
    opener.awaitStreamCount(1);
    opener.emitEvent(0, "first");
    opener.closeStream(0);
    opener.awaitStreamCount(2);
    opener.emitEvent(1, "second");
    if (!events.await(1, TimeUnit.SECONDS)) {
      throw new AssertionError("Expected delegate events were not observed");
    }
    subscription.close();
  }

  private static void reconnectsOnceWhenErrorAndClose() throws Exception {
    final RecordingStreamOpener opener = new RecordingStreamOpener();
    final ToriiEventStreamSubscription subscription =
        ToriiEventStreamSubscription.builder(opener, new ForwardingListener(null))
            .setInitialBackoff(Duration.ofMillis(100))
            .setMaxBackoff(Duration.ofMillis(100))
            .build()
            .start();
    opener.awaitStreamCount(1);
    opener.failStream(0, new RuntimeException("boom"));
    opener.closeStream(0);
    opener.awaitStreamCount(2);
    Thread.sleep(200);
    if (opener.streamCount.get() != 2) {
      throw new AssertionError(
          "Expected a single reconnection after error+close, saw " + opener.streamCount.get());
    }
    subscription.close();
  }

  private static void staleCloseDoesNotReconnect() throws Exception {
    final RecordingStreamOpener opener = new RecordingStreamOpener();
    final ToriiEventStreamSubscription subscription =
        ToriiEventStreamSubscription.builder(opener, new ForwardingListener(null))
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(5))
            .build()
            .start();
    opener.awaitStreamCount(1);
    opener.failStream(0, new RuntimeException("boom"));
    opener.awaitStreamCount(2);
    opener.closeStream(0);
    Thread.sleep(50);
    if (opener.streamCount.get() != 2) {
      throw new AssertionError(
          "Expected no reconnection after stale close, saw " + opener.streamCount.get());
    }
    subscription.close();
  }

  private static void staleEventsAreIgnored() throws Exception {
    final RecordingStreamOpener opener = new RecordingStreamOpener();
    final RecordingListener listener = new RecordingListener();
    final ToriiEventStreamSubscription subscription =
        ToriiEventStreamSubscription.builder(opener, listener)
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(5))
            .build()
            .start();
    opener.awaitStreamCount(1);
    opener.failStream(0, new RuntimeException("boom"));
    opener.awaitStreamCount(2);
    opener.emitEvent(0, "stale");
    if (listener.eventCount.get() != 0) {
      throw new AssertionError(
          "Expected stale events to be ignored, saw " + listener.eventCount.get());
    }
    subscription.close();
  }

  private static void stopsWhenClosed() throws Exception {
    final RecordingStreamOpener opener = new RecordingStreamOpener();
    final ToriiEventStreamSubscription subscription =
        ToriiEventStreamSubscription.builder(opener, new ForwardingListener(null))
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(10))
            .build()
            .start();
    opener.awaitStreamCount(1);
    subscription.close();
    opener.closeStream(0);
    // Give the scheduler a moment; no new streams should be opened.
    Thread.sleep(50);
    if (opener.streamCount.get() != 1) {
      throw new AssertionError("Expected no reconnection after close");
    }
  }

  private static void observerReceivesNotifications() throws Exception {
    final RecordingStreamOpener opener = new RecordingStreamOpener();
    final RecordingObserver observer = new RecordingObserver();
    final ToriiEventStreamSubscription subscription =
        ToriiEventStreamSubscription.builder(opener, new ForwardingListener(null))
            .setInitialBackoff(Duration.ofMillis(5))
            .setMaxBackoff(Duration.ofMillis(10))
            .addObserver(observer)
            .build()
            .start();
    observer.expectEvent("reconnect:INITIAL");
    opener.awaitStreamCount(1);
    observer.expectEvent("open");

    opener.closeStream(0);
    observer.expectEvent("closed");
    observer.expectEvent("reconnect:STREAM_CLOSED");
    opener.awaitStreamCount(2);
    observer.expectEvent("open");

    opener.failStream(1, new RuntimeException("boom"));
    observer.expectEvent("failure");
    observer.expectEvent("reconnect:STREAM_FAILURE");
    opener.awaitStreamCount(3);
    observer.expectEvent("open");
    subscription.close();
  }

  private static final class RecordingStreamOpener
      implements ToriiEventStreamSubscription.StreamOpener {
    private final List<FakeStream> streams = new ArrayList<>();
    private final AtomicInteger streamCount = new AtomicInteger();

    @Override
    public synchronized ToriiEventStream open(final ToriiEventStreamListener listener) {
      final FakeStream stream = new FakeStream(listener);
      streams.add(stream);
      streamCount.incrementAndGet();
      return stream;
    }

    void awaitStreamCount(final int count) throws InterruptedException {
      long deadline = System.currentTimeMillis() + 1_000;
      while (streamCount.get() < count && System.currentTimeMillis() < deadline) {
        Thread.sleep(5);
      }
      if (streamCount.get() < count) {
        throw new AssertionError("Timed out waiting for stream count " + count);
      }
    }

    void emitEvent(final int index, final String data) {
      final FakeStream stream = streams.get(index);
      stream.emit(data);
    }

    void closeStream(final int index) {
      final FakeStream stream = streams.get(index);
      stream.close();
    }

    void failStream(final int index, final RuntimeException error) {
      final FakeStream stream = streams.get(index);
      stream.fail(error);
    }
  }

  private static final class FakeStream implements ToriiEventStream {
    private final ToriiEventStreamListener listener;
    private final CompletableFuture<Void> completion = new CompletableFuture<>();
    private volatile boolean closed = false;

    FakeStream(final ToriiEventStreamListener listener) {
      this.listener = listener;
      listener.onOpen();
    }

    void emit(final String payload) {
      if (!closed) {
        listener.onEvent(new ServerSentEvent("message", payload, null));
      }
    }

    void fail(final RuntimeException error) {
      if (!closed) {
        listener.onError(error);
      }
    }

    @Override
    public boolean isOpen() {
      return !closed;
    }

    @Override
    public CompletableFuture<Void> completion() {
      return completion;
    }

    @Override
    public void close() {
      if (!closed) {
        closed = true;
        listener.onClosed();
        completion.complete(null);
      }
    }
  }

  private static final class ForwardingListener implements ToriiEventStreamListener {
    private final java.util.function.Consumer<ServerSentEvent> onEvent;

    ForwardingListener(final java.util.function.Consumer<ServerSentEvent> onEvent) {
      this.onEvent = onEvent;
    }

    @Override
    public void onEvent(final ServerSentEvent event) {
      if (onEvent != null) {
        onEvent.accept(event);
      }
    }
  }

  private static final class RecordingObserver implements ToriiEventStreamObserver {
    private final LinkedBlockingQueue<String> events = new LinkedBlockingQueue<>();

    @Override
    public void onStreamOpened() {
      events.add("open");
    }

    @Override
    public void onStreamClosed() {
      events.add("closed");
    }

    @Override
    public void onStreamFailure(final Throwable error) {
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

  private static final class RecordingListener implements ToriiEventStreamListener {
    private final AtomicInteger eventCount = new AtomicInteger();

    @Override
    public void onEvent(final ServerSentEvent event) {
      eventCount.incrementAndGet();
    }
  }
}
