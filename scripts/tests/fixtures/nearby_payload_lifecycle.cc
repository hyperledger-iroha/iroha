// Native Nearby payload lifecycle qualification using actual SwiftPM objects.
// The channel is upstream's fake transport; managers, files and callbacks are
// real.
#include "connections/implementation/endpoint_channel_manager.h"
#include "connections/implementation/fake_endpoint_channel.h"
#include "connections/implementation/offline_frames.h"
#include "connections/implementation/payload_manager.h"
#include <cassert>
#include <chrono>
#include <condition_variable>
#include <filesystem>
#include <iostream>
#include <mutex>
#include <string>
#include <vector>
using namespace nearby;
using namespace nearby::connections;
using Frame = location::nearby::connections::OfflineFrame;
using Transfer = location::nearby::connections::PayloadTransferFrame;
using Medium = location::nearby::proto::connections::Medium;
using Progress = PayloadProgressInfo;
struct Fixture {
  std::mutex mutex;
  std::condition_variable changed;
  std::vector<Payload> received;
  std::vector<Progress> progress;
  ClientProxy client;
  EndpointChannelManager channels;
  EndpointManager endpoint{&channels};
  PayloadManager manager{endpoint};
  std::shared_ptr<FakeEndpointChannel> channel =
      std::make_shared<FakeEndpointChannel>(Medium::WIFI_LAN, "lifecycle-test");
  const std::string remote = "TEST";
  Fixture(bool accepted = true) {
    client.OnConnectionInitiated(remote, ConnectionResponseInfo{},
                                 ConnectionOptions{}, ConnectionListener{},
                                 "synthetic-token");
    client.LocalEndpointAcceptedConnection(
        remote,
        PayloadListener{.payload_cb =
                            [this](absl::string_view, Payload p) {
                              std::lock_guard guard(mutex);
                              received.push_back(std::move(p));
                              changed.notify_all();
                            },
                        .payload_progress_cb =
                            [this](absl::string_view, const Progress &p) {
                              std::lock_guard guard(mutex);
                              progress.push_back(p);
                              changed.notify_all();
                            }});
    if (accepted) {
      client.RemoteEndpointAcceptedConnection(remote);
      client.OnConnectionAccepted(remote);
      assert(client.IsConnectedToEndpoint(remote));
    }
    channels.RegisterChannelForEndpoint(&client, remote, channel);
  }
  bool wait_received(size_t n) {
    std::unique_lock lock(mutex);
    return changed.wait_for(lock, std::chrono::seconds(5),
                            [&] { return received.size() >= n; });
  }
  bool wait_progress(Payload::Id id, Progress::Status status,
                     int64_t offset = -1) {
    std::unique_lock lock(mutex);
    return changed.wait_for(lock, std::chrono::seconds(5), [&] {
      for (auto &p : progress)
        if (p.payload_id == id && p.status == status &&
            (offset < 0 || p.bytes_transferred == offset))
          return true;
      return false;
    });
  }
  void send_in(Payload::Id id, Transfer::PayloadHeader::PayloadType type,
               std::string name, int64_t total, int64_t offset,
               std::string bytes, int flags) {
    Transfer::PayloadHeader header;
    header.set_id(id);
    header.set_type(type);
    header.set_total_size(total);
    header.set_file_name(name);
    header.set_parent_folder("");
    Transfer::PayloadChunk chunk;
    chunk.set_offset(offset);
    chunk.set_body(bytes);
    chunk.set_flags(flags);
    Frame frame;
    assert(
        frame.ParseFromString(parser::ForDataPayloadTransfer(header, chunk)));
    manager.OnIncomingFrame(frame, remote, &client, Medium::WIFI_LAN);
  }
  void file(Payload::Id id, std::string name, int64_t total, int64_t offset,
            std::string bytes, int flags = 0) {
    send_in(id, Transfer::PayloadHeader::FILE, name, total, offset, bytes,
            flags);
  }
};
int main() {
  char path[] = "/tmp/iroha-nearby-lifecycle-XXXXXX";
  auto *made = mkdtemp(path);
  assert(made);
  const std::filesystem::path dir(made);
  {
    Fixture f(false);
    f.manager.SetCustomSavePath(&f.client, dir.string());
    f.file(1, "unaccepted", 7, 0, "message");
    assert(!std::filesystem::exists(dir / "unaccepted"));
    assert(!f.wait_progress(1, Progress::Status::kSuccess));
    std::lock_guard guard(f.mutex);
    assert(f.received.empty());
  }
  {
    Fixture f;
    f.send_in(2, Transfer::PayloadHeader::BYTES, "", 7, 0, "message", 1);
    assert(f.wait_received(1));
    assert(f.wait_progress(2, Progress::Status::kSuccess));
    std::lock_guard guard(f.mutex);
    assert(std::string(f.received[0].AsBytes()) == "message");
  }
  {
    Fixture f;
    f.manager.SetCustomSavePath(&f.client, dir.string());
    f.file(3, "cancel", 14, 0, "message");
    assert(f.wait_received(1));
    assert(std::filesystem::exists(dir / "cancel"));
    assert(f.manager.CancelPayload(&f.client, 3) == Status{Status::kSuccess});
    assert(!std::filesystem::exists(dir / "cancel"));
  }
  {
    Fixture f;
    f.manager.SetCustomSavePath(&f.client, dir.string());
    f.file(4, "duplicate-a", 14, 0, "message");
    assert(f.wait_received(1));
    assert(std::filesystem::exists(dir / "duplicate-a"));
    f.file(4, "duplicate-b", 14, 0, "message");
    assert(f.wait_progress(4, Progress::Status::kFailure));
    assert(f.client.IsConnectedToEndpoint(f.remote));
    assert(!std::filesystem::exists(dir / "duplicate-a"));
    assert(!std::filesystem::exists(dir / "duplicate-b"));
  }
  {
    Fixture f;
    f.manager.SetCustomSavePath(&f.client, dir.string());
    f.file(5, "disconnect", 21, 0, "message");
    assert(f.wait_received(1));
    f.file(5, "disconnect", 21, 7, "message");
    CountDownLatch barrier(1);
    f.manager.OnEndpointDisconnect(
        &f.client, "lifecycle-test", f.remote, barrier,
        location::nearby::proto::connections::REMOTE_DISCONNECTION);
    assert(barrier.Await(absl::Seconds(5)).result());
    assert(f.wait_progress(5, Progress::Status::kFailure, 7));
    assert(!std::filesystem::exists(dir / "disconnect"));
  }
  {
    Fixture f;
    f.manager.SetCustomSavePath(&f.client, dir.string());
    f.file(6, "shutdown", 14, 0, "message");
    assert(f.wait_received(1));
    assert(std::filesystem::exists(dir / "shutdown"));
  }
  assert(!std::filesystem::exists(dir / "shutdown"));
  {
    Fixture f;
    Payload payload(ByteArray("outgoing"));
    auto id = payload.GetId();
    f.manager.SendPayload(&f.client, {f.remote}, std::move(payload));
    assert(f.wait_progress(id, Progress::Status::kSuccess));
    std::string bytes;
    for (auto &encoded : f.channel->GetWrittenFrames()) {
      Frame frame;
      assert(frame.ParseFromString(encoded));
      if (frame.v1().has_payload_transfer()) {
        auto &transfer = frame.v1().payload_transfer();
        if (transfer.packet_type() == Transfer::DATA)
          bytes += transfer.payload_chunk().body();
      }
    }
    assert(bytes == "outgoing");
  }
  std::filesystem::remove_all(dir);
  std::cout << "7 real Nearby payload lifecycle cases passed: preaccept "
               "refusal, incoming bytes, cancel cleanup, duplicate ID, "
               "disconnect offset, shutdown cleanup, outgoing frames\n";
}
