---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/soranet/puzzle-service-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 357595875d699080d71e7fbf545087f20536f7a42dd13ac32d7bbcd927343d8c
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: puzzle-service-operations
lang: he
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
משקף `docs/source/soranet/puzzle_service_operations.md`. שמרו על שתי הגרסאות מסונכרנות עד שהדוקס הישנים יפרשו.
:::

# מדריך תפעול שירות הפאזלים

ה-daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) מנפיק
admission tickets מגובים Argon2 שמשקפים את מדיניות `pow.puzzle.*` של ה-relay
וכאשר מוגדר, מתווך ML-DSA admission tokens בשם edge relays.
הוא חושף חמישה HTTP endpoints:

- `GET /healthz` - בדיקת liveness.
- `GET /v1/puzzle/config` - מחזיר את פרמטרי ה-PoW/puzzle האפקטיביים שנמשכים
  מ-JSON של ה-relay (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - מנפיק טיקט Argon2; גוף JSON אופציונלי
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  מבקש TTL קצר יותר (נחתך לחלון המדיניות), קושר את הטיקט ל-transcript hash,
  ומחזיר טיקט חתום על ידי relay + fingerprint של החתימה כאשר מפתחות חתימה מוגדרים.
- `GET /v1/token/config` - כאשר `pow.token.enabled = true`, מחזיר את מדיניות
  admission-token הפעילה (issuer fingerprint, גבולות TTL/clock-skew, relay ID,
  וסט revocation ממוזג).
- `POST /v1/token/mint` - מנפיק ML-DSA admission token הקשור ל-resume hash שסופק;
  גוף הבקשה מקבל `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

הטיקטים שהשירות מייצר מאומתים בבדיקת האינטגרציה
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, שבודקת גם throttles של relay
במהלך תרחישי DoS נפחיים.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## הגדרת הנפקת tokens

הגדירו את שדות ה-JSON של ה-relay תחת `pow.token.*` (ראו
`tools/soranet-relay/deploy/config/relay.entry.json` כדוגמה) כדי לאפשר
ML-DSA tokens. לכל הפחות ספקו את המפתח הציבורי של ה-issuer ורשימת revocation
אופציונלית:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

שירות הפאזלים עושה reuse לערכים האלה ומרענן אוטומטית את קובץ ה-revocation בפורמט
Norito JSON בזמן ריצה. השתמשו ב-CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) כדי להנפיק ולבדוק
tokens offline, להוסיף רשומות `token_id_hex` לקובץ ה-revocation, ולבצע audit
ל-credentials קיימים לפני דחיפת updates לפרודקשן.

העבירו את המפתח הסודי של ה-issuer לשירות הפאזלים דרך CLI flags:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` זמין גם כאשר הסוד מנוהל בצנרת tooling חיצונית. ה-watcher של
קובץ ה-revocation שומר על `/v1/token/config` עדכני; תיאמו עדכונים עם הפקודה
`soranet-admission-token revoke` כדי למנוע state מיושן של revocation.

הגדירו `pow.signed_ticket_public_key_hex` ב-JSON של ה-relay כדי לפרסם את המפתח
הציבורי ML-DSA-44 שמאמת signed PoW tickets; `/v1/puzzle/config` מהדהד את המפתח
ואת fingerprint ה-BLAKE3 שלו (`signed_ticket_public_key_fingerprint_hex`) כדי
שה-clients יוכלו להצמיד את המאמת. signed tickets מאומתים מול relay ID ו-transcript
bindings ומשתפים את אותו מאגר revocation; טיקטים PoW גולמיים בגודל 74 בתים נשארים
תקפים כאשר ה-verifier של signed-ticket מוגדר. העבירו את הסוד של החותם דרך
`--signed-ticket-secret-hex` או `--signed-ticket-secret-path` בזמן הפעלת השירות;
ההפעלה דוחה keypairs שלא תואמים אם הסוד לא מאומת מול `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` מקבל `"signed": true` (ואופציונלית `"transcript_hash_hex"`) כדי
להחזיר טיקט חתום בקידוד Norito לצד ה-bytes של הטיקט הגולמי; התגובות כוללות
`signed_ticket_b64` ו-`signed_ticket_fingerprint_hex` כדי לעזור לעקוב אחרי
fingerprints של replay. בקשות עם `signed = true` נדחות אם הסוד של החותם לא מוגדר.

## Playbook לסבב מפתחות

1. **איסוף descriptor commit חדש.** Governance מפרסמת את relay descriptor commit
   ב-directory bundle. העתקו את מחרוזת ה-hex אל `handshake.descriptor_commit_hex`
   בתוך קונפיג ה-JSON של ה-relay המשותף לשירות הפאזלים.
2. **סקירת גבולות מדיניות הפאזל.** אשרו שערכי
   `pow.puzzle.{memory_kib,time_cost,lanes}` המעודכנים מיושרים עם תכנית ה-release.
   המפעילים צריכים לשמור על תצורת Argon2 דטרמיניסטית בין relays (מינימום 4 MiB זיכרון,
   1 <= lanes <= 16).
3. **הכנת restart.** טענו מחדש את יחידת systemd או את הקונטיינר לאחר ש-governance
   מודיעה על cutover של הרוטציה. השירות לא תומך ב-hot-reload; נדרש restart כדי
   לקלוט את descriptor commit החדש.
4. **אימות.** הנפיקו טיקט דרך `POST /v1/puzzle/mint` ואשרו ש-`difficulty` ו-`expires_at`
   תואמים למדיניות החדשה. דו"ח ה-soak (`docs/source/soranet/reports/pow_resilience.md`)
   מתעד גבולות latency צפויים לעיון. כאשר tokens מופעלים, משכו `/v1/token/config`
   כדי לוודא שה-issuer fingerprint המפורסם ומספר ה-revocation תואמים לצפוי.

## נוהל השבתת חירום

1. הגדירו `pow.puzzle.enabled = false` בתצורת ה-relay המשותפת. שמרו על
   `pow.required = true` אם טיקטי hashcash fallback חייבים להישאר חובה.
2. אופציונלית, אכפו רשומות `pow.emergency` כדי לדחות descriptors ישנים בזמן ששער
   Argon2 offline.
3. הפעילו מחדש גם את ה-relay וגם את שירות הפאזלים כדי להחיל את השינוי.
4. עקבו אחרי `soranet_handshake_pow_difficulty` כדי לוודא שהקושי יורד לערך hashcash
   הצפוי, ואמתו ש-`/v1/puzzle/config` מדווח `puzzle = null`.

## ניטור והתראות

- **Latency SLO:** עקבו אחרי `soranet_handshake_latency_seconds` ושמרו על P95
  מתחת ל-300 ms. ה-offsets של soak test מספקים נתוני כיול ל-guard throttles.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** השתמשו ב-`soranet_guard_capacity_report.py` עם metrics של relay
  כדי לכייל את cooldowns של `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` צריך להתאים לקושי שמוחזר
  מ-`/v1/puzzle/config`. סטיה מצביעה על תצורת relay מיושנת או restart כושל.
- **Token readiness:** התריעו אם `/v1/token/config` יורד ל-`enabled = false` באופן
  בלתי צפוי או אם `revocation_source` מדווח timestamps מיושנים. המפעילים צריכים
  לסובב את קובץ ה-revocation של Norito דרך ה-CLI בכל פעם שטוקן נגרע כדי לשמור
  על הדיוק של endpoint זה.
- **Service health:** בדקו `/healthz` בקצב liveness הרגיל והתריעו אם `/v1/puzzle/mint`
  מחזיר תגובות HTTP 500 (מצביע על mismatch בפרמטרי Argon2 או כשלי RNG). שגיאות
  token minting מופיעות דרך תגובות HTTP 4xx/5xx ב-`/v1/token/mint`; יש להתייחס
  לכשלונות חוזרים כמצב paging.

## תאימות ו-audit logging

Relays פולטים אירועי `handshake` מובנים שכוללים סיבות throttle ומשך cooldown.
ודאו שצינור ה-compliance המתואר ב-`docs/source/soranet/relay_audit_pipeline.md`
קולט את הלוגים הללו כדי ששינויים במדיניות הפאזל יישארו auditables. כאשר שער
הפאזל מופעל, ארכבו דגימות טיקטים שהונפקו ואת snapshot תצורת Norito יחד עם
כרטיס ה-rollout לצורך audits עתידיים. admission tokens שהונפקו לפני חלונות
תחזוקה צריכים להיות במעקב לפי ערכי `token_id_hex` שלהם ולהיכנס לקובץ ה-revocation
לאחר שפג תוקפם או הוחזרו.
