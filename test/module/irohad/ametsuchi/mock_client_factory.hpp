/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#ifndef IROHA_MOCK_CLIENT_FACTORY_HPP
#define IROHA_MOCK_CLIENT_FACTORY_HPP

#include "network/impl/client_factory.hpp"

#include <gmock/gmock.h>

namespace iroha {
  namespace network {

    template <typename Service>
    class MockClientFactory : public ClientFactory<Service> {
     public:
      MOCK_CONST_METHOD1_T(createClient,
                           std::unique_ptr<typename Service::StubInterface>(
                               const shared_model::interface::Peer &));
    };

  }  // namespace network
}  // namespace iroha

#endif
