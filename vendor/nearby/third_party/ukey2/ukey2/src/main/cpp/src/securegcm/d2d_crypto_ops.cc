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

#include "src/main/cpp/include/securegcm/d2d_crypto_ops.h"

#include <sstream>

#include "src/securemessage/include/securemessage/secure_message_builder.h"
#include "src/securemessage/include/securemessage/secure_message_parser.h"
#include "src/securemessage/include/securemessage/util.h"

namespace securegcm {

using securemessage::CryptoOps;
using securemessage::HeaderAndBody;
using securemessage::SecureMessage;
using securemessage::SecureMessageBuilder;
using securemessage::SecureMessageParser;
using securemessage::Util;

namespace {

// The current protocol version.
const int kSecureGcmProtocolVersion = 1;

// The number of bytes in an expected AES256 key.
const int kAes256KeyLength = 32;
}

// static.
const uint8_t D2DCryptoOps::kSalt[] = {
    0x82, 0xAA, 0x55, 0xA0, 0xD3, 0x97, 0xF8, 0x83, 0x46, 0xCA, 0x1C,
    0xEE, 0x8D, 0x39, 0x09, 0xB9, 0x5F, 0x13, 0xFA, 0x7D, 0xEB, 0x1D,
    0x4A, 0xB3, 0x83, 0x76, 0xB8, 0x25, 0x6D, 0xA8, 0x55, 0x10};

// static.
const size_t D2DCryptoOps::kSaltLength = sizeof(D2DCryptoOps::kSalt);

D2DCryptoOps::Payload::Payload(Type type, const string& message)
    : type_(type), message_(message) {}

D2DCryptoOps::D2DCryptoOps() {}

// static.
std::unique_ptr<string> D2DCryptoOps::SigncryptPayload(
    const Payload& payload, const CryptoOps::SecretKey& secret_key) {
  GcmMetadata gcm_metadata;
  gcm_metadata.set_type(payload.type());
  gcm_metadata.set_version(kSecureGcmProtocolVersion);

  SecureMessageBuilder builder;
  builder.SetPublicMetadata(gcm_metadata.SerializeAsString());

  std::unique_ptr<SecureMessage> secure_message =
      builder.BuildSignCryptedMessage(secret_key, CryptoOps::HMAC_SHA256,
                                      secret_key, CryptoOps::AES_256_CBC,
                                      payload.message());
  if (!secure_message) {
    Util::LogError("Unable to encrypt payload.");
    return nullptr;
  }

  return std::unique_ptr<string>(
      new string(secure_message->SerializeAsString()));
}

// static.
std::unique_ptr<D2DCryptoOps::Payload> D2DCryptoOps::VerifyDecryptPayload(
    const string& signcrypted_message, const CryptoOps::SecretKey& secret_key) {
  SecureMessage secure_message;
  if (!secure_message.ParseFromString(signcrypted_message)) {
    Util::LogError("VerifyDecryptPayload: error parsing SecureMessage.");
    return nullptr;
  }

  std::unique_ptr<HeaderAndBody> header_and_body =
      SecureMessageParser::ParseSignCryptedMessage(
          secure_message, secret_key, CryptoOps::HMAC_SHA256, secret_key,
          CryptoOps::AES_256_CBC, string() /* associated_data */);
  if (!header_and_body) {
    Util::LogError("VerifyDecryptPayload: error verifying SecureMessage.");
    return nullptr;
  }

  if (!header_and_body->header().has_public_metadata()) {
    Util::LogError("VerifyDecryptPayload: no public metadata in header.");
    return nullptr;
  }

  GcmMetadata metadata;
  if (!metadata.ParseFromString(header_and_body->header().public_metadata())) {
    Util::LogError("VerifyDecryptPayload: Failed to parse GcmMetadata.");
    return nullptr;
  }

  if (metadata.version() != kSecureGcmProtocolVersion) {
    std::ostringstream stream;
    stream << "VerifyDecryptPayload: Unsupported protocol version "
           << metadata.version();
    Util::LogError(stream.str());
    return nullptr;
  }

  return std::unique_ptr<Payload>(
      new Payload(metadata.type(), header_and_body->body()));
}

// static.
std::unique_ptr<CryptoOps::SecretKey> D2DCryptoOps::DeriveNewKeyForPurpose(
    const securemessage::CryptoOps::SecretKey& master_key,
    const string& purpose) {
  if (master_key.data().size() != kAes256KeyLength) {
    Util::LogError("DeriveNewKeyForPurpose: Invalid master_key length.");
    return nullptr;
  }

  if (purpose.empty()) {
    Util::LogError("DeriveNewKeyForPurpose: purpose is empty.");
    return nullptr;
  }

  std::unique_ptr<string> raw_derived_key = CryptoOps::Hkdf(
      master_key.data().String(),
      string(reinterpret_cast<const char *>(kSalt), kSaltLength),
      purpose);
  if (!raw_derived_key) {
    Util::LogError("DeriveNewKeyForPurpose: hkdf failed.");
    return nullptr;
  }

  if (raw_derived_key->size() != kAes256KeyLength) {
    Util::LogError("DeriveNewKeyForPurpose: Unexpected size of derived key.");
    return nullptr;
  }

  return std::unique_ptr<CryptoOps::SecretKey>(
      new CryptoOps::SecretKey(*raw_derived_key, CryptoOps::AES_256_KEY));
}

}  // namespace securegcm
