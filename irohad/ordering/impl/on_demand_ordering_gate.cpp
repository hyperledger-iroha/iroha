/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "ordering/impl/on_demand_ordering_gate.hpp"

#include <boost/range/adaptor/filtered.hpp>
#include <boost/range/adaptor/indexed.hpp>
#include <boost/range/adaptor/transformed.hpp>
#include <boost/range/empty.hpp>
#include <iterator>

#include "ametsuchi/tx_presence_cache.hpp"
#include "ametsuchi/tx_presence_cache_utils.hpp"
#include "common/visitor.hpp"
#include "interfaces/iroha_internal/transaction_batch.hpp"
#include "interfaces/iroha_internal/transaction_batch_impl.hpp"
#include "interfaces/iroha_internal/transaction_batch_parser_impl.hpp"
#include "logger/logger.hpp"
#include "ordering/impl/on_demand_common.hpp"

using iroha::ordering::OnDemandOrderingGate;

OnDemandOrderingGate::OnDemandOrderingGate(
    std::shared_ptr<OnDemandOrderingService> ordering_service,
    std::shared_ptr<transport::OdOsNotification> network_client,
    std::shared_ptr<shared_model::interface::UnsafeProposalFactory> factory,
    std::shared_ptr<ametsuchi::TxPresenceCache> tx_cache,
    size_t transaction_limit,
    logger::LoggerPtr log)
    : log_(std::move(log)),
      transaction_limit_(transaction_limit),
      ordering_service_(std::move(ordering_service)),
      connection_manager_(std::move(network_client)),
      proposal_factory_(std::move(factory)),
      tx_cache_(std::move(tx_cache)) {}

OnDemandOrderingGate::~OnDemandOrderingGate() {
  stop();
}

void OnDemandOrderingGate::propagateBatch(
    std::shared_ptr<shared_model::interface::TransactionBatch> batch) {
  std::shared_lock<std::shared_timed_mutex> stop_lock(stop_mutex_);
  if (stop_requested_) {
    log_->warn("Not propagating {} because stop was requested.", *batch);
    return;
  }

  // TODO iceseer 14.01.21 IR-959 Refactor to avoid copying.
  ordering_service_->onBatches(
      transport::OdOsNotification::CollectionType{batch});
  connection_manager_->onBatches(
      transport::OdOsNotification::CollectionType{batch});
}

void OnDemandOrderingGate::processRoundSwitch(RoundSwitch const &event) {
  log_->debug("Current: {}", event.next_round);
  current_round_ = event.next_round;
  current_ledger_state_ = event.ledger_state;

  std::shared_lock<std::shared_timed_mutex> stop_lock(stop_mutex_);
  if (stop_requested_) {
    log_->warn("Not doing anything because stop was requested.");
    return;
  }

  // notify our ordering service about new round
  ordering_service_->onCollaborationOutcome(event.next_round);

  // ToDo reduce network load: first send proposal hash to announce the
  // proposal with its hash, then remote peers should request propos if they
  // does not have it. Or send announce with transactions metainfo, remote peers
  // request only those they do not have. Order is guarantied by BatchesCache
  // and BatchesSet.
  this->sendCachedTransactions();

  // request proposal from remote peer for the current round,
  // send hash of own proposal
  connection_manager_->onRequestProposal(
      event.next_round, ordering_service_->getProposalHash(event.next_round));
}

void OnDemandOrderingGate::stop() {
  std::lock_guard<std::shared_timed_mutex> stop_lock(stop_mutex_);
  if (not stop_requested_) {
    stop_requested_ = true;
    log_->info("Stopping.");
    connection_manager_.reset();
  }
}

std::optional<iroha::network::OrderingEvent>
OnDemandOrderingGate::processProposalRequest(ProposalEvent const &event) const {
  if (not current_ledger_state_ || event.round != current_round_) {
    return std::nullopt;
  }
  std::shared_ptr<const shared_model::interface::Proposal> proposal;
  if (std::holds_alternative<shared_model::crypto::Hash>(
          event.proposal_or_hash)) {
    // assume proposal_or_hash already exist in ordering_service's cache and has
    // same hash for the round
    auto [opt_proposal, hash] =
        ordering_service_->getProposalWithHash(event.round);
    assert(opt_proposal);
    assert(*opt_proposal);
    assert(hash
           == std::get<shared_model::crypto::Hash>(event.proposal_or_hash));
    if (opt_proposal)
      proposal = *opt_proposal;
  }
  if (not proposal) {
    return network::OrderingEvent{
        std::nullopt, event.round, current_ledger_state_};
  }
  auto result_proposal = removeReplaysAndDuplicates(proposal);
  // no need to check empty proposal
  if (boost::empty(result_proposal->transactions())) {
    return network::OrderingEvent{
        std::nullopt, event.round, current_ledger_state_};
  }
  shared_model::interface::types::SharedTxsCollectionType transactions;
  for (auto &transaction : result_proposal->transactions()) {
    transactions.push_back(clone(transaction));
  }
  auto batch_txs =
      shared_model::interface::TransactionBatchParserImpl().parseBatches(
          transactions);
  shared_model::interface::types::BatchesCollectionType batches;
  for (auto &txs : batch_txs) {
    batches.push_back(
        std::make_shared<shared_model::interface::TransactionBatchImpl>(
            std::move(txs)));
  }
  ordering_service_->processReceivedProposal(std::move(batches));
  return network::OrderingEvent{
      std::move(result_proposal), event.round, current_ledger_state_};
}

void OnDemandOrderingGate::sendCachedTransactions() {
  assert(not stop_mutex_.try_lock());  // lock must be taken before
  // TODO iceseer 14.01.21 IR-958 Check that OS is remote
  ordering_service_->forCachedBatches([this](auto const &batches) {
    auto end_iterator = batches.begin();
    auto current_number_of_transactions = 0u;
    for (; end_iterator != batches.end(); ++end_iterator) {
      auto batch_size = (*end_iterator)->transactions().size();
      if (current_number_of_transactions + batch_size <= transaction_limit_) {
        current_number_of_transactions += batch_size;
      } else {
        break;
      }
    }

    if (not batches.empty()) {
      connection_manager_->onBatches(
          transport::OdOsNotification::CollectionType{batches.begin(),
                                                      end_iterator});
    }
  });
}

std::shared_ptr<const shared_model::interface::Proposal>
OnDemandOrderingGate::removeReplaysAndDuplicates(
    std::shared_ptr<const shared_model::interface::Proposal> proposal) const {
  std::vector<bool> proposal_txs_validation_results;
  auto dup_hashes = std::make_shared<OnDemandOrderingService::HashesSetType>();

  auto tx_is_not_processed = [this, &dup_hashes](const auto &tx) {
    auto tx_result = tx_cache_->check(tx.hash());
    if (not tx_result) {
      // TODO andrei 30.11.18 IR-51 Handle database error
      return false;
    }
    auto is_processed = ametsuchi::isAlreadyProcessed(*tx_result);
    if (is_processed) {
      dup_hashes->insert(tx.hash());
      log_->warn("Duplicate transaction: {}",
                 iroha::ametsuchi::getHash(*tx_result).hex());
    }
    return !is_processed;
  };

  std::unordered_set<std::string> hashes;
  auto tx_is_unique = [&hashes](const auto &tx) {
    auto tx_hash = tx.hash().hex();

    if (hashes.count(tx_hash)) {
      return false;
    } else {
      hashes.insert(tx_hash);
      return true;
    }
  };

  shared_model::interface::TransactionBatchParserImpl batch_parser;

  bool has_invalid_txs = false;
  auto batches = batch_parser.parseBatches(proposal->transactions());
  for (auto &batch : batches) {
    bool txs_are_valid =
        std::all_of(batch.begin(), batch.end(), [&](const auto &tx) {
          return tx_is_not_processed(tx) and tx_is_unique(tx);
        });
    proposal_txs_validation_results.insert(
        proposal_txs_validation_results.end(), batch.size(), txs_are_valid);
    has_invalid_txs |= not txs_are_valid;
  }

  if (not has_invalid_txs) {
    return proposal;
  }

  if (!dup_hashes->empty()) {
    ordering_service_->onDuplicates(*dup_hashes);
  }

  auto unprocessed_txs =
      proposal->transactions() | boost::adaptors::indexed()
      | boost::adaptors::filtered(
          [proposal_txs_validation_results =
               std::move(proposal_txs_validation_results)](const auto &el) {
            return proposal_txs_validation_results.at(el.index());
          })
      | boost::adaptors::transformed(
          [](const auto &el) -> decltype(auto) { return el.value(); });

  return proposal_factory_->unsafeCreateProposal(
      proposal->height(), proposal->createdTime(), unprocessed_txs);
}
