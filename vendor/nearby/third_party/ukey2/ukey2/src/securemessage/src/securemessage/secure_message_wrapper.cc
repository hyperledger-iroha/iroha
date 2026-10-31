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

#include "src/securemessage/include/securemessage/secure_message_wrapper.h"

#include "securemessage.pb.h"
#include "src/securemessage/include/securemessage/util.h"

namespace securemessage {

std::unique_ptr<std::string> SecureMessageWrapper::ParseHeaderIv(
    const std::string& header_and_body_bytes) {
  HeaderAndBody header_and_body;
  if (header_and_body.ParseFromString(header_and_body_bytes) &&
      header_and_body.has_header() && header_and_body.header().has_iv()) {
    return std::unique_ptr<std::string>(
        new std::string(header_and_body.header().iv()));
  } else {
    return nullptr;
  }
}

std::unique_ptr<std::string> SecureMessageWrapper::ParseHeader(
    const std::string& header_and_body_bytes) {
  HeaderAndBody header_and_body;
  if (header_and_body.ParseFromString(header_and_body_bytes) &&
      header_and_body.has_header()) {
    return std::unique_ptr<std::string>(
        new std::string(header_and_body.header().SerializeAsString()));
  } else {
    return nullptr;
  }
}

std::unique_ptr<std::string> SecureMessageWrapper::ParseInternalHeader(
    const std::string& header_and_body_bytes) {
  HeaderAndBodyInternal header_and_body;
  if (header_and_body.ParseFromString(header_and_body_bytes) &&
      header_and_body.has_header()) {
    return std::unique_ptr<std::string>(
        new std::string(header_and_body.header()));
  } else {
    return nullptr;
  }
}

std::unique_ptr<std::string> SecureMessageWrapper::ParseBody(
    const std::string& header_and_body_bytes) {
  HeaderAndBody header_and_body;
  if (header_and_body.ParseFromString(header_and_body_bytes) &&
      header_and_body.has_body()) {
    return std::unique_ptr<std::string>(
        new std::string(header_and_body.body()));
  } else {
    return nullptr;
  }
}

std::unique_ptr<std::string> SecureMessageWrapper::BuildHeaderAndBody(
    const std::string& header_bytes, const std::string& body_bytes) {
  HeaderAndBody header_and_body;
  Header* header = header_and_body.mutable_header();
  header->ParseFromString(header_bytes);
  header_and_body.set_body(body_bytes);

  return std::unique_ptr<std::string>(
      new std::string(header_and_body.SerializeAsString()));
}

int SecureMessageWrapper::GetSignatureScheme(const std::string& header_bytes) {
  Header header;
  header.ParseFromString(header_bytes);
  return header.signature_scheme();
}

int SecureMessageWrapper::GetEncryptionScheme(const std::string& header_bytes) {
  Header header;
  header.ParseFromString(header_bytes);
  return header.encryption_scheme();
}

bool SecureMessageWrapper::HasDecryptionKeyId(const std::string& header_bytes) {
  Header header;
  header.ParseFromString(header_bytes);
  return header.has_decryption_key_id();
}

bool SecureMessageWrapper::HasVerificationKeyId(
    const std::string& header_bytes) {
  Header header;
  header.ParseFromString(header_bytes);
  return header.has_verification_key_id();
}

uint32_t SecureMessageWrapper::GetAssociatedDataLength(
    const std::string& header_bytes) {
  Header header;
  header.ParseFromString(header_bytes);
  return header.associated_data_length();
}

int SecureMessageWrapper::GetSigScheme(CryptoOps::SigType sig_type) {
  switch (sig_type) {
    case CryptoOps::SigType::ECDSA_P256_SHA256:
      return securemessage::ECDSA_P256_SHA256;
    case CryptoOps::SigType::HMAC_SHA256:
      return securemessage::HMAC_SHA256;
    case CryptoOps::SigType::RSA2048_SHA256:
      return securemessage::RSA2048_SHA256;
    case CryptoOps::SigType::SIG_TYPE_END:
      Util::LogErrorAndAbort("wrong sigtype");
      return 0;
  }

  // This should never happen and is undefined behavior if it does.
  Util::LogErrorAndAbort("wrong sigtype");
  return 0;
}

int SecureMessageWrapper::GetEncScheme(CryptoOps::EncType enc_type) {
  switch (enc_type) {
    case CryptoOps::EncType::AES_256_CBC:
      return securemessage::AES_256_CBC;
    case CryptoOps::EncType::NONE:
      return securemessage::NONE;
    case CryptoOps::EncType::ENC_TYPE_END:
      Util::LogErrorAndAbort("wrong enctype");
  }

  // This should never happen and is undefined behavior if it does.
  Util::LogErrorAndAbort("wrong enctype");
}

}  // namespace securemessage
