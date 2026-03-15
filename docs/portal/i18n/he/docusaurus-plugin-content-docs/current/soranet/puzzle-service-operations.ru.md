---
lang: he
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פאזל-שירות-פעולות
כותרת: Руководство по эксплуатации שירות פאזלים
sidebar_label: אופציות שירות פאזל
תיאור: Эксплуатация daemon `soranet-puzzle-service` לכרטיסי כניסה של Argon2/ML-DSA.
---

:::note Канонический источник
:::

# Руководство по эксплуатации שירות פאזלים

Daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) выпускает
כרטיסי כניסה בגיבוי ארגון 2, מדיניות מדיניות `pow.puzzle.*` у ממסר
и, когда настроено, брокерит ML-DSA tokens от имени edge relays.
Он предоставляет пять נקודות קצה HTTP:

- `GET /healthz` - בדיקה חיה.
- `GET /v1/puzzle/config` - возвращает эффективные параметры PoW/פאזל,
  считанные из ממסר JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - כרטיס выпускает Argon2; גוף JSON אופציונלי
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  запрашивает более короткий TTL (חלון מדיניות מהודק), привязывает כרטיס
  к תעתיק hash и возвращает כרטיס חתום ממסר + טביעת אצבע חתימה
  при наличии מפתחות חתימה.
- `GET /v1/token/config` - когда `pow.token.enabled = true`, возвращает активную
  מדיניות אסימון קבלה (טביעת אצבע של מנפיק, גבולות הטיית TTL/שעון, מזהה ממסר,
  и סט ביטולים ממוזגים).
- `POST /v1/token/mint` - אסימון קבלה выпускает ML-DSA, связанный с מסופק
  לחדש hash; גוף принимает `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

כרטיסים, выпущенные сервисом, проверяются в интеграционном тесте
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, который также
упражняет מצערות ממסר во время volumetric DoS сценариев.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Настройка выпуска токенов

Задайте поля ממסר JSON под `pow.token.*` (см.
`tools/soranet-relay/deploy/config/relay.entry.json` как пример), чтобы
включить אסימוני ML-DSA. Минимально предоставьте המנפיק מפתח ציבורי и אופציונלי
רשימת ביטולים:

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

שירות פאזלים повторно использует эти значения и автоматически перезагружает
Norito ביטול JSON בזמן ריצה. Используйте CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) чтобы выпускать и
проверять אסימונים במצב לא מקוון, добавлять `token_id_hex` ערכים בביטול файл и
הצג אישורי ביקורת עבור עדכונים בהפקה.

הצג מפתח סודי של מנפיק בשירות פאזלים через דגלי CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` דוסטר, כונן סודי управляется חוץ מהפס
צינור. שומר קבצי ביטול держит `/v1/token/config` актуальным;
координируйте обновления с командой `soranet-admission-token revoke`, чтобы
избежать отставания מצב ביטול.Установите `pow.signed_ticket_public_key_hex` в relay JSON, чтобы объявить
מפתח ציבורי ML-DSA-44 עבור проверки כרטיסי PoW חתומים; `/v1/puzzle/config` эхо
возвращает מפתח и его טביעת אצבע BLAKE3 (`signed_ticket_public_key_fingerprint_hex`),
чтобы clients могли pin-ить מאמת. כרטיסים חתומים проверяются по מזהה ממסר и
כריכות תמלול и используют тот же חנות ביטול; כרטיסי PoW גולמיים של 74 בתים
остаются валидными при настроенном מאמת כרטיס חתום. סוד החותם Передайте
через `--signed-ticket-secret-hex` או `--signed-ticket-secret-path` при запуске
שירות פאזלים; старт отклонит несоответствующие צמדי מפתחות, если secret не валидируется
против `pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` принимает
`"signed": true` (ו `"transcript_hash_hex"` אופציונלי) чтобы вернуть
כרטיס חתום מקודד Norito вместе с raw ticket bytes; ответы включают
`signed_ticket_b64` ו-`signed_ticket_fingerprint_hex` להפעלה חוזרת של טביעות אצבע.
Запросы с `signed = true` отклоняются, если הסוד של החותם אינו זמין.

## Playbook ротации ключей

1. **התחייבות מתאר חדש.** מתאר ממסר ממשל публикует
   commit в חבילת ספריות. מחרוזת משושה в `handshake.descriptor_commit_hex`
   внутри relay JSON конфигурации, которой пользуется שירות פאזלים.
2. **פאזל מדיניות גבולות.** Подтвердите, что обновленные значения
   `pow.puzzle.{memory_kib,time_cost,lanes}` соответствуют שחרור плану. Операторы
   должны держать Argon2 конфигурацию детерминированной между ממסרים (מינימום 4 MiB
   памяти, 1 <= נתיבים <= 16).
3. **Подготовьте рестарт.** Перезагрузите systemd unit или container после того, как
   ממשל объявит חיתוך סיבוב. Сервис не поддерживает טעינה חוזרת חמה; для
   применения нового descriptor commit требуется הפעלה מחדש.
4. **Провалидируйте.** Выпустите ticket через `POST /v1/puzzle/mint` и подтвердите,
   что `difficulty` ו-`expires_at` соответствуют новой מדיניות. דוח להשרות
   (`docs/source/soranet/reports/pow_resilience.md`) содержит ожидаемые גבולות האחזור
   для справки. Когда tokens включены, запросите `/v1/token/config`, чтобы убедиться,
   что פרסם טביעת אצבע של מנפיק וספירת ביטולים соответствуют ожидаемым значениям.

## חירום השבת процедура

1. Установите `pow.puzzle.enabled = false` в общей ממסר конфигурации. Оставьте
   `pow.required = true`, כרטיסי hashcash fallback eсли должны оставаться обязательными.
2. Опционально примените `pow.emergency` ערכים, чтобы отклонять устаревшие
   תיאורים пока שער Argon2 במצב לא מקוון.
3. צור ממסר ושירות פאזלים, чтобы применить изменение.
4. Мониторьте `soranet_handshake_pow_difficulty`, чтобы убедиться, что сложность
   упала до ожидаемого hashcash значения, и проверьте, что `/v1/puzzle/config`
   сообщает `puzzle = null`.

## ניטור והתראה- ** SLO חביון:** Отслеживайте `soranet_handshake_latency_seconds` и держите P95
  לא 300 אלפיות השנייה. קיזוז מבחן השרייה дают נתוני כיול לשמירה על מצערות.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **לחץ מכסה:** Используйте `soranet_guard_capacity_report.py` с מדדי ממסר
  для настройки `pow.quotas` צינון (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **יישור פאזל:** `soranet_handshake_pow_difficulty` должен совпадать с
  קושי של `/v1/puzzle/config`. Дивергенция указывает על תצורת ממסר מעופש
  או אתחול מחדש.
- **מוכנות אסימון:** התראה, если `/v1/token/config` неожиданно падает до
  `enabled = false` או `revocation_source` חותמות זמן מיושנות. Операторы
  должны ротировать Norito קובץ ביטול через CLI при выводе токена, чтобы
  נקודת קצה оставался точным.
- **בריאות השירות:** Пробуйте `/healthz` в обычной liveness cadence и alert,
  если `/v1/puzzle/mint` возвращает HTTP 500 (указывает על חוסר התאמה של פרמטר Argon2
  או כשלים ב-RNG). Ошибки טביעת אסימונים проявляются как HTTP 4xx/5xx на
  `/v1/token/mint`; повторяющиеся сбои следует считать מצב ההחלפה.

## תאימות и רישום ביקורת

ממסרים публикуют структурированные `handshake` אירועים, включающие סיבות למצערת
и משכי התקררות. Убедитесь, что צינור תאימות из
`docs/source/soranet/relay_audit_pipeline.md` поглощает эти יומני, чтобы изменения
מדיניות פאזל оставались auditables. Когда פאזל שער включен, архивируйте
образцы כרטיסים מוטבעים и Norito תמונת מצב של תצורה вместе с כרטיס השקה
для будущих ביקורת. אסימוני כניסה, выпущенные перед חלונות תחזוקה,
следует отслеживать по `token_id_hex` и добавлять в ביטול файл после
истечения или отзыва.