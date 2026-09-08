import {
  buildParliamentTransitionDraftRequestV1,
  type ParliamentLifecycleTransitionV1,
  type ParliamentPublicTransitionTagV1,
} from "../../../index.js";

const initialTag: ParliamentPublicTransitionTagV1 = "RegisterInitialSortition";
const initial: ParliamentLifecycleTransitionV1 = { transition: initialTag };
const complete: ParliamentLifecycleTransitionV1 = { transition: "CompleteQualification" };
const regular: ParliamentLifecycleTransitionV1 = {
  transition: "FailPublicFindingNoResult",
  payload: { body_instance_id: "01".repeat(32) },
};
buildParliamentTransitionDraftRequestV1("ab".repeat(32), initial);
buildParliamentTransitionDraftRequestV1("ab".repeat(32), complete);
buildParliamentTransitionDraftRequestV1("ab".repeat(32), regular);

const nullPayload: ParliamentLifecycleTransitionV1 = {
  transition: "RegisterInitialSortition",
  // @ts-expect-error the public initial intent has no payload field, including null.
  payload: null,
};
const objectPayload: ParliamentLifecycleTransitionV1 = {
  transition: "RegisterInitialSortition",
  // @ts-expect-error the caller cannot supply an initial sortition payload.
  payload: { target_seats: 1 },
};
const callerSelection: ParliamentLifecycleTransitionV1 = {
  transition: "RegisterInitialSortition",
  // @ts-expect-error initial seat counts are derived by consensus.
  target_seats: 1,
};
// @ts-expect-error existing non-unit transitions still require their payload.
const missingRegularPayload: ParliamentLifecycleTransitionV1 = {
  transition: "FailPublicFindingNoResult",
};
void nullPayload;
void objectPayload;
void callerSelection;
void missingRegularPayload;
