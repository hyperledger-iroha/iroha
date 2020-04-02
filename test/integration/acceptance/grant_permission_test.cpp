/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "integration/acceptance/grantable_permissions_fixture.hpp"

using namespace integration_framework;

using namespace shared_model;
using namespace shared_model::interface;
using namespace shared_model::interface::permissions;
using namespace common_constants;

/**
 * TODO mboldyrev 18.01.2019 IR-216 remove, covered by
 * postgres_executor_test GrantPermissions.NoAccount
 *
 * C256 Grant permission to a non-existing account
 * @given an account with rights to grant rights to other accounts
 * @when the account grants rights to non-existing account
 * @then this transaction is stateful invalid
 */
TEST_F(GrantablePermissionsFixture, GrantToInexistingAccount) {
  IntegrationTestFramework(1)
      .setInitialState(kAdminSigner)
      .sendTx(makeAccountWithPerms(
          kAccount1, kAccount1Signer->publicKey(), kCanGrantAll, kRole1))
      .skipProposal()
      .skipVerifiedProposal()
      .skipBlock()
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kAddMySignatory))
      .skipProposal()
      .checkVerifiedProposal(
          [](auto &proposal) { ASSERT_EQ(proposal->transactions().size(), 0); })
      .checkBlock(
          [](auto block) { ASSERT_EQ(block->transactions().size(), 0); });
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 transform to command executor storage
 * integration test
 * the part with queries is covered by permission SFV integration tests
 *
 * C257 Grant add signatory permission
 * @given an account with rights to grant rights to other accounts
 * AND the account grants add signatory rights to an existing account
 * (permittee)
 * @when the permittee adds signatory to the account
 * @then a block with transaction to add signatory to the account is written
 * AND there is a signatory added by the permittee
 */
TEST_F(GrantablePermissionsFixture, GrantAddSignatoryPermission) {
  IntegrationTestFramework itf(1);
  itf.setInitialState(kAdminSigner);
  auto &x = createTwoAccounts(
      itf, {Role::kAddMySignatory, Role::kGetMySignatories}, {Role::kReceive});
  x.sendTxAwait(grantPermission(kAccount1,
                                *kAccount1Signer,
                                kAccount2,
                                permissions::Grantable::kAddMySignatory),
                [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      // Add signatory
      .sendTxAwait(
          permitteeAddSignatory(kAccount2, *kAccount2Signer, kAccount1),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendQuery(
          querySignatories(kAccount1, *kAccount1Signer),
          makeSignatoryResponceChecker(::testing::UnorderedElementsAre(
              kAccount1Signer->publicKey(), kAccount2Signer->publicKey())));
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 transform to command executor storage
 * integration test
 * the part with queries is covered by permission SFV integration tests
 *
 * C258 Grant remove signatory permission
 * @given an account with rights to grant rights to other accounts
 * AND the account grants add and remove signatory rights to an existing account
 * AND the permittee has added his/her signatory to the account
 * AND the account revoked add signatory from the permittee
 * @when the permittee removes signatory from the account
 * @then a block with transaction to remove signatory from the account is
 * written AND there is no signatory added by the permittee
 */
TEST_F(GrantablePermissionsFixture, GrantRemoveSignatoryPermission) {
  IntegrationTestFramework itf(1);
  itf.setInitialState(kAdminSigner);
  createTwoAccounts(itf,
                    {Role::kAddMySignatory,
                     Role::kRemoveMySignatory,
                     Role::kGetMySignatories},
                    {Role::kReceive})
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kAddMySignatory))
      .skipProposal()
      .skipVerifiedProposal()
      .skipBlock()
      .sendTxAwait(
          grantPermission(kAccount1,
                          *kAccount1Signer,
                          kAccount2,
                          permissions::Grantable::kRemoveMySignatory),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendTxAwait(
          permitteeAddSignatory(kAccount2, *kAccount2Signer, kAccount1),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendTxAwait(
          permitteeRemoveSignatory(kAccount2, *kAccount2Signer, kAccount1),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendQuery(querySignatories(kAccount1, *kAccount1Signer),
                 makeSignatoryResponceChecker(::testing::UnorderedElementsAre(
                     kAccount1Signer->publicKey())));
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 remove, covered by
 * postgres_executor_test GrantPermissions.Valid
 * and permission tests
 *
 * C259 Grant set quorum permission
 * @given an account with rights to grant rights to other accounts
 * AND the account grants add signatory rights
 * AND set quorum rights to an existing account
 * AND the permittee has added his/her signatory to the account
 * @when the permittee changes the number of quorum in the account
 * @then a block with transaction to change quorum in the account is written
 * AND the quorum number of account equals to the number, set by permittee
 */
TEST_F(GrantablePermissionsFixture, GrantSetQuorumPermission) {
  auto quorum_quantity = 2;
  auto check_quorum_quantity = checkQuorum(quorum_quantity);

  IntegrationTestFramework itf(1);
  itf.setInitialState(kAdminSigner);
  createTwoAccounts(
      itf,
      {Role::kSetMyQuorum, Role::kAddMySignatory, Role::kGetMyAccount},
      {Role::kReceive})
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kSetMyQuorum))
      .skipProposal()
      .skipVerifiedProposal()
      .skipBlock()
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kAddMySignatory))
      .skipProposal()
      .skipVerifiedProposal()
      .skipBlock()
      .sendTx(permitteeAddSignatory(kAccount2, *kAccount2Signer, kAccount1))
      .skipProposal()
      .skipVerifiedProposal()
      .skipBlock()
      .sendTxAwait(
          setQuorum(kAccount2, *kAccount2Signer, kAccount1, 2),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendQuery(queryAccount(kAccount1, *kAccount1Signer),
                 check_quorum_quantity);
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 transform to command executor storage
 * integration test
 * the part with queries is covered by permission SFV integration tests
 *
 * C260 Grant set account detail permission
 * @given an account with rights to grant rights to other accounts
 * AND the account grants set account detail permission to a permittee
 * @when the permittee sets account detail to the account
 * @then a block with transaction to set account detail is written
 * AND the permittee is able to read the data
 * AND the account is able to read the data
 */
TEST_F(GrantablePermissionsFixture, GrantSetAccountDetailPermission) {
  auto check_account_detail =
      checkAccountDetail(kAccountDetailKey, kAccountDetailValue);

  IntegrationTestFramework itf(1);
  itf.setInitialState(kAdminSigner);
  createTwoAccounts(
      itf, {Role::kSetMyAccountDetail, Role::kGetMyAccDetail}, {Role::kReceive})
      .sendTxAwait(
          grantPermission(kAccount1,
                          *kAccount1Signer,
                          kAccount2,
                          permissions::Grantable::kSetMyAccountDetail),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendTxAwait(
          setAccountDetail(kAccount2,
                           *kAccount2Signer,
                           kAccount1,
                           kAccountDetailKey,
                           kAccountDetailValue),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendQuery(queryAccountDetail(kAccount1, *kAccount1Signer),
                 check_account_detail);
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 transform to command executor storage
 * integration test
 * the part with queries is covered by permission SFV integration tests
 *
 * C261 Grant transfer permission
 * @given an account with rights to grant transfer of his/her assets
 * AND the account can receive assets
 * AND the account has some amount of assets
 * AND the account has permitted to some other account in the system
 * to transfer his/her assets
 * @when the permittee transfers assets of the account
 * @then a block with transaction to grant right is written
 * AND the transfer is made
 */
TEST_F(GrantablePermissionsFixture, GrantTransferPermission) {
  auto amount_of_asset = "1000.0";

  IntegrationTestFramework itf(1);
  itf.setInitialState(kAdminSigner);
  createTwoAccounts(itf,
                    {Role::kTransferMyAssets, Role::kReceive},
                    {Role::kTransfer, Role::kReceive})
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kTransferMyAssets))
      .skipProposal()
      .checkBlock(
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendTxAwait(
          addAssetAndTransfer(
              kAdminName, *kAdminSigner, amount_of_asset, kAccount1),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .sendTxAwait(
          transferAssetFromSource(kAccount2,
                                  *kAccount2Signer,
                                  kAccount1,
                                  amount_of_asset,
                                  kAccount2),
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 1); })
      .done();
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 remove, covered by
 * postgres_executor_test GrantPermissions.NoPerms
 * the part with queries is covered by permission SFV integration tests
 *
 * C262 GrantPermission without such permissions
 * @given an account !without! rights to grant rights to other accounts
 * @when the account grants rights to an existing account
 * @then this transaction is statefully invalid
 */
TEST_F(GrantablePermissionsFixture, GrantWithoutGrantPermissions) {
  for (auto &perm : kAllGrantable) {
    IntegrationTestFramework itf(1);
    itf.setInitialState(kAdminSigner);
    createTwoAccounts(itf, {Role::kReceive}, {Role::kReceive})
        .sendTx(grantPermission(kAccount1, *kAccount1Signer, kAccount2, perm))
        .skipProposal()
        .checkVerifiedProposal([](auto &proposal) {
          ASSERT_EQ(proposal->transactions().size(), 0);
        })
        .checkBlock(
            [](auto &block) { ASSERT_EQ(block->transactions().size(), 0); });
  }
}

/**
 * TODO mboldyrev 18.01.2019 IR-216 transform to command executor storage
 * integration test
 *
 * C263 GrantPermission more than once
 * @given an account with rights to grant rights to other accounts
 * AND an account that have already granted a permission to a permittee
 * @when the account grants the same permission to the same permittee
 * @then this transaction is statefully invalid
 */

TEST_F(GrantablePermissionsFixture, GrantMoreThanOnce) {
  IntegrationTestFramework itf(1);
  itf.setInitialState(kAdminSigner);
  createTwoAccounts(itf, {kCanGrantAll}, {Role::kReceive})
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kAddMySignatory))
      .skipProposal()
      .skipVerifiedProposal()
      .skipBlock()
      .sendTx(grantPermission(kAccount1,
                              *kAccount1Signer,
                              kAccount2,
                              permissions::Grantable::kAddMySignatory))
      .skipProposal()
      .checkVerifiedProposal(
          [](auto &proposal) { ASSERT_EQ(proposal->transactions().size(), 0); })
      .checkBlock(
          [](auto &block) { ASSERT_EQ(block->transactions().size(), 0); });
}
