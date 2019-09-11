/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#ifndef IROHA_PEER_TLS_CERTIFICATES_PROVIDER_HPP
#define IROHA_PEER_TLS_CERTIFICATES_PROVIDER_HPP

#include <memory>
#include <string>

#include "common/result.hpp"
#include "interfaces/common_objects/types.hpp"

namespace shared_model {
  namespace interface {
    class Peer;
  }
}  // namespace shared_model

namespace iroha {
  namespace network {

    class PeerTlsCertificatesProvider {
     public:
      virtual ~PeerTlsCertificatesProvider() = default;

      virtual iroha::expected::Result<
          shared_model::interface::types::TLSCertificateType,
          std::string>
      get(const shared_model::interface::Peer &peer) const = 0;

      virtual iroha::expected::Result<
          shared_model::interface::types::TLSCertificateType,
          std::string>
      get(const shared_model::interface::types::PubkeyType &public_key)
          const = 0;
    };

  }  // namespace network
}  // namespace iroha

#endif
