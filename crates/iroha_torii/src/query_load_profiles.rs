//! Validated Torii query load-profile definitions used by benchmark binaries.

use std::fmt;

use iroha_data_model::query::parameters::MAX_FETCH_SIZE;

/// Upper bound for built-in benchmark concurrency.
pub const MAX_BENCH_CONCURRENCY: usize = 256;
/// Upper bound for measured operations in one sustained profile run.
pub const MAX_BENCH_MEASURED_OPS: usize = 1_000_000;
/// Upper bound for synthetic accounts created by a profile fixture.
pub const MAX_BENCH_ACCOUNTS: usize = 1_000_000;
/// Upper bound for committed transactions created by a profile fixture.
pub const MAX_BENCH_COMMITTED_TRANSACTIONS: usize = 1_000_000;
/// Upper bound for signed-query continuation depth in one operation.
pub const MAX_BENCH_CONTINUATION_DEPTH: usize = 10_000;

/// Sustained Torii query workload categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryLoadWorkload {
    /// Signed iterable `/query` using stored cursors and continuations.
    SignedIterableStoredContinuation,
    /// App-facing account query with primary alias projections.
    AccountAliasProjection,
    /// App-facing account-asset query with predicate filtering.
    AccountAssetsPredicate,
    /// App-facing asset-holder listing.
    AssetHolders,
    /// App-facing contract activity query over committed transaction metadata.
    ContractActivityPredicate,
    /// App-facing generic aggregate query.
    GenericAggregate,
}

impl QueryLoadWorkload {
    /// Stable label used in Criterion ids and profile output.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SignedIterableStoredContinuation => "signed_iterable_stored_continuation",
            Self::AccountAliasProjection => "account_alias_projection",
            Self::AccountAssetsPredicate => "account_assets_predicate",
            Self::AssetHolders => "asset_holders",
            Self::ContractActivityPredicate => "contract_activity_predicate",
            Self::GenericAggregate => "generic_aggregate",
        }
    }

    const fn requires_continuation(self) -> bool {
        matches!(self, Self::SignedIterableStoredContinuation)
    }
}

/// One sustained query-load profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryLoadProfile {
    /// Stable profile name.
    pub name: &'static str,
    /// Workload executed by the profile.
    pub workload: QueryLoadWorkload,
    /// Warmup operations executed before measured operations.
    pub warmup_ops: usize,
    /// Measured operations per profile run.
    pub measured_ops: usize,
    /// Concurrent logical HTTP clients.
    pub concurrency: usize,
    /// Requested query fetch size.
    pub fetch_size: usize,
    /// Requested app page limit.
    pub page_limit: usize,
    /// Synthetic account count used by the fixture.
    pub dataset_accounts: usize,
    /// Synthetic asset definitions per account.
    pub assets_per_account: usize,
    /// Synthetic committed transactions used by committed-history workloads.
    pub committed_transactions: usize,
    /// Continuation requests after the initial signed iterable query.
    pub continuation_depth: usize,
}

impl QueryLoadProfile {
    /// Validate the profile before a benchmark constructs fixtures.
    ///
    /// # Errors
    /// Returns a structured error when a profile is nonsensical, too large for
    /// a local benchmark run, or unable to exercise the requested continuation
    /// path.
    pub fn validate(&self) -> Result<(), QueryLoadProfileError> {
        validate_profile_name(self.name)?;
        if self.warmup_ops > MAX_BENCH_MEASURED_OPS {
            return Err(QueryLoadProfileError::WarmupOpsTooLarge);
        }
        if self.measured_ops == 0 {
            return Err(QueryLoadProfileError::MeasuredOpsZero);
        }
        if self.measured_ops > MAX_BENCH_MEASURED_OPS {
            return Err(QueryLoadProfileError::MeasuredOpsTooLarge);
        }
        if self.concurrency == 0 {
            return Err(QueryLoadProfileError::ConcurrencyZero);
        }
        if self.concurrency > MAX_BENCH_CONCURRENCY {
            return Err(QueryLoadProfileError::ConcurrencyTooLarge);
        }
        if self.fetch_size == 0 {
            return Err(QueryLoadProfileError::FetchSizeZero);
        }
        if self.fetch_size > MAX_FETCH_SIZE.get() as usize {
            return Err(QueryLoadProfileError::FetchSizeTooLarge);
        }
        if self.page_limit == 0 {
            return Err(QueryLoadProfileError::PageLimitZero);
        }
        if self.dataset_accounts == 0 {
            return Err(QueryLoadProfileError::DatasetAccountsZero);
        }
        if self.dataset_accounts > MAX_BENCH_ACCOUNTS {
            return Err(QueryLoadProfileError::DatasetAccountsTooLarge);
        }
        if self.assets_per_account == 0 {
            return Err(QueryLoadProfileError::AssetsPerAccountZero);
        }
        if self.committed_transactions > MAX_BENCH_COMMITTED_TRANSACTIONS {
            return Err(QueryLoadProfileError::CommittedTransactionsTooLarge);
        }
        if matches!(self.workload, QueryLoadWorkload::ContractActivityPredicate) {
            if self.committed_transactions == 0 {
                return Err(QueryLoadProfileError::CommittedTransactionsZero);
            }
        } else if self.committed_transactions != 0 {
            return Err(QueryLoadProfileError::UnexpectedCommittedTransactions);
        }
        let page_limit_base = match self.workload {
            QueryLoadWorkload::AccountAssetsPredicate => self.assets_per_account,
            QueryLoadWorkload::ContractActivityPredicate => self.committed_transactions,
            _ => self.dataset_accounts,
        };
        if self.page_limit > page_limit_base {
            return Err(QueryLoadProfileError::PageLimitExceedsDataset);
        }
        if self.continuation_depth > MAX_BENCH_CONTINUATION_DEPTH {
            return Err(QueryLoadProfileError::ContinuationDepthTooLarge);
        }
        if self.workload.requires_continuation() {
            if self.continuation_depth == 0 {
                return Err(QueryLoadProfileError::ContinuationDepthZero);
            }
            let needed_rows = self
                .continuation_depth
                .checked_add(1)
                .and_then(|pages| pages.checked_mul(self.fetch_size))
                .ok_or(QueryLoadProfileError::ContinuationRowsOverflow)?;
            if needed_rows >= self.dataset_accounts {
                return Err(QueryLoadProfileError::ContinuationDepthExceedsDataset);
            }
        } else if self.continuation_depth != 0 {
            return Err(QueryLoadProfileError::UnexpectedContinuationDepth);
        }
        Ok(())
    }
}

fn validate_profile_name(name: &str) -> Result<(), QueryLoadProfileError> {
    if name.is_empty() {
        return Err(QueryLoadProfileError::NameEmpty);
    }
    if name.len() > 80 {
        return Err(QueryLoadProfileError::NameTooLong);
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(QueryLoadProfileError::NameInvalid);
    }
    Ok(())
}

/// Built-in sustained Torii query benchmark profiles.
#[must_use]
pub const fn standard_query_load_profiles() -> [QueryLoadProfile; 6] {
    [
        QueryLoadProfile {
            name: "signed_iterable_stored_depth_16",
            workload: QueryLoadWorkload::SignedIterableStoredContinuation,
            warmup_ops: 8,
            measured_ops: 64,
            concurrency: 8,
            fetch_size: 32,
            page_limit: 64,
            dataset_accounts: 2_048,
            assets_per_account: 1,
            committed_transactions: 0,
            continuation_depth: 16,
        },
        QueryLoadProfile {
            name: "accounts_alias_projection_4x128",
            workload: QueryLoadWorkload::AccountAliasProjection,
            warmup_ops: 16,
            measured_ops: 128,
            concurrency: 4,
            fetch_size: 128,
            page_limit: 128,
            dataset_accounts: 2_048,
            assets_per_account: 1,
            committed_transactions: 0,
            continuation_depth: 0,
        },
        QueryLoadProfile {
            name: "account_assets_predicate_4x64",
            workload: QueryLoadWorkload::AccountAssetsPredicate,
            warmup_ops: 16,
            measured_ops: 128,
            concurrency: 4,
            fetch_size: 64,
            page_limit: 64,
            dataset_accounts: 512,
            assets_per_account: 128,
            committed_transactions: 0,
            continuation_depth: 0,
        },
        QueryLoadProfile {
            name: "asset_holders_4x128",
            workload: QueryLoadWorkload::AssetHolders,
            warmup_ops: 16,
            measured_ops: 128,
            concurrency: 4,
            fetch_size: 128,
            page_limit: 128,
            dataset_accounts: 2_048,
            assets_per_account: 1,
            committed_transactions: 0,
            continuation_depth: 0,
        },
        QueryLoadProfile {
            name: "contract_activity_predicate_4x64",
            workload: QueryLoadWorkload::ContractActivityPredicate,
            warmup_ops: 16,
            measured_ops: 128,
            concurrency: 4,
            fetch_size: 64,
            page_limit: 64,
            dataset_accounts: 512,
            assets_per_account: 1,
            committed_transactions: 2_048,
            continuation_depth: 0,
        },
        QueryLoadProfile {
            name: "accounts_generic_aggregate_4x128",
            workload: QueryLoadWorkload::GenericAggregate,
            warmup_ops: 16,
            measured_ops: 128,
            concurrency: 4,
            fetch_size: 128,
            page_limit: 128,
            dataset_accounts: 2_048,
            assets_per_account: 1,
            committed_transactions: 0,
            continuation_depth: 0,
        },
    ]
}

/// Profile validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryLoadProfileError {
    /// Profile name is empty.
    NameEmpty,
    /// Profile name is too long.
    NameTooLong,
    /// Profile name contains characters outside `[A-Za-z0-9_-]`.
    NameInvalid,
    /// Warmup operation count exceeds the local safety bound.
    WarmupOpsTooLarge,
    /// Measured operation count is zero.
    MeasuredOpsZero,
    /// Measured operation count exceeds the local safety bound.
    MeasuredOpsTooLarge,
    /// Concurrency is zero.
    ConcurrencyZero,
    /// Concurrency exceeds the local safety bound.
    ConcurrencyTooLarge,
    /// Fetch size is zero.
    FetchSizeZero,
    /// Fetch size exceeds the data-model maximum.
    FetchSizeTooLarge,
    /// Page limit is zero.
    PageLimitZero,
    /// Dataset account count is zero.
    DatasetAccountsZero,
    /// Dataset account count exceeds the local safety bound.
    DatasetAccountsTooLarge,
    /// Asset definition count per account is zero.
    AssetsPerAccountZero,
    /// Committed transaction count for committed-history workload is zero.
    CommittedTransactionsZero,
    /// Committed transaction count exceeds the local safety bound.
    CommittedTransactionsTooLarge,
    /// Non-committed-history workload unexpectedly requested committed transactions.
    UnexpectedCommittedTransactions,
    /// Page limit cannot be served by the configured dataset.
    PageLimitExceedsDataset,
    /// Signed continuation workload needs non-zero continuation depth.
    ContinuationDepthZero,
    /// Continuation depth exceeds the local safety bound.
    ContinuationDepthTooLarge,
    /// Continuation row requirement overflowed.
    ContinuationRowsOverflow,
    /// Dataset is too shallow for the requested continuation chain.
    ContinuationDepthExceedsDataset,
    /// Non-continuation workload unexpectedly specified continuations.
    UnexpectedContinuationDepth,
}

impl fmt::Display for QueryLoadProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NameEmpty => "profile name is empty",
            Self::NameTooLong => "profile name is too long",
            Self::NameInvalid => "profile name contains invalid characters",
            Self::WarmupOpsTooLarge => "warmup operation count is too large",
            Self::MeasuredOpsZero => "measured operation count is zero",
            Self::MeasuredOpsTooLarge => "measured operation count is too large",
            Self::ConcurrencyZero => "concurrency is zero",
            Self::ConcurrencyTooLarge => "concurrency is too large",
            Self::FetchSizeZero => "fetch size is zero",
            Self::FetchSizeTooLarge => "fetch size is too large",
            Self::PageLimitZero => "page limit is zero",
            Self::DatasetAccountsZero => "dataset account count is zero",
            Self::DatasetAccountsTooLarge => "dataset account count is too large",
            Self::AssetsPerAccountZero => "assets-per-account is zero",
            Self::CommittedTransactionsZero => "committed transaction count is zero",
            Self::CommittedTransactionsTooLarge => "committed transaction count is too large",
            Self::UnexpectedCommittedTransactions => "unexpected committed transactions",
            Self::PageLimitExceedsDataset => "page limit exceeds dataset",
            Self::ContinuationDepthZero => "continuation depth is zero",
            Self::ContinuationDepthTooLarge => "continuation depth is too large",
            Self::ContinuationRowsOverflow => "continuation row requirement overflowed",
            Self::ContinuationDepthExceedsDataset => {
                "continuation depth exceeds available dataset rows"
            }
            Self::UnexpectedContinuationDepth => "unexpected continuation depth",
        })
    }
}

impl std::error::Error for QueryLoadProfileError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> QueryLoadProfile {
        standard_query_load_profiles()[0]
    }

    fn assert_invalid(profile: QueryLoadProfile, error: QueryLoadProfileError) {
        assert_eq!(profile.validate(), Err(error));
    }

    #[test]
    fn built_in_profiles_validate() {
        for profile in standard_query_load_profiles() {
            profile
                .validate()
                .unwrap_or_else(|err| panic!("{} rejected: {err}", profile.name));
        }
    }

    #[test]
    fn rejects_invalid_names() {
        assert_invalid(
            QueryLoadProfile {
                name: "",
                ..valid_profile()
            },
            QueryLoadProfileError::NameEmpty,
        );
        assert_invalid(
            QueryLoadProfile {
                name: "contains space",
                ..valid_profile()
            },
            QueryLoadProfileError::NameInvalid,
        );
        assert_invalid(
            QueryLoadProfile {
                name: "contains\nnewline",
                ..valid_profile()
            },
            QueryLoadProfileError::NameInvalid,
        );
        assert_invalid(
            QueryLoadProfile {
                name: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ..valid_profile()
            },
            QueryLoadProfileError::NameTooLong,
        );
    }

    #[test]
    fn rejects_zero_or_oversized_operation_counts() {
        assert_invalid(
            QueryLoadProfile {
                measured_ops: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::MeasuredOpsZero,
        );
        assert_invalid(
            QueryLoadProfile {
                measured_ops: MAX_BENCH_MEASURED_OPS + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::MeasuredOpsTooLarge,
        );
        assert_invalid(
            QueryLoadProfile {
                warmup_ops: MAX_BENCH_MEASURED_OPS + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::WarmupOpsTooLarge,
        );
    }

    #[test]
    fn rejects_invalid_concurrency_and_fetch_limits() {
        assert_invalid(
            QueryLoadProfile {
                concurrency: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::ConcurrencyZero,
        );
        assert_invalid(
            QueryLoadProfile {
                concurrency: MAX_BENCH_CONCURRENCY + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::ConcurrencyTooLarge,
        );
        assert_invalid(
            QueryLoadProfile {
                fetch_size: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::FetchSizeZero,
        );
        assert_invalid(
            QueryLoadProfile {
                fetch_size: MAX_FETCH_SIZE.get() as usize + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::FetchSizeTooLarge,
        );
    }

    #[test]
    fn rejects_invalid_dataset_shapes() {
        assert_invalid(
            QueryLoadProfile {
                page_limit: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::PageLimitZero,
        );
        assert_invalid(
            QueryLoadProfile {
                dataset_accounts: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::DatasetAccountsZero,
        );
        assert_invalid(
            QueryLoadProfile {
                dataset_accounts: MAX_BENCH_ACCOUNTS + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::DatasetAccountsTooLarge,
        );
        assert_invalid(
            QueryLoadProfile {
                assets_per_account: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::AssetsPerAccountZero,
        );
        assert_invalid(
            QueryLoadProfile {
                page_limit: 64,
                dataset_accounts: 63,
                ..valid_profile()
            },
            QueryLoadProfileError::PageLimitExceedsDataset,
        );
        assert_invalid(
            QueryLoadProfile {
                workload: QueryLoadWorkload::AccountAssetsPredicate,
                page_limit: 65,
                dataset_accounts: 512,
                assets_per_account: 64,
                continuation_depth: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::PageLimitExceedsDataset,
        );
        assert_invalid(
            QueryLoadProfile {
                workload: QueryLoadWorkload::ContractActivityPredicate,
                committed_transactions: 0,
                continuation_depth: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::CommittedTransactionsZero,
        );
        assert_invalid(
            QueryLoadProfile {
                committed_transactions: MAX_BENCH_COMMITTED_TRANSACTIONS + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::CommittedTransactionsTooLarge,
        );
        assert_invalid(
            QueryLoadProfile {
                workload: QueryLoadWorkload::AccountAliasProjection,
                committed_transactions: 1,
                continuation_depth: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::UnexpectedCommittedTransactions,
        );
        assert_invalid(
            QueryLoadProfile {
                workload: QueryLoadWorkload::ContractActivityPredicate,
                page_limit: 65,
                committed_transactions: 64,
                continuation_depth: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::PageLimitExceedsDataset,
        );
    }

    #[test]
    fn rejects_bad_continuation_profiles() {
        assert_invalid(
            QueryLoadProfile {
                continuation_depth: 0,
                ..valid_profile()
            },
            QueryLoadProfileError::ContinuationDepthZero,
        );
        assert_invalid(
            QueryLoadProfile {
                continuation_depth: MAX_BENCH_CONTINUATION_DEPTH + 1,
                ..valid_profile()
            },
            QueryLoadProfileError::ContinuationDepthTooLarge,
        );
        assert_invalid(
            QueryLoadProfile {
                fetch_size: usize::MAX / 2 + 1,
                continuation_depth: 3,
                ..valid_profile()
            },
            QueryLoadProfileError::FetchSizeTooLarge,
        );
        assert_invalid(
            QueryLoadProfile {
                fetch_size: 64,
                continuation_depth: 4,
                dataset_accounts: 320,
                page_limit: 64,
                ..valid_profile()
            },
            QueryLoadProfileError::ContinuationDepthExceedsDataset,
        );
        assert_invalid(
            QueryLoadProfile {
                workload: QueryLoadWorkload::AccountAliasProjection,
                continuation_depth: 1,
                ..valid_profile()
            },
            QueryLoadProfileError::UnexpectedContinuationDepth,
        );
    }
}
