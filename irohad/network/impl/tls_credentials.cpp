/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "network/impl/tls_credentials.hpp"

#include <fstream>
#include <sstream>

#include "common/bind.hpp"

using namespace iroha::expected;
using namespace iroha::network;

using iroha::operator|;

Result<std::unique_ptr<TlsCredentials>, std::string> TlsCredentials::load(
    const std::string &path) {
  static const auto read_file = [](const std::string &path) {
    std::ifstream certificate_file(path);
    std::stringstream ss;
    ss << certificate_file.rdbuf();
    return ss.str();
  };
  auto creds = std::make_unique<TlsCredentials>();
  try {
    creds->private_key = read_file(path + ".key");
    creds->certificate = read_file(path + ".crt");
  } catch (std::exception e) {
    return makeError(e.what());
  }
  return makeValue(std::move(creds));
}
