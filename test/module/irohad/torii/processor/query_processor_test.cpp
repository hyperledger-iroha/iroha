/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include <boost/variant.hpp>
#include "backend/protobuf/block.hpp"
#include "backend/protobuf/proto_query_response_factory.hpp"
#include "backend/protobuf/query_responses/proto_error_query_response.hpp"
#include "cryptography/crypto_provider/crypto_defaults.hpp"
#include "cryptography/keypair.hpp"
#include "framework/common_constants.hpp"
#include "framework/result_gtest_checkers.hpp"
#include "framework/test_logger.hpp"
#include "framework/test_subscriber.hpp"
#include "interfaces/query_responses/block_error_response.hpp"
#include "interfaces/query_responses/block_query_response.hpp"
#include "module/irohad/ametsuchi/mock_block_query.hpp"
#include "module/irohad/ametsuchi/mock_query_executor.hpp"
#include "module/irohad/ametsuchi/mock_storage.hpp"
#include "module/irohad/ametsuchi/mock_wsv_query.hpp"
#include "module/irohad/validation/validation_mocks.hpp"
#include "module/shared_model/builders/protobuf/test_block_builder.hpp"
#include "module/shared_model/builders/protobuf/test_query_builder.hpp"
#include "module/shared_model/builders/protobuf/test_transaction_builder.hpp"
#include "network/ordering_gate.hpp"
#include "torii/processor/query_processor_impl.hpp"
#include "utils/query_error_response_visitor.hpp"

using namespace iroha;
using namespace iroha::ametsuchi;
using namespace iroha::validation;
using namespace framework::test_subscriber;

using ::testing::_;
using ::testing::A;
using ::testing::Invoke;
using ::testing::Return;

class QueryProcessorTest : public ::testing::Test {
 public:
  void SetUp() override {
    qry_exec = new MockQueryExecutor();
    storage = std::make_shared<MockStorage>();
    query_response_factory =
        std::make_shared<shared_model::proto::ProtoQueryResponseFactory>();
    qpi = std::make_shared<torii::QueryProcessorImpl>(
        storage,
        storage,
        nullptr,
        query_response_factory,
        getTestLogger("QueryProcessor"));
    EXPECT_CALL(*storage, getBlockQuery())
        .WillRepeatedly(Return(block_queries));
  }

  void provideValidQueryExecutor() {
    EXPECT_CALL(*storage, createQueryExecutorRaw(_, _))
        .Times(::testing::AtMost(1))
        .WillRepeatedly(Return(qry_exec));
  }

  auto getBlocksQuery(const std::string &creator_account_id) {
    return TestUnsignedBlocksQueryBuilder()
        .createdTime(kCreatedTime)
        .creatorAccountId(creator_account_id)
        .queryCounter(kCounter)
        .build()
        .signAndAddSignature(keypair)
        .finish();
  }

  const decltype(iroha::time::now()) kCreatedTime = iroha::time::now();
  const std::string kAccountId = "account@domain";
  const uint64_t kCounter = 1048576;
  shared_model::crypto::Keypair keypair =
      shared_model::crypto::DefaultCryptoAlgorithmType::generateKeypair();

  std::vector<shared_model::interface::types::PubkeyType> signatories = {
      keypair.publicKey()};
  MockQueryExecutor *qry_exec;
  std::shared_ptr<MockBlockQuery> block_queries;
  std::shared_ptr<MockStorage> storage;
  std::shared_ptr<shared_model::interface::QueryResponseFactory>
      query_response_factory;
  std::shared_ptr<torii::QueryProcessorImpl> qpi;
};

/**
 * @given QueryProcessorImpl and GetAccountDetail query
 * @when queryHandle called at normal flow, but QueryExecutor fails to create
 * @then query error response is returned
 */
TEST_F(QueryProcessorTest,
       QueryProcessorWhereInvokeInvalidQueryAndQueryExecutorFailsToCreate) {
  auto qry = TestUnsignedQueryBuilder()
                 .creatorAccountId(kAccountId)
                 .getAccountDetail(kMaxPageSize, kAccountId)
                 .build()
                 .signAndAddSignature(keypair)
                 .finish();

  const std::string error_text{"QueryExecutor fails to create"};
  EXPECT_CALL(*storage, createQueryExecutorRaw(_, _))
      .WillRepeatedly(Return(error_text));

  auto response = qpi->queryHandle(qry);
  IROHA_ASSERT_RESULT_ERROR(response);
  EXPECT_THAT(response.assumeError(), ::testing::HasSubstr(error_text));
}

/**
 * @given QueryProcessorImpl and GetAccountDetail query
 * @when queryHandle called at normal flow
 * @then the mocked value of validateAndExecute is returned
 */
TEST_F(QueryProcessorTest, QueryProcessorWhereInvokeInvalidQuery) {
  auto qry = TestUnsignedQueryBuilder()
                 .creatorAccountId(kAccountId)
                 .getAccountDetail(kMaxPageSize, kAccountId)
                 .build()
                 .signAndAddSignature(keypair)
                 .finish();
  auto *qry_resp =
      query_response_factory
          ->createAccountDetailResponse("", 1, boost::none, qry.hash())
          .release();

  provideValidQueryExecutor();
  EXPECT_CALL(*qry_exec, validateAndExecute_(_)).WillOnce(Return(qry_resp));

  auto response = qpi->queryHandle(qry);
  IROHA_ASSERT_RESULT_VALUE(response);
  ASSERT_NE(boost::get<const shared_model::interface::AccountDetailResponse &>(
                &response.assumeValue()->get()),
            nullptr);
}

/**
 * @given QueryProcessorImpl and GetAccountDetail query with wrong signature
 * @when queryHandle called at normal flow
 * @then Query Processor returns StatefulFailed response
 */
TEST_F(QueryProcessorTest, QueryProcessorWithWrongKey) {
  auto query = TestUnsignedQueryBuilder()
                   .creatorAccountId(kAccountId)
                   .getAccountDetail(kMaxPageSize, kAccountId)
                   .build()
                   .signAndAddSignature(
                       shared_model::crypto::DefaultCryptoAlgorithmType::
                           generateKeypair())
                   .finish();
  auto *qry_resp = query_response_factory
                       ->createErrorQueryResponse(
                           shared_model::interface::QueryResponseFactory::
                               ErrorQueryType::kStatefulFailed,
                           "query signatories did not pass validation",
                           3,
                           query.hash())
                       .release();

  provideValidQueryExecutor();
  EXPECT_CALL(*qry_exec, validateAndExecute_(_)).WillOnce(Return(qry_resp));

  auto response = qpi->queryHandle(query);
  IROHA_ASSERT_RESULT_VALUE(response);
  ASSERT_NO_THROW(boost::apply_visitor(
      shared_model::interface::QueryErrorResponseChecker<
          shared_model::interface::StatefulFailedErrorResponse>(),
      response.assumeValue()->get()));
}

/**
 * @given account, ametsuchi queries
 * @when valid block query is sent, but QueryExecutor fails to create
 * @then Query Processor should emit an error to the observable
 */
TEST_F(QueryProcessorTest, GetBlocksQueryWhenQueryExecutorFailsToCreate) {
  auto block_number = 5;
  auto block_query = getBlocksQuery(kAccountId);

  EXPECT_CALL(*storage, createQueryExecutorRaw(_, _))
      .WillRepeatedly(Return("QueryExecutor fails to create"));

  auto wrapper =
      make_test_subscriber<CallExact>(qpi->blocksQueryHandle(block_query), 1);
  wrapper.subscribe([](auto response) {
    ASSERT_NO_THROW({
      const auto &error_response =
          boost::get<const shared_model::interface::BlockErrorResponse &>(
              response->get());
      EXPECT_THAT(
          error_response.message(),
          ::testing::HasSubstr("Internal error during query validation."));
    });
  });
  for (int i = 0; i < block_number; i++) {
    storage->notifier.get_subscriber().on_next(
        clone(TestBlockBuilder().build()));
  }
  ASSERT_TRUE(wrapper.validate());
}

/**
 * @given account, ametsuchi queries
 * @when valid block query is sent
 * @then Query Processor should start emitting BlockQueryResponses to the
 * observable
 */
TEST_F(QueryProcessorTest, GetBlocksQuery) {
  auto block_number = 5;
  auto block_query = getBlocksQuery(kAccountId);

  provideValidQueryExecutor();
  EXPECT_CALL(*qry_exec, validate(_, _)).WillOnce(Return(true));

  auto wrapper = make_test_subscriber<CallExact>(
      qpi->blocksQueryHandle(block_query), block_number);
  wrapper.subscribe([](auto response) {
    ASSERT_NO_THROW({
      boost::get<const shared_model::interface::BlockResponse &>(
          response->get());
    });
  });
  for (int i = 0; i < block_number; i++) {
    storage->notifier.get_subscriber().on_next(
        clone(TestBlockBuilder().build()));
  }
  ASSERT_TRUE(wrapper.validate());
}

/**
 * @given account, ametsuchi queries
 * @when valid block query is invalid (no can_get_blocks permission)
 * @then Query Processor should return an observable with BlockError
 */
TEST_F(QueryProcessorTest, GetBlocksQueryNoPerms) {
  auto block_number = 5;
  auto block_query = getBlocksQuery(kAccountId);

  provideValidQueryExecutor();
  EXPECT_CALL(*qry_exec, validate(_, _)).WillOnce(Return(false));

  auto wrapper =
      make_test_subscriber<CallExact>(qpi->blocksQueryHandle(block_query), 1);
  wrapper.subscribe([](auto response) {
    ASSERT_NO_THROW({
      boost::get<const shared_model::interface::BlockErrorResponse &>(
          response->get());
    });
  });
  for (int i = 0; i < block_number; i++) {
    storage->notifier.get_subscriber().on_next(
        clone(TestBlockBuilder()
                  .height(1)
                  .prevHash(shared_model::crypto::Hash(std::string(32, '0')))
                  .build()));
  }
  ASSERT_TRUE(wrapper.validate());
}
