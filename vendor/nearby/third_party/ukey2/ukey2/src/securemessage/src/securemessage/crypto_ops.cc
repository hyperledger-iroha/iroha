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

//
// This file is a partial implementation of the CryptoOps class.  The remaining
// functions are specified in a crypto-library specific continuation (e.g., the
// OpenSSL versions are in cryto_ops_openssl.cc
//

#include <stddef.h>

#include "src/securemessage/include/securemessage/crypto_ops.h"
#include "src/securemessage/include/securemessage/secure_message_wrapper.h"
#include "src/securemessage/include/securemessage/util.h"

using std::unique_ptr;

// Maximum number of bytes in a 2's complement encoding of a NIST P-256 elliptic
// curve point.
static const size_t kMaxP256EncodingBytes = 33;

// Maximum number of bytes in a 2's complement encoding of a 2048 bit RSA
// key modulus.
static const size_t kMaxRsa2048ModulusEncodingBytes = 257;

// The expected byte size of a 2048 bit RSA key modulus. The modulus bit size
// is 8 * |kRsa2048ModulusByteSize|.
static const size_t kRsa2048ModulusByteSize = 256;

// TODO(aczeskis): get rid of as many uses of string as possible and move to
// using ByteBuffer because of secure wipe. If easier and less refactoring,
// make a custom string that wipes memory on deallocation
namespace securemessage {

// We have to initialize salt this way because C++ doesn't allow initialization
// and declaration in the same place... Go figure.
// The value below is SHA256("SecureMessage").  This is tested in the TestSalt
// test in crypto_ops_test
const uint8_t CryptoOps::kSalt[kSaltSize] = {
    0xbf, 0x9d, 0x2a, 0x53, 0xc6, 0x36, 0x16, 0xd7, 0x5d, 0xb0, 0xa7,
    0x16, 0x5b, 0x91, 0xc1, 0xef, 0x73, 0xe5, 0x37, 0xf2, 0x42, 0x74,
    0x05, 0xfa, 0x23, 0x61, 0x0a, 0x4b, 0xe6, 0x57, 0x64, 0x2e};

unique_ptr<ByteBuffer> CryptoOps::HkdfSha256Extract(
    const ByteBuffer& inputKeyMaterial,
    const ByteBuffer& salt) {
  // Computes an HMAC of the inputKeyMaterial keyed with the salt
  unique_ptr<ByteBuffer> result = Sha256hmac(salt, inputKeyMaterial);

  if (result == nullptr) {
    Util::LogError("HMAC returned null");
  }

  return result;
}

unique_ptr<ByteBuffer> CryptoOps::HkdfSha256Expand(
    const ByteBuffer& pseudoRandomKey,
    const ByteBuffer& info) {
  if (pseudoRandomKey.size() == 0) {
    Util::LogError("length of PseudoRandomKey is zero");
    return nullptr;
  }

  // Computes an HMAC of info || 0x01 (where || is concatenation)
  ByteBuffer extendedInfo = info;
  extendedInfo.Append(static_cast<size_t>(1), static_cast<uint8_t>(0x01));
  unique_ptr<ByteBuffer> result = Sha256hmac(pseudoRandomKey, extendedInfo);

  if (result == nullptr) {
    Util::LogError("HMAC returned null");
  }

  return result;
}

unique_ptr<string> CryptoOps::Hkdf(const string& inputKeyMaterial,
                                   const string& salt,
                                   const string& info) {
  unique_ptr<ByteBuffer> extracted_result =
      HkdfSha256Extract(ByteBuffer(inputKeyMaterial), ByteBuffer(salt));
  if (extracted_result == nullptr || extracted_result->size() == 0) {
    Util::LogError("HKDF_Extract returned an invalid result");
    return nullptr;
  }

  unique_ptr<ByteBuffer> expanded_result =
      HkdfSha256Expand(*extracted_result, ByteBuffer(info));

  if (expanded_result == nullptr) {
    Util::LogError("HkdfSha256Expand return an invalid result");
    return nullptr;
  }

  return unique_ptr<string>(new string(expanded_result->String()));
}

unique_ptr<CryptoOps::SecretKey> CryptoOps::DeriveAes256KeyFor(
    const CryptoOps::SecretKey& masterKey, const string& purpose) {
  string key_string(masterKey.data().String());
  string salt_string(reinterpret_cast<const char*>(kSalt), kSaltSize);
  unique_ptr<string> key_data = Hkdf(key_string, salt_string, purpose);

  if (key_data == nullptr || key_data->size() != kAesKeySize) {
    Util::LogError("HKDF returned invalid key");
    return nullptr;
  }

  unique_ptr<CryptoOps::SecretKey> derived_key =
      unique_ptr<CryptoOps::SecretKey>(
          new CryptoOps::SecretKey(*key_data, CryptoOps::AES_256_KEY));
  return derived_key;
}

unique_ptr<string> CryptoOps::Digest(const string& data) {
  unique_ptr<ByteBuffer> full_digest = Sha256(ByteBuffer(data));

  if (full_digest == nullptr || full_digest->size() != kSha256DigestSize) {
    return nullptr;
  }

  return unique_ptr<string>(
      new string(full_digest->SubArray(0, kDigestLength)->String()));
}

unique_ptr<string> CryptoOps::Decrypt(const CryptoOps::SecretKey& decryptionKey,
                                      CryptoOps::EncType encType,
                                      const string& iv,
                                      const string& ciphertext) {
  if (ciphertext.empty()) {
    Util::LogError("Cannot decrypt empty ciphertext");
    return nullptr;
  }
  if (decryptionKey.data().size() == 0) {
    Util::LogError("Cannot decrypt using empty decryption key");
    return nullptr;
  }
  if (iv.empty()) {
    Util::LogError("Cannot decrypt with empty iv");
    return nullptr;
  }

  unique_ptr<ByteBuffer> plaintext = nullptr;

  switch (encType) {
    case NONE: {
      plaintext = nullptr;
      break;
    }
    case AES_256_CBC: {
      unique_ptr<SecretKey> derived_key =
          CryptoOps::DeriveAes256KeyFor(decryptionKey, GetPurpose(encType));
      plaintext = Aes256CBCDecrypt(*derived_key, ByteBuffer(iv),
                                   ByteBuffer(ciphertext));
      break;
    }
    default:
      // This should never happen and might indicate an attack
      Util::LogError("Invalid encryption type");
      return nullptr;
  }

  if (plaintext == nullptr) {
    Util::LogError("Could not decrypt ciphertext");
    return nullptr;
  }

  return unique_ptr<string>(new string(plaintext->String()));
}

unique_ptr<string> CryptoOps::Encrypt(const CryptoOps::SecretKey& encryptionKey,
                                      CryptoOps::EncType encType,
                                      const string& iv,
                                      const string& plaintext) {
  if (encryptionKey.data().size() == 0) {
    Util::LogError("Cannot encrypt using empty encryption key");
    return nullptr;
  }

  unique_ptr<ByteBuffer> ciphertext = nullptr;

  switch (encType) {
    case NONE: {
      ciphertext = nullptr;
      break;
    }
    case AES_256_CBC: {
      unique_ptr<SecretKey> derived_key =
          CryptoOps::DeriveAes256KeyFor(encryptionKey, GetPurpose(encType));
      ciphertext =
          Aes256CBCEncrypt(*derived_key, ByteBuffer(iv), ByteBuffer(plaintext));
      break;
    }
    default:
      // This should never happen and might indicate an attack
      Util::LogError("Invalid encryption type");
      return nullptr;
  }

  if (ciphertext == nullptr) {
    Util::LogError("Could not encrypt plaintext");
    return nullptr;
  }

  return unique_ptr<string>(new string(ciphertext->String()));
}

unique_ptr<CryptoOps::SecretKey> CryptoOps::KeyAgreementSha256(
    const PrivateKey& private_key,
    const PublicKey& peer_key) {
  if (private_key.algorithm() != peer_key.algorithm()) {
    Util::LogError("Algorithms for public and private key must match");
    return nullptr;
  }
  if (private_key.algorithm() != ECDSA_KEY) {
    Util::LogError("Only ECDSA algorithms are supported for Key agreements");
    return nullptr;
  }

  unique_ptr<ByteBuffer> secret = EcdhKeyAgreement(private_key, peer_key);
  if (secret == nullptr) {
    Util::LogError("Error computing key agreement");
    return nullptr;
  }

  unique_ptr<ByteBuffer> secret_digest = Sha256(*secret);
  if (secret_digest == nullptr) {
    Util::LogError("Error computing Sha256");
    return nullptr;
  }

  return unique_ptr<SecretKey>(new SecretKey(*secret_digest, AES_256_KEY));
}

string CryptoOps::GetPurpose(EncType encType) {
  return "ENC:" +
      std::to_string(SecureMessageWrapper::GetEncScheme(encType));
}

string CryptoOps::GetPurpose(SigType sigType) {
  return "SIG:" +
      std::to_string(SecureMessageWrapper::GetSigScheme(sigType));
}

bool CryptoOps::IsPublicKeyScheme(SigType sigType) {
  switch (sigType) {
    case HMAC_SHA256:
      return false;
    case ECDSA_P256_SHA256:
      return true;
    case RSA2048_SHA256:
      return true;
    case SIG_TYPE_END:
      // Exists only for testing, should never actually be called.
      Util::LogErrorAndAbort("wrong sigtype");
  }

  // Control should never reach here, this is here to make the compiler happy
  Util::LogErrorAndAbort("wrong sigtype");
  return false;
}

unique_ptr<string> CryptoOps::Sign(SigType sigType, const Key& signingKey,
                                   const string& data) {
  if (signingKey.data().size() == 0) {
    Util::LogError("Signing Key cannot be empty!");
    return nullptr;
  }
  if (data.length() == 0) {
    Util::LogError("Cannot sign empty data");
    return nullptr;
  }

  unique_ptr<ByteBuffer> signature = nullptr;
  switch (sigType) {
    case HMAC_SHA256: {
      if (signingKey.type() != SECRET) {
        Util::LogError("Invalid signing key type");
        return nullptr;
      }
      unique_ptr<SecretKey> derived_key = DeriveAes256KeyFor(
          SecretKey(signingKey.data(), signingKey.algorithm()),
          GetPurpose(sigType));
      if (derived_key == nullptr) {
        Util::LogError("Invalid derived key");
        return nullptr;
      }
      signature = Sha256hmac(derived_key->data(), ByteBuffer(data));
      break;
    }
    case ECDSA_P256_SHA256: {
      if (signingKey.type() != PRIVATE) {
        Util::LogError("Expected a private key");
        return nullptr;
      }
      signature = EcdsaP256Sha256Sign(
          PrivateKey(signingKey.data(), signingKey.algorithm()),
          ByteBuffer(data));
      break;
    }
    case RSA2048_SHA256: {
      if (signingKey.type() != PRIVATE) {
        Util::LogError("Expected a private key");
        return nullptr;
      }
      signature = Rsa2048Sha256Sign(
          PrivateKey(signingKey.data(), signingKey.algorithm()),
          ByteBuffer(data));
      break;
    }
    case SIG_TYPE_END:
      // Case only exists for testing and is never expected to actually be
      // called.
     Util::LogErrorAndAbort("wrong sigtype");
  }

  if (signature == nullptr) {
    return nullptr;
  } else {
    return unique_ptr<string>(new string(signature->String()));
  }
}

bool CryptoOps::Verify(SigType sigType,
                       const Key& verificationKey,
                       const string& signature,
                       const string& data) {
  // Do some basic checks
  if (verificationKey.data().size() == 0) {
    Util::LogError("Verification Key cannot be empty!");
    return false;
  }
  if (signature.length() == 0) {
    Util::LogError("Signature cannot be empty");
    return false;
  }
  if (data.length() == 0) {
    Util::LogError("Cannot verify signature over empty data");
    return false;
  }

  // Decide what to do based on the signature type
  switch (sigType) {
    case HMAC_SHA256: {
      if (verificationKey.type() != SECRET) {
        Util::LogError("Invalid signing key type");
        return false;
      }
      // Compute expected signature
      unique_ptr<SecretKey> derived_key = DeriveAes256KeyFor(
          SecretKey(verificationKey.data(), verificationKey.algorithm()),
          GetPurpose(sigType));
      if (derived_key == nullptr) {
        Util::LogError("Invalid derived key");
        return false;
      }
      unique_ptr<ByteBuffer> expected_signature =
          Sha256hmac(ByteBuffer(derived_key->data()), ByteBuffer(data));
      if (expected_signature == nullptr) {
        Util::LogError("Invalid expected signature");
        return false;
      }

      // Constant time array comparison
      return expected_signature->Equals(ByteBuffer(signature));
    }

    case ECDSA_P256_SHA256: {
      if (verificationKey.type() != PUBLIC) {
        Util::LogError("Expected a public key");
        return false;
      }
      if (verificationKey.algorithm() != KeyAlgorithm::ECDSA_KEY) {
        Util::LogError("Wrong key type");
        return false;
      }

      return EcdsaP256Sha256Verify(
          PublicKey(verificationKey.data(), verificationKey.algorithm()),
          ByteBuffer(signature), ByteBuffer(data));
    }

    case RSA2048_SHA256: {
      if (verificationKey.type() != PUBLIC) {
        Util::LogError("Expected a public key");
        return false;
      }
      if (verificationKey.algorithm() != KeyAlgorithm::RSA_KEY) {
        Util::LogError("Wrong key type");
        return false;
      }

      return Rsa2048Sha256Verify(
          PublicKey(verificationKey.data(), verificationKey.algorithm()),
          ByteBuffer(signature), ByteBuffer(data));
    }

    default:
      return false;
  }
}

bool CryptoOps::TaggedPlaintextRequired(const Key& signing_key,
                                        SigType sig_type,
                                        const Key& encryption_key) {
  // We need a tag if different keys are being used to "sign" vs. encrypt
  return IsPublicKeyScheme(sig_type) ||
      !signing_key.data().Equals(encryption_key.data());
}

bool CryptoOps::IsValidEcP256CoordinateEncoding(const string& bytes) {
  // The bytes must be between 1 and Max P256 encoding bytes, inclusive. If
  // the bytes are full, then the first byte must not be 0.
  return !(
      (bytes.size() == 0) ||
      (bytes.size() > kMaxP256EncodingBytes) ||
      (bytes.size() == kMaxP256EncodingBytes && bytes.data()[0] != 0));
}

bool CryptoOps::IsValidRsa2048ModulusEncoding(const string& bytes) {
  if (bytes.size() > kMaxRsa2048ModulusEncodingBytes)
    return false;

  if (bytes.size() < kRsa2048ModulusByteSize)
    return false;

  // Index at which the first non-zero byte in |bytes| should be.
  const size_t leading_byte_index = bytes.size() - kRsa2048ModulusByteSize;
  for (size_t i = 0; i < leading_byte_index; ++i) {
    if (bytes[i] != 0)
      return false;
  }

  // Make sure that the first non-zero byte has leading 1.
  return static_cast<uint8_t>(bytes[leading_byte_index]) & 0x80;
}

string CryptoOps::Int32BytesToString(int32_t value) {
  string result(sizeof(value), '\0');
  for (size_t i = sizeof(value); i > 0 && value != 0; --i) {
    result[i - 1] = static_cast<char>(value & 0xFF);
    value >>= 8;
  }
  return result;
}

bool CryptoOps::StringToInt32Bytes(const string& value, int32_t* result) {
  if (value.length() > sizeof(*result))
    return false;

  *result = 0;
  for (size_t i = 0; i < value.length(); ++i) {
    *result <<= 8;
    *result |= static_cast<uint8_t>(value[i]);
  }
  return true;
}

CryptoOps::~CryptoOps() {}

}  // namespace securemessage
