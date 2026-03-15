---
lang: he
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פאזל-שירות-פעולות
כותרת: Guia de operacoes do Puzzle Service
sidebar_label: Ops do Puzzle Service
תיאור: Operacao do daemon `soranet-puzzle-service` לכרטיסי כניסה Argon2/ML-DSA.
---

:::שים לב Fonte canonica
Espelha `docs/source/soranet/puzzle_service_operations.md`. Mantenha ambas as copias sincronizadas.
:::

# Guia de operacoes do Puzzle Service

O daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emite
כרטיסי כניסה respaldados por Argon2 que refletem a policy `pow.puzzle.*`
עשה relay e, quando configurado, faz broker de ML-DSA אסימוני קבלה em nome
דוס ממסרי קצה. Ele expoe cinco נקודות קצה HTTP:

- `GET /healthz` - בדיקה חיה.
- `GET /v2/puzzle/config` - retorna os parametros efetivos de PoW/puzzle extraidos
לעשות ממסר JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v2/puzzle/mint` - emite um ticket Argon2; um body JSON אופציונלי
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  pede um TTL menor (clamp na janela de policy), vincula o ticket a um תמליל
  hash e retorna um כרטיס assinado pelo relay + טביעת אצבע da assinatura quando
  כמו chaves de assinatura estao configuradas.
- `GET /v2/token/config` - quando `pow.token.enabled = true`, חזור על מדיניות
  ativa de admission-token (טביעת אצבע של מנפיק, מגבלות של TTL/הטיית שעון, מזהה ממסר
  e o revocation set mesclado).
- `POST /v2/token/mint` - emite um ML-DSA admission token vinculado ao resume hash
  fornecido; o גוף aceita `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Os tickets produzidos pelo servico sao verificados no teste de integracao
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que tambem exercita os
מצערות ממסרות את טווחי ה-DoS volumetrico.
(tools/soranet-relay/tests/adaptive_and_puzzle.rs:337)

## הגדרת אסימונים

Defina os campos JSON do relay em `pow.token.*` (veja
`tools/soranet-relay/deploy/config/relay.entry.json` כמו דוגמה) למען החבילה
אסימוני ML-DSA. אין מינימו, פורנקה א צ'ווה פובליקא לעשות מנפיק e uma ביטול רשימת
אופציונלי:

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

שירות הפאזל מחדש esses valores e recregga automaticamente o arquivo
Norito JSON de revogacao עם זמן ריצה. השתמש ב-CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para emitier e
inspecionar tokens offline, anexar entradas `token_id_hex` ao arquivo de revogacao
e Auditar credenciais existentes antes de publicar עדכונים עם producao.

העברת מפתח סודי של מנפיק לשירות פאזלים באמצעות דגלים CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` tambem esta disponivel quando o secret e gerenciado por um
pipeline de tooling fora de banda. הו צופה לעשות arquivo de revogacao mantem
`/v2/token/config` אטואליזדו; coordene עדכונים com o comando
`soranet-admission-token revoke` para evitar estado de revogacao defasado.Defina `pow.signed_ticket_public_key_hex` ללא JSON לעשות ממסר עבור הודעה על צ'אב
פרסם ML-DSA-44 ארה"ב עבור אימות כרטיסים ל-PoW Assinados; `/v2/puzzle/config`
reproduz a chave e seu טביעת אצבע BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
עבור לקוחות possam fixar o Verifier. כרטיסים assinados sao validados contra
o זיהוי ממסר e כריכות תמלול e compartilham o חנות ביטול mesmo; PoW
כרטיסים ברוטוס של 74 בתים קבועים תקפים או מאמת כרטיסים חתומים
ainda esta configurado. העבר את סוד החותם דרך `--signed-ticket-secret-hex` ou
`--signed-ticket-secret-path` או יצירת פאזלים; o startup rejeita
keypairs divergentes se o secret nao valida contra `pow.signed_ticket_public_key_hex`.
`POST /v2/puzzle/mint` aceita `"signed": true` (אופציונלי `"transcript_hash_hex"`) para
retornar um כרטיס חתום Norito junto com os bytes do ticket bruto; בתור תשובות
כולל `signed_ticket_b64` e `signed_ticket_fingerprint_hex` לעזרה
טביעות אצבעות בשידור חוזר. בקשות com `signed = true` sao rejeitadas se o signer secret
nao estiver configurado.

## Playbook de rotacao de chaves

1. **התחייבות של קולטר או תיאור חדש.** פרסום ממשל או ממסר
   descriptor commit no bundel מדריך. העתק פס קס של מחרוזת
   `handshake.descriptor_commit_hex` dentro da configuracao JSON לעשות ממסר ממסר
   com o שירות פאזלים.
2. **Revisar bounds da policy de puzzle.** אשר que os valores atualizados
   `pow.puzzle.{memory_kib,time_cost,lanes}` estejam alinhados ao plano de release.
   המפעילים מפתחים את ההגדרה של ממסרים פנימיים של Argon2
   (מינימום 4 MiB de memoria, 1 <= נתיבים <= 16).
3. **הכנה או הפעלה מחדש.** קבע מחדש מערכת אחידה או קוד מיכל
   מודעת ממשל o cutover de rotacao. O servico nao supporta Hot-Reload;
   אום התחל מחדש והכרחי עבור התחייבות לתיאור חדש.
4. **Validar.** Emita um כרטיס דרך `POST /v2/puzzle/mint` e confirme que
   `difficulty` ו-`expires_at` מתכתבים עם מדיניות חדשנית. הו לספוג דוח
   (`docs/source/soranet/reports/pow_resilience.md`) מצמצם מגבלות דה-latcia
   esperados para referencia. Quando tokens estiverem habilitados, busque
   `/v2/token/config` para garantir que o מנפיק טביעת אצבע הודעה
   contagem de revogacoes correspondam aos valores esperados.

## Procedimento de desativacao de emergencia

1. הגדרה של `pow.puzzle.enabled = false` עם הגדרת ממסר.
   Mantenha `pow.required = true` se os tickets hashcash fallback precisarem
   permanecer obrigatorios.
2. אופציונלי, כפו על `pow.emergency` עבור תיאורים דחויים
   antigos enquanto o gate Argon2 estiver במצב לא מקוון.
3. Reinicie o ממסר ושירות פאזלים עבור אפליקאר א מודנקה.
4. Monitore `soranet_handshake_pow_difficulty` למען הבטחה
   caia para o valor hashcash esperado e verifique `/v2/puzzle/config` reportando
   `puzzle = null`.

## מעקב אחר התראות- **SLO חביון:** Acompanhe `soranet_handshake_latency_seconds` e mantenha o P95
  abaixo de 300 ms. Os offsets לעשות לספוג בדיקה fornecem dados de calibracao para
  מצערות שומר. (docs/source/soranet/reports/pow_resilience.md:1)
- **לחץ מכסה:** השתמש במדדי ממסר `soranet_guard_capacity_report.py` com
  para ajustar cooldowns de `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`). (docs/source/soranet/relay_audit_pipeline.md:68)
- **יישור פאזל:** `soranet_handshake_pow_difficulty` deve corresponder a
  dificuldade retornada por `/v2/puzzle/config`. תצורת ממסר Divergencia indica
  מעופש או הפעל מחדש את Falho.
- **מוכנות אסימון:** Alerta se `/v2/token/config` cair para `enabled = false`
  חותמות זמן מדווחות מיושנות ב-`revocation_source`. Operadores
  devem rotacionar o arquivo Norito de revogacao דרך CLI quando um token for
  retirado para manter esse נקודת קצה מדויקת.
- **בריאות השירות:** גישוש `/healthz` ב-cadencia usual de liveness e alertar
  se `/v2/puzzle/mint` retornar HTTP 500 (אינדיקציה אי התאמה של parametros Argon2 ou
  falhas de RNG). שגיאות הטבעת אסימון בשילוב עם HTTP 4xx/5xx
  `/v2/token/mint`; trate falhas repetidas como condicao de paging.

## תאימות רישום ביקורת

ממסרים emitem eventos `handshake` estruturados que כוללים מניעים למצערת e
משכי התקררות. Garanta que o pipeline de compliance decrito em
`docs/source/soranet/relay_audit_pipeline.md` ingira esses logs para que mudancas
da policy de puzzle fiquem auditavis. Quando o פאזל שער estiver habilitado,
arquive amostras de tickets emitidos e o תמונת מצב de configuracao Norito com o
כרטיס השקה ל-futuras auditorias. אסימוני כניסה emitidos antes de janelas
de manutencao devem ser rastreados pelos valores `token_id_hex` e inseridos no
arquivo de revogacao quando expirarem ou forem revogados.