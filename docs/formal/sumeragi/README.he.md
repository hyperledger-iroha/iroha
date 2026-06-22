<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi דגם רשמי (TLA+ / Apalache)

ספרייה זו מכילה דגמים רשמיים מוגבלים לבטיחות וחיות Sumeragi.

## היקף

`Sumeragi.tla` לוכד את נתיב ההתחייבות:
- התקדמות שלב (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ספי הצבעה ומניין (`CommitQuorum`, `ViewQuorum`),
- מניין יתד משוקלל (`StakeQuorum`) לשומרי מחויבות בסגנון NPoS,
- סיבתיות RBC (`Init -> Chunk -> Ready -> Deliver`) עם עדות כותרת/עיכול,
- GST והנחות הוגנות חלשות על פני פעולות התקדמות כנות.

`SumeragiFrontierRecovery.tla` לוכדת את שיעור התלייה הממוקד של Taira סביב אחד
בלוק גבול רציף ממתין:
- הוכחה להצבעה מתחת או במניין,
- צבר תור ההצבעה וניקוז מקומי,
- חסר לעומת מצב מטען מקומי,
- בעלות חדשה לעומת מיושנת לשחזור גבול,
- סמן מחדש של המניין/קצב חלון,
- ראיות עתידיות/ראיות חדשות שיכולות לעגן מחדש את הגבול המקומי,
- התחייבות לאחר GST דטרמיניסטית, שידור חוזר, סיבוב תצוגה מוגבל, ו
  תוצאות ירידה של אפס ראיות.

שני הדגמים מופשטים בכוונה פורמטים של חוטים, ECDSA/חתימה
אימות, ופרטי רשת מלאים.

## קבצים- `Sumeragi.tla`: דגם פרוטוקול ומאפיינים.
- `Sumeragi_fast.cfg`: ערכת פרמטרים קטנה יותר ידידותית ל-CI.
- `Sumeragi_deep.cfg`: סט פרמטרי מתח גדול יותר.
- `SumeragiFrontierRecovery.tla`: מודל שחזור גבולות ממוקד.
- `SumeragiFrontierRecovery_fast.cfg`: ערכת פרמטרים קטנה יותר ידידותית ל-CI.
- `SumeragiFrontierRecovery_deep.cfg`: ערכת צבר גבול/חלון/תצוגה גדול יותר.
- `SumeragiFrontierRecovery_wide.cfg`: סט כרוך גבולות רחב יותר.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: מוטציה צפויה לכשל בעלים.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: מוטציית תור הצבעה צפויה להיכשל.

## מאפיינים

אינוריאנטים:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

נכס זמני:
- `EventuallyCommit` (`[] (gst => <> committed)`), עם קידוד הגינות לאחר ה-GST
  באופן תפעולי ב-`Next` (משגי פסק זמן/מניעת תקלות מופעלים
  פעולות התקדמות). זה שומר על בדיקת הדגם עם Apalache 0.52.x, אשר
  אינו תומך במפעילי הגינות `WF_` בתוך מאפיינים זמניים מסומנים.

אינוריאנטי התאוששות בגבולות:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, שפוסל טרמינל
  מצב לאחר GST שבו ל-`pending /\ voteBacked /\ ~committed` אין התאוששות,
  ביצוע, שידור חוזר, סיבוב או מעבר מוגבל-ירידה.רכוש זמני לשחזור גבול:
- `PostGstVoteBackedFrontierEventuallyResolves`: אחרי GST, כל לא פתור
  מדינת הגבול הממתינה בתמיכת הצבעה מגיעה בסופו של דבר להתחייבות, מטען
  התאוששות, שידור חוזר של המניין, הנחת גבול עתידית או ראייה מוגבלת
  סיבוב.
- `RecoveredPayloadEventuallyAdvances`: מדינת גבול עם גיבוי קולות שיש
  התאושש המטען לא יכול להישאר בהמתנה לנצח ללא התחייבות,
  שידור חוזר, עיגון מחדש או סיבוב.
- `QuorumRetransmitEventuallyLeavesPending`: ברגע שהשידור החוזר המניין בוצע
  עבור מדינת גבול הנתמכת בהצבעה, העטיפה הממתינה חייבת בסופו של דבר להתנקות.
- `FutureFrontierEvidenceEventuallyReanchors`: ראיות מאוחרות יותר על גבול/נוף חדש
  חייב לנקות את העטיפה הממתינה או להיות נצרך כמעגן גבול.

## מפת ההנחה

מודל הגבול הוא סופי בכוונה. אלו הם היישום
משטחים זה מופשט:| קונספט דגם | משטח יישום |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | טיפול ב-`PendingBlock` ובדיקות מטען מקומיות ב-`crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, בתוספת התממשות בעלות על גבולה ב-`proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | ספירת קולות התחייבות וכניסת קולות המופעלים על ידי `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` ו-`reschedule_ignores_quorum_timeout_vote_queue_backlog` ב-`crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | מצב בעל גבול פעיל/מיושן ב-`frontier_slot_has_active_owner_state_for_view(...)`, תשואה-בעלים מיושן ב-`maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`, ומחליף את הניקוי ב-`drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | קצב תזמון מחדש של המניין המגובה בהצבעות ב-`reschedule_stale_pending_blocks_with_now(...)`; כיסוי הרגרסיה כולל `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | תיקון פחחות מדוייק וכניסה לתיקון RBC מעופש ב-`request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` ו-`stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | הקוורום משדר מחדש בחירת יעד, `rebroadcast_pending_block_updates(...)`, וקריאות שינוי צפייה דטרמיניסטיות ב-`reschedule_stale_pending_blocks_with_now(...)`. |
| `futureFrontierEvidence` | ראיות עתידיות למניין מניין חדש / גבול גבוה יותר ב-`on_pacemaker_propose_ready(...)`, מכוסה על ידי `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## ריצה

משורש המאגר:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

הרץ מגדיר Apalache מפורש `--length` עבור כל מצב:| מצב | אורך | שימוש מיועד |
| --- | ---: | --- |
| `fast` | 10 | בדיקת נתיב CI |
| `deep` | 10 | בדיקת נתיב התחייבות גדולה יותר |
| `frontier-fast` | 10 | בדיקת גבול CI |
| `frontier-deep` | 12 | בדיקת גבול גדולה יותר |
| `frontier-wide` | 14 | בדיקת לחץ ידנית/לילית |

`APALACHE_LENGTH=<n>` עוקף את ברירת המחדל לכל מצב בעת חקירה מקומית של
דוגמה נגדית או הרחבת הוכחה מוגבלת.

### הגדרה מקומית ניתנת לשחזור (אין צורך ב-Docker)

התקן את שרשרת הכלים המקומית המוצמדת של Apalache המשמשת את המאגר הזה:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

הרץ מזהה אוטומטית את ההתקנה הזו ב:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
לאחר ההתקנה, `ci/check_sumeragi_formal.sh` אמור לעבוד ללא וריאציות מיותרות:

```bash
bash ci/check_sumeragi_formal.sh
```

המוטציות הצפוי-כישלון נמצאות בכוונה מחוץ ל-CI רגיל. הם צריכים
נכשלים תחת Apalache והם שימושיים בעת שינוי המודל:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

אם Apalache אינו ב-`PATH`, אתה יכול:

- הגדר את `APALACHE_BIN` לנתיב ההפעלה, או
- השתמש ב-Docker (מופעל כברירת מחדל כאשר `docker` זמין):
  - תמונה: `APALACHE_DOCKER_IMAGE` (ברירת מחדל `ghcr.io/apalache-mc/apalache:0.52.2`)
  - דורש דמון Docker פועל
  - השבת את החזרה עם `APALACHE_ALLOW_DOCKER=0`.

דוגמאות:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## הערות- דגם זה משלים (לא מחליף) בדיקות מודל Rust הניתנות להפעלה ב
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  ו
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ההמחאות מוגבלות לערכים קבועים בקבצי `.cfg`.
- PR CI מפעיל את הבדיקות הללו ב-`.github/workflows/pr.yml` באמצעות
  `ci/check_sumeragi_formal.sh`.
