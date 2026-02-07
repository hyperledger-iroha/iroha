---
lang: he
direction: rtl
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: פאזל-שירות-פעולות
כותרת: Guide d'operations du service de puzzles
sidebar_label: אופציית פאזלים
תיאור: Exploitation du daemon `soranet-puzzle-service` pour les tickets d'admission Argon2/ML-DSA.
---

:::הערה מקור קנוניק
:::

# מדריך תפעול של פאזלים

Le daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emet des
כרטיסי כניסה בסיסים sur Argon2 qui refletent la policy `pow.puzzle.*` du
relay et, lorsque configure, orchester des tokens d'admission ML-DSA pour les
קצה ממסר. אני חשוף את נקודות הקצה של cinq HTTP:

- `GET /healthz` - בדיקה דה חיים.
- `GET /v1/puzzle/config` - חזור על הפרמטרים PoW/פאזל אפקטים.
  ממסר du JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - emet un ticket Argon2; un body JSON optionnel
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  demande un TTL plus Court (clamp au window de policy), lie le ticket a un
  תמליל hash et renvoie un ticket signe par le relay + l'empreinte de
  חתימה lorsque des cles de signature sont configurees.
- `GET /v1/token/config` - quand `pow.token.enabled = true`, retourne la policy
  d'admission-token פעיל (טביעת אצבע של מנפיק, מגביל TTL/הטיית שעון, מזהה ממסר,
  et l'ensemble de revocation fusionne).
- `POST /v1/token/mint` - Emet un token d'admission ML-DSA lie au resume hash
  fourni; le body accepte `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Les tickets produits par le service sont מאמת dans le test d'integration
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, זה תרגול אוסטרלי
throttles du relay lors de scenaries DoS volumetriques.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## קביעת התצורה של פליטת האסימונים

Definissez les champs JSON du relay sous `pow.token.*` (voir
`tools/soranet-relay/deploy/config/relay.entry.json` pour un exemple) afin
d'activer les tokens ML-DSA. או מינימום, fournissez la cle publique de l'issuer
et une list de revocation optionnelle:

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

שירות הפאזל מנצל מחדש את הערכים והטעינה האוטומטית לטעינה
Norito JSON de revocation בזמן ריצה. Utilisez le CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) pour emettre et
inspecter des tokens במצב לא מקוון, ajouter des entrees `token_id_hex` au fichier de
ביטול, ומבקר את האישורים הקיימים אוונט de pousser des עדכונים
בהפקה.

Passez la cle secrete de l'issuer au שירות פאזלים דרך les flags CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` est aussi disponible lorsque le secret est gere par un
צינור מחוץ לפס. Le watcher du fichier de revocation garde `/v1/token/config`
ז'ור; coordonnez les mises a jour avec la commande `soranet-admission-token revoke`
pour eviter un etat de revocation en retard.Definissez `pow.signed_ticket_public_key_hex` ב-JSON ממסר פרסום
la cle publique ML-DSA-44 utilisee pour verifier les pow cards signs; `/v1/puzzle/config`
repete la cle et son empreinte BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) afin
que les clients puissent pinner le verificateur. Les tickets signes sont valides
מול מזהה ממסר וחיבורי תמליל וחלק מאגר לממים חנות דה
ביטול; Les PoW tickets bruts de 74 octets restent valides quand le verifier
כרטיס חתום מוגדר. Passez le secret de signature דרך `--signed-ticket-secret-hex`
ou `--signed-ticket-secret-path` au lancement du שירות פאזלים; le demarrage
rejette les keypairs incoherents si le secret ne valide pas contre
`pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` קבל `"signed": true`
(et optionnel `"transcript_hash_hex"`) pour renvoyer un ticket signe Norito en
plus des bytes du ticket brut; התשובות כוללות `signed_ticket_b64` et
`signed_ticket_fingerprint_hex` pour suivre les טביעות אצבעות בשידור חוזר. לס
requêtes avec `signed = true` sont rejetees si le secret de signature n'est pas
להגדיר.

## Playbook de rotation des cles

1. **התחייבות של Coller le nouveau descriptor.** Governance publie le relay
   מתאר מתחייב בצרור הספרייה. Copiez la chaine hex dans
   `handshake.descriptor_commit_hex` בתצורת ממסר JSON partagee
   עם שירות פאזלים.
2. **מאמת חידת המדיניות.** אישור que les valeurs
   `pow.puzzle.{memory_kib,time_cost,lanes}` מפספס תוכנית יושרה
   דה שחרור. המפעילים מחזיקים בתצורה של Argon2 deterministe
   ממסרי כניסה (מינימום 4 MiB de memoire, 1 <= נתיבים <= 16).
3. **מכין את הנישואין מחדש.** Rechargez l'unite systemd ou le container une
   fois que governance annonce le cutover de rotation. Le service ne supporte
   pas le חם-טעינה מחדש; un redemarrage est requis pour prendre le nouveau descriptor
   להתחייב.
4. ** Valider.** Emettez un ticket via `POST /v1/puzzle/mint` et confirmez que
   `difficulty` et `expires_at` כתב א-לה-נובל מדיניות. לקשר
   להשרות (`docs/source/soranet/reports/pow_resilience.md`) לכידת דה בורס דה
   איחור משתתף לשפוך התייחסות. Lorsque les tokens sont actives, lisez
   `/v1/token/config` pour verifier que l'suer טביעת אצבע annonce et le
   נוכחות כתבת ביטול זכויות יוצרים aux valeurs.

## נוהל ביטול דחיפות

1. Definissez `pow.puzzle.enabled = false` בשיתוף ממסר תצורה.
   Gardez `pow.required = true` si les כרטיסים hashcash fallback doivent rester
   חובות.
2. Optionnellement, imposez des entrees `pow.emergency` pour rejeter les
   תיאורים מיושנים תליון que la porte Argon2 est לא מקוון.
3. Redemarrez a la fois le relay et le puzzle service pour appliquer le
   שינוי.
4. Surveillez `soranet_handshake_pow_difficulty` pour Verifier que la schwéier
   tombe a la valeur hashcash attendue, et validez que `/v1/puzzle/config`
   rapporte `puzzle = null`.

## ניטור והתראה- **SLO SLO:** Suivez `soranet_handshake_latency_seconds` et gardez le P95
  sous 300 ms. Les offsets du soak test fournissent des donnees de calibration
  pour les throttles de guard.【docs/source/soranet/reports/pow_resilience.md:1】
- **לחץ מכסה:** Utilisez `soranet_guard_capacity_report.py` avec les
  ממסר מדדים pour ajuster les cooldowns `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **יישור פאזל:** `soranet_handshake_pow_difficulty` doit correspondre a la
  harde retournee par `/v1/puzzle/config`. Une divergence indique une config
  ממסר מיושן או שיעור נישואין מחדש.
- **מוכנות אסימון:** Alertez si `/v1/token/config` מצנח א `enabled = false`
  de maniere inattendue ou si `revocation_source` מקורה של חותמות זמן מיושן.
  Les operators doivent faire tourner le fichier de revocation Norito via le CLI
  des qu'un token est retire pour garder cet endpoint precis.
- **בריאות השירות:** Probez `/healthz` avec la cadence de liveness habituelle et
  alertez si `/v1/puzzle/mint` renvoie des reponses HTTP 500 (אינדיקציה לא התאמה
  des paramets Argon2 או des echecs RNG). Les erreurs de token minting se
  manifestent via des reponses HTTP 4xx/5xx sur `/v1/token/mint`; traitez les
  echecs חוזר על תנאי ההחלפה.

## ציות ורישום ביקורת

Les relays emettent des evenements `handshake` מבנים שכוללים את הסיבות
מצערת et les durees de cooldown. Assurz-vous que le pipeline de compliance
descrit dans `docs/source/soranet/relay_audit_pipeline.md` ingere ces logs afin
que les changements de policy פאזל restent auditables. פאזל קוואנד לה פורט
est active, archivez des echantillons de tickets mintes et le snapshot de
קונפיגורציה Norito עם כרטיס כניסה לביקורות עתידיות. לס
אסימוני כניסה mintes avant les fenetres de maintenance doivent etre suivis
avec leurs valeurs `token_id_hex` et inseres dans le fichier de revocation une
fois expires ou revoques.