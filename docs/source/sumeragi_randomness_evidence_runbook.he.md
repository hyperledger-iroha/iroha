---
lang: he
direction: rtl
source: docs/source/sumeragi_randomness_evidence_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9a7b2f030cb798b78947c0d7cb298ccbd8a94a006be2e804f1ed043dc15dabc
source_last_modified: "2025-11-15T08:00:21.780712+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל docs/source/sumeragi_randomness_evidence_runbook.md -->

# Runbook אקראיות ו-evidence של Sumeragi

מדריך זה עומד בפריט Milestone A6 ברודמאפ, שדרש נהלי מפעילים מעודכנים עבור
אקראיות VRF ו-evidence של slashing. השתמשו בו יחד עם {doc}`sumeragi` ו-
{doc}`sumeragi_chaos_performance_runbook` בכל פעם שמרימים גרסת ולידטור חדשה
או אוספים artifacts של readiness עבור governance.


Note: For the v1 release, VRF penalties jail offenders after the activation lag, and consensus slashing is delayed by `sumeragi.npos.reconfig.slashing_delay_blocks` (default 259200 blocks, ~3 days at 1s) so governance can cancel with `CancelConsensusEvidencePenalty` before it applies.

## היקף ודרישות מקדימות

- `iroha_cli` מוגדר לקלאסטר היעד (ראו `docs/source/cli.md`).
- `curl`/`jq` לשליפת payload `/status` של Torii בעת הכנת קלטים.
- גישה ל-Prometheus (או snapshot exports) עבור מדדי `sumeragi_vrf_*`.
- מודעות ל-epoch ול-roster הנוכחיים כדי להתאים את פלט ה-CLI ל-snapshot ה-staking
  או ל-manifest של governance.

## 1. אימות בחירת מצב והקשר epoch

1. הריצו `iroha --output-format text ops sumeragi params` כדי לוודא שהבינארי טען
   `sumeragi.consensus_mode="npos"` ולתעד `k_aggregators`,
   `redundant_send_r`, אורך epoch והיסטי VRF commit/reveal.
2. בדקו את תצוגת ה-runtime:

   ```bash
   iroha --output-format text ops sumeragi status
   iroha --output-format text ops sumeragi collectors
   iroha --output-format text ops sumeragi rbc status
   ```

   שורת `status` מדפיסה tuple leader/view, backlog של RBC, נסיונות DA,
   היסטי epoch ו-pacemaker deferrals; `collectors` ממפה אינדקסי collectors
   ל-IDs של peers כדי להראות מי מהוולידטורים נושא את מטלות האקראיות בגובה הנבדק.
3. תעדו את מספר ה-epoch שברצונכם לבדוק:

   ```bash
   EPOCH=$(curl -s "$TORII/status" | jq '.sumeragi.epoch.height // 0')
   printf "auditing epoch %s\n" "$EPOCH"
   ```

   שמרו את הערך (decimal או עם prefix `0x`) לפקודות ה-VRF בהמשך.

## 2. Snapshot של epochs VRF ועונשים

השתמשו בתתי-הפקודות היעודיות של ה-CLI כדי לשלוף את רשומות ה-VRF השמורות מכל
ולידטור:

```bash
iroha --output-format text ops sumeragi vrf-epoch --epoch "$EPOCH"
iroha ops sumeragi vrf-epoch --epoch "$EPOCH" > artifacts/vrf_epoch_${EPOCH}.json

iroha --output-format text ops sumeragi vrf-penalties --epoch "$EPOCH"
iroha ops sumeragi vrf-penalties --epoch "$EPOCH" > artifacts/vrf_penalties_${EPOCH}.json
```

הסיכומים מציגים האם ה-epoch סופי, כמה משתתפים שלחו commits/reveals, את אורך
ה-roster ואת ה-seed הנגזר. ה-JSON כולל רשימת משתתפים, סטטוס ענישה לפי signer
וערך `seed_hex` שמשמש את ה-pacemaker. השוו את מספר המשתתפים ל-roster של staking
ואמתו שמערכי הענישה משקפים את ההתראות שהופעלו בבדיקות chaos (late reveals תחת
`late_reveals`, ולידטורים forfeited תחת `no_participation`).

## 3. ניטור telemetria של VRF והתרעות

Prometheus חושף את המונים הנדרשים ברודמאפ:

- `sumeragi_vrf_commits_emitted_total`
- `sumeragi_vrf_reveals_emitted_total`
- `sumeragi_vrf_reveals_late_total`
- `sumeragi_vrf_non_reveal_penalties_total`
- `sumeragi_vrf_non_reveal_by_signer{signer="peer_id"}`
- `sumeragi_vrf_no_participation_total`
- `sumeragi_vrf_no_participation_by_signer{signer="peer_id"}`
- `sumeragi_vrf_rejects_total_by_reason{reason="..."}`

דוגמת PromQL לדוח שבועי:

```promql
increase(sumeragi_vrf_non_reveal_by_signer[1w]) > 0
```

במהלך readiness drills ודאו כי:

- `sumeragi_vrf_commits_emitted_total` ו-`..._reveals_emitted_total` עולים
  עבור כל בלוק בתוך חלונות commit/reveal.
- תרחישי late-reveal מפעילים `sumeragi_vrf_reveals_late_total` ומנקים את
  הרשומה המתאימה ב-JSON `vrf_penalties`.
- `sumeragi_vrf_no_participation_total` מזנק רק כאשר אתם מחזיקים commits
  בכוונה במהלך בדיקות chaos.

לוח Grafana (`docs/source/grafana_sumeragi_overview.json`) כולל פאנלים לכל מונה;
צלמו מסכים אחרי כל ריצה וצרפו אותם לחבילת artifacts המוזכרת ב-
{doc}`sumeragi_chaos_performance_runbook`.

## 4. קליטת evidence ו-streaming

Evidence של slashing חייבים להיאסף בכל ולידטור ולהישלח ל-Torii. השתמשו ב-CLI
helpers כדי להראות התאמה ל-HTTP endpoints המתועדים ב-
{doc}`torii/sumeragi_evidence_app_api`:

```bash
# Count and list persisted evidence
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5

# Show JSON for audits
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

ודאו שה-`total` המדווח תואם ל-widget Grafana שמוזן מ-
`sumeragi_evidence_records_total`, ואשרו שרשומות ישנות מ-
`sumeragi.npos.reconfig.evidence_horizon_blocks` נדחות (ה-CLI מדפיס את סיבת
הנפילה). בעת בדיקות alerting, שלחו payload מוכר ותקין באמצעות:

```bash
iroha --output-format text ops sumeragi evidence submit --evidence-hex-file fixtures/evidence/double_prevote.hex
```

נטרו `/v2/events/sse` באמצעות stream מסונן כדי להוכיח שה-SDKs רואים את אותם
הנתונים: השתמשו ב-one-liner של Python מ-{doc}`torii/sumeragi_evidence_app_api`
כדי לבנות את הפילטר וללכוד frames גולמיים של `data:`. ה-payloads של SSE צריכים
להדהד את סוג ה-evidence ואת ה-signer שהופיע בפלט ה-CLI.

## 5. אריזה ודיווח evidence

בכל rehearsal או release candidate:

1. שמרו את קבצי ה-CLI JSON (`vrf_epoch_*.json`, `vrf_penalties_*.json`,
   `evidence_snapshot.json`) תחת ספריית artifacts של הריצה (אותו root שמשמש
   את סקריפטי chaos/performance).
2. רשמו את תוצאות Prometheus או snapshot exports עבור המונים המופיעים לעיל.
3. צרפו את לכידת ה-SSE ואת אישורי ההתראות ל-README של artifacts.
4. עדכנו את `status.md` ואת
   `docs/source/project_tracker/npos_sumeragi_phase_a.md` עם נתיבי artifacts
   ומספר ה-epoch שנבדק.

שמירה על צ'ק ליסט זה הופכת את הוכחות האקראיות VRF ו-evidence של slashing
לברות בדיקה במהלך rollout ה-NPoS ונותנת למבקרי governance נתיב דטרמיניסטי
חזרה אל המדדים וה-snapshots של ה-CLI.

## 6. אותות troubleshooting

- **Mode selection mismatch** — אם `iroha --output-format text ops sumeragi params` מציג
  `consensus_mode="permissioned"` או `k_aggregators` שונה מה-manifest,
  מחקו את artifacts שנלכדו, תקנו את `iroha_config`, אתחלו את הוולידטור,
  והריצו מחדש את זרימת האימות המתוארת ב-{doc}`sumeragi`.
- **Missing commits or reveals** — סדרה שטוחה של `sumeragi_vrf_commits_emitted_total`
  או `sumeragi_vrf_reveals_emitted_total` משמעה ש-Torii לא מעביר frames של VRF.
  בדקו את לוגי הוולידטור עבור שגיאות `handle_vrf_*`, ואז שלחו את ה-payload ידנית
  דרך helpers ה-POST המתועדים לעיל.
- **Unexpected penalties** — כאשר `sumeragi_vrf_no_participation_total` קופץ,
  בדקו את הקובץ `vrf_penalties_<epoch>.json` כדי לאשר את ה-signer ID והשוו אותו
  ל-roster של staking. עונשים שלא תואמים ל-chaos drills מעידים על clock skew
  של הוולידטור או על replay protection של Torii; תקנו את ה-peer הבעייתי לפני
  הרצה חוזרת של הבדיקה.
- **Evidence ingestion stalls** — כאשר `sumeragi_evidence_records_total` נתקע
  בזמן ש-chaos tests פולטים תקלות, הריצו `iroha ops sumeragi evidence count`
  על מספר ולידטורים ואשרו ש-`/v2/sumeragi/evidence/count` תואם ל-CLI. כל סטיה
  פירושה שצרכני SSE/webhook עשויים להיות stale, לכן שלחו מחדש fixture מוכר
  והסלימו לצוות Torii אם המונה לא גדל.

</div>
