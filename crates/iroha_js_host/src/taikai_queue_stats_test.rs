    #[test]
    fn taikai_queue_stats_conversion_populates_js_struct() {
        let queue = TaikaiPullQueueStats {
            pending_segments: 2,
            pending_bytes: 3,
            pending_batches: 4,
            in_flight_batches: 5,
            hedged_batches: 6,
            shaper_denials: QosStats {
                priority: 1,
                standard: 2,
                bulk: 3,
            },
            dropped_segments: 7,
            failovers: 8,
            open_circuits: 9,
        };

        let js = JsTaikaiQueueStats::from(queue);
        assert_eq!(js.pending_segments.0, 2);
        assert_eq!(js.shaper_denials.bulk.0, 3);
        assert_eq!(js.hedged_batches.0, 6);
        assert_eq!(js.open_circuits.0, 9);
    }
