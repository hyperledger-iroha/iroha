// Native Nearby file timestamp regression using the real SwiftPM provider.
// Fixed wire times and independent stat/utimensat checks avoid clock
// tolerances.
#include "connections/implementation/internal_payload_factory.h"
#include "connections/implementation/proto/offline_wire_formats.pb.h"
#include "connections/payload.h"
#include "internal/platform/file.h"

#include <cassert>
#include <cstdint>
#include <fcntl.h>
#include <filesystem>
#include <iostream>
#include <string>
#include <sys/stat.h>

using namespace nearby;
using namespace nearby::connections;
using Transfer = location::nearby::connections::PayloadTransferFrame;

absl::Time ModifiedTime(const std::string &path) {
  struct stat metadata{};
  assert(stat(path.c_str(), &metadata) == 0);
  return absl::TimeFromTimespec(metadata.st_mtimespec);
}

void SetModifiedTime(const std::string &path, absl::Time time) {
  const struct timespec times[2] = {{0, UTIME_OMIT}, absl::ToTimespec(time)};
  assert(utimensat(AT_FDCWD, path.c_str(), times, 0) == 0);
}

void CheckIncomingTimestamp(const std::string &root, int64_t millis) {
  const auto expected = absl::FromUnixMillis(millis);
  Transfer frame;
  frame.set_packet_type(Transfer::DATA);
  auto &header = *frame.mutable_payload_header();
  header.set_type(Transfer::PayloadHeader::FILE);
  header.set_id(millis);
  header.set_total_size(7);
  header.set_parent_folder("received");
  header.set_file_name(std::to_string(millis));
  header.set_last_modified_timestamp_millis(millis);
  auto result = CreateIncomingInternalPayload(frame, root);
  assert(!result.has_error());
  auto incoming = std::move(result.value());
  auto payload = incoming->ReleasePayload();
  const auto path = payload.AsFile()->GetFilePath();
  assert(payload.GetLastModifiedTime() == expected);
  assert(ModifiedTime(path) == expected);
  assert(incoming->AttachNextChunk("message").Ok());
  // A real write changes mtime. Override it deterministically as well so this
  // assertion cannot accidentally pass because the write and input time match.
  SetModifiedTime(path, expected + absl::Hours(24));
  assert(payload.GetLastModifiedTime() == expected);
  assert(incoming->AttachNextChunk("").Ok());
  assert(ModifiedTime(path) == expected);
  InputFile completed(path);
  assert(completed.GetLastModifiedTime() == expected);
  Payload forwarded("forwarded", "copy", std::move(completed));
  assert(forwarded.GetLastModifiedTime() == expected);
  assert(std::filesystem::file_size(path) == 7);
  std::cout << "Timestamp roundtrip passed: " << millis << '\n';
}

int main() {
  char pattern[] = "/tmp/iroha-nearby-timestamps-XXXXXX";
  const auto *root = mkdtemp(pattern);
  assert(root);
  // Before Unix epoch, exact epoch, historical millisecond and future beyond
  // signed 32-bit seconds all use exact equality, with no sleeps or tolerances.
  for (const int64_t millis : {-123LL, 0LL, 946684800123LL, 4102444800987LL}) {
    CheckIncomingTimestamp(root, millis);
  }
  std::filesystem::remove_all(root);
  std::cout << "Nearby file timestamps: 4 cases passed\n";
}
