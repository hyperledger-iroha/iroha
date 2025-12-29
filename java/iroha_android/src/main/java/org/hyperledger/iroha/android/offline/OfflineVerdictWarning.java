package org.hyperledger.iroha.android.offline;

/** Describes an impending or elapsed verdict deadline for UI consumption. */
public final class OfflineVerdictWarning {

  public enum DeadlineKind {
    REFRESH,
    POLICY,
    CERTIFICATE
  }

  public enum State {
    WARNING,
    EXPIRED
  }

  private final String certificateIdHex;
  private final String controllerId;
  private final String controllerDisplay;
  private final String verdictIdHex;
  private final DeadlineKind deadlineKind;
  private final long deadlineMs;
  private final long millisecondsRemaining;
  private final State state;
  private final String headline;
  private final String details;

  public OfflineVerdictWarning(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String verdictIdHex,
      final DeadlineKind deadlineKind,
      final long deadlineMs,
      final long millisecondsRemaining,
      final State state,
      final String headline,
      final String details) {
    this.certificateIdHex = certificateIdHex;
    this.controllerId = controllerId;
    this.controllerDisplay = controllerDisplay;
    this.verdictIdHex = verdictIdHex;
    this.deadlineKind = deadlineKind;
    this.deadlineMs = deadlineMs;
    this.millisecondsRemaining = millisecondsRemaining;
    this.state = state;
    this.headline = headline;
    this.details = details;
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }

  public String controllerId() {
    return controllerId;
  }

  public String controllerDisplay() {
    return controllerDisplay;
  }

  public String verdictIdHex() {
    return verdictIdHex;
  }

  public DeadlineKind deadlineKind() {
    return deadlineKind;
  }

  public long deadlineMs() {
    return deadlineMs;
  }

  public long millisecondsRemaining() {
    return millisecondsRemaining;
  }

  public State state() {
    return state;
  }

  public String headline() {
    return headline;
  }

  public String details() {
    return details;
  }
}
