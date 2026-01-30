---
lang: ar
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b608c0451be3c9db087e33e6075f3b5561af953036aa67a0a669f273129a131
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
id: transport
lang: he
direction: rtl
source: docs/portal/docs/soranet/transport.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note מקור קנוני
דף זה משקף את מפרט התעבורה SNNet-1 ב-`docs/source/soranet/spec.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של התיעוד יופסק.
:::

SoraNet היא שכבת האנונימיות שמגבה range fetches של SoraFS, streaming של Norito RPC ו-data lanes עתידיים של Nexus. תוכנית התעבורה (פריטי roadmap **SNNet-1**, **SNNet-1a**, ו-**SNNet-1b**) הגדירה handshake דטרמיניסטי, משא ומתן יכולות פוסט-קוונטיות (PQ) ותוכנית סיבוב salts כדי שכל relay, client ו-gateway יראו את אותה עמדת אבטחה.

## יעדים ומודל רשת

- לבנות מעגלים עם שלושה hop (entry -> middle -> exit) מעל QUIC v1 כך ש-peers פוגעניים לעולם לא יגיעו ל-Torii ישירות.
- לשלב handshake Noise XX *hybrid* (Curve25519 + Kyber768) מעל QUIC/TLS כדי לקשור את מפתחות הסשן ל-transcript של TLS.
- לחייב TLVs של יכולות שמכריזים על תמיכת PQ KEM/חתימה, תפקיד relay וגרסת פרוטוקול; להשתמש ב-GREASE עבור סוגים לא מוכרים כדי להשאיר הרחבות עתידיות ברות פריסה.
- לסובב salts של תוכן מסווה מדי יום ולהצמיד guard relays ל-30 יום כדי ששחיקה בספריה לא תוכל לזהות לקוחות.
- לשמור על cells קבועות של 1024 B, להזריק padding/cells dummy, ולייצא telemetry דטרמיניסטית כדי לזהות נסיונות downgrade במהירות.

## מסלול handshake (SNNet-1a)

1. **QUIC/TLS envelope** - clients מתחברים ל-relays דרך QUIC v1 ומשלימים handshake של TLS 1.3 עם תעודות Ed25519 חתומות על ידי governance CA. ה-TLS exporter (`tls-exporter("soranet handshake", 64)`) מזין את שכבת Noise כך שה-transcripts יהיו בלתי ניתנים להפרדה.
2. **Noise XX hybrid** - מחרוזת פרוטוקול `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` עם prologue = TLS exporter. זרימת ההודעות:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   פלט ה-DH של Curve25519 ושתי ה-encapsulation של Kyber מתערבבים במפתחות הסימטריים הסופיים. כשל במשא ומתן על חומר PQ מבטל את ה-handshake לחלוטין - אין fallback קלאסי בלבד.

3. **Puzzle tickets ו-tokens** - relays יכולים לדרוש ticket של Argon2id proof-of-work לפני `ClientHello`. ה-tickets הם frames עם prefix של אורך שנושאים פתרון Argon2 מגובב ופגים בתוך גבולות המדיניות:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   tokens של admission עם prefix `SNTK` עוקפים את ה-puzzles כאשר חתימת ML-DSA-44 מה-issuer מאומתת מול המדיניות הפעילה ורשימת הביטולים.

4. **Capability TLV exchange** - ה-payload האחרון של Noise מעביר את TLVs היכולות המתוארים להלן. clients מפסיקים את החיבור אם חסרה יכולת חובה (PQ KEM/חתימה, role או version) או שהיא לא תואמת לכניסת הספריה.

5. **Transcript logging** - relays מתעדים את ה-hash של ה-transcript, טביעת האצבע של TLS ותוכן ה-TLV כדי להזין מזהי downgrade וצינורות compliance.

## Capability TLVs (SNNet-1c)

היכולות משתמשות במעטפת TLV קבועה של `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

סוגים מוגדרים כיום:

- `snnet.pqkem` - רמת Kyber (`kyber768` ל-rollout הנוכחי).
- `snnet.pqsig` - suite חתימות PQ (`ml-dsa-44`).
- `snnet.role` - תפקיד relay (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - מזהה גרסת פרוטוקול.
- `snnet.grease` - רשומות מילוי אקראיות בתחום השמור כדי להבטיח סבילות ל-TLVs עתידיים.

clients שומרים allow-list של TLVs חובה ונכשלים ב-handshake אם הם הושמטו או הונמכו. relays מפרסמים את אותו set במיקרו-דסקריפטור של הספריה כדי שהאימות יהיה דטרמיניסטי.

## סיבוב salts ו-CID blinding (SNNet-1b)

- Governance מפרסמת רשומת `SaltRotationScheduleV1` עם הערכים `(epoch_id, salt, valid_after, valid_until)`. relays ו-gateways מושכים את לוח הזמנים החתום מ-directory publisher.
- clients מיישמים את ה-salt החדש ב-`valid_after`, שומרים את ה-salt הקודם לתקופת חסד של 12 שעות ומחזיקים היסטוריה של 7 epochs כדי לסבול עדכונים מאוחרים.
- מזהי blinding קנוניים משתמשים ב:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  gateways מקבלים את המפתח המוסווה דרך `Sora-Req-Blinded-CID` ומחזירים אותו ב-`Sora-Content-CID`. blinding של circuit/request (`CircuitBlindingKey::derive`) נמצא ב-`iroha_crypto::soranet::blinding`.
- אם relay מפספס epoch, הוא עוצר מעגלים חדשים עד שיטען את לוח הזמנים ומפיק `SaltRecoveryEventV1`, שבקרי on-call מתייחסים אליו כאות paging.

## נתוני ספריה ומדיניות guards

- Microdescriptors נושאים זהות relay (Ed25519 + ML-DSA-65), מפתחות PQ, TLVs של יכולות, תגיות אזור, זכאות guard, ואת epoch ה-salt המוצהר הנוכחי.
- clients מצמידים guard sets ל-30 יום ומאחסנים מטמונים `guard_set` לצד snapshot חתום של הספריה. עטיפות CLI ו-SDK מציגות את טביעת האצבע של המטמון כדי שניתן יהיה לצרף ראיות rollout לסקירות שינוי.

## Telemetry ורשימת בדיקה ל-rollout

- מדדים שיש לייצא לפני פרודקשן:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- ספי התראה חיים לצד מטריצת SLO של SOP סיבוב salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) וחייבים להופיע ב-Alertmanager לפני קידום הרשת.
- התראות: שיעור כשל >5% בתוך 5 דקות, salt lag >15 דקות, או mismatches ביכולות שנצפו בפרודקשן.
- צעדי rollout:
  1. להריץ בדיקות תאימות relay/client ב-staging עם handshake היברידי ו-stack PQ פעיל.
  2. לתרגל את SOP סיבוב salts (`docs/source/soranet_salt_plan.md`) ולצרף artefacts של drill לרשומת השינוי.
  3. להפעיל משא ומתן יכולות בספריה ואז לפרוס ל-entry relays, middle relays, exit relays ולבסוף clients.
  4. לתעד fingerprints של guard cache, לוחות זמנים של salt ולוחות telemetry לכל שלב; לצרף את חבילת הראיות ל-`status.md`.

מעקב אחר רשימה זו מאפשר לצוותי operator, client ו-SDK לאמץ את transports של SoraNet יחד תוך עמידה בדטרמיניזם ובדרישות הביקורת שתועדו ב-roadmap של SNNet.
