// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#include "src/main/cpp/include/securegcm/d2d_connection_context_v1.h"

#include <limits>
#include <sstream>

#include "absl/strings/string_view.h"
#include "device_to_device_messages.pb.h"
#include "securegcm.pb.h"
#include "src/main/cpp/include/securegcm/d2d_crypto_ops.h"
#include "src/main/cpp/include/securegcm/java_util.h"
#include "src/securemessage/include/securemessage/secure_message_builder.h"
#include "src/securemessage/include/securemessage/util.h"

namespace securegcm {

using securemessage::CryptoOps;
using securemessage::ByteBuffer;
using securemessage::Util;

namespace {

// Fields to fill in the GcmMetadata proto.
const Type kGcmMetadataType = DEVICE_TO_DEVICE_MESSAGE;

// Represents the version of this context.
const uint8_t kProtocolVersion = 1;

// The following represent the starting positions of the each entry within
// the string representation of this D2DConnectionContextV1.
//
// The saved session has a 1 byte protocol version, two 4 byte sequence numbers,
// and two 32 byte AES keys: (1 + 4 + 4 + 32 + 32 = 73).

// The two sequence numbers are 4 bytes each.
const int kSequenceNumberLength = 4;

// 32 byte AES keys.
const int kAesKeyLength = 32;

// The encode sequence number starts at 1 to account for the 1 byte version
// number.
const int kEncodeSequenceStart = 1;
const int kEncodeSequenceEnd = kEncodeSequenceStart + kSequenceNumberLength;

const int kDecodeSequenceStart = kEncodeSequenceEnd;
const int kDecodeSequenceEnd = kDecodeSequenceStart + kSequenceNumberLength;

const int kEncodeKeyStart = kDecodeSequenceEnd;
const int kEncodeKeyEnd = kEncodeKeyStart + kAesKeyLength;

const int kDecodeKeyStart = kEncodeKeyEnd;
const int kSavedSessionLength = kDecodeKeyStart + kAesKeyLength;

// Convenience function to creates a DeviceToDeviceMessage proto with |payload|
// and |sequence_number|.
DeviceToDeviceMessage CreateDeviceToDeviceMessage(absl::string_view payload,
                                                  uint32_t sequence_number) {
  DeviceToDeviceMessage device_to_device_message;
  device_to_device_message.set_sequence_number(sequence_number);
  device_to_device_message.set_message(payload);
  return device_to_device_message;
}

// Convert 4 bytes in big-endian representation into an unsigned int.
uint32_t BytesToUnsignedInt(std::vector<uint8_t> bytes) {
  return bytes[0] << 24 | bytes[1] << 12 | bytes[2] << 8 | bytes[3];
}

// Convert an unsigned int into a 4 byte big-endian representation.
std::vector<uint8_t> UnsignedIntToBytes(uint32_t val) {
  return {static_cast<uint8_t>(val >> 24), static_cast<uint8_t>(val >> 12),
          static_cast<uint8_t>(val >> 8), static_cast<uint8_t>(val)};
}

}  // namespace

D2DConnectionContextV1::D2DConnectionContextV1(
    const CryptoOps::SecretKey& encode_key,
    const CryptoOps::SecretKey& decode_key, uint32_t encode_sequence_number,
    uint32_t decode_sequence_number)
    : encode_key_(encode_key),
      decode_key_(decode_key),
      encode_sequence_number_(encode_sequence_number),
      decode_sequence_number_(decode_sequence_number) {}

std::unique_ptr<std::string> D2DConnectionContextV1::EncodeMessageToPeer(
    const std::string& payload) {
  return EncodeMessageToPeer(absl::string_view(payload));
}

std::unique_ptr<std::string> D2DConnectionContextV1::EncodeMessageToPeer(
    absl::string_view payload) {
  encode_sequence_number_++;
  const DeviceToDeviceMessage message =
      CreateDeviceToDeviceMessage(payload, encode_sequence_number_);

  const D2DCryptoOps::Payload payload_with_type(kGcmMetadataType,
                                                message.SerializeAsString());
  return D2DCryptoOps::SigncryptPayload(payload_with_type, encode_key_);
}

std::unique_ptr<std::string> D2DConnectionContextV1::DecodeMessageFromPeer(
    const std::string& message) {
  std::unique_ptr<D2DCryptoOps::Payload> payload =
      D2DCryptoOps::VerifyDecryptPayload(message, decode_key_);
  if (!payload) {
    Util::LogError("DecodeMessageFromPeer: Failed to verify message.");
    return nullptr;
  }

  if (kGcmMetadataType != payload->type()) {
    Util::LogError("DecodeMessageFromPeer: Wrong message type in D2D message.");
    return nullptr;
  }

  DeviceToDeviceMessage d2d_message;
  if (!d2d_message.ParseFromString(payload->message())) {
    Util::LogError("DecodeMessageFromPeer: Unable to parse D2D message proto.");
    return nullptr;
  }

  decode_sequence_number_++;
  if (d2d_message.sequence_number() != decode_sequence_number_) {
    std::ostringstream stream;
    stream << "DecodeMessageFromPeer: Seqno in D2D message ("
           << d2d_message.sequence_number()
           << ") does not match expected seqno (" << decode_sequence_number_
           << ").";
    Util::LogError(stream.str());
    return nullptr;
  }

  return std::unique_ptr<std::string>(d2d_message.release_message());
}

std::unique_ptr<std::string> D2DConnectionContextV1::GetSessionUnique() {
  const ByteBuffer encode_key_data = encode_key_.data();
  const ByteBuffer decode_key_data = decode_key_.data();
  const int32_t encode_key_hash = java_util::JavaHashCode(encode_key_data);
  const int32_t decode_key_hash = java_util::JavaHashCode(decode_key_data);

  const ByteBuffer& first_buffer =
      encode_key_hash < decode_key_hash ? encode_key_data : decode_key_data;
  const ByteBuffer& second_buffer =
      encode_key_hash < decode_key_hash ? decode_key_data : encode_key_data;

  ByteBuffer data_to_hash(D2DCryptoOps::kSalt, D2DCryptoOps::kSaltLength);
  data_to_hash = ByteBuffer::Concat(data_to_hash, first_buffer);
  data_to_hash = ByteBuffer::Concat(data_to_hash, second_buffer);

  std::unique_ptr<ByteBuffer> hash = CryptoOps::Sha256(data_to_hash);
  if (!hash) {
    Util::LogError("GetSessionUnique: SHA-256 hash failed.");
    return nullptr;
  }

  return std::unique_ptr<std::string>(new std::string(hash->String()));
}

// Structure of saved session is:
//
// +---------------------------------------------------------------------------+
// | 1 Byte  |      4 Bytes      |      4 Bytes      |  32 Bytes  |  32 Bytes  |
// +---------------------------------------------------------------------------+
// | Version | encode seq number | decode seq number | encode key | decode key |
// +---------------------------------------------------------------------------+
//
// The sequence numbers are represented in big-endian.
std::unique_ptr<std::string> D2DConnectionContextV1::SaveSession() {
  ByteBuffer byteBuffer = ByteBuffer(&kProtocolVersion, static_cast<size_t>(1));

  // Append encode sequence number.
  std::vector<uint8_t> encode_sequence_number_bytes =
      UnsignedIntToBytes(encode_sequence_number_);
  for (int i = 0; i < encode_sequence_number_bytes.size(); i++) {
    byteBuffer.Append(static_cast<size_t>(1), encode_sequence_number_bytes[i]);
  }

  // Append decode sequence number.
  std::vector<uint8_t> decode_sequence_number_bytes =
      UnsignedIntToBytes(decode_sequence_number_);
  for (int i = 0; i < decode_sequence_number_bytes.size(); i++) {
    byteBuffer.Append(static_cast<size_t>(1), decode_sequence_number_bytes[i]);
  }

  // Append encode key.
  byteBuffer = ByteBuffer::Concat(byteBuffer, encode_key_.data());

  // Append decode key.
  byteBuffer = ByteBuffer::Concat(byteBuffer, decode_key_.data());

  return std::unique_ptr<std::string>(new std::string(byteBuffer.String()));
}

// static.
std::unique_ptr<D2DConnectionContextV1>
D2DConnectionContextV1::FromSavedSession(const std::string& savedSessionInfo) {
  ByteBuffer byteBuffer = ByteBuffer(savedSessionInfo);

  if (byteBuffer.size() != kSavedSessionLength) {
    return nullptr;
  }

  uint32_t encode_sequence_number = BytesToUnsignedInt(
      byteBuffer.SubArray(kEncodeSequenceStart, kEncodeSequenceEnd)->Vector());
  uint32_t decode_sequence_number = BytesToUnsignedInt(
      byteBuffer.SubArray(kDecodeSequenceStart, kDecodeSequenceEnd)->Vector());

  const CryptoOps::SecretKey encode_key =
      CryptoOps::SecretKey(*byteBuffer.SubArray(kEncodeKeyStart, kAesKeyLength),
                           CryptoOps::KeyAlgorithm::AES_256_KEY);
  const CryptoOps::SecretKey decode_key =
      CryptoOps::SecretKey(*byteBuffer.SubArray(kDecodeKeyStart, kAesKeyLength),
                           CryptoOps::KeyAlgorithm::AES_256_KEY);

  return std::unique_ptr<D2DConnectionContextV1>(new D2DConnectionContextV1(
      encode_key, decode_key, encode_sequence_number, decode_sequence_number));
}

}  // namespace securegcm
