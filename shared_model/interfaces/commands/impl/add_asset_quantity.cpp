/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "interfaces/commands/add_asset_quantity.hpp"

namespace shared_model {
  namespace interface {

    std::string AddAssetQuantity::toString() const {
      return detail::PrettyStringBuilder()
          .init("AddAssetQuantity")
          .appendNamed("asset_id", assetId())
          .appendNamed("amount", amount())
          .appendNamed("title", title())
          .finalize();
    }

    bool AddAssetQuantity::operator==(const ModelType &rhs) const {
      return assetId() == rhs.assetId() and amount() == rhs.amount() and title() == rhs.title();
    }

  }  // namespace interface
}  // namespace shared_model
