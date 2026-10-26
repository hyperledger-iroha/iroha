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

#include <openssl/aes.h>
#include <openssl/bn.h>
#include <openssl/ec.h>
#include <openssl/ecdsa.h>
#include <openssl/evp.h>
#include <openssl/hmac.h>
#include <openssl/obj_mac.h>
#include <openssl/ossl_typ.h>
#include <openssl/rand.h>
#include <openssl/rsa.h>
#include <openssl/sha.h>
#include <openssl/x509.h>

#include <stddef.h>
#include <climits>
#include <cstdint>
#include <cstdlib>
#include <memory>
#include <string>

#include "src/securemessage/include/securemessage/byte_buffer.h"
#include "src/securemessage/include/securemessage/crypto_ops.h"
#include "src/securemessage/include/securemessage/util.h"

#if defined(OPENSSL_IS_BORINGSSL)
#include <openssl/bytestring.h>
#include <openssl/mem.h>
#endif

using std::unique_ptr;

class EvpKeyPtr : public std::unique_ptr<EVP_PKEY, void (*)(EVP_PKEY *)> {
 public:
  explicit EvpKeyPtr(EVP_PKEY *pkp)
      : std::unique_ptr<EVP_PKEY, void (*)(EVP_PKEY *)>(pkp, EVP_PKEY_free) {}
};

namespace securemessage {

static unique_ptr<ByteBuffer> PublicKeyToBytes(EC_KEY *eckey) {
  int pub_key_len = i2d_EC_PUBKEY(eckey, NULL);
  if (pub_key_len <= 0) {
    Util::LogError("Unexpected pub key length for generated key");
    return nullptr;
  }

  unsigned char *pub_key_buffer = NULL;
  int return_code = i2d_EC_PUBKEY(eckey, &pub_key_buffer);
  if (return_code != pub_key_len) {
    OPENSSL_free(pub_key_buffer);
    Util::LogErrorAndAbort("i2d_EC_PUBKEY returned an unexpected value");
    return nullptr;
  }

  auto public_key_bytes = unique_ptr<ByteBuffer>(new ByteBuffer(
      pub_key_buffer, pub_key_len));
  OPENSSL_free(pub_key_buffer);
  return public_key_bytes;
}

// Create an EVP_PKEY structure using exported data for a private key.
static EvpKeyPtr CreateEvpPrivateKey(const CryptoOps::PrivateKey& key) {
  // Extract key from storage.
#if defined(OPENSSL_IS_BORINGSSL)
  // Use EVP_parse_private_key in BoringSSL to avoid a dependency on the legacy
  // ASN.1 stack and the large objects table.
  CBS cbs;
  CBS_init(&cbs, key.data().ImmutableUInt8(), key.data().size());
  EvpKeyPtr extracted_evpkey(EVP_parse_private_key(&cbs));
#else
  const unsigned char *stored_priv_key = key.data().ImmutableUChar();
  PKCS8_PRIV_KEY_INFO *extracted_pkcs8 =
      d2i_PKCS8_PRIV_KEY_INFO(NULL, &stored_priv_key, key.data().size());
  if (extracted_pkcs8 == NULL) {
    Util::LogError("could not decode pkcs8 structure");
    return EvpKeyPtr(NULL);
  }

  // Put it into an EVP structure.
  EvpKeyPtr extracted_evpkey(EVP_PKCS82PKEY(extracted_pkcs8));
  // Careful to free this before we return null anywhere.
  PKCS8_PRIV_KEY_INFO_free(extracted_pkcs8);
#endif

  if (extracted_evpkey.get() == NULL) {
    Util::LogError("could not extract key from pkcs8 structure");
    return EvpKeyPtr(NULL);
  }

  return extracted_evpkey;
}

// Create an EVP_PKEY structure using exported data for a public key.
static EvpKeyPtr CreateEvpPublicKey(const CryptoOps::PublicKey& key) {
  const unsigned char *pub_key_ptr = key.data().ImmutableUChar();
  EvpKeyPtr extracted_public_evpkey(
      d2i_PUBKEY(NULL, &pub_key_ptr, key.data().size()));

  if (extracted_public_evpkey == nullptr) {
    Util::LogError("could not import public key");
    return EvpKeyPtr(NULL);
  }

  return extracted_public_evpkey;
}

unique_ptr<ByteBuffer> CryptoOps::Sha256(const ByteBuffer& message) {
  if (message.size() == 0) {
    Util::LogError("Message too short");
    return nullptr;
  }

  // Create an empty digest with enough space. OpenSSL will tell us how much it
  // actually wrote.
  uint8_t digest[SHA256_DIGEST_LENGTH];
  SHA256(message.ImmutableUInt8(), message.size(), digest);

  return unique_ptr<ByteBuffer>(new ByteBuffer(digest, sizeof(digest)));
}

unique_ptr<ByteBuffer> CryptoOps::Sha512(const ByteBuffer &message) {
  if (message.size() == 0) {
    Util::LogError("Message too short");
    return nullptr;
  }

  uint8_t digest[SHA512_DIGEST_LENGTH];
  SHA512(message.ImmutableUInt8(), message.size(), digest);

  return unique_ptr<ByteBuffer>(new ByteBuffer(digest, sizeof(digest)));
}

unique_ptr<ByteBuffer> CryptoOps::Sha256hmac(const ByteBuffer& key,
                                             const ByteBuffer& message) {
  // Sanity check
  if (key.size() == 0 || message.size() == 0) {
    Util::LogError("Hmac key or message too short");
    return nullptr;
  }

  // Overflow check because of HMAC function signature
  if (key.size() > INT_MAX) {
    Util::LogError("Key too big");
    return nullptr;
  }

  // Create an empty digest with enough space. OpenSSL will tell us how much it
  // actually wrote.
  uint8_t md_value[EVP_MAX_MD_SIZE];
  unsigned md_length;

  if (HMAC(EVP_sha256(), key.ImmutableUInt8(), static_cast<int>(key.size()),
           message.ImmutableUInt8(), message.size(), md_value, &md_length) ==
      NULL) {
    Util::LogErrorAndAbort("HMAC failed");
    return nullptr;
  }

  if (md_length != kSha256DigestSize) {
    Util::LogErrorAndAbort("HMAC returned an unexpected result");
    return nullptr;
  }

  return unique_ptr<ByteBuffer>(new ByteBuffer(md_value, kSha256DigestSize));
}

unique_ptr<ByteBuffer> CryptoOps::Aes256CBCDecrypt(
    const CryptoOps::SecretKey& decryptionKey,
    const ByteBuffer& iv,
    const ByteBuffer& ciphertext) {
  if (decryptionKey.type() != SECRET) {
    Util::LogError("AES Decrypt only works with SECRET keys");
    return nullptr;
  }
  if (decryptionKey.algorithm() != AES_256_KEY) {
    Util::LogError("AES Decrypt only works with AES 256 keys");
    return nullptr;
  }
  if (decryptionKey.data().size() != kAesKeySize) {
    Util::LogError("AES 256 key should be 32 bytes long");
    return nullptr;
  }
  if (iv.size() != AES_BLOCK_SIZE) {
    Util::LogError("Incorrect IV size");
    return nullptr;
  }
  if (ciphertext.size() > INT_MAX) {
    Util::LogError("Ciphertext too large to decrypt");
    return nullptr;
  }

  ByteBuffer plaintext(ciphertext.size());  // Allocate space for plaintext
  int return_code;                          // OpenSSL error checking

  // Initialize the cipher context
  std::unique_ptr<EVP_CIPHER_CTX, void (*)(EVP_CIPHER_CTX *)>
      scoped_ctx(EVP_CIPHER_CTX_new(), EVP_CIPHER_CTX_free);
  EVP_CIPHER_CTX* ctx = scoped_ctx.get();
  if (ctx == nullptr) {
    Util::LogErrorAndAbort("Could not allocate AES cipher context");
    return nullptr;
  }
  return_code = EVP_DecryptInit_ex(ctx, EVP_aes_256_cbc(), NULL,
                                   decryptionKey.data().ImmutableUInt8(),
                                   iv.ImmutableUInt8());
  if (return_code != 1) {
    Util::LogErrorAndAbort("Could not initialize AES decryption");
    return nullptr;
  }

  // Decrypt data
  int plaintext_bytes_written = 0;
  return_code =
      EVP_DecryptUpdate(ctx,
                        plaintext.MutableUInt8(),
                        &plaintext_bytes_written,
                        ciphertext.ImmutableUInt8(),
                        static_cast<int>(ciphertext.size()));
  if (return_code != 1) {
    Util::LogError("Could not decrypt data using AES");
    return nullptr;
  }

  // Handle the last block and padding
  int last_block_bytes_written = 0;
  return_code = EVP_DecryptFinal_ex(
      ctx, plaintext.MutableUInt8() + plaintext_bytes_written,
      &last_block_bytes_written);
  if (return_code != 1) {
    Util::LogError("Could not decrypt data using AES; likely a padding error");
    return nullptr;
  }

  // Check that the correct number of bytes were written
  if (plaintext_bytes_written > INT_MAX - last_block_bytes_written ||
      static_cast<size_t>(plaintext_bytes_written + last_block_bytes_written) >
          plaintext.size()) {
    Util::LogErrorAndAbort("Unexpected amount of bytes written by AES Encrypt");
    return nullptr;
  }

  return unique_ptr<ByteBuffer>(
      new ByteBuffer(plaintext.ImmutableUInt8(),
                     plaintext_bytes_written + last_block_bytes_written));
}

unique_ptr<ByteBuffer> CryptoOps::Aes256CBCEncrypt(
    const CryptoOps::SecretKey& encryptionKey,
    const ByteBuffer& iv,
    const ByteBuffer& plaintext) {
  if (encryptionKey.type() != SECRET) {
    Util::LogError("AES Encrypt only works with SECRET keys");
    return nullptr;
  }
  if (encryptionKey.algorithm() != AES_256_KEY) {
    Util::LogError("AES Encrypt only works with AES 256 keys");
    return nullptr;
  }
  if (encryptionKey.data().size() != kAesKeySize) {
    Util::LogError("AES 256 key should be 32 bytes long");
    return nullptr;
  }
  if (iv.size() != AES_BLOCK_SIZE) {
    Util::LogError("Incorrect IV size");
    return nullptr;
  }

  // Calculate PKCS #5 padding size
  size_t padding_size = AES_BLOCK_SIZE - (plaintext.size() % AES_BLOCK_SIZE);

  // Paranoid bounds check to make sure that the static_cast<int> for
  // EVP_EncryptUpdate doesn't cause an overflow
  if (plaintext.size() > INT_MAX) {
    Util::LogError("Plaintext is too large");
    return nullptr;
  }

  // Allocate space for the ciphertext
  unique_ptr<ByteBuffer> ciphertext(
      new ByteBuffer(plaintext.size() + padding_size));

  // Initialize the cipher context
  std::unique_ptr<EVP_CIPHER_CTX, void (*)(EVP_CIPHER_CTX *)>
      scoped_ctx(EVP_CIPHER_CTX_new(), EVP_CIPHER_CTX_free);
  EVP_CIPHER_CTX* ctx = scoped_ctx.get();
  if (ctx == nullptr) {
    Util::LogErrorAndAbort("Could not allocate AES cipher context");
    return nullptr;
  }
  int return_code = EVP_EncryptInit_ex(ctx, EVP_aes_256_cbc(), NULL,
                                       encryptionKey.data().ImmutableUInt8(),
                                       iv.ImmutableUInt8());
  if (return_code != 1) {
    Util::LogErrorAndAbort("Could not initialize encryption");
    return nullptr;
  }

  // Encrypt data
  int full_block_bytes_written = 0;
  int last_block_bytes_written = 0;
  return_code = EVP_EncryptUpdate(
      ctx, ciphertext->MutableUInt8(), &full_block_bytes_written,
      plaintext.ImmutableUInt8(), static_cast<int>(plaintext.size()));
  if (return_code != 1) {
    Util::LogErrorAndAbort("Error during encryption");
    return nullptr;
  }

  return_code = EVP_EncryptFinal_ex(
      ctx, ciphertext->MutableUInt8() + full_block_bytes_written,
      &last_block_bytes_written);
  if (return_code != 1) {
    Util::LogErrorAndAbort("Error during encryption");
    return nullptr;
  }

  // Check that the correct number of bytes were written
  if (full_block_bytes_written > INT_MAX - last_block_bytes_written ||
      static_cast<size_t>(full_block_bytes_written +
                          last_block_bytes_written) != ciphertext->size()) {
    Util::LogErrorAndAbort("Unexpected amount of bytes written by AES Encrypt");
    return nullptr;
  }

  return ciphertext;
}

unique_ptr<string> CryptoOps::GenerateIv(EncType encType) {
  unique_ptr<string> iv = nullptr;

  switch (encType) {
    case NONE:
      iv = nullptr;
      break;

    case AES_256_CBC:
      unsigned char iv_bytes[AES_BLOCK_SIZE];

      if (RAND_bytes(iv_bytes, AES_BLOCK_SIZE) != 1) {
        Util::LogError("OpenSSL could not generate random bytes");
        return nullptr;
      }
      iv = Util::MakeUniquePtrString(iv_bytes, AES_BLOCK_SIZE);
      break;

    default:
      // Should never happen
      Util::LogErrorAndAbort("Invalid encryption type");
      return nullptr;
  }

  return iv;
}

unique_ptr<ByteBuffer> CryptoOps::EcdsaP256Sha256Sign(
    const PrivateKey &private_key, const ByteBuffer &data) {
  if (private_key.algorithm() != KeyAlgorithm::ECDSA_KEY ||
      private_key.data().size() == 0 || data.size() == 0) {
    return nullptr;
  }

  EvpKeyPtr extracted_evpkey = CreateEvpPrivateKey(private_key);
  if (extracted_evpkey == nullptr) {
    return nullptr;
  }

  // Create an empty signature buffer with enough space. OpenSSL will tell us
  // how much it actually wrote.
  int maximum_size = EVP_PKEY_size(extracted_evpkey.get());
  if (maximum_size <= 0) {
    Util::LogErrorAndAbort("unexpected max size when computing evp key size");
    return nullptr;
  }

  ByteBuffer signature_buffer(maximum_size);
  size_t signature_length = signature_buffer.size();

  // Create a new message digest context and initialize it.
  unique_ptr<EVP_MD_CTX, void (*)(EVP_MD_CTX *)> mdctx(EVP_MD_CTX_new(),
                                                       EVP_MD_CTX_free);
  int return_code = EVP_DigestSignInit(mdctx.get(), nullptr, EVP_sha256(),
                                       nullptr, extracted_evpkey.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not initialize digest context");
    return nullptr;
  }

  // On public signature types, we add salt.
  return_code = EVP_DigestSignUpdate(mdctx.get(), kSalt, sizeof(kSalt));
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not add salt");
    return nullptr;
  }

  // Set the message to sign.
  return_code =
      EVP_DigestSignUpdate(mdctx.get(), data.ImmutableUChar(), data.size());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not set data to sign");
    return nullptr;
  }

  // Compute the signature.
  return_code = EVP_DigestSignFinal(
      mdctx.get(), signature_buffer.MutableUChar(), &signature_length);
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not execute signature");
    return nullptr;
  }

  // Verify we got a signature.
  if (signature_length <= 0) {
    Util::LogErrorAndAbort("signature length is too small");
    return nullptr;
  }

  // Verify the signature fits in the buffer.
  if (signature_length > maximum_size) {
    Util::LogErrorAndAbort("signature buffer is too small");
    return nullptr;
  }

  return unique_ptr<ByteBuffer>(
      new ByteBuffer(signature_buffer.ImmutableUChar(), signature_length));
}

bool CryptoOps::EcdsaP256Sha256Verify(const PublicKey& public_key,
                                      const ByteBuffer& signature,
                                      const ByteBuffer& data) {
  if (public_key.algorithm() != KeyAlgorithm::ECDSA_KEY ||
      public_key.data().size() == 0 || data.size() == 0) {
    return false;
  }

  EvpKeyPtr extracted_public_evpkey = CreateEvpPublicKey(public_key);
  if (extracted_public_evpkey == nullptr) {
    return false;
  }

  // Create a verify digest context and initialize it
  unique_ptr<EVP_MD_CTX, void (*)(EVP_MD_CTX *)> verifyctx(EVP_MD_CTX_new(),
                                                           EVP_MD_CTX_free);
  int return_code =
      EVP_DigestVerifyInit(verifyctx.get(), nullptr, EVP_sha256(), nullptr,
                           extracted_public_evpkey.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not initialize verify digest context");
    return false;
  }

  // On public signatures, we added salt to the signature.
  return_code = EVP_DigestVerifyUpdate(verifyctx.get(), kSalt, sizeof(kSalt));
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not add salt to verify");
    return false;
  }

  // Set the message to verify
  return_code = EVP_DigestVerifyUpdate(verifyctx.get(), data.ImmutableUChar(),
                                       data.size());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not set data to verify");
    return false;
  }

  // Verify the signature
  return_code = EVP_DigestVerifyFinal(
      verifyctx.get(), signature.ImmutableUChar(), signature.size());

  if (return_code != 1) {
    Util::LogError("could not verify signature");
    return false;
  }

  return true;
}

unique_ptr<ByteBuffer> CryptoOps::EcdhKeyAgreement(
    const PrivateKey& private_key,
    const PublicKey& peer_key) {
  // Shared return code.
  int return_code;

  // Create the private key from the data.
  EvpKeyPtr evp_private_key = CreateEvpPrivateKey(private_key);
  if (evp_private_key == nullptr) {
    return nullptr;
  }

  // Create the public key from the data.
  EvpKeyPtr evp_peer_key = CreateEvpPublicKey(peer_key);
  if (evp_peer_key == nullptr) {
    return nullptr;
  }

  // Create the context for the shared secret derivation.
  unique_ptr<EVP_PKEY_CTX, void (*)(EVP_PKEY_CTX *)> ctx(
      EVP_PKEY_CTX_new(evp_private_key.get(), NULL), EVP_PKEY_CTX_free);
  if (ctx == nullptr) {
    Util::LogErrorAndAbort("could not create ECDH context");
    return nullptr;
  }

  // Initialize.
  return_code = EVP_PKEY_derive_init(ctx.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not initialize context");
    return nullptr;
  }

  // Provide the peer public key.
  return_code = EVP_PKEY_derive_set_peer(ctx.get(), evp_peer_key.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not set peer");
    return nullptr;
  }

  // Determine buffer length for shared secret.
  size_t secret_len;
  return_code = EVP_PKEY_derive(ctx.get(), NULL, &secret_len);
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not determine secret length");
    return nullptr;
  }

  // Create the buffer.
  auto secret = unique_ptr<ByteBuffer>(new ByteBuffer(secret_len));

  // Derive the shared secret.
  return_code = EVP_PKEY_derive(ctx.get(), secret->MutableUChar(),
                                &secret_len);
  if (return_code != 1 || secret_len != secret->size()) {
    Util::LogErrorAndAbort("could not derive secret");
    return nullptr;
  }

  return secret;
}

unique_ptr<ByteBuffer> CryptoOps::Rsa2048Sha256Sign(
    const PrivateKey &private_key, const ByteBuffer &data) {
  if (private_key.algorithm() != KeyAlgorithm::RSA_KEY ||
      private_key.data().size() == 0 || data.size() == 0) {
    return nullptr;
  }

  int return_code;
  const uint8_t *key_data = private_key.data().ImmutableUInt8();

  // Import bytes into an RSA key structure
  unique_ptr<RSA, void (*)(RSA *)> rsa_key(
      d2i_RSAPrivateKey(NULL, &key_data, private_key.data().size()), RSA_free);

  if (rsa_key.get() == NULL) {
    Util::LogError("could not re-create rsa key");
    return nullptr;
  }

  // Create an EVP key structure
  EvpKeyPtr rsa_pkey(EVP_PKEY_new());
  if (rsa_pkey.get() == NULL) {
    Util::LogErrorAndAbort("Could not create new EVP_PKEY structure");
    return nullptr;
  }

  // Place the RSA key into the EVP structure
  return_code = EVP_PKEY_set1_RSA(rsa_pkey.get(), rsa_key.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("Could not set rsa key into pkey");
    return nullptr;
  }

  // Set up a new signature context
  unique_ptr<EVP_MD_CTX, void (*)(EVP_MD_CTX *)> ctx(EVP_MD_CTX_new(),
                                                     EVP_MD_CTX_free);
  if (ctx.get() == NULL) {
    Util::LogErrorAndAbort("Could not create signature context");
    return nullptr;
  }

  // Initialize it
  return_code = EVP_DigestSignInit(ctx.get(), nullptr, EVP_sha256(), nullptr,
                                   rsa_pkey.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("Could not initialize context");
    return nullptr;
  }

  // On public signature types, we add salt.
  return_code = EVP_DigestSignUpdate(ctx.get(), kSalt, sizeof(kSalt));
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not add salt");
    return nullptr;
  }

  // Set the message to sign
  return_code =
      EVP_DigestSignUpdate(ctx.get(), data.ImmutableUChar(), data.size());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not set data to sign");
    return nullptr;
  }

  // Make space for the signature
  size_t signature_size = EVP_PKEY_size(rsa_pkey.get());
  if (signature_size == 0) {
    Util::LogErrorAndAbort("Invalid signature size");
    return nullptr;
  }
  ByteBuffer signature(signature_size);

  // Perform the signature
  return_code =
      EVP_DigestSignFinal(ctx.get(), signature.MutableUInt8(), &signature_size);
  if (return_code != 1) {
    Util::LogErrorAndAbort("Error while signing");
    return nullptr;
  }

  return unique_ptr<ByteBuffer>(
      new ByteBuffer(signature.ImmutableUInt8(), signature_size));
}

bool CryptoOps::Rsa2048Sha256Verify(const PublicKey& public_key,
                                    const ByteBuffer& signature,
                                    const ByteBuffer& data) {
  if (public_key.algorithm() != KeyAlgorithm::RSA_KEY ||
      public_key.data().size() == 0 || data.size() == 0) {
    return false;
  }

  int return_code;
  const uint8_t *key_data = public_key.data().ImmutableUInt8();

  // Import bytes into an RSA key structure
  unique_ptr<RSA, void (*)(RSA *)> rsa_public_key(
      d2i_RSA_PUBKEY(NULL, &key_data, public_key.data().size()), RSA_free);
  if (rsa_public_key.get() == NULL) {
    Util::LogErrorAndAbort(
        "could not import bytes into RSA key object for public key");
    return false;
  }

  // Create an EVP key structure
  EvpKeyPtr rsa_public_pkey(EVP_PKEY_new());
  if (rsa_public_pkey.get() == NULL) {
    Util::LogErrorAndAbort(
        "could not create EVP structure for storing public RSA key");
    return false;
  }

  // Place the RSA key into the EVP structure
  return_code = EVP_PKEY_set1_RSA(rsa_public_pkey.get(), rsa_public_key.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not set RSA public key into EVP structure");
    return false;
  }

  // Create verification context
  unique_ptr<EVP_MD_CTX, void (*)(EVP_MD_CTX *)> verifyctx(EVP_MD_CTX_new(),
                                                           EVP_MD_CTX_free);

  // Initialize it
  return_code = EVP_DigestVerifyInit(verifyctx.get(), nullptr, EVP_sha256(),
                                     nullptr, rsa_public_pkey.get());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not initialize verify digest context");
    return false;
  }

  // On public signatures, we added salt to the signature.
  return_code = EVP_DigestVerifyUpdate(verifyctx.get(), kSalt, sizeof(kSalt));
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not add salt to verify");
    return false;
  }

  // Set the message to verify
  return_code = EVP_DigestVerifyUpdate(verifyctx.get(), data.ImmutableUChar(),
                                       data.size());
  if (return_code != 1) {
    Util::LogErrorAndAbort("could not set data to verify");
    return false;
  }

  // Verify the signature
  return_code = EVP_DigestVerifyFinal(
      verifyctx.get(), signature.ImmutableUChar(), signature.size());

  if (return_code != 1) {
    Util::LogError("could not verify signature");
    return false;
  }

  return true;
}

unique_ptr<CryptoOps::PublicKey> CryptoOps::ImportEcP256Key(const string& x,
                                                            const string& y) {
  int return_code;  // Used to check all of OpenSSL's return codes

  if (!IsValidEcP256CoordinateEncoding(x) ||
      !IsValidEcP256CoordinateEncoding(y)) {
    Util::LogError("error, invalid key format");
    return nullptr;
  }

  // Generate new ec key structure for the P256 curve
  unique_ptr<EC_KEY, void (*)(EC_KEY *)> eckey(
      EC_KEY_new_by_curve_name(NID_X9_62_prime256v1),
      EC_KEY_free);
  if (eckey.get() == NULL) {
    Util::LogErrorAndAbort("error, couldn't create new key structure");
    return nullptr;
  }

  auto x_data = reinterpret_cast<const unsigned char *>(x.data());
  auto y_data = reinterpret_cast<const unsigned char *>(y.data());

  unique_ptr<BIGNUM, void (*)(BIGNUM *)> x_value(
      BN_bin2bn(x_data, static_cast<unsigned int>(x.length()), NULL),
      BN_free);
  unique_ptr<BIGNUM, void (*)(BIGNUM *)> y_value(
      BN_bin2bn(y_data, static_cast<unsigned int>(y.length()), NULL),
      BN_free);

  return_code = EC_KEY_set_public_key_affine_coordinates(eckey.get(),
                                                         x_value.get(),
                                                         y_value.get());

  if (return_code != 1) {
    Util::LogError("error, couldn't set public key");
    return nullptr;
  }

  unique_ptr<ByteBuffer> public_key_bytes = PublicKeyToBytes(eckey.get());
  if (public_key_bytes == nullptr) {
    Util::LogErrorAndAbort("error, couldn't unwrap public key");
    return nullptr;
  }

  return unique_ptr<PublicKey>(
      new PublicKey(*public_key_bytes, KeyAlgorithm::ECDSA_KEY));
}

bool CryptoOps::ExportEcP256Key(const CryptoOps::PublicKey& key,
                                string *x,
                                string *y) {
  int return_code;  // Used to check all of OpenSSL's return codes

  if (key.algorithm() != ECDSA_KEY) {
    Util::LogError("error, invalid key type");
    return false;
  }

  const unsigned char *pub_key_buffer = key.data().ImmutableUChar();
  unique_ptr<EC_KEY, void (*)(EC_KEY *)> eckey(
      d2i_EC_PUBKEY(NULL, &pub_key_buffer, key.data().size()),
      EC_KEY_free);

  if (eckey.get() == NULL) {
    Util::LogError("error, couldn't import new key structure");
    return false;
  }

  const EC_POINT *ecpoint = EC_KEY_get0_public_key(eckey.get());
  const EC_GROUP *ecgroup = EC_KEY_get0_group(eckey.get());

  if (ecpoint == NULL || ecgroup == NULL) {
    Util::LogErrorAndAbort("error, couldn't get the public key details");
    return false;
  }

  unique_ptr<BIGNUM, void (*)(BIGNUM *)> x_value(
      BN_new(),
      BN_free);
  unique_ptr<BIGNUM, void (*)(BIGNUM *)> y_value(
      BN_new(),
      BN_free);

  return_code = EC_POINT_get_affine_coordinates_GFp(ecgroup,
                                                    ecpoint,
                                                    x_value.get(),
                                                    y_value.get(),
                                                    NULL);

  if (return_code != 1) {
    Util::LogErrorAndAbort("error, could not capture coordinates of key");
    return false;
  }

  ByteBuffer x_bytes(BN_num_bytes(x_value.get()));
  ByteBuffer y_bytes(BN_num_bytes(y_value.get()));
  BN_bn2bin(x_value.get(), x_bytes.MutableUChar());
  BN_bn2bin(y_value.get(), y_bytes.MutableUChar());

  // Make sure the byte format is in two's complement by adding a byte of 0's.
  x_bytes.Prepend(string(1, '\x00'));
  y_bytes.Prepend(string(1, '\x00'));

  if (x != NULL) {
    x->assign(x_bytes.String());
  }
  if (y != NULL) {
    y->assign(y_bytes.String());
  }
  return true;
}

unique_ptr<CryptoOps::PublicKey> CryptoOps::ImportRsa2048Key(const string& n,
                                                             int32_t e) {
  if (!IsValidRsa2048ModulusEncoding(n)) {
    Util::LogError("error, invalid key encoding");
    return nullptr;
  }

  unique_ptr<RSA, void (*)(RSA *)> rsa(RSA_new(), RSA_free);
  if (rsa.get() == NULL) {
    Util::LogErrorAndAbort("error, couldn't create new key structure");
    return nullptr;
  }

  auto n_data = reinterpret_cast<const unsigned char *>(n.data());

  string e_str = Int32BytesToString(e);
  auto e_data = reinterpret_cast<const unsigned char *>(e_str.data());

  unique_ptr<BIGNUM, void (*)(BIGNUM *)> n_value(
      BN_bin2bn(n_data, static_cast<unsigned int>(n.length()), NULL),
      BN_free);
  unique_ptr<BIGNUM, void (*)(BIGNUM *)> e_value(
      BN_bin2bn(e_data, static_cast<unsigned int>(e_str.length()), NULL),
      BN_free);

  BIGNUM* rsa_n = n_value.release();
  BIGNUM* rsa_e = e_value.release();
  int result = RSA_set0_key(rsa.get(), rsa_n, rsa_e, nullptr);
  if (!result) {
    Util::LogErrorAndAbort("unable to set public key components");
    return nullptr;
  }

  unsigned char *public_key_buffer = NULL;
  int public_key_len = i2d_RSA_PUBKEY(rsa.get(), &public_key_buffer);

  if (public_key_len <= 0) {
    Util::LogErrorAndAbort("invalid public key length");
    return nullptr;
  }

  if (public_key_buffer == NULL) {
    Util::LogErrorAndAbort("couldn't allocate public key buffer");
    return nullptr;
  }

  ByteBuffer public_key_bytes(public_key_buffer, public_key_len);
  OPENSSL_free(public_key_buffer);

  return unique_ptr<PublicKey>(
      new PublicKey(public_key_bytes, KeyAlgorithm::RSA_KEY));
}

bool CryptoOps::ExportRsa2048Key(const CryptoOps::PublicKey& key,
                                 string *n,
                                 int32_t *e) {
  if (key.algorithm() != RSA_KEY) {
    Util::LogError("error, invalid key type");
    return false;
  }

  const unsigned char *pub_key_buffer = key.data().ImmutableUChar();
  unique_ptr<RSA, void (*)(RSA *)> rsa(
      d2i_RSA_PUBKEY(NULL, &pub_key_buffer, key.data().size()),
      RSA_free);

  if (rsa.get() == NULL) {
    Util::LogError("error, couldn't import new key structure");
    return false;
  }

  const BIGNUM* rsa_n;
  const BIGNUM* rsa_e;
  RSA_get0_key(rsa.get(), &rsa_n, &rsa_e, nullptr);

  if (rsa_n == NULL || rsa_e == NULL) {
    Util::LogErrorAndAbort("error, couldn't get the public key details");
    return false;
  }

  if (static_cast<size_t>(BN_num_bytes(rsa_e)) > sizeof(*e)) {
    Util::LogErrorAndAbort("error, invalid exponent size");
    return false;
  }

  ByteBuffer n_bytes(BN_num_bytes(rsa_n));
  ByteBuffer e_bytes(BN_num_bytes(rsa_e));

  BN_bn2bin(rsa_n, n_bytes.MutableUChar());
  BN_bn2bin(rsa_e, e_bytes.MutableUChar());

  // Make sure the modulus bytes are in two's complement by ensuring the
  // leading bit is not 1.
  n_bytes.Prepend(string(1, '\x00'));

  int32_t e_int32 = 0;
  if (!StringToInt32Bytes(e_bytes.String(), &e_int32)) {
    Util::LogErrorAndAbort("error, invalid exponent bytes");
    return false;
  }

  if (n != NULL) {
    n->assign(n_bytes.String());
  }

  if (e != NULL) {
    *e = e_int32;
  }
  return true;
}

unique_ptr<CryptoOps::SecretKey> CryptoOps::GenerateAes256SecretKey() {
  size_t key_size_in_bytes = EVP_CIPHER_key_length(EVP_aes_256_cbc());
  ByteBuffer key(key_size_in_bytes);

  if (RAND_bytes(key.MutableUInt8(),
                 static_cast<unsigned int>(key_size_in_bytes)) != 1) {
    Util::LogError(
        "Could not get enough cryptographically secure bytes for key");
    return nullptr;
  }

  return unique_ptr<SecretKey>(
      new SecretKey(key.String(), KeyAlgorithm::AES_256_KEY));
}

unique_ptr<CryptoOps::KeyPair> CryptoOps::GenerateEcP256KeyPair() {
  int return_code;  // Used to check all of OpenSSL's return codes

  // Create a new EVP key structure for the EC key
  EvpKeyPtr ecpkey(EVP_PKEY_new());
  if (ecpkey.get() == NULL) {
    Util::LogErrorAndAbort("Could not create EVP PKEY structure");
    return nullptr;
  }

  // Generate new ec key structure for the P256 curve
  EC_KEY *eckey = EC_KEY_new_by_curve_name(NID_X9_62_prime256v1);
  if (eckey == NULL) {
    Util::LogErrorAndAbort("error, couldn't create new key structure");
    return nullptr;
  }

  // Generate a new key and stick it in the structure
  return_code = EC_KEY_generate_key(eckey);
  if (return_code != 1) {
    Util::LogErrorAndAbort("Couldn't generate new key");
    return nullptr;
  }

  // Assign the EC Key to a EVP PKEY structure
  return_code = EVP_PKEY_assign_EC_KEY(ecpkey.get(), eckey);
  if (return_code != 1) {
    Util::LogErrorAndAbort("Could not assign EC key to EVP structure");
    return nullptr;
  }

  // Convert public key to bytes
  unique_ptr<ByteBuffer> public_key_bytes = PublicKeyToBytes(eckey);
  if (public_key_bytes == nullptr) {
    return nullptr;
  }

  // Convert private key to bytes
#if defined(OPENSSL_IS_BORINGSSL)
  // Use EVP_marshal_private_key in BoringSSL to avoid a dependency on the
  // legacy ASN.1 stack and the large objects table.
  bssl::ScopedCBB cbb;
  if (!CBB_init(cbb.get(), /*initial_capacity=*/128) ||
      !EVP_marshal_private_key(cbb.get(), ecpkey.get())) {
    Util::LogError("could not serialize private key");
    return nullptr;
  }
  ByteBuffer private_key_bytes(CBB_data(cbb.get()), CBB_len(cbb.get()));
#else
  unique_ptr<PKCS8_PRIV_KEY_INFO, void (*)(PKCS8_PRIV_KEY_INFO *)> pkcs8(
      EVP_PKEY2PKCS8(ecpkey.get()), PKCS8_PRIV_KEY_INFO_free);

  if (pkcs8.get() == NULL) {
    Util::LogError("could not convert evp key into pkcs8 key");
    return nullptr;
  }
  int priv_key_len = i2d_PKCS8_PRIV_KEY_INFO(pkcs8.get(), NULL);
  if (priv_key_len <= 0) {
    Util::LogErrorAndAbort("unexpected private key length returned");
    return nullptr;
  }
  unsigned char *priv_key_buffer = NULL;
  return_code = i2d_PKCS8_PRIV_KEY_INFO(pkcs8.get(), &priv_key_buffer);
  if (return_code != priv_key_len) {
    Util::LogErrorAndAbort("wrote an unexpected number of private key bytes");
    return nullptr;
  }
  ByteBuffer private_key_bytes(priv_key_buffer, priv_key_len);
  OPENSSL_free(priv_key_buffer);
#endif

  return unique_ptr<KeyPair>(
      new KeyPair(unique_ptr<PublicKey>(new PublicKey(
                      *public_key_bytes, KeyAlgorithm::ECDSA_KEY)),
                  unique_ptr<PrivateKey>(new PrivateKey(
                      private_key_bytes, KeyAlgorithm::ECDSA_KEY))));
}

unique_ptr<CryptoOps::KeyPair> CryptoOps::GenerateRsa2048KeyPair() {
  // Create the exponent
  unique_ptr<BIGNUM, void (*)(BIGNUM *)> exp(BN_new(), BN_clear_free);
  if (exp.get() == NULL) {
    Util::LogErrorAndAbort("Could not create new bignum");
    return nullptr;
  }

  int return_code = BN_set_word(exp.get(), RSA_F4);
  if (return_code != 1) {
    Util::LogErrorAndAbort("Error setting exponent");
    return nullptr;
  }

  // Generate a key
  unique_ptr<RSA, void (*)(RSA *)> rsa(RSA_new(), RSA_free);
  if (rsa.get() == NULL) {
    Util::LogErrorAndAbort("Error creating RSA structure");
    return nullptr;
  }

  return_code = RSA_generate_key_ex(rsa.get(), 2048, exp.get(), NULL);
  if (return_code != 1) {
    Util::LogErrorAndAbort("Error generating RSA key");
    return nullptr;
  }

  // Store private key
  unsigned char *private_key_buffer = NULL;
  int private_key_len = i2d_RSAPrivateKey(rsa.get(), &private_key_buffer);
  if (private_key_buffer == NULL) {
    Util::LogErrorAndAbort("couldn't allocate private key buffer");
    return nullptr;
  }

  if (private_key_len <= 0) {
    Util::LogErrorAndAbort(
        "invalid private key length encountered during key generation");
    return nullptr;
  }
  ByteBuffer private_key(private_key_buffer, private_key_len);
  OPENSSL_free(private_key_buffer);

  // Store public key
  unsigned char *public_key_buffer = NULL;
  int public_key_len = i2d_RSA_PUBKEY(rsa.get(), &public_key_buffer);
  if (public_key_buffer == NULL) {
    Util::LogErrorAndAbort("couldn't allocate public key buffer");
    return nullptr;
  }

  if (public_key_len <= 0) {
    Util::LogErrorAndAbort("invalid public key length");
    return nullptr;
  }
  ByteBuffer public_key(public_key_buffer, public_key_len);
  OPENSSL_free(public_key_buffer);

  return unique_ptr<KeyPair>(new KeyPair(
      unique_ptr<PublicKey>(new PublicKey(public_key, KeyAlgorithm::RSA_KEY)),
      unique_ptr<PrivateKey>(
          new PrivateKey(private_key, KeyAlgorithm::RSA_KEY))));
}

unique_ptr<ByteBuffer> CryptoOps::SecureRandom(size_t length) {
  if (length < 1) {
    Util::LogError("SecureRandom: length must be greater than zero.");
    return nullptr;
  }

  unique_ptr<ByteBuffer> random_bytes(new ByteBuffer(length));

  if (RAND_bytes(random_bytes->MutableUInt8(),
                 static_cast<unsigned int>(length)) != 1) {
    Util::LogError("SecureRandom: Failed to generate random bytes.");
    return nullptr;
  }

  return random_bytes;
}

}  // namespace securemessage
