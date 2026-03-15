---
lang: he
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פאזל-שירות-פעולות
כותרת: Guia de operaciones del servicio de puzzles
sidebar_label: אופציות של שירות פאזלים
תיאור: Operacion del daemon `soranet-puzzle-service` עבור כרטיסי כניסה Argon2/ML-DSA.
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/soranet/puzzle_service_operations.md`. Mantengan ambas versiones sincronizadas hasta que los docs heredados se retiren.
:::

# מדריך פעולות פאזלים

El daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emite
כרטיסי כניסה respaldados por Argon2 que reflejan la policy `pow.puzzle.*`
del relay y, cuando esta configurado, intermedia tokens de admision ML-DSA en
nombre de relays edge. חשיפה של נקודות קצה של cinco HTTP:

- `GET /healthz` - בדיקה דה חיים.
- `GET /v1/puzzle/config` - devuelve los parametros efectivos de PoW/puzzle
  tomados del JSON del relay (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - acuña un ticket Argon2; un body JSON אופציונלי
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  solicita un TTL mas corto (clamp al window de policy), ata el ticket a un
  תמליל hash y devuelve un ticket firmado por el relay + la huella de
  la firma cuando hay claves de firmado configuradas.
- `GET /v1/token/config` - cuando `pow.token.enabled = true`, devuelve la
  מדיניות הפעלת אסימון הכניסה (טביעת אצבע של המנפיק, מגבלות TTL/הטיית שעון,
  מזהה ממסר y el set de revocacion combinado).
- `POST /v1/token/mint` - acuña un token de admision ML-DSA ligado al resume
  hash provisto; el body acepta `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Los tickets producidos por el servicio se verifican en la prueba de integracion
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que tambien ejercita
los throttles del relay durante escenarios de DoS volumetrico.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## הגדר את פליטת האסימונים

תצורה של JSON של ממסר באחו `pow.token.*` (ver
`tools/soranet-relay/deploy/config/relay.entry.json` como emplo) para habilitar
los tokens ML-DSA. Como minimo, provee la clave publica del issuer y una list
optional de revocacion:

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

שירות הפאזל reutiliza estos valores y recarga automaticamente el archivo
Norito JSON החזר בזמן ריצה. Usa el CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) עבור אקוניאר ה
בדיקת אסימונים במצב לא מקוון, מאגר מידע `token_id_hex` al archivo de
revocaciones y auditar credenciales existentes antes de publicar עדכונים א
ייצור.

Pasa la clave secreta del מנפיק שירות פאזלים באמצעות דגלים CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` tambien esta disponible cuando el secreto se gestiona por
una pipeline de tooling out-of-band. El watcher del archivo de revocacion
mantiene `/v1/token/config` בפועל; coordina actualizaciones con el comando
`soranet-admission-token revoke` para evitar desfases en el estado de revocacion.Configura `pow.signed_ticket_public_key_hex` en el JSON של ממסר עבור הודעה
la clave publica ML-DSA-44 usada para אימות כרטיסים PoW firmados; `/v1/puzzle/config`
העתק לה clave y su huella BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
para que los clients puedan fijar el verificador. Los tickets firmados se validan
contra el relay ID y los כריכות תמלול y comparten el mismo store de revocacion;
los tickets PoW crudos de 74 bytes siguen siendo validos cuando el verificador de
כרטיס חתום esta configurado. Pasa el secreto del firmante דרך `--signed-ticket-secret-hex`
o `--signed-ticket-secret-path` אל שירות פאזלים; el arranque rechaza
pares de claves que no coinciden si el secreto no valida contra `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` acepta `"signed": true` (y אופציונלי `"transcript_hash_hex"`) para
devolver un ticket firmado Norito junto con los bytes del ticket en bruto; לאס
תשובות כוללות `signed_ticket_b64` y `signed_ticket_fingerprint_hex` עבור איודר
שידור חוזר של טביעות אצבעות. Las solicitudes con `signed = true` se rechazan
si el secreto del firmante no esta configurado.

## Playbook de rotacion de claves

1. **Recoleccion del nuevo descriptor commit.** פרסום ממשל אל ממסר
   descriptor commit en el bundle del directory. Copia la cadena hex en
   `handshake.descriptor_commit_hex` תצורת ממסר JSON
   שירות חידות compartido con el.
2. **Revision de limites de policy del puzzle.** אשר que los valores
   `pow.puzzle.{memory_kib,time_cost,lanes}` אקטואליזאדוס se alinean con el plan
   דה שחרור. Los מפעילי deben mantener la configuracion Argon2 determinista
   ממסרי כניסה (מינימום 4 MiB de memoria, 1 <= נתיבים <= 16).
3. **Preparar el reinicio.** Recarga la unidad systemd o el contenedor una vez
   que governance anuncie el cutover de rotacion. El servicio no soporta חם-טעינה מחדש;
   se requiere reinicio para tomar el nuevo descriptor commit.
4. **Validar.** Emite un ticket via `POST /v1/puzzle/mint` y confirma que los
   valores `difficulty` y `expires_at` צירוף מקרים של מדיניות נובעת. אל מודיע
   de soak (`docs/source/soranet/reports/pow_resilience.md`) captura los limites
   de latencia esperados como referencia. Cuando los tokens estan habilitados,
   consulta `/v1/token/config` para asegurar que el מנפיק טביעת אצבע הודעה
   y el conteo de revocaciones coincidan con los valores esperados.

## הליך חירום

1. קביעת התצורה של `pow.puzzle.enabled = false` בתצורת ממסר.
   Mantiene `pow.required = true` si los tickets hashcash fallback deben seguir
   siendo obligatorios.
2. Opcionalmente aplica entradas `pow.emergency` עבור תיאורים חוזרים
   מיושן mientras la puerta Argon2 esta לא מקוון.
3. Reinicia el relay y el service para aplicar el cambio.
4. Monitorea `soranet_handshake_pow_difficulty` para asegurar que la dificultad
   cae al valor hashcash esperado, y verifica que `/v1/puzzle/config` reporte
   `puzzle = null`.

## מעקב אחר התראות- **SLO SLO:** Rastrea `soranet_handshake_latency_seconds` y mantén el P95
  por debajo de 300 ms. Los offsets del soak test beviseen datas de libracion
  para throttles de guard.【docs/source/soranet/reports/pow_resilience.md:1】
- **לחץ מכסה:** ארה"ב `soranet_guard_capacity_report.py` עם מדד ממסר
  para ajustar cooldowns de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **יישור פאזל:** `soranet_handshake_pow_difficulty` debe coincidir con la
  dificultad devuelta por `/v1/puzzle/config`. La divergencia indica configuracion
  stale del relay o un reinicio fallido.
- **מוכנות אסימון:** Alerta si `/v1/token/config` cae a `enabled = false`
  חותמות זמן מיושנות. לוס מפעילים
  deben rotar el archivo de revocaciones Norito דרך CLI cuando se retire un token
  para mantener este נקודת קצה מדויקת.
- **בריאות השירות:** Sondea `/healthz` en la cadencia habitual de liveness y alerta
  si `/v1/puzzle/mint` devuelve respuestas HTTP 500 (אינדיקציה אי התאמה של פרמטרים
  Argon2 או fallas de RNG). Los errores de token minting aparecen como respuestas
  HTTP 4xx/5xx en `/v1/token/mint`; trata fallas repetidas como condicion de paging.

## תאימות ורישום ביקורת

Los relays emiten eventos `handshake` estructurados que incluyen razones de
מצערת y duraciones de cooldown. Asegura que la pipeline de compliance decrita
en `docs/source/soranet/relay_audit_pipeline.md` ingeste estos logs para que los
cambios de policy del puzzle sigan siendo auditables. Cuando la puerta de puzzle
este habilitada, archiva muestras de tickets emitidos y el snapshot de configuracion
Norito עם כרטיס כניסה למשחקים עתידיים. לוס אסימוני קבלה
emitidos antes de ventanas de mantenimiento deben rastrearse con sus valores
`token_id_hex` e insertarse en el archivo de revocaciones una vez que expiren o
שון ריבוקדו.