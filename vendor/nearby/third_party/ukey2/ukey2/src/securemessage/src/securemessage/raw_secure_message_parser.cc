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

// Utility class to parse and verify raw {@link SecureMessage} protos.
// Verifies the signature on the message, and decrypts "signcrypted" messages
// (while simultaneously verifying the signature).
//
// @see RawSecureMessageBuilder

#include "src/securemessage/include/securemessage/raw_secure_message_parser.h"

#include "src/securemessage/include/securemessage/secure_message_wrapper.h"
#include "src/securemessage/include/securemessage/util.h"

using std::unique_ptr;

namespace securemessage {

unique_ptr<string> RawSecureMessageParser::ParseSignedCleartextMessage(
    const string& signature,
    const string& header_and_body,
    const CryptoOps::Key& verification_key,
    CryptoOps::SigType sig_type,
    const string& associated_data) {

  // suppress_associated_data is always false signed cleartext
  if (VerifyHeaderAndBody(signature, header_and_body, verification_key,
                          sig_type, CryptoOps::EncType::NONE, associated_data,
                          false)) {
    return unique_ptr<string>(new string(header_and_body));
  } else {
    return nullptr;
  }
}

unique_ptr<string> RawSecureMessageParser::ParseSignCryptedMessage(
    const string& signature,
    const string& header_and_body,
    const CryptoOps::Key& verification_key,
    CryptoOps::SigType sig_type,
    const CryptoOps::SecretKey& decryption_key,
    CryptoOps::EncType enc_type,
    const string& associated_data) {
  if (enc_type == CryptoOps::EncType::NONE) {
    Util::LogError("Not a signcrypted message");
    return nullptr;
  }

  bool tagRequired = CryptoOps::TaggedPlaintextRequired(
      verification_key, sig_type, decryption_key);
  if (!VerifyHeaderAndBody(signature, header_and_body, verification_key,
                           sig_type, enc_type, associated_data, tagRequired)) {
    return nullptr;
  }

  unique_ptr<string> header =
      SecureMessageWrapper::ParseInternalHeader(header_and_body);
  unique_ptr<string> iv =
      SecureMessageWrapper::ParseHeaderIv(header_and_body);
  unique_ptr<string> body =
      SecureMessageWrapper::ParseBody(header_and_body);

  if (header == nullptr || iv == nullptr || body == nullptr) {
    Util::LogError("Header and body missing some fields");
    return nullptr;
  }

  unique_ptr<string> raw_decrypted_body =
      CryptoOps::Decrypt(decryption_key, enc_type, *iv, *body);

  if (raw_decrypted_body == nullptr) {
    Util::LogError("Failed to decrypt body");
    return nullptr;
  }

  if (!tagRequired) {
    // No tag expected, so we're all done
    return SecureMessageWrapper::BuildHeaderAndBody(*header,
                                                    *raw_decrypted_body);
  }

  // Verify the tag that binds the ciphertext to the header, and remove it
  bool binding_is_verified = false;
  unique_ptr<string> expected_tag =
      CryptoOps::Digest(*header + associated_data);
  if (expected_tag == nullptr) {
    Util::LogError("Error computing expected tag");
    return nullptr;
  }

  if (raw_decrypted_body->length() >= CryptoOps::kDigestLength) {
    unique_ptr<ByteBuffer> actual_tag =
        ByteBuffer(*raw_decrypted_body).SubArray(0, CryptoOps::kDigestLength);

    if (actual_tag != nullptr && actual_tag->Equals(ByteBuffer(*expected_tag))) {
      binding_is_verified = true;
    }
  }
  if (!binding_is_verified) {
    Util::LogError("Signature exception");
    return nullptr;
  }

  unsigned int body_len = static_cast<unsigned int>(raw_decrypted_body->size())
      - CryptoOps::kDigestLength;

  // Remove the tag.
  unique_ptr<ByteBuffer> body_bytes =
      ByteBuffer(*raw_decrypted_body).SubArray(CryptoOps::kDigestLength,
                                               body_len);

  if (body_bytes == nullptr) {
    Util::LogError("Invalid body length");
    return nullptr;
  }

  return SecureMessageWrapper::BuildHeaderAndBody(*header,
                                                  body_bytes->String());
}

bool RawSecureMessageParser::VerifyHeaderAndBody(
    const string& signature,
    const string& header_and_body,
    const CryptoOps::Key& verification_key,
    CryptoOps::SigType sig_type,
    CryptoOps::EncType enc_type,
    const string& associated_data,
    bool suppress_associated_data /* in case it is in the tag instead */) {

  string signed_data = header_and_body;
  if (!suppress_associated_data) {
    signed_data.append(associated_data);
  }
  bool verified =
      CryptoOps::Verify(sig_type, verification_key, signature, signed_data);

  unique_ptr<string> header =
      SecureMessageWrapper::ParseHeader(header_and_body);

  if (header == nullptr) {
    Util::LogError("message must have header");
    return false;
  }

  // Does not return early in order to avoid timing attacks
  verified &= SecureMessageWrapper::GetSigScheme(sig_type) ==
              SecureMessageWrapper::GetSignatureScheme(*header);
  verified &= SecureMessageWrapper::GetEncScheme(enc_type) ==
              SecureMessageWrapper::GetEncryptionScheme(*header);
  verified &= associated_data.length() ==
              SecureMessageWrapper::GetAssociatedDataLength(*header);

  // Check that either a decryption operation is expected, or no DecryptionKeyId
  // is set.
  verified &= enc_type != CryptoOps::EncType::NONE ||
              !SecureMessageWrapper::HasDecryptionKeyId(*header);

  // If encryption was used, check that either we are not using a public key
  // signature or a VerificationKeyId was set (as is required for public key
  // based signature + encryption).
  verified &= enc_type != CryptoOps::EncType::NONE ||
              CryptoOps::IsPublicKeyScheme(sig_type) ||
              !SecureMessageWrapper::HasVerificationKeyId(*header);

  return verified;
}

// Constructor and Destructor are never used
RawSecureMessageParser::RawSecureMessageParser() {}
RawSecureMessageParser::~RawSecureMessageParser() {}

}  // namespace securemessage
