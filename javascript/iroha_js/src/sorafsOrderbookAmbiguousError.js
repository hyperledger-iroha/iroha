/** A submitted SoraFS orderbook transaction requires identity-based reconciliation. */
export class SorafsOrderbookSubmissionAmbiguousError extends Error {
  constructor(route, identity, cause) {
    super(
      "SoraFS orderbook submission outcome is ambiguous after dispatch; "
      + "do not resubmit automatically, reconcile the expected transaction identity",
      cause === undefined ? undefined : { cause },
    );
    this.name = "SorafsOrderbookSubmissionAmbiguousError";
    Object.defineProperties(this, {
      route: { value: route, enumerable: true },
      expectedIdentity: {
        value: Object.freeze({ ...identity }),
        enumerable: true,
      },
    });
  }
}
