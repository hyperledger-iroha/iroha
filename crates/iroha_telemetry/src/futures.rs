//! Module with telemetry future telemetry processing
use std::{collections::HashMap, marker::Unpin, time::Duration};

use iroha_futures::FuturePollTelemetry;
use iroha_logger::telemetry::Event as Telemetry;
use tokio::time;
use tokio_stream::{Stream, StreamExt, wrappers::errors::BroadcastStreamRecvError};

pub mod post_process {
    //! Module with telemetry post processing

    use super::*;

    /// Post processed info of function
    #[derive(
        Clone, Debug, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize,
    )]
    pub struct PostProcessedInfo {
        /// Function name
        pub name: String,
        /// Standard deviation
        pub stddev: f64,
        /// Variance
        pub variance: f64,
        /// Median
        pub median: f64,
        /// Mean
        pub mean: f64,
        /// Minimum
        pub min: f64,
        /// Maximum
        pub max: f64,
    }

    #[allow(clippy::fallible_impl_from)]
    impl From<(String, HashMap<u64, Vec<Duration>>)> for PostProcessedInfo {
        fn from((name, entries): (String, HashMap<u64, Vec<Duration>>)) -> Self {
            let values: Vec<f64> = entries
                .values()
                .flat_map(|v| v.iter().map(Duration::as_secs_f64))
                .collect();

            if values.is_empty() {
                return Self {
                    name,
                    stddev: 0.0,
                    variance: 0.0,
                    median: 0.0,
                    mean: 0.0,
                    min: 0.0,
                    max: 0.0,
                };
            }

            let minmax = values.iter().copied().collect::<stats::MinMax<f64>>();

            let mean = stats::mean(values.iter().copied());
            let median = stats::median(values.iter().copied()).unwrap_or_default();
            let variance = stats::variance(values.iter().copied());
            let stddev = stats::stddev(values.iter().copied());
            let min = minmax.min().copied().unwrap_or_default();
            let max = minmax.max().copied().unwrap_or_default();

            Self {
                name,
                stddev,
                variance,
                median,
                mean,
                min,
                max,
            }
        }
    }

    /// Collects info from stream of future poll telemetry
    pub async fn collect_info(
        mut receiver: impl Stream<Item = FuturePollTelemetry> + Unpin + Send,
    ) -> Vec<PostProcessedInfo> {
        let mut out = HashMap::<String, HashMap<u64, Vec<_>>>::new();
        let timeout = Duration::from_millis(100);
        while let Ok(Some(item)) = time::timeout(timeout, receiver.next()).await {
            let FuturePollTelemetry { id, name, duration } = item;
            out.entry(name)
                .or_default()
                .entry(id)
                .or_default()
                .push(Duration::from_nanos(duration));
        }
        out.into_iter().map(Into::into).collect()
    }
}

/// Gets stream of future poll telemetry out of general telemetry stream
pub fn get_stream(
    receiver: impl Stream<Item = Result<Telemetry, BroadcastStreamRecvError>> + Unpin,
) -> impl Stream<Item = FuturePollTelemetry> + Unpin {
    receiver
        .filter_map(Result::ok)
        .map(FuturePollTelemetry::try_from)
        .filter_map(Result::ok)
        .map(
            |FuturePollTelemetry {
                 id,
                 mut name,
                 duration,
             }| {
                name.retain(|c| !c.is_whitespace());
                FuturePollTelemetry { id, name, duration }
            },
        )
}

#[cfg(test)]
mod tests {
    use tokio_stream::empty;

    use super::*;

    #[tokio::test]
    async fn collect_info_handles_empty_stream() {
        let info = post_process::collect_info(empty()).await;
        assert!(info.is_empty());
    }

    #[test]
    fn post_processed_info_handles_empty_data() {
        use std::{collections::HashMap, time::Duration};

        let info = post_process::PostProcessedInfo::from((
            String::from("test"),
            HashMap::<u64, Vec<Duration>>::new(),
        ));

        assert!((info.median - 0.0).abs() < f64::EPSILON);
        assert!((info.min - 0.0).abs() < f64::EPSILON);
        assert!((info.max - 0.0).abs() < f64::EPSILON);
    }
}
