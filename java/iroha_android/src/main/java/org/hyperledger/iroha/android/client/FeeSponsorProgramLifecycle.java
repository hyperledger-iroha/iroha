package org.hyperledger.iroha.android.client;

/** Consensus-visible lifecycle of one exact fee sponsor program. */
public enum FeeSponsorProgramLifecycle {
  STAGED,
  PAUSED,
  ACTIVE,
  CLOSING,
  CLOSED
}
