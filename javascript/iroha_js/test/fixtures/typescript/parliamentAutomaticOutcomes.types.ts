import {
  PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1,
  type ParliamentAutomaticExecutionOutcomeLayoutV1,
} from "../../../index.js";

type EventKindIndex = ParliamentAutomaticExecutionOutcomeLayoutV1["eventKindIndex"];

// Core's audit kinds number MarkEnacted, MarkSuperseded and MarkExecutionFailed 17-19.
const coreKinds: ReadonlyArray<EventKindIndex> = [17, 18, 19];
const declared: 17 | 18 | 19 = PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1[0].eventKindIndex;
// @ts-expect-error the retired timed-OVN kinds still occupy 10-16 in Core's codec.
const renumbered: EventKindIndex = 10;
void coreKinds;
void declared;
void renumbered;
