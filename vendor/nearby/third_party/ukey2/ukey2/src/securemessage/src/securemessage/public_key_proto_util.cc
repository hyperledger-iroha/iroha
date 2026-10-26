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

#include "src/securemessage/include/securemessage/public_key_proto_util.h"

using std::unique_ptr;

namespace securemessage {

unique_ptr<GenericPublicKey> PublicKeyProtoUtil::EncodePublicKey(
    const CryptoOps::PublicKey& key) {
  switch (key.algorithm()) {
    case CryptoOps::KeyAlgorithm::ECDSA_KEY: {
      unique_ptr<EcP256PublicKey> ec_key = EncodeEcPublicKey(key);
      if (ec_key == nullptr) {
        return nullptr;
      }

      unique_ptr<GenericPublicKey> result(new GenericPublicKey());
      result->set_type(PublicKeyType::EC_P256);
      result->mutable_ec_p256_public_key()->CopyFrom(*ec_key);
      return result;
    }
    case CryptoOps::KeyAlgorithm::RSA_KEY: {
      unique_ptr<SimpleRsaPublicKey> rsa_key = EncodeRsaPublicKey(key);
      if (rsa_key == nullptr) {
        return nullptr;
      }

      unique_ptr<GenericPublicKey> result(new GenericPublicKey());
      result->set_type(PublicKeyType::RSA2048);
      result->mutable_rsa2048_public_key()->CopyFrom(*rsa_key);
      return result;
    }
    default:
      return nullptr;
  }
}

unique_ptr<EcP256PublicKey> PublicKeyProtoUtil::EncodeEcPublicKey(
    const CryptoOps::PublicKey& key) {
  std::string x;
  std::string y;
  if (!CryptoOps::ExportEcP256Key(key, &x, &y)) {
    return nullptr;
  }

  auto result = unique_ptr<EcP256PublicKey>(new EcP256PublicKey());
  result->set_x(x);
  result->set_y(y);
  return result;
}

unique_ptr<SimpleRsaPublicKey> PublicKeyProtoUtil::EncodeRsaPublicKey(
    const CryptoOps::PublicKey& key) {
  std::string n;
  int32_t e;
  if (!CryptoOps::ExportRsa2048Key(key, &n, &e)) {
    return nullptr;
  }

  auto result = unique_ptr<SimpleRsaPublicKey>(new SimpleRsaPublicKey());
  result->set_n(n);
  result->set_e(e);
  return result;
}

unique_ptr<CryptoOps::PublicKey> PublicKeyProtoUtil::ParsePublicKey(
    const GenericPublicKey& key) {
  switch (key.type()) {
    case EC_P256: {
      if (!key.has_ec_p256_public_key()) {
        return nullptr;
      }
      return ParseEcPublicKey(key.ec_p256_public_key());
    }
    case RSA2048: {
      if (!key.has_rsa2048_public_key()) {
        return nullptr;
      }
      return ParseRsaPublicKey(key.rsa2048_public_key());
    }
    default:
      return nullptr;
  }
}

unique_ptr<CryptoOps::PublicKey> PublicKeyProtoUtil::ParseEcPublicKey(
    const EcP256PublicKey& key) {
  return CryptoOps::ImportEcP256Key(key.x(), key.y());
}

unique_ptr<CryptoOps::PublicKey> PublicKeyProtoUtil::ParseRsaPublicKey(
    const SimpleRsaPublicKey& key) {
  return CryptoOps::ImportRsa2048Key(key.n(), key.e());
}

}  // namespace securemessage
