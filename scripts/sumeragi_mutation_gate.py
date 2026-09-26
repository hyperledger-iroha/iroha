#!/usr/bin/env python3
"""Sumeragi mutation gate: the meta-check of spec §13.4 for `crates/iroha_sumeragi`.

Purpose
    Every mutation of §13.4 exists in the crate only as a `cfg(sumeragi_mutation = "<ID>")`
    switch, compiled in when the crate feature `mutation-testing` is on and the environment
    variable `SUMERAGI_MUTATION=<ID>` is set (see `crates/iroha_sumeragi/build.rs`). For each
    mutation this script builds the mutated crate and runs

        SUMERAGI_MUTATION=<ID> cargo test -p iroha_sumeragi --release \\
            --features mutation-testing,sim --lib -- <named det test(s)>

    expecting a FAILURE, then (unless --fast) the mutation's randomized scenario(s) with
    SUMERAGI_SIM_SEEDS=200, again expecting a failure. It also runs the unmutated build once
    (every named test and every listed scenario), expecting success. `--lib` only skips the
    doctest pass; every named test is a unit test of the library.

    A mutation is
      killed_by_test           at least one of its named tests failed (the §13.4 requirement);
      killed_by_scenario_only  its named tests passed, a listed scenario failed;
      survived                 nothing failed;
      error                    it did not build, or a named test filter matched no test.

    The table MUTATIONS mirrors §13.4 (MS*/ML* rows and the MA* rows of the commit-attestation
    extension, §3.7) plus ME* (the as-built rules E1-E7 of Appendix E, with their regression
    tests) and MR-* (revision-4 rules with det_r4 tests).

Prerequisites
    Python 3.9+ (stdlib only) and a working `cargo` for the workspace. Builds go to a dedicated
    target directory (default `<repo>/target/sumeragi-mutants`), one sub-directory per job
    (`job0`, `job1`, ...), so only this crate rebuilds between mutations and parallel jobs never
    share a build lock. Nothing outside that directory is written; the source tree is never
    modified.

Outputs
    <target-dir>/report.json   per-mutation results and a summary
    <target-dir>/logs/*.log    full cargo output of every step

Exit status
    0 when the baseline passes and no mutation survived or errored; 1 otherwise. With --strict
    (the literal §13.4 CI rule) a mutation killed only by its scenario also fails the gate.

Examples
    scripts/sumeragi_mutation_gate.py --jobs 4
    scripts/sumeragi_mutation_gate.py --only MS2,MS27 --fast
    scripts/sumeragi_mutation_gate.py --list
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import re
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CRATE = "iroha_sumeragi"
CRATE_SRC = REPO / "crates" / CRATE / "src"
FEATURES = "mutation-testing,sim"

# Randomized scenarios of §13.3 (test functions in `sim::tests`).
SCENARIOS = {
    "f01": "sim::tests::f01_crashed_leaders",
    "f02": "sim::tests::f02_silent_proxy_tail",
    "f03": "sim::tests::f03_withholding_proxy_tail",
    "f04": "sim::tests::f04_silent_or_slow_set_a",
    "f05": "sim::tests::f05_equivocating_leader",
    "f06": "sim::tests::f06_vote_withholders",
    "f07": "sim::tests::f07_adversarial_tc_composition",
    "f08": "sim::tests::f08_split_brain",
    "f09": "sim::tests::f09_loss_dup_reorder",
    "f10": "sim::tests::f10_delay_spikes",
    "f11": "sim::tests::f11_partitions",
    "f12": "sim::tests::f12_clock_skew_drift",
    "f13": "sim::tests::f13_crash_restart_churn",
    "f14": "sim::tests::f14_whole_cluster_restart",
    "f15": "sim::tests::f15_slow_executors",
    "f16": "sim::tests::f16_validator_set_change",
    "f17": "sim::tests::f17_far_behind_joiner",
    "f18": "sim::tests::f18_floods",
    "f19": "sim::tests::f19_poison_payload",
    "f20": "sim::tests::f20_cross_instance_replay",
    "f21": "sim::tests::f21_divergent_executor",
    "f22": "sim::tests::f22_idle_chain",
    "f23": "sim::tests::f23_non_3f1_committees",
    "f24": "sim::tests::f24_record_corruption_and_loss",
    "f25": "sim::tests::f25_relay_tampering",
    "f26": "sim::tests::f26_byzantine_responders",
    "f27": "sim::tests::f27_storage_faults",
    "f28": "sim::tests::f28_key_rotation",
    "f29": "sim::tests::f29_cpu_flood",
    "f30": "sim::tests::f30_max_size_blocks",
    "f31": "sim::tests::f31_independent_finality",
    "f32": "sim::tests::f32_cluster_restart_lock_or_cqc",
    "f33": "sim::tests::f33_hidden_pqc",
    "f34": "sim::tests::f34_late_entrants",
    "f35": "sim::tests::f35_local_queue_asymmetry",
    "f36": "sim::tests::f36_late_leaders",
    "f37": "sim::tests::f37_commit_attestation",
    # F9 variant for ML5a: a vote blackout ending at GST (sim/mutation_group_2.rs).
    "f09r": "sim::mutation_group_2::f09r_vote_blackout_until_gst",
    # F32 with up to f proposers of the needed blocks kept down after the restart (ML10).
    "f32s": "sim::mutation_group_3::f32s_cluster_restart_without_proposers",
    # F21 with the O3 halt oracle: the divergent node must halt by ApplyDiverged (MS34).
    "f21h": "sim::mutation_group_1::f21_divergent_executor_halts",
}


@dataclass(frozen=True)
class Mutation:
    id: str
    site: str
    tests: tuple
    scenarios: tuple


def m(mid, site, tests, scenarios=()):
    return Mutation(mid, site, tuple(tests), tuple(scenarios))


# id, site and change, named deterministic test(s), randomized scenario(s) — mirrors §13.4.
MUTATIONS = [
    # ---- safety rules (one mutation per rule of §7.1)
    m("MS1", "propose/restore: rule 0 skipped, a restarted leader builds a new block",
      ["det_s1_leader_restart_no_second_proposal"], ["f13"]),
    m("MS2", "try_prepare: condition 2 (sign-once on safety.prepare) deleted",
      ["det_s2_s27_prepare_once_across_restart"], ["f05"]),
    m("MS3", "advance_to keeps proposal and try_prepare ignores proposal.view",
      ["det_s3_no_prepare_for_old_view_proposal"], ["f10", "f33"]),
    m("MS4", "try_prepare/try_commit: timeout fence deleted",
      ["det_s4_fence_across_restart"], ["f07", "f13"]),
    m("MS5", "try_commit: high_pqc.view == view deleted",
      ["det_s5_no_commit_stale_qc"], ["f08"]),
    m("MS7", "on_qc 2a: high_pqc = c unconditionally",
      ["det_s7_lock_monotone"], ["f07"]),
    m("MS8", "sign_timeout: carries None instead of high_pqc",
      ["det_s8_timeout_carries_lock"], ["f08"]),
    m("MS9", "on_tick rebroadcast 1: re-signs the timeout with the current lock",
      ["det_s9_timeout_resend_exact"], ["f09", "f07"]),
    m("MS10a", "propose rule 2 always fresh and on_proposal checks TC proposals as fresh",
      ["det_s10a_tc_rule_forces_reproposal"], ["f08"]),
    m("MS10b", "on_proposal step 6: TC-rule branch deleted",
      ["det_s10b_voter_rejects_tc_violation"], ["f08"]),
    m("MS11", "form_tc: lowest-hq entries and the lowest non-None hq's PrepareQC",
      ["det_s11_tc_max_hq"], ["f07"]),
    m("MS12", "verify_tc: high_pqc.view == max(hq) deleted",
      ["det_s12_tc_verify_rejects_low_high_pqc"], ["f07"]),
    m("MS13", "quorum(n) returns 2f + 1",
      ["det_s13_quorum_n5"], ["f23"]),
    m("MS14", "verify_qc accepts popcount >= q - 1",
      ["det_s14_qc_popcount"], ["f03", "f18"]),
    m("MS15", "verify_qc callers pass the configuration of tip.height",
      ["det_s15_committee_of_height"], ["f16"]),
    m("MS16", "vote/prop/tmo preimages omit the instance id",
      ["det_s16_cross_instance_replay"], ["f20"]),
    m("MS17", "vote_preimage omits R",
      ["det_s17_result_bound"], ["f03"]),
    m("MS18", "on_proposal step 1: signature of any member of C_h accepted",
      ["det_s18_non_leader_proposal_dropped"], ["f18"]),
    m("MS19", "on_proposal steps 5-6: parent_qc/parent_hash/parent_result checks deleted",
      ["det_s19_wrong_parent_rejected"], ["f05"]),
    m("MS20", "body_ok always true",
      ["det_s20_forged_body_under_real_header"], ["f26"]),
    m("MS21", "commit_height: no verification for Status / parent_qc CommitQCs",
      ["det_s21_forged_commitqc_via_status", "det_s21_forged_commitqc_via_parent_qc"],
      ["f18"]),
    m("MS22", "sync: block_hash and parent-link checks deleted",
      ["det_s22_sync_forged_block"], ["f17"]),
    m("MS23", "try_prepare: vote routed before persist()",
      ["det_s23_crash_between_persist_and_durable"], ["f13"]),
    m("MS24", "fake driver: O2 barrier holds only Send/Broadcast",
      ["det_s24_local_cqc_not_exposed_before_durable_strong",
       "det_s24_o2_barrier_holds_every_effect"], ["f13"]),
    m("MS25", "persist: lock not copied into the record",
      ["det_s25_lock_persisted_with_commit"], ["f08"]),
    m("MS26", "sign_timeout: safety.timeout not recorded",
      ["det_s26_forget_timeout"], ["f13"]),
    m("MS27", "try_prepare: safety.prepare not recorded",
      ["det_s2_s27_prepare_once_across_restart"], ["f05"]),
    m("MS29", "propose: safety.proposal not recorded",
      ["det_s29_forget_proposal"], ["f13"]),
    m("MS30", "restore: high_pqc not restored from the record's lock",
      ["det_s30_lock_restored"], ["f08"]),
    m("MS31", "restore: Absent classified as R3",
      ["det_s31_deleted_record_abstains"], ["f24"]),
    m("MS31b", "restore R2: anchored at once from the local tip, no probe",
      ["det_s31b_record_and_store_lost"], ["f24"]),
    m("MS31c", "on_status: probes answered while unanchored or abstaining",
      ["det_s31c_abstaining_node_does_not_answer"], ["f24"]),
    m("MS31d", "on_status: every member Status counts as a probe reply",
      ["det_s31d_stale_status_is_not_a_reply"], ["f24"]),
    m("MS31e", "on_status step 1: echo signature not verified",
      ["det_s31e_echo_signature"], ["f24"]),
    m("MS32a", "restore R5: parent_commit_qc committed without verification",
      ["det_s32a_r5_forged_parent_qc"], ["f24"]),
    m("MS32b", "restore: R6 classified as R3",
      ["det_s32b_store_behind_record"], ["f27"]),
    m("MS32c", "pending_apply flush: parent-link check deleted",
      ["det_s32c_r5_block_not_extending_tip"], []),
    m("MS33a", "fake driver record store: one file per instance for all keys",
      ["det_s33_key_rotation_restart"], ["f28"]),
    m("MS33b", "enter_height: the first configured key signs regardless of membership",
      ["det_s33_key_rotation_restart"], ["f28"]),
    m("MS33c", "restore: keys classified against the pre-R5 tip (no composition)",
      ["det_s33_key_rotation_restart"], ["f28"]),
    m("MS33d", "fake driver record store: store-id comparison deleted",
      ["det_s33d_installation_log_rollback"], ["f24"]),
    m("MS34", "fake driver apply: commitment comparison skipped",
      ["det_s34_apply_divergence_halts_strong"], ["f21h"]),
    m("MS35", "on_proposal step 8: body_ok failure treated as a signed defect",
      ["det_s35_relay_tamper_no_evidence"], ["f25"]),
    m("MS36a", "on_executed step 3: certified mismatch handled like uncertified Invalid",
      ["det_s36_certified_mismatch_is_local"], ["f21"]),
    m("MS36b", "on_executed step 2: Failed handled like Invalid",
      ["det_s36_certified_mismatch_is_local"], ["f15"]),
    m("MS37", "on_qc step 1: safety-monitor branch deleted",
      ["det_s37_conflicting_commitqc_halts"], []),
    m("MS38", "on_vote: pooled without the signature check",
      ["det_s38_forged_votes_never_pooled"], ["f18"]),
    # ---- liveness rules
    m("ML1", "level returns start(h)", ["det_l1_levels_grow"], ["f15"]),
    m("ML2", "on_tick rebroadcast 1 deleted", ["det_l2_lost_timeout_resent"], ["f09"]),
    m("ML3", "try_prepare/try_commit: 'set A or stage >= 1' becomes 'set A'",
      ["det_l3_setb_joins_stage1"], ["f06"]),
    m("ML4", "stage-2 triggers (a) and (c) deleted", ["det_l4_stage2_timing"], ["f03"]),
    m("ML5a", "on_tick: vote retransmit deleted", ["det_l5_lost_vote_retransmitted_strong"],
      ["f09", "f09r"]),
    m("ML5b", "on_vote step 2 (PrepareQC answer) deleted",
      ["det_l5_lost_vote_retransmitted"], ["f09"]),
    m("ML6", "start-level decay branch deleted", ["det_l6_level_decays"], ["f15"]),
    m("ML7", "demoted_set returns the empty set", ["det_l7_demotion_golden"], ["f01"]),
    m("ML8", "timeout_insert: join rule deleted", ["det_l8_join_f_plus_1"], ["f11"]),
    m("ML9", "persist: high_tc not copied", ["det_l9_restart_after_tc_entry"], ["f13", "f11"]),
    m("ML10", "on_proposal step 8: StoreBody omitted",
      ["det_l10_cluster_restart_lock_no_cqc_strong"], ["f32", "f32s"]),
    m("ML11", "on_block_request ignores heights <= tip.height",
      ["det_l11_pending_apply_after_peers_moved_on"], ["f16"]),
    m("ML12", "fake driver scheduler: FIFO ingress without O5 lanes",
      ["det_l12_tick_ahead_of_flood"], ["f29"]),
    m("ML13", "propose never forces EMPTY and on_proposal drops the payload_len rule",
      ["det_l13_empty_after_views"], ["f19"]),
    m("ML14", "fake builder ignores PayloadRejected", ["det_l14_poison_quarantined_strong"],
      ["f19"]),
    m("ML15", "Status at keepalive cadence only", ["det_l15_unsettled_status_rate"], ["f11"]),
    m("ML16", "initial_stage returns 0", ["det_l16_hint_from_parent_commitqc"], ["f06"]),
    m("ML17", "discard_exec keeps the exec entries", ["det_l17_hidden_pqc_reexecutes"], ["f33"]),
    m("ML18", "leader: views over permutation slots",
      ["det_l18_f_plus_1_distinct_leaders"], ["f01"]),
    m("ML19a", "request_proposal clause (a) deleted", ["det_l19_late_entrant_repush"], ["f34"]),
    m("ML19b", "on_status: late-entrant re-push deleted", ["det_l19_late_entrant_repush"], ["f34"]),
    m("ML19c", "on_status: proposal request after the rate limit",
      ["det_l19_late_entrant_repush"], ["f34"]),
    m("ML20", "commit_height step 1a deleted", ["det_l20_p_broadcasts_commitqc"], ["f09"]),
    m("ML21", "anchor: revision-3 t_tx term restored",
      ["det_l21_local_queue_moves_no_timer"], ["f35"]),
    m("ML22", "stage-1 trigger (a): Commit-phase clause deleted",
      ["det_l3_setb_joins_stage1"], ["f06"]),
    m("ML23", "commit: the committed block's pending execution dropped",
      ["det_l23_commit_before_own_execution"], ["f34"]),
    m("ML24", "request_proposal clause (b) deleted", ["det_l24_lost_proposal_copy"], ["f09"]),
    m("ML25", "sign_timeout/on_commit: raise on a timeout with a held proposal restored",
      ["det_l26_raise_only_on_slow_commit_or_exec"], ["f05"]),
    m("ML26", "on_proposal step 2: no early timeout on proven leader equivocation",
      ["det_l25_equivocating_leader_early_timeout"], ["f05"]),
    m("ML27", "discard_exec/commit_height: a pending execution records nothing",
      ["det_l27_pending_execution_counts"], ["f15"]),
    m("ML28", "record_exec keeps the last execution of the height, not the maximum",
      ["det_l28_exec_duration_is_the_height_maximum", "det_l26_raise_only_on_slow_commit_or_exec",
       "det_l27_pending_execution_counts"], ["f15"]),
    m("ML29", "commit_height: d_c measured from the anchor (E40)",
      ["det_l29_late_leader_does_not_raise"], ["f36"]),
    m("ML30", "commit_height: d_c measured from t_prop of any held proposal",
      ["det_l29_late_leader_does_not_raise"], ["f36"]),
    # ---- commit-attestation rules (§3.7, SR39-SR42)
    m("MA1", "on_vote (attested): a flagged Commit vote is pooled without a verifying attestation",
      ["det_a2_unattested_commit_votes_not_counted"], ["f37"]),
    m("MA2", "verify_qc: the attestation check of a flagged CommitQC skipped",
      ["det_a4_commitqc_attestations_checked"], ["f37"]),
    m("MA3", "att_preimage omits R", ["det_a3_attestation_binds_result"], []),
    m("MA4", "att_preimage omits h", ["golden_attestation_preimage"], []),
    m("MA5", "try_commit: a node without authority Commit-votes without an attestation",
      ["det_a5_no_authority_abstains_from_commit_only"], ["f37"]),
    m("MA6", "vote_preimage omits the flag", ["det_a6_flag_is_signed"], ["f37"]),
    m("MA7", "propose_fresh: the builder's flag is dropped",
      ["det_a1_flagged_block_commits_with_attestations"], ["f37"]),
    m("MA8", "on_proposal step 6: the unflagged-EMPTY rule from empty_after_views deleted",
      ["det_a7_empty_after_views_never_flagged"], []),
    m("MA9", "restore_round: the recorded Prepare is rebuilt unflagged",
      ["det_a8_restart_resends_identical_attested_votes"], []),
    m("MA10", "try_commit: an attestation the node's own verifier rejects is used anyway",
      ["det_a5_no_authority_abstains_from_commit_only"], ["f37"]),
    m("MA11", "verify_attestations: a flagged CommitQC with more than q signers accepted",
      ["det_a4_flagged_commitqc_has_exactly_q_signers", "det_a4_commitqc_attestations_checked"],
      ["f37"]),
    m("MA12", "on_outcome: no try_commit after the lock's block executes (Pending attestor)",
      ["det_a9_pending_attestor_commits_after_execution"], []),
    # ---- as-built rules of Appendix E (E1-E7) and their regression tests
    m("ME1", "on_status: rate-limited Status drops its fresh CommitQC (E1)",
      ["rate_limited_status_still_delivers_a_fresh_commit_qc"], ["f03"]),
    m("ME2", "restore_round: record inconsistent with the store not halted (E2)",
      ["resume_rejects_a_record_inconsistent_with_the_store"], ["f24"]),
    m("ME3", "sync: buffered entries above a gap kept (E3)",
      ["sync_refetches_a_gap_left_by_a_dropped_forged_prefix"], ["f17"]),
    m("ME4", "on_payload_ready: readiness during an outstanding build dropped (E4)",
      ["payload_ready_during_an_outstanding_build_ends_the_idle_wait"], ["f02"]),
    m("ME5", "unsettled: stage-2 clause deleted (E5)",
      ["stage_two_makes_a_node_unsettled"], ["f03"]),
    m("ME6", "on_block_applied: anchoring re-check deleted (E6)",
      ["anchoring_is_checked_when_the_next_configuration_is_known"], ["f24"]),
    m("ME7", "prune_probe: probe table cleared while C_{t'+2} is unknown (E7)",
      ["det_s31e_echo_signature"], ["f24"]),
    # ---- revision-4 rules with det_r4 tests
    m("MR-block-applied", "on_block_applied: height/header/hash checks deleted",
      ["det_r4_block_applied_checks"], []),
    m("MR-tc-locks-only", "on_qc: a TC's high_pqc runs all of step 2",
      ["det_r4_tc_high_pqc_locks_only"], ["f07"]),
    m("MR-exec-budget", "exec_budget depends on the level",
      ["det_r4_exec_budget_level_independent"], ["f15"]),
    m("MR-stage-resend", "raise_stage: retransmit schedule not restarted",
      ["det_r4_stage_entry_resend_restarts_schedule"], ["f03"]),
    m("MR-unverified-target", "unsettled: an unverified sync target counts",
      ["det_r4_unverified_target_never_unsettles"], ["f18"]),
    m("MR-sync-late", "on_sync_response: only the outstanding peer is heard",
      ["det_r4_sync_responses_from_requested_sources"], ["f17"]),
    m("MR-restore-commit", "restore_round: final try_commit deleted",
      ["det_r4_restore_recommits_identical"], ["f13"]),
    m("MR-probe-status", "status_message: probe nonce not carried",
      ["det_r4_probe_statuses_and_echoes", "det_s31_deleted_record_abstains"], ["f24"]),
    m("MR-request-asked", "request_proposal: asked not set (asks every call)",
      ["det_r4_request_proposal_conditions"], ["f34"]),
    m("MR-repush-recipients", "repush_on_request: recipient check deleted",
      ["det_r4_repush_only_to_recipients_once"], []),
    m("MR-advance-prune", "advance_to: reported keys not pruned",
      ["det_r4_advance_to_prunes_blocks_and_reported"], ["f18"]),
    m("MR-window-init", "Core::new: W fixed at 128 instead of Init.demotion_window",
      ["det_r4_demotion_window_from_init"], ["f01"]),
    m("MR-cert-lru", "cert cache: a hit is not refreshed (FIFO, not LRU)",
      ["det_r4_cert_cache_lru_cleared_on_entry"], []),
    m("MR-monitor-prev", "monitor: the tip.height - 1 branch deleted",
      ["det_r4_monitor_previous_height"], []),
    m("MR-fetch-cycle", "fetch: sources not cycled",
      ["det_r4_fetch_sources_cycle"], ["f26"]),
    m("MR-restart-leader", "restore_round: leader rules (propose) deleted",
      ["det_r4_restart_leader_rules"], ["f14"]),
    m("MR-nested-pqc", "on_timeout: nested PrepareQC cheap-rejected like a top-level one",
      ["det_r4_nested_pqc_timeout_counted", "det_s7_lock_monotone"], ["f07"]),
    m("MR-fresh-nonce", "fake driver: Init.nonce not fresh per start",
      ["det_r4_fresh_nonce_per_init"], ["f24"]),
]

BY_ID = {mu.id: mu for mu in MUTATIONS}

TEST_LINE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)", re.M)


@dataclass
class Step:
    status: str  # pass | fail | build-error | timeout | missing-test | skipped
    seconds: float = 0.0
    failed: list = field(default_factory=list)
    ran: list = field(default_factory=list)
    detail: list = field(default_factory=list)
    log: str = ""


def cargo_test(args, target_dir, mutation, filters, seeds, timeout, log_path, no_run=False):
    """Run `cargo test` for the crate; returns (returncode or None on timeout, output)."""
    env = dict(os.environ)
    env.pop("SUMERAGI_MUTATION", None)
    env.pop("SUMERAGI_SIM_SEED", None)
    if mutation:
        env["SUMERAGI_MUTATION"] = mutation
    if seeds is not None:
        env["SUMERAGI_SIM_SEEDS"] = str(seeds)
    else:
        env.pop("SUMERAGI_SIM_SEEDS", None)
    env["CARGO_TARGET_DIR"] = str(target_dir)
    cmd = ["cargo", "test", "-p", CRATE, "--release", "--features", FEATURES, "--lib"]
    if no_run:
        cmd.append("--no-run")
    else:
        cmd += ["--"] + list(filters)
    started = time.monotonic()
    # A new session: on timeout only this command's own process group is killed.
    proc = subprocess.Popen(cmd, cwd=REPO, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, start_new_session=True)
    try:
        out, _ = proc.communicate(timeout=timeout)
        code = proc.returncode
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
        out, _ = proc.communicate()
        code = None
    elapsed = time.monotonic() - started
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with open(log_path, "w") as f:
        f.write(f"$ {' '.join(cmd)}\n# SUMERAGI_MUTATION={mutation or ''} "
                f"SUMERAGI_SIM_SEEDS={seeds or ''} CARGO_TARGET_DIR={target_dir}\n")
        f.write(out or "")
        f.write(f"\n# exit {code} after {elapsed:.1f}s\n")
    return code, out or "", elapsed


def run_step(args, target_dir, mutation, filters, seeds, timeout, log_path):
    code, out, elapsed = cargo_test(args, target_dir, mutation, filters, seeds, timeout, log_path)
    results = TEST_LINE.findall(out)
    ran = sorted({name for name, _ in results})
    failed = sorted({name for name, verdict in results if verdict == "FAILED"})
    step = Step(status="pass", seconds=round(elapsed, 1), failed=failed, ran=ran,
                log=str(log_path))
    missing = [flt for flt in filters if not any(flt in name for name in ran)]
    if code is None:
        step.status = "timeout"
        step.detail.append(f"timed out after {timeout}s")
    elif "error[E" in out or "could not compile" in out:
        step.status = "build-error"
        step.detail = [line for line in out.splitlines() if line.startswith("error")][:5]
    elif failed or code != 0:
        step.status = "fail"
        seeds_failed = re.findall(r"failing seeds (\[[^\]]*\])", out)
        violations = re.findall(r"violation: (.*)", out)
        panics = re.findall(r"panicked at [^\n]*\n([^\n]*)", out)
        step.detail = (seeds_failed[:3] + violations[:3] + panics[:3])[:6]
        if not failed:
            step.detail.insert(0, f"cargo exited {code} without a failed test line")
    if missing and step.status in ("pass", "fail"):
        step.detail.append(f"no test matched: {missing}")
        if step.status == "pass":
            step.status = "missing-test"
    return step


def build(args, target_dir, mutation, log_path):
    code, out, elapsed = cargo_test(args, target_dir, mutation, [], None, args.timeout_build,
                                    log_path, no_run=True)
    ok = code == 0
    detail = [] if ok else ([line for line in out.splitlines() if line.startswith("error")][:5]
                            or ["timeout" if code is None else f"cargo exited {code}"])
    return Step(status="pass" if ok else "build-error", seconds=round(elapsed, 1),
                detail=detail, log=str(log_path))


def has_switch(mid):
    needle = f'sumeragi_mutation = "{mid}"'
    return any(needle in p.read_text() for p in CRATE_SRC.rglob("*.rs"))


def evaluate(args, target_dir, mu):
    logs = args.target_dir / "logs"
    result = {"id": mu.id, "site": mu.site, "named_tests": list(mu.tests),
              "scenarios": [SCENARIOS[s] for s in mu.scenarios]}
    started = time.monotonic()
    if not has_switch(mu.id):
        result.update(verdict="error", reason="no cfg switch for this id in the crate")
        return result
    b = build(args, target_dir, mu.id, logs / f"{mu.id}.build.log")
    result["build"] = b.__dict__
    if b.status != "pass":
        result.update(verdict="error", reason="mutant does not build")
        return result
    named = run_step(args, target_dir, mu.id, mu.tests, None, args.timeout_test,
                     logs / f"{mu.id}.named.log")
    result["named"] = named.__dict__
    killed_by_test = named.status in ("fail", "timeout")
    scen = None
    if mu.scenarios and not args.fast:
        filters = [SCENARIOS[s] for s in mu.scenarios]
        scen = run_step(args, target_dir, mu.id, filters, args.seeds, args.timeout_scenario,
                        logs / f"{mu.id}.scenario.log")
        result["scenario"] = scen.__dict__
    killed_by_scenario = scen is not None and scen.status in ("fail", "timeout")
    if named.status in ("build-error", "missing-test"):
        result.update(verdict="error", reason=f"named tests: {named.status}")
    elif killed_by_test:
        result["verdict"] = "killed_by_test"
    elif killed_by_scenario:
        result["verdict"] = "killed_by_scenario_only"
    else:
        result["verdict"] = "survived"
    result["seconds"] = round(time.monotonic() - started, 1)
    return result


def evaluate_baseline(args, target_dir, mutations):
    logs = args.target_dir / "logs"
    result = {"id": "baseline"}
    b = build(args, target_dir, None, logs / "baseline.build.log")
    result["build"] = b.__dict__
    if b.status != "pass":
        result["verdict"] = "error"
        return result
    tests = sorted({t for mu in mutations for t in mu.tests})
    named = run_step(args, target_dir, None, tests, None, args.timeout_test * 2,
                     logs / "baseline.named.log")
    result["named"] = named.__dict__
    ok = named.status == "pass"
    scenarios = sorted({s for mu in mutations for s in mu.scenarios})
    if scenarios and not args.fast:
        filters = [SCENARIOS[s] for s in scenarios]
        scen = run_step(args, target_dir, None, filters, args.seeds,
                        args.timeout_scenario * 3, logs / "baseline.scenario.log")
        result["scenario"] = scen.__dict__
        ok = ok and scen.status == "pass"
    result["verdict"] = "pass" if ok else "fail"
    return result


def main():
    parser = argparse.ArgumentParser(
        description="Sumeragi §13.4 mutation gate: every mutation must be killed by its named "
                    "deterministic test (then by its randomized scenario); the unmutated build "
                    "must pass.",
        formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__)
    parser.add_argument("--only", help="comma-separated mutation ids (default: all)")
    parser.add_argument("--jobs", type=int, default=1,
                        help="parallel jobs, each with its own target sub-directory")
    parser.add_argument("--fast", action="store_true",
                        help="named deterministic tests only (skip the randomized scenarios)")
    parser.add_argument("--seeds", type=int, default=200,
                        help="SUMERAGI_SIM_SEEDS for the scenarios (default 200)")
    parser.add_argument("--target-dir", type=Path, default=REPO / "target" / "sumeragi-mutants",
                        help="dedicated CARGO_TARGET_DIR root (default: target/sumeragi-mutants)")
    parser.add_argument("--skip-baseline", action="store_true",
                        help="do not run the unmutated build")
    parser.add_argument("--strict", action="store_true",
                        help="also fail when a mutation is killed only by its scenario "
                             "(the literal §13.4 CI rule)")
    parser.add_argument("--timeout-build", type=int, default=1800, help="seconds per build")
    parser.add_argument("--timeout-test", type=int, default=900,
                        help="seconds per named-test run")
    parser.add_argument("--timeout-scenario", type=int, default=3600,
                        help="seconds per scenario run")
    parser.add_argument("--list", action="store_true", help="print the mutation table and exit")
    args = parser.parse_args()
    args.target_dir = args.target_dir.resolve()

    if args.list:
        for mu in MUTATIONS:
            print(f"{mu.id:22} {', '.join(mu.tests):60} {' '.join(mu.scenarios) or '-':10} "
                  f"{mu.site}")
        return 0

    selected = MUTATIONS
    if args.only:
        wanted = [x.strip() for x in args.only.split(",") if x.strip()]
        unknown = [x for x in wanted if x not in BY_ID]
        if unknown:
            parser.error(f"unknown mutation id(s): {', '.join(unknown)}")
        selected = [BY_ID[x] for x in wanted]
    args.target_dir.mkdir(parents=True, exist_ok=True)

    work = queue.Queue()
    if not args.skip_baseline:
        work.put(("baseline", None))
    for mu in selected:
        work.put(("mutation", mu))
    results = {}
    lock = threading.Lock()
    total = len(selected) + (0 if args.skip_baseline else 1)
    done = [0]

    def worker(index):
        target_dir = args.target_dir / f"job{index}"
        while True:
            try:
                kind, mu = work.get_nowait()
            except queue.Empty:
                return
            if kind == "baseline":
                res = evaluate_baseline(args, target_dir, selected)
            else:
                res = evaluate(args, target_dir, mu)
            with lock:
                results[res["id"]] = res
                done[0] += 1
                extra = res.get("reason", "")
                failed = res.get("named", {}).get("failed", [])
                print(f"[{done[0]}/{total}] {res['id']}: {res['verdict']} {extra} "
                      f"{' '.join(t.split('::')[-1] for t in failed)}", flush=True)

    threads = [threading.Thread(target=worker, args=(i,), daemon=True)
               for i in range(max(1, args.jobs))]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    verdicts = {}
    for mu in selected:
        verdicts.setdefault(results[mu.id]["verdict"], []).append(mu.id)
    baseline = results.get("baseline")
    summary = {
        "baseline": baseline["verdict"] if baseline else "skipped",
        "mutations": len(selected),
        "killed_by_test": verdicts.get("killed_by_test", []),
        "killed_by_scenario_only": verdicts.get("killed_by_scenario_only", []),
        "survived": verdicts.get("survived", []),
        "error": verdicts.get("error", []),
        "scenario_missed": [mu.id for mu in selected
                            if results[mu.id].get("scenario", {}).get("status") == "pass"],
    }
    report = {
        "generated": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "command": sys.argv,
        "seeds": None if args.fast else args.seeds,
        "fast": args.fast,
        "summary": summary,
        "baseline": baseline,
        "mutations": [results[mu.id] for mu in selected],
    }
    report_path = args.target_dir / "report.json"
    report_path.write_text(json.dumps(report, indent=1))
    print(json.dumps(summary, indent=1))
    print(f"report: {report_path}")
    failed = (summary["baseline"] == "fail" or summary["survived"] or summary["error"]
              or (args.strict and summary["killed_by_scenario_only"]))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
