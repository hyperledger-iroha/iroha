/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "integration/executor/executor_fixture.hpp"

#include <gtest/gtest.h>
#include "backend/plain/account_detail_record_id.hpp"
#include "common/result.hpp"
#include "framework/common_constants.hpp"
#include "integration/executor/account_detail_checker.hpp"
#include "integration/executor/executor_fixture_param_provider.hpp"
#include "interfaces/query_responses/account_detail_response.hpp"
#include "module/shared_model/mock_objects_factories/mock_command_factory.hpp"
#include "module/shared_model/mock_objects_factories/mock_query_factory.hpp"

using namespace common_constants;
using namespace executor_testing;
using namespace framework::expected;
using namespace shared_model::interface::types;

using iroha::ametsuchi::QueryExecutorResult;
using shared_model::interface::AccountDetailResponse;
using shared_model::interface::permissions::Grantable;
using shared_model::interface::permissions::Role;
using shared_model::plain::AccountDetailRecordId;

static const AccountDetailKeyType kKey{"key"};
static const AccountDetailValueType kVal{"value"};

class SetAccountDetailTest : public BasicExecutorTest<ExecutorTestBase> {
 public:
  iroha::ametsuchi::CommandResult setDetail(const AccountIdType &target,
                                            const AccountDetailKeyType &key,
                                            const AccountDetailValueType &value,
                                            const AccountIdType &issuer) {
    return getItf().executeCommandAsAccount(
        *getItf().getMockCommandFactory()->constructSetAccountDetail(
            target, key, value),
        issuer,
        true);
  }

  void checkDetails(AccountIdType account,
                    DetailsByKeyByWriter reference_details) {
    assertResultValue(
        getItf()
            .executeQueryAndConvertResult(
                *getItf().getMockQueryFactory()->constructGetAccountDetail(
                    account, boost::none, boost::none, boost::none))
            .specific_response
        | [&reference_details](const auto &response) {
            checkJsonData(response.detail(), reference_details);
            return iroha::expected::Value<void>{};
          });
  }
};

/**
 * C274
 * @given a user without can_set_detail permission
 * @when execute SetAccountDetail command to set own detail
 * @then the command succeeds and the detail is added
 */
TEST_P(SetAccountDetailTest, Self) {
  getItf().createUserWithPerms(kUser, kDomain, kUserKeypair.publicKey(), {});
  assertResultValue(setDetail(kUserId, kKey, kVal, kUserId));
  checkDetails(kUserId, DetailsByKeyByWriter{{{kUserId, {{kKey, kVal}}}}});
}

/**
 * C273
 * @given a user with all required permissions
 * @when execute SetAccountDetail command with nonexistent user
 * @then the command fails with error code 3
 */
TEST_P(SetAccountDetailTest, NonExistentUser) {
  checkCommandError(setDetail(kUserId, kKey, kVal, kAdminId), 3);
}

/**
 * C280
 * @given a pair of users and first one without permissions
 * @when the first one tries to execute SetAccountDetail on the second
 * @then the command does not succeed and the detail is not added
 */
TEST_P(SetAccountDetailTest, NoPerms) {
  getItf().createUserWithPerms(kUser, kDomain, kUserKeypair.publicKey(), {});
  getItf().createUserWithPerms(
      kSecondUser, kDomain, kSameDomainUserKeypair.publicKey(), {});
  assertResultError(setDetail(kSameDomainUserId, kKey, kVal, kUserId));
  checkDetails(kSameDomainUserId, DetailsByKeyByWriter{});
}

/**
 * @given a pair of users and first one has can_set_detail permission
 * @when the first one executes SetAccountDetail on the second
 * @then the command succeeds and the detail is added
 */
TEST_P(SetAccountDetailTest, ValidRolePerm) {
  getItf().createUserWithPerms(
      kUser, kDomain, kUserKeypair.publicKey(), {Role::kSetDetail});
  getItf().createUserWithPerms(
      kSecondUser, kDomain, kSameDomainUserKeypair.publicKey(), {});
  assertResultValue(setDetail(kUserId, kKey, kVal, kUserId));
  checkDetails(kUserId, DetailsByKeyByWriter{{{kUserId, {{kKey, kVal}}}}});
}

/**
 * @given a pair of users and first one has can_set_my_detail grantable
 * permission from the second
 * @when the first one executes SetAccountDetail on the second
 * @then the command succeeds and the detail is added
 */
TEST_P(SetAccountDetailTest, ValidGrantablePerm) {
  getItf().createUserWithPerms(
      kUser, kDomain, kUserKeypair.publicKey(), {Role::kSetMyAccountDetail});
  getItf().createUserWithPerms(
      kSecondUser, kDomain, kSameDomainUserKeypair.publicKey(), {});
  assertResultValue(getItf().executeCommandAsAccount(
      *getItf().getMockCommandFactory()->constructGrantPermission(
          kSameDomainUserId, Grantable::kSetMyAccountDetail),
      kUserId,
      true));
  assertResultValue(setDetail(kUserId, kKey, kVal, kUserId));
  checkDetails(kUserId, DetailsByKeyByWriter{{{kUserId, {{kKey, kVal}}}}});
}

/**
 * @given a pair of users and first one has root permission
 * @when the first one executes SetAccountDetail on the second
 * @then the command succeeds and the detail is added
 */
TEST_P(SetAccountDetailTest, RootPermission) {
  getItf().createUserWithPerms(
      kUser, kDomain, kUserKeypair.publicKey(), {Role::kRoot});
  getItf().createUserWithPerms(
      kSecondUser, kDomain, kSameDomainUserKeypair.publicKey(), {});
  assertResultValue(setDetail(kUserId, kKey, kVal, kUserId));
  checkDetails(kUserId, DetailsByKeyByWriter{{{kUserId, {{kKey, kVal}}}}});
}

INSTANTIATE_TEST_CASE_P(Base,
                        SetAccountDetailTest,
                        executor_testing::getExecutorTestParams(),
                        executor_testing::paramToString);
