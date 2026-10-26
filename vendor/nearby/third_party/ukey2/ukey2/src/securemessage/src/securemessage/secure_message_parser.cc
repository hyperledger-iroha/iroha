/*
 * Copyright 2014 Google Inc. All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// Utility class to parse and verify {@link SecureMessage} protos. Verifies the
// signature on the message, and decrypts "signcrypted" messages (while
// simultaneously verifying the signature).
//
// @see SecureMessageBuilder

#include "src/securemessage/include/securemessage/secure_message_parser.h"

#include "src/securemessage/include/securemessage/raw_secure_message_parser.h"
#include "src/securemessage/include/securemessage/util.h"

namespace securemessage {

std::unique_ptr<Header> SecureMessageParser::GetUnverifiedHeader(
    const SecureMessage& secmsg) {
  if (!secmsg.has_header_and_body()) {
    Util::LogError("message must have header and body");
    return nullptr;
  }

  HeaderAndBody header_and_body;
  if (!header_and_body.ParseFromString(secmsg.header_and_body())) {
    Util::LogError("Invalid header and body string");
    return nullptr;
  }
  if (!header_and_body.IsInitialized() || !header_and_body.has_header()) {
    Util::LogError("Header missing from header and body");
    return nullptr;
  }
  std::unique_ptr<Header> header(new Header(header_and_body.header()));

  if (!header->has_signature_scheme()) {
    Util::LogError("Header must have a valid signature scheme");
    return nullptr;
  }
  if (!header->has_encryption_scheme()) {
    Util::LogError("Header must have a valid encryption scheme");
    return nullptr;
  }

  return header;
}

std::unique_ptr<HeaderAndBody> SecureMessageParser::ParseSignedCleartextMessage(
    const SecureMessage& secmsg, const CryptoOps::Key& verification_key,
    CryptoOps::SigType sig_type) {
  // associated_data is not used here, so we pass in an empty string
  return ParseSignedCleartextMessage(secmsg, verification_key, sig_type,
                                     string());
}

std::unique_ptr<HeaderAndBody> SecureMessageParser::ParseSignedCleartextMessage(
        const SecureMessage& secmsg, const CryptoOps::Key& verification_key,
        CryptoOps::SigType sig_type, const string& associated_data) {
  if (!secmsg.has_header_and_body()) {
    Util::LogError("message must have header and body");
    return nullptr;
  }
  if (!secmsg.has_signature()) {
    Util::LogError("message must have signature");
    return nullptr;
  }
  std::unique_ptr<string> header_and_body_bytes =
      RawSecureMessageParser::ParseSignedCleartextMessage(
          secmsg.signature(), secmsg.header_and_body(),
          verification_key, sig_type, associated_data);

  if (header_and_body_bytes != nullptr) {
    HeaderAndBody *header_and_body = new HeaderAndBody();
    header_and_body->ParseFromString(*header_and_body_bytes);
    return std::unique_ptr<HeaderAndBody>(header_and_body);
  } else {
    return nullptr;
  }
}

std::unique_ptr<HeaderAndBody> SecureMessageParser::ParseSignCryptedMessage(
    const SecureMessage& secmsg, const CryptoOps::Key& verification_key,
    CryptoOps::SigType sig_type, const CryptoOps::SecretKey& decryption_key,
    CryptoOps::EncType enc_type, const string& associated_data) {
  if (!secmsg.has_header_and_body()) {
    Util::LogError("message must have header and body");
    return nullptr;
  }
  if (!secmsg.has_signature()) {
    Util::LogError("message must have signature");
    return nullptr;
  }

  std::unique_ptr<string> header_and_body_bytes =
      RawSecureMessageParser::ParseSignCryptedMessage(
          secmsg.signature(), secmsg.header_and_body(),
          verification_key, sig_type, decryption_key, enc_type,
          associated_data);

  if (header_and_body_bytes != nullptr) {
    HeaderAndBody *header_and_body = new HeaderAndBody();
    header_and_body->ParseFromString(*header_and_body_bytes);
    return std::unique_ptr<HeaderAndBody>(header_and_body);
  } else {
    return nullptr;
  }
}

// Constructor and Destructor are never used
SecureMessageParser::SecureMessageParser() {}
SecureMessageParser::~SecureMessageParser() {}

}  // namespace securemessage
