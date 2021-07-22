/**
 * Copyright Soramitsu Co., Ltd. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "ordering/impl/on_demand_ordering_service_impl.hpp"

#include <unordered_set>

#include <boost/range/adaptor/indirected.hpp>
#include <boost/range/size.hpp>
#include <numeric>
#include "ametsuchi/tx_presence_cache.hpp"
#include "ametsuchi/tx_presence_cache_utils.hpp"
#include "datetime/time.hpp"
#include "interfaces/iroha_internal/proposal.hpp"
#include "interfaces/iroha_internal/transaction_batch.hpp"
#include "interfaces/transaction.hpp"
#include "logger/logger.hpp"
#include "main/subscription.hpp"

using iroha::ordering::OnDemandOrderingServiceImpl;

OnDemandOrderingServiceImpl::OnDemandOrderingServiceImpl(
    size_t transaction_limit,
    std::shared_ptr<shared_model::interface::UnsafeProposalFactory>
        proposal_factory,
    std::shared_ptr<ametsuchi::TxPresenceCache> tx_cache,
    logger::LoggerPtr log,
    size_t number_of_proposals)
    : transaction_limit_(transaction_limit),
      number_of_proposals_(number_of_proposals),
      cached_txs_size_(0ull),
      proposal_factory_(std::move(proposal_factory)),
      tx_cache_(std::move(tx_cache)),
      log_(std::move(log)) {}

// -------------------------| OnDemandOrderingService |-------------------------

void OnDemandOrderingServiceImpl::onCollaborationOutcome(
    consensus::Round round) {
  log_->info("onCollaborationOutcome => {}", round);
  {
    std::lock_guard lock(proposals_mutex_);
    current_round_ = round;
  }
  tryErase(round);
}

void OnDemandOrderingServiceImpl::onBatches(CollectionType batches) {
  for (auto &batch : batches)
    if (not batchAlreadyProcessed(*batch))
      if (!insertBatchToCache(batch))
        break;

  log_->info("onBatches => collection size = {}", batches.size());
}

// ---------------------------------| Private |---------------------------------
bool OnDemandOrderingServiceImpl::insertBatchToCache(
    std::shared_ptr<shared_model::interface::TransactionBatch> const &batch) {
  std::lock_guard<std::shared_timed_mutex> lock(batches_cache_cs_);
  if (cached_txs_size_ + boost::size(batch->transactions())
      > (transaction_limit_ * 1000ull)) {
    log_->warn("Transactions cache overload. Batch {} skipped.", *batch);
    return false;
  }
  if (used_batches_cache_.find(batch) == used_batches_cache_.end()) {
    batches_cache_.insert(batch);
    cached_txs_size_ += boost::size(batch->transactions());
    getSubscription()->notify(EventTypes::kOnNewBatchInCache,
                              std::shared_ptr(batch));
    getSubscription()->notify(EventTypes::kOdOsCachedTxsSizeChanged,
                              cached_txs_size_);
  }
  assert(std::accumulate(batches_cache_.begin(),
                         batches_cache_.end(),
                         0ull,
                         [](unsigned long long sum, auto const &batch) {
                           return sum + boost::size(batch->transactions());
                         })
         == cached_txs_size_);
  return true;
}

void OnDemandOrderingServiceImpl::removeFromBatchesCache(
    const OnDemandOrderingService::HashesSetType &hashes) {
  std::lock_guard<std::shared_timed_mutex> lock(batches_cache_cs_);
  batches_cache_.merge(used_batches_cache_);
  assert(used_batches_cache_.empty());
  for (auto it = batches_cache_.begin(); it != batches_cache_.end();) {
    if (std::any_of(it->get()->transactions().begin(),
                    it->get()->transactions().end(),
                    [&hashes](const auto &tx) {
                      return hashes.find(tx->hash()) != hashes.end();
                    })) {
      auto const erased_size = boost::size((*it)->transactions());
      it = batches_cache_.erase(it);
      assert(cached_txs_size_ >= erased_size);
      cached_txs_size_ -= erased_size;
    } else {
      ++it;
    }
  }
  getSubscription()->notify(EventTypes::kOdOsCachedTxsSizeChanged,
                            cached_txs_size_);
}

bool OnDemandOrderingServiceImpl::isEmptyBatchesCache() const {
  std::shared_lock<std::shared_timed_mutex> lock(batches_cache_cs_);
  return batches_cache_.empty();
}

void OnDemandOrderingServiceImpl::forCachedBatches(
    std::function<void(const BatchesSetType &)> const &f) {
  std::shared_lock<std::shared_timed_mutex> lock(batches_cache_cs_);
  f(batches_cache_);
}

std::vector<std::shared_ptr<shared_model::interface::Transaction>>
OnDemandOrderingServiceImpl::getTransactionsFromBatchesCache(
    size_t requested_tx_amount) {
  std::vector<std::shared_ptr<shared_model::interface::Transaction>> collection;
  collection.reserve(requested_tx_amount);

  std::lock_guard<std::shared_timed_mutex> lock(batches_cache_cs_);
  auto it = batches_cache_.begin();
  while (it != batches_cache_.end()
         and collection.size() + boost::size((*it)->transactions())
             <= requested_tx_amount) {
    collection.insert(std::end(collection),
                      std::begin((*it)->transactions()),
                      std::end((*it)->transactions()));
    used_batches_cache_.insert(*it);
    batches_cache_.erase(it);
    it = batches_cache_.begin();
  }

  return collection;
}

std::optional<std::shared_ptr<const OnDemandOrderingServiceImpl::ProposalType>>
OnDemandOrderingServiceImpl::onRequestProposal(consensus::Round round) {
  log_->debug("Requesting a proposal for round {}", round);
  std::optional<
      std::shared_ptr<const OnDemandOrderingServiceImpl::ProposalType>>
      result;
  do {
    std::lock_guard<std::mutex> lock(proposals_mutex_);
    auto it = proposal_map_.find(round);
    if (it != proposal_map_.end()) {
      result = it->second;
      break;
    }

    bool const is_current_round_or_next2 =
        (round.block_round == current_round_.block_round
             ? (round.reject_round - current_round_.reject_round)
             : (round.block_round - current_round_.block_round))
        <= 2ull;

    if (is_current_round_or_next2) {
      result = packNextProposals(round);
      getSubscription()->notify(EventTypes::kOnPackProposal, round);
    }
  } while (false);
  log_->debug("uploadProposal, {}, {}returning a proposal.",
              round,
              result ? "" : "NOT ");
  return result;
}

std::optional<std::shared_ptr<shared_model::interface::Proposal>>
OnDemandOrderingServiceImpl::tryCreateProposal(
    consensus::Round const &round,
    const TransactionsCollectionType &txs,
    shared_model::interface::types::TimestampType created_time) {
  std::optional<std::shared_ptr<shared_model::interface::Proposal>> proposal;
  if (not txs.empty()) {
    proposal = proposal_factory_->unsafeCreateProposal(
        round.block_round, created_time, txs | boost::adaptors::indirected);
    log_->debug(
        "packNextProposal: data has been fetched for {}. "
        "Number of transactions in proposal = {}.",
        round,
        txs.size());
  } else {
    proposal = std::nullopt;
    log_->debug("No transactions to create a proposal for {}", round);
  }

  assert(proposal_map_.find(round) == proposal_map_.end());
  proposal_map_.emplace(round, proposal);
  return proposal;
}

std::optional<std::shared_ptr<shared_model::interface::Proposal>>
OnDemandOrderingServiceImpl::packNextProposals(const consensus::Round &round) {
  auto now = iroha::time::now();
  std::vector<std::shared_ptr<shared_model::interface::Transaction>> txs;
  if (!isEmptyBatchesCache())
    txs = getTransactionsFromBatchesCache(transaction_limit_);

  log_->debug("Packed proposal contains: {} transactions.", txs.size());
  return tryCreateProposal(round, txs, now);
}

void OnDemandOrderingServiceImpl::tryErase(
    const consensus::Round &current_round) {
  // find first round that is not less than current_round
  auto current_proposal_it = proposal_map_.lower_bound(current_round);
  // save at most number_of_proposals_ rounds that are less than current_round
  for (size_t i = 0; i < number_of_proposals_
       and current_proposal_it != proposal_map_.begin();
       ++i) {
    current_proposal_it--;
  }

  // do not proceed if there is nothing to remove
  if (current_proposal_it == proposal_map_.begin()) {
    return;
  }

  detail::ProposalMapType proposal_map{current_proposal_it,
                                       proposal_map_.end()};

  {
    std::lock_guard<std::mutex> lock(proposals_mutex_);
    proposal_map_.swap(proposal_map);
  }

  for (auto it = proposal_map.begin(); it != current_proposal_it; ++it) {
    log_->debug("tryErase: erased {}", it->first);
  }
}

bool OnDemandOrderingServiceImpl::batchAlreadyProcessed(
    const shared_model::interface::TransactionBatch &batch) {
  auto tx_statuses = tx_cache_->check(batch);
  if (not tx_statuses) {
    // TODO andrei 30.11.18 IR-51 Handle database error
    log_->warn("Check tx presence database error. Batch: {}", batch);
    return true;
  }
  // if any transaction is commited or rejected, batch was already processed
  // Note: any_of returns false for empty sequence
  return std::any_of(
      tx_statuses->begin(), tx_statuses->end(), [this](const auto &tx_status) {
        if (iroha::ametsuchi::isAlreadyProcessed(tx_status)) {
          log_->warn("Duplicate transaction: {}",
                     iroha::ametsuchi::getHash(tx_status).hex());
          return true;
        }
        return false;
      });
}

bool OnDemandOrderingServiceImpl::hasProposal(consensus::Round round) const {
  std::lock_guard<std::mutex> lock(proposals_mutex_);
  return proposal_map_.find(round) != proposal_map_.end();
}

void OnDemandOrderingServiceImpl::processReceivedProposal(
    CollectionType batches) {
  std::lock_guard<std::shared_timed_mutex> lock(batches_cache_cs_);
  for (auto &batch : batches) {
    batches_cache_.erase(batch);
    used_batches_cache_.insert(batch);
  }
}
