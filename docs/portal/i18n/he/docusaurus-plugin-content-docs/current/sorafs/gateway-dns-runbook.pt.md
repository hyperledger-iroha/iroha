---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de kickoff של Gateway e DNS da SoraFS

Esta copia do portal espelha o runbook canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Ela Captura OS מעקות בטיחות מבצעים את זרם העבודה של DNS descentralizado e Gateway
para que lideres de networking, ops e documentacao possam ensaiar a pilha de
automatizacao ante עושה בעיטה 2025-03.

## Escopo e entregaveis

- Conectar os marcos de DNS (SF-4) e gateway (SF-5) ensaiando derivacao
  deterministica de hosts, releases de retorio de resolvers, automatizacao TLS/GAR
  e captura de evidencias.
- Manter os insumos do kickoff (agenda, convite, tracker de presenca, snapshot de
  telemetria GAR) sincronizados com as ultimas atribuicoes de בעלים.
- Produzir um bundle de artefatos auditavel para revisores de governanca: notas de
  שחרר לעשות מדריך דה resolvers, יומני בדיקה לעשות שער, saida לעשות לרתום דה
  conformidade e o resumo de Docs/DevRel.

## Papeis e responsabilidades

| זרם עבודה | אחריות אחריות | Artefatos requeridos |
|------------|----------------|------------------------|
| רשת TL (מחסנית DNS) | מנה או פלונית קביעת של מארחים, משחררי הפעלה של RAD, כניסות פומביות לטלמטריה של פותרים. | `artifacts/soradns_directory/<ts>/`, הבדלים של `docs/source/soradns/deterministic_hosts.md`, מטא נתונים RAD. |
| Ops Automation Lead (שער) | תרגילי הפעלה אוטומטיים TLS/ECH/GAR, רודar `sorafs-gateway-probe`, ווים מפעילים את PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON בדיקה, רכיבי `ops/drill-log.md`. |
| QA Guild & Tooling WG | Rodar `ci/check_sorafs_gateway_conformance.sh`, אביזרי קוראר, חבילות אקוודור אישור עצמי Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | צילום דקות, התקן או קריאה מוקדמת של עיצוב + נספחים, פרסום או פורטל הוכחות. | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` הושלמו. |

## דרישות מוקדמות

- Especificacao de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) e
  o פיגומים de atestacao de resolvers (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefatos de gateway: מפעיל ידנית, עוזרי דה אוטומטי TLS/ECH,
  הדרכה של מצב ישיר וזרימת עבודה לאישור עצמי של `docs/source/sorafs_gateway_*`.
- כלי עבודה: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e helpers de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- הגדרות: chave de release GAR, אישור ACME DNS/TLS, מפתח ניתוב עבור PagerDuty,
  token de auth do Torii עבור מביאים פתרונות.

## רשימת רשימת טרום טיסה

1. אשר את המשתתפים ואת סדר היום
   `docs/source/sorafs_gateway_dns_design_attendance.md` e circulando a agenda
   atual (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. מכינים raizes de artefatos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` ה
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. להטמיע מתקנים (מציג את GAR, פרופס RAD, bundles de conformidade do gateway) e
   garanta que o estado de `git submodule` esteja alinhado ao ultimo tag de ensaio.
4. Verifique Segredos (Chave de release Ed25519, arquivo de conta ACME, token to PagerDuty)
   e se batem com checksums do vault.
5. Faça Test-test nos targets de telemetria (נקודת קצה Pushgateway, לוח GAR Grafana)
   אנטים כן מקדחים.## Etapas de ensaio de automatizacao

### מפה מוגדרת של המארחים וההפצה ב-RAD

1. Rode o helper de derivacao deterministica de hosts contra o set de manifests
   proposto e confirme que nao ha drift em relacao a
   `docs/source/soradns/deterministic_hosts.md`.
2. ראה חבילה של מדריך פתרונות:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. הרשמה o ID do diretorio, o SHA-256 e os caminhos de saida impressos dentro de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` enas minos לעשות בעיטת הפתיחה.

### Captura de telemetria DNS

- Faça tail dos logs de transparencia de resolvers por ≥10 דקות usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- ייצא מדדים ל-Pushgateway ו-arquive OS צילומי מצב NDJSON ao lado do
  diretorio do run ID.

### Drills de automatizacao do gateway

1. בצע בדיקה של TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Rode o harness de conformidade (`ci/check_sorafs_gateway_conformance.sh`) ה
   o עוזר אישור עצמי (`scripts/sorafs_gateway_self_cert.sh`) עבור אטואליסר
   o bundle de atestacoes Norito.
3. לכידת אירועים של PagerDuty/Webhook למען הוכחה ל-Caminho de Automatizacao
   funciona de ponta a ponta.

### Empacotamento de evidencias

- להטמיע את חותמות הזמן של `ops/drill-log.md` com, המשתתפים ו-hashes de probes.
- Armazene artefatos nos diretorios de run ID e publicque um resumo executivo
  נאs minutos do Docs/DevRel.
- Linke o bundle de evidencias no ticket de governanca antes da revisao do kickoff.

## Facilitacao de sessao e hand-off de evidencias

- **Linha do tempo do moderator:**
  - T-24 שעות — ניהול תוכנית לאחר ביצוע + תמונת מצב של אג'נדה/הצגה על `#nexus-steering`.
  - T-2 h — Networking TL אטואליזה או תמונת מצב של טלמטריה GAR e registra os deltas em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 מ' — אוטומציה אוטומציה אוטומציה אוטומציה של אוטומציה ובדיקת בדיקות escreve o run ID ativo em `artifacts/sorafs_gateway_dns/current`.
  - Durante a chamada - O moderador compartilha este runbook e designa um escriba ao vivo; Docs/DevRel capturam itens de acao inline.
- **Template de minutos:** Copie o esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (טמבם espelhado ללא חבילה
  do portal) e comite uma instancia preenchida por sessao. כולל רשימה של
  participantes, decisoes, itens de acao, hashes de evidencias e riscos pendentes.
- **העלאת הוכחות:** Zip o diretorio `runbook_bundle/` do ensaio,
  תוספת או עיבוד של PDF של דקות, רישום hashes SHA-256 לאחר דקות +
  אג'נדה, e depois avise o alias de reviewers de governanca quando os uploads
  chegarem em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## תמונת מצב של evidencias (בעיטת מרקו 2025)

Os ultimos artefatos de ensaio/live referenciados ללא מפת דרכים e nas minutos
ficam no bucket `s3://sora-governance/sorafs/gateway_dns/`. Os hashes abaixo
espelham o manifest canonico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **ריצה יבשה — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - חבילת Tarball do: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF זה דקות: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **סדנה ao vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(העלה תלויה: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel anexara או SHA-256 quando או PDF renderizado chegar או חבילה.)_

## relacionado חומרי- [Operations Playbook do gateway](./operations-playbook.md)
- [Plano de observabilidade da SoraFS](./observability-plan.md)
- [מעקב אחר DNS descentralizado e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)