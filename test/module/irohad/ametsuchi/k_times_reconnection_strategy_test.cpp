/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "ametsuchi/impl/k_times_reconnection_strategy.hpp"

#include <gtest/gtest.h>

using namespace iroha::ametsuchi;

/**
 * @given initialized strategy with k limit
 * @when  canReconnect invokes k + 1 times
 * @then  k times it returns true
 *        @and last returns false
 */
TEST(KTimesReconnectionStrategyTest, FirstUse) {
  size_t K = 10;
  KTimesReconnectionStrategy strategy(K);

  for (size_t i = 0; i < K; ++i) {
    ASSERT_TRUE(strategy.canReconnect());
  }
  ASSERT_FALSE(strategy.canReconnect());
}

/**
 * @given initialized strategy with k limit
 *        @and canReconnect invokes k times
 * @when  reset invokes
 *        @and k + 1 times invokes canReconnect
 * @then  checks that k times it returns true
 *        @and last time it returns false
 */
TEST(KTimesReconnectionStrategyTest, UseAfterReset) {
  size_t K = 10;
  KTimesReconnectionStrategy strategy(K);
  for (size_t i = 0; i < K; ++i) {
    strategy.canReconnect();
  }
  strategy.reset();
  for (size_t i = 0; i < K; ++i) {
    ASSERT_TRUE(strategy.canReconnect());
  }
  ASSERT_FALSE(strategy.canReconnect());
}
