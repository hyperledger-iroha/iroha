//! Scenario catalog for the blockchain communication-vulnerability matrix.
//!
//! The catalog is derived from "Blockchain Communication Vulnerabilities" by
//! Andrei Lebedev and Vincent Gramoli, arXiv:2603.02661v1.  It lets Izanami
//! runs report Iroha results next to the paper's Algorand, Aptos, Avalanche,
//! Redbelly, and Solana baseline without baking the comparison into ad hoc
//! shell output.

/// Paper title used in generated reports.
pub const PAPER_TITLE: &str = "Blockchain Communication Vulnerabilities";
/// Paper arXiv identifier.
pub const PAPER_ARXIV_ID: &str = "2603.02661v1";
/// Paper DOI.
pub const PAPER_DOI: &str = "10.48550/arXiv.2603.02661";
/// Baseline transaction rate used by the paper's experiments.
pub const PAPER_TPS: u16 = 200;
/// Baseline duration used by the paper's experiments.
pub const PAPER_DURATION_SECS: u16 = 800;
/// Attack start offset used by the paper's timed fault experiments.
pub const PAPER_ATTACK_START_SECS: u16 = 133;
/// Attack end offset used by the paper's timed fault experiments.
pub const PAPER_ATTACK_END_SECS: u16 = 266;
/// Validator/node count used by the paper's comparison experiments.
pub const PAPER_NODE_COUNT: u8 = 20;

/// Protocol-agnostic communication attacks evaluated by the paper.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CommunicationAttack {
    /// Sustained valid request traffic sent to a single blockchain node.
    TargetedLoad,
    /// A small fraction of nodes crashes temporarily and later rejoins.
    TransientFailure,
    /// A fraction of packets is dropped between two node groups.
    PacketLoss,
    /// A large fraction of nodes crashes temporarily and tests whether the chain recovers.
    Stopping,
    /// The current consensus leader has its network connectivity impaired.
    LeaderIsolation,
}

impl CommunicationAttack {
    /// All paper attack cases in report order.
    pub const ALL: [Self; 5] = [
        Self::TargetedLoad,
        Self::TransientFailure,
        Self::PacketLoss,
        Self::Stopping,
        Self::LeaderIsolation,
    ];

    /// Stable machine-readable name.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::TargetedLoad => "targeted-load",
            Self::TransientFailure => "transient-failure",
            Self::PacketLoss => "packet-loss",
            Self::Stopping => "stopping",
            Self::LeaderIsolation => "leader-isolation",
        }
    }

    /// Human-readable report label.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::TargetedLoad => "Targeted load",
            Self::TransientFailure => "Transient failure",
            Self::PacketLoss => "Packet loss",
            Self::Stopping => "Stopping",
            Self::LeaderIsolation => "Leader isolation",
        }
    }

    /// Concise description of the paper setup.
    #[must_use]
    pub const fn paper_setup(self) -> &'static str {
        match self {
            Self::TargetedLoad => {
                "single client submits valid transfers at 200 TPS to one blockchain node"
            }
            Self::TransientFailure => "a small node fraction crashes at 133s and recovers at 266s",
            Self::PacketLoss => {
                "25-75% packet loss is injected between a fault-threshold-sized group and the rest of the network"
            }
            Self::Stopping => {
                "a large node fraction crashes at 133s and recovers at 266s; liveness after recovery is the key signal"
            }
            Self::LeaderIsolation => {
                "the current consensus leader receives 75% inbound and outbound packet loss during its leadership window"
            }
        }
    }

    /// Primary signals used to classify an Izanami run.
    #[must_use]
    pub const fn primary_metrics(self) -> &'static [&'static str] {
        match self {
            Self::TargetedLoad => &["p50/p95 commit latency", "ingress queue pressure", "loss"],
            Self::TransientFailure => &[
                "recovery time",
                "committed/offered ratio",
                "height progress",
            ],
            Self::PacketLoss => &[
                "height progress",
                "p50/p95 commit latency",
                "P2P/DA drop counters",
            ],
            Self::Stopping => &[
                "post-recovery liveness",
                "height progress",
                "committed/offered ratio",
            ],
            Self::LeaderIsolation => &[
                "zero-throughput windows",
                "leader/proposer telemetry",
                "recovery time",
            ],
        }
    }
}

/// Blockchains compared by the paper.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ReferenceBlockchain {
    /// Algorand.
    Algorand,
    /// Aptos.
    Aptos,
    /// Avalanche.
    Avalanche,
    /// Redbelly.
    Redbelly,
    /// Solana.
    Solana,
}

impl ReferenceBlockchain {
    /// Stable display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Algorand => "Algorand",
            Self::Aptos => "Aptos",
            Self::Avalanche => "Avalanche",
            Self::Redbelly => "Redbelly",
            Self::Solana => "Solana",
        }
    }
}

/// Paper outcome bucket used for cross-chain comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaperOutcome {
    /// The paper observed no material vulnerability under the tested condition.
    Resilient,
    /// The paper observed material degradation but not a full stop/loss classification.
    Degraded,
    /// The paper identified a significant vulnerability.
    Vulnerable,
    /// The case was not applicable, usually because the protocol has no single leader.
    NotApplicable,
    /// The paper setup required a mitigation or did not make a clean classification.
    Inconclusive,
}

impl PaperOutcome {
    /// Stable report label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Resilient => "resilient",
            Self::Degraded => "degraded",
            Self::Vulnerable => "vulnerable",
            Self::NotApplicable => "n/a",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// One paper outcome and the short evidence note behind it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceOutcome {
    /// Attack being classified.
    pub attack: CommunicationAttack,
    /// Outcome observed in the paper.
    pub outcome: PaperOutcome,
    /// Short evidence note from the paper.
    pub note: &'static str,
}

/// Baseline for one blockchain from the paper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceBaseline {
    /// Blockchain name.
    pub blockchain: ReferenceBlockchain,
    /// Communication topology or mitigation family from the paper.
    pub communication: &'static str,
    /// Claimed fault tolerance threshold cited by the paper.
    pub fault_tolerance: &'static str,
    /// Per-attack outcomes.
    pub outcomes: &'static [ReferenceOutcome],
}

const ALGORAND_OUTCOMES: &[ReferenceOutcome] = &[
    ReferenceOutcome {
        attack: CommunicationAttack::TargetedLoad,
        outcome: PaperOutcome::Resilient,
        note: "committed the targeted 200 TPS workload without material loss",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::TransientFailure,
        outcome: PaperOutcome::Resilient,
        note: "no impact was observed under the tested transient failures",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::PacketLoss,
        outcome: PaperOutcome::Vulnerable,
        note: "25% packet loss can fill broadcast queues and lose transactions",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::Stopping,
        outcome: PaperOutcome::Resilient,
        note: "recovered after large crash/restart stopping tests",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::LeaderIsolation,
        outcome: PaperOutcome::NotApplicable,
        note: "randomized consensus does not expose a deterministic single leader",
    },
];

const APTOS_OUTCOMES: &[ReferenceOutcome] = &[
    ReferenceOutcome {
        attack: CommunicationAttack::TargetedLoad,
        outcome: PaperOutcome::Vulnerable,
        note: "single receiving validator becomes a QuorumStore bottleneck",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::TransientFailure,
        outcome: PaperOutcome::Degraded,
        note: "35% transient failures caused severe degradation and about 11% loss",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::PacketLoss,
        outcome: PaperOutcome::Degraded,
        note: "TCP loss raised tail latency and delayed finalization",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::Stopping,
        outcome: PaperOutcome::Resilient,
        note: "recovered liveness after the paper's large stopping tests",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::LeaderIsolation,
        outcome: PaperOutcome::Vulnerable,
        note: "isolating each deterministic leader stops commits during the window",
    },
];

const AVALANCHE_OUTCOMES: &[ReferenceOutcome] = &[
    ReferenceOutcome {
        attack: CommunicationAttack::TargetedLoad,
        outcome: PaperOutcome::Inconclusive,
        note: "base-fee escalation had to be disabled to isolate communication effects",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::TransientFailure,
        outcome: PaperOutcome::Vulnerable,
        note: "10% transient failures plus throttling caused sustained transaction loss",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::PacketLoss,
        outcome: PaperOutcome::Degraded,
        note: "50-75% TCP packet loss produced missing latency samples and stalled finalization",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::Stopping,
        outcome: PaperOutcome::Vulnerable,
        note: "near-halt appears from about 25-30% transient failures",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::LeaderIsolation,
        outcome: PaperOutcome::Degraded,
        note: "soft proposer fallback preserved progress but increased latency",
    },
];

const REDBELLY_OUTCOMES: &[ReferenceOutcome] = &[
    ReferenceOutcome {
        attack: CommunicationAttack::TargetedLoad,
        outcome: PaperOutcome::Resilient,
        note: "handled targeted 200 TPS load with low latency in the paper setup",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::TransientFailure,
        outcome: PaperOutcome::Resilient,
        note: "no impact was observed under the tested transient failures",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::PacketLoss,
        outcome: PaperOutcome::Degraded,
        note: "TCP peer bandwidth collapsed by more than 95% during 50% packet loss",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::Stopping,
        outcome: PaperOutcome::Resilient,
        note: "recovered after large crash/restart stopping tests",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::LeaderIsolation,
        outcome: PaperOutcome::NotApplicable,
        note: "leaderless consensus has no single leader to isolate",
    },
];

const SOLANA_OUTCOMES: &[ReferenceOutcome] = &[
    ReferenceOutcome {
        attack: CommunicationAttack::TargetedLoad,
        outcome: PaperOutcome::Resilient,
        note: "committed the targeted 200 TPS workload, though slower than some peers",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::TransientFailure,
        outcome: PaperOutcome::Degraded,
        note: "candidate-leader failures delayed stabilization but did not match the stopping threshold",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::PacketLoss,
        outcome: PaperOutcome::Resilient,
        note: "QUIC plus Turbine erasure coding sustained dissemination under packet loss",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::Stopping,
        outcome: PaperOutcome::Vulnerable,
        note: "85-95% transient failures can cause unrecoverable liveness stalls",
    },
    ReferenceOutcome {
        attack: CommunicationAttack::LeaderIsolation,
        outcome: PaperOutcome::Vulnerable,
        note: "scheduled leader isolation stops commits during the attack window",
    },
];

/// Paper baselines in the same order used by the report script.
pub const REFERENCE_BASELINES: &[ReferenceBaseline] = &[
    ReferenceBaseline {
        blockchain: ReferenceBlockchain::Algorand,
        communication: "gossip/TCP",
        fault_tolerance: "n/5",
        outcomes: ALGORAND_OUTCOMES,
    },
    ReferenceBaseline {
        blockchain: ReferenceBlockchain::Aptos,
        communication: "hierarchical/TCP",
        fault_tolerance: "n/3",
        outcomes: APTOS_OUTCOMES,
    },
    ReferenceBaseline {
        blockchain: ReferenceBlockchain::Avalanche,
        communication: "throttled/TCP",
        fault_tolerance: "n/5",
        outcomes: AVALANCHE_OUTCOMES,
    },
    ReferenceBaseline {
        blockchain: ReferenceBlockchain::Redbelly,
        communication: "rate-limited/TCP",
        fault_tolerance: "n/3",
        outcomes: REDBELLY_OUTCOMES,
    },
    ReferenceBaseline {
        blockchain: ReferenceBlockchain::Solana,
        communication: "hierarchical/QUIC",
        fault_tolerance: "n/3",
        outcomes: SOLANA_OUTCOMES,
    },
];

/// How closely Izanami can exercise a paper case today.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IzanamiCoverage {
    /// Izanami exercises the same category directly.
    Native,
    /// Izanami exercises the relevant stress shape but not the exact network primitive.
    Approximation,
}

impl IzanamiCoverage {
    /// Stable report label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Approximation => "approximation",
        }
    }
}

/// Izanami command profile for one paper scenario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IzanamiScenarioProfile {
    /// Attack being exercised.
    pub attack: CommunicationAttack,
    /// How directly Izanami covers the paper primitive.
    pub coverage: IzanamiCoverage,
    /// Default CLI arguments for a paper-like run.
    pub paper_like_args: &'static [&'static str],
    /// Short note for operators.
    pub note: &'static str,
}

/// Izanami profiles for the five paper cases.
pub const IZANAMI_SCENARIO_PROFILES: &[IzanamiScenarioProfile] = &[
    IzanamiScenarioProfile {
        attack: CommunicationAttack::TargetedLoad,
        coverage: IzanamiCoverage::Native,
        paper_like_args: &[
            "--allow-net",
            "--peers",
            "20",
            "--faulty",
            "0",
            "--duration",
            "800s",
            "--tps",
            "200",
            "--submitters",
            "1",
            "--max-inflight",
            "512",
            "--fault-enable-network-packet-loss=false",
        ],
        note: "one submitter is pinned to one preferred Torii endpoint, matching a targeted valid-load shape",
    },
    IzanamiScenarioProfile {
        attack: CommunicationAttack::TransientFailure,
        coverage: IzanamiCoverage::Native,
        paper_like_args: &[
            "--allow-net",
            "--peers",
            "20",
            "--faulty",
            "2",
            "--duration",
            "800s",
            "--fault-window-start",
            "133s",
            "--fault-window-end",
            "266s",
            "--tps",
            "200",
            "--submitters",
            "20",
            "--max-inflight",
            "512",
            "--fault-enable-crash-restart=true",
            "--fault-enable-wipe-storage=false",
            "--fault-enable-spam-invalid-transactions=false",
            "--fault-enable-network-latency=false",
            "--fault-enable-network-partition=false",
            "--fault-enable-network-packet-loss=false",
            "--fault-enable-cpu-stress=false",
            "--fault-enable-disk-saturation=false",
        ],
        note: "crash/restart faults exercise transient peer failure inside the paper's 133s-266s attack window",
    },
    IzanamiScenarioProfile {
        attack: CommunicationAttack::PacketLoss,
        coverage: IzanamiCoverage::Native,
        paper_like_args: &[
            "--allow-net",
            "--peers",
            "20",
            "--faulty",
            "5",
            "--duration",
            "800s",
            "--fault-window-start",
            "133s",
            "--fault-window-end",
            "266s",
            "--tps",
            "200",
            "--submitters",
            "20",
            "--max-inflight",
            "512",
            "--fault-enable-crash-restart=false",
            "--fault-enable-wipe-storage=false",
            "--fault-enable-spam-invalid-transactions=false",
            "--fault-enable-network-latency=false",
            "--fault-enable-network-partition=false",
            "--fault-enable-network-packet-loss=true",
            "--fault-enable-cpu-stress=false",
            "--fault-enable-disk-saturation=false",
        ],
        note: "Izanami injects 75% in-process P2P application-frame loss during the paper's timed attack window",
    },
    IzanamiScenarioProfile {
        attack: CommunicationAttack::Stopping,
        coverage: IzanamiCoverage::Native,
        paper_like_args: &[
            "--allow-net",
            "--peers",
            "20",
            "--faulty",
            "18",
            "--duration",
            "800s",
            "--fault-window-start",
            "133s",
            "--fault-window-end",
            "266s",
            "--tps",
            "200",
            "--submitters",
            "20",
            "--max-inflight",
            "512",
            "--fault-enable-crash-restart=true",
            "--fault-enable-wipe-storage=false",
            "--fault-enable-spam-invalid-transactions=false",
            "--fault-enable-network-latency=false",
            "--fault-enable-network-partition=false",
            "--fault-enable-network-packet-loss=false",
            "--fault-enable-cpu-stress=false",
            "--fault-enable-disk-saturation=false",
        ],
        note: "large crash/restart population tests whether Iroha resumes progress after mass recovery",
    },
    IzanamiScenarioProfile {
        attack: CommunicationAttack::LeaderIsolation,
        coverage: IzanamiCoverage::Native,
        paper_like_args: &[
            "--allow-net",
            "--peers",
            "20",
            "--faulty",
            "1",
            "--duration",
            "800s",
            "--fault-window-start",
            "133s",
            "--fault-window-end",
            "266s",
            "--tps",
            "200",
            "--submitters",
            "20",
            "--max-inflight",
            "512",
            "--fault-enable-crash-restart=false",
            "--fault-enable-wipe-storage=false",
            "--fault-enable-spam-invalid-transactions=false",
            "--fault-enable-network-latency=false",
            "--fault-enable-network-partition=false",
            "--fault-enable-network-packet-loss=true",
            "--fault-enable-cpu-stress=false",
            "--fault-enable-disk-saturation=false",
        ],
        note: "Izanami samples Sumeragi leader telemetry and applies 75% in-process P2P packet loss to the current leader during the timed attack window",
    },
];

/// Return the baseline for a reference blockchain.
#[must_use]
pub fn baseline_for(blockchain: ReferenceBlockchain) -> Option<&'static ReferenceBaseline> {
    REFERENCE_BASELINES
        .iter()
        .find(|baseline| baseline.blockchain == blockchain)
}

/// Return the paper outcome for one blockchain and attack.
#[must_use]
pub fn outcome_for(
    blockchain: ReferenceBlockchain,
    attack: CommunicationAttack,
) -> Option<ReferenceOutcome> {
    baseline_for(blockchain)?
        .outcomes
        .iter()
        .copied()
        .find(|outcome| outcome.attack == attack)
}

/// Return the Izanami profile for one attack.
#[must_use]
pub fn izanami_profile_for(attack: CommunicationAttack) -> Option<&'static IzanamiScenarioProfile> {
    IZANAMI_SCENARIO_PROFILES
        .iter()
        .find(|profile| profile.attack == attack)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_covers_the_five_paper_attacks_once() {
        let slugs: BTreeSet<_> = CommunicationAttack::ALL
            .iter()
            .map(|attack| attack.slug())
            .collect();

        assert_eq!(slugs.len(), CommunicationAttack::ALL.len());
        assert!(slugs.contains("targeted-load"));
        assert!(slugs.contains("transient-failure"));
        assert!(slugs.contains("packet-loss"));
        assert!(slugs.contains("stopping"));
        assert!(slugs.contains("leader-isolation"));
    }

    #[test]
    fn reference_baselines_cover_each_attack_for_each_chain() {
        for baseline in REFERENCE_BASELINES {
            assert_eq!(
                baseline.outcomes.len(),
                CommunicationAttack::ALL.len(),
                "{} should classify every paper attack",
                baseline.blockchain.name()
            );
            for attack in CommunicationAttack::ALL {
                assert!(
                    baseline
                        .outcomes
                        .iter()
                        .any(|outcome| outcome.attack == attack),
                    "{} missing {}",
                    baseline.blockchain.name(),
                    attack.slug()
                );
            }
        }
    }

    #[test]
    fn reported_headline_vulnerabilities_are_pinned() {
        assert_eq!(
            outcome_for(
                ReferenceBlockchain::Algorand,
                CommunicationAttack::PacketLoss
            )
            .expect("Algorand packet loss outcome")
            .outcome,
            PaperOutcome::Vulnerable
        );
        assert_eq!(
            outcome_for(
                ReferenceBlockchain::Aptos,
                CommunicationAttack::TargetedLoad
            )
            .expect("Aptos targeted load outcome")
            .outcome,
            PaperOutcome::Vulnerable
        );
        assert_eq!(
            outcome_for(
                ReferenceBlockchain::Avalanche,
                CommunicationAttack::TransientFailure
            )
            .expect("Avalanche transient failure outcome")
            .outcome,
            PaperOutcome::Vulnerable
        );
        assert_eq!(
            outcome_for(
                ReferenceBlockchain::Redbelly,
                CommunicationAttack::PacketLoss
            )
            .expect("Redbelly packet loss outcome")
            .outcome,
            PaperOutcome::Degraded
        );
        assert_eq!(
            outcome_for(ReferenceBlockchain::Solana, CommunicationAttack::Stopping)
                .expect("Solana stopping outcome")
                .outcome,
            PaperOutcome::Vulnerable
        );
        assert_eq!(
            outcome_for(
                ReferenceBlockchain::Solana,
                CommunicationAttack::LeaderIsolation
            )
            .expect("Solana leader isolation outcome")
            .outcome,
            PaperOutcome::Vulnerable
        );
    }

    #[test]
    fn izanami_profiles_cover_each_attack_and_mark_native_faults() {
        let profiled: BTreeSet<_> = IZANAMI_SCENARIO_PROFILES
            .iter()
            .map(|profile| profile.attack)
            .collect();
        assert_eq!(profiled.len(), CommunicationAttack::ALL.len());
        for attack in CommunicationAttack::ALL {
            assert!(profiled.contains(&attack), "missing {}", attack.slug());
        }

        assert_eq!(
            izanami_profile_for(CommunicationAttack::PacketLoss)
                .expect("packet-loss profile")
                .coverage,
            IzanamiCoverage::Native
        );
        assert_eq!(
            izanami_profile_for(CommunicationAttack::LeaderIsolation)
                .expect("leader-isolation profile")
                .coverage,
            IzanamiCoverage::Native
        );
    }

    #[test]
    fn paper_like_profiles_preserve_baseline_shape() {
        for profile in IZANAMI_SCENARIO_PROFILES {
            let args = profile.paper_like_args;
            assert!(args.windows(2).any(|pair| pair == ["--peers", "20"]));
            assert!(args.windows(2).any(|pair| pair == ["--duration", "800s"]));
            assert!(args.windows(2).any(|pair| pair == ["--tps", "200"]));
            assert!(
                args.contains(&"--allow-net"),
                "{} profile must be runnable as an Izanami command",
                profile.attack.slug()
            );
            if profile.attack != CommunicationAttack::TargetedLoad {
                assert!(
                    args.windows(2)
                        .any(|pair| pair == ["--fault-window-start", "133s"]),
                    "{} profile must preserve the paper attack-window start",
                    profile.attack.slug()
                );
                assert!(
                    args.windows(2)
                        .any(|pair| pair == ["--fault-window-end", "266s"]),
                    "{} profile must preserve the paper attack-window end",
                    profile.attack.slug()
                );
            }
        }
    }
}
