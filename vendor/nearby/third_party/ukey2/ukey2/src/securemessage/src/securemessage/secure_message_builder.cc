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

#include "src/securemessage/include/securemessage/secure_message_builder.h"

#include "src/securemessage/include/securemessage/secure_message_wrapper.h"
#include "src/securemessage/include/securemessage/util.h"

using std::unique_ptr;

namespace securemessage {

SecureMessageBuilder::SecureMessageBuilder() {
  Reset();
}

SecureMessageBuilder::~SecureMessageBuilder() {}

SecureMessageBuilder* SecureMessageBuilder::Reset() {
  public_metadata_ = nullptr;
  decryption_key_id_ = nullptr;
  verification_key_id_ = nullptr;
  associated_data_ = nullptr;
  return this;
}

SecureMessageBuilder* SecureMessageBuilder::SetPublicMetadata(
    const string& public_metadata) {
  public_metadata_ = unique_ptr<ByteBuffer>(new ByteBuffer(public_metadata));
  return this;
}

SecureMessageBuilder* SecureMessageBuilder::SetVerificationKeyId(
    const string& verification_key_id) {
  verification_key_id_ =
      unique_ptr<ByteBuffer>(new ByteBuffer(verification_key_id));
  return this;
}

SecureMessageBuilder* SecureMessageBuilder::SetDecryptionKeyId(
    const string& decryption_key_id) {
  decryption_key_id_ =
      unique_ptr<ByteBuffer>(new ByteBuffer(decryption_key_id));
  return this;
}

SecureMessageBuilder* SecureMessageBuilder::SetAssociatedData(
    const string& associated_data) {
  if (associated_data.length() > 0) {
    associated_data_ = unique_ptr<ByteBuffer>(new ByteBuffer(associated_data));
  } else {
    associated_data_ = nullptr;
  }
  return this;
}

std::unique_ptr<SecureMessage>
SecureMessageBuilder::BuildSignedCleartextMessage(
    const CryptoOps::Key& signing_key, CryptoOps::SigType sig_type,
    const string& body) {

  if (body.length() == 0) {
    Util::LogError("Cannot build a secure message with an empty body");
    return nullptr;
  }

  if (decryption_key_id_ != nullptr) {
    Util::LogError("Cannot set decryption key id for a cleartext message");
    return nullptr;
  }

  ByteBuffer associated_data;
  if (associated_data_ != nullptr) {
    associated_data = *associated_data_;
  }

  unique_ptr<Header> header =
      BuildHeader(sig_type, CryptoOps::EncType::NONE, nullptr);

  ByteBuffer header_and_body =
      SerializeHeaderAndBody(header->SerializeAsString(), body);

  return CreateSignedResult(signing_key, sig_type, header_and_body,
                            associated_data);
}

std::unique_ptr<SecureMessage> SecureMessageBuilder::BuildSignCryptedMessage(
    const CryptoOps::Key& signing_key, CryptoOps::SigType sig_type,
    const CryptoOps::SecretKey& encryption_key, CryptoOps::EncType enc_type,
    const string& body) {

  if (body.length() == 0) {
    Util::LogError("Cannot build a secure message with an empty body");
    return nullptr;
  }

  if (enc_type == CryptoOps::EncType::NONE) {
    Util::LogError("NONE type not supported for encrypted messages");
    return nullptr;
  }

  if (CryptoOps::IsPublicKeyScheme(sig_type) &&
      verification_key_id_ == nullptr) {
    Util::LogError(
        "Must set a verification key id when using public key signature");
    return nullptr;
  }

  unique_ptr<string> iv = CryptoOps::GenerateIv(enc_type);
  if (iv == nullptr) {
    Util::LogError("Could not generate IV for given encryption type");
    return nullptr;
  }

  unique_ptr<Header> header = BuildHeader(sig_type, enc_type, iv);
  if (header == nullptr) {
    Util::LogError("Could not build header");
    return nullptr;
  }

  // We may or may not need an extra tag in front of the plaintext body
  ByteBuffer tagged_body;
  ByteBuffer associated_data_to_be_signed;
  string header_bytes = header->SerializeAsString();

  if (CryptoOps::TaggedPlaintextRequired(signing_key, sig_type,
                                         encryption_key)) {
    // Place a "tag" in front of the the plaintext message containing a
    // digest of the header
    string to_digest = header_bytes;
    if (associated_data_ != nullptr) {
      to_digest += associated_data_->String();
    }

    unique_ptr<string> digest = CryptoOps::Digest(to_digest);
    if (digest == nullptr) {
      Util::LogError("Error computing digest");
      return nullptr;
    }

    tagged_body = ByteBuffer::Concat(
        ByteBuffer(*digest),
        ByteBuffer(body));
  } else {
    tagged_body = ByteBuffer(body);

    if (associated_data_ != nullptr) {
      associated_data_to_be_signed = *associated_data_;
    }
  }

  // Compute the encrypted body, which binds the tag to the message inside the
  // ciphertext
  unique_ptr<string> encrypted_body =
      CryptoOps::Encrypt(encryption_key, enc_type, *iv, tagged_body.String());
  if (encrypted_body == nullptr) {
    Util::LogError("Could not encrypt body");
    return nullptr;
  }

  ByteBuffer header_and_body =
      SerializeHeaderAndBody(header_bytes, *encrypted_body);

  return CreateSignedResult(signing_key, sig_type, header_and_body,
                            associated_data_to_be_signed);
}

unique_ptr<Header> SecureMessageBuilder::BuildHeader(
    CryptoOps::SigType sig_type, CryptoOps::EncType enc_type,
    const unique_ptr<string>& iv) {
  int sig_scheme = SecureMessageWrapper::GetSigScheme(sig_type);
  int enc_scheme = SecureMessageWrapper::GetEncScheme(enc_type);
  if (!sig_scheme || !enc_scheme) {
    return NULL;
  }

  unique_ptr<Header> result(new Header());
  result->set_signature_scheme(static_cast<SigScheme>(sig_scheme));
  result->set_encryption_scheme(static_cast<EncScheme>(enc_scheme));

  // If we need to, set the rest
  if (verification_key_id_ != nullptr) {
    result->set_verification_key_id(verification_key_id_->ImmutableUInt8(),
                                    verification_key_id_->size());
  }
  if (decryption_key_id_ != nullptr) {
    result->set_decryption_key_id(decryption_key_id_->ImmutableUInt8(),
                                  decryption_key_id_->size());
  }
  if (public_metadata_ != nullptr) {
    result->set_public_metadata(public_metadata_->ImmutableUInt8(),
                                public_metadata_->size());
  }
  if (associated_data_ != nullptr) {
    result->set_associated_data_length(associated_data_->size());
  }
  if (iv != nullptr) {
    result->set_iv(*iv);
  }

  return result;
}

ByteBuffer SecureMessageBuilder::SerializeHeaderAndBody(const string& header,
                                                        const string& body) {
  HeaderAndBodyInternal header_and_body_internal;
  header_and_body_internal.set_header(header);
  header_and_body_internal.set_body(body);

  return ByteBuffer(header_and_body_internal.SerializeAsString());
}

std::unique_ptr<SecureMessage> SecureMessageBuilder::CreateSignedResult(
    const CryptoOps::Key& signing_key, const CryptoOps::SigType& sig_type,
    const ByteBuffer& header_and_body, const ByteBuffer& associated_data) {
  ByteBuffer header_and_body_with_associated_data =
      ByteBuffer::Concat(header_and_body, associated_data);

  unique_ptr<string> signature =
      CryptoOps::Sign(sig_type, signing_key,
                      header_and_body_with_associated_data.String());

  // Return nullptr if an error occurred
  if (signature == nullptr) {
    return nullptr;
  }

  unique_ptr<SecureMessage> result(new SecureMessage());
  result->set_header_and_body(header_and_body.String());
  result->set_signature(*signature);

  return result;
}

}  // namespace securemessage
