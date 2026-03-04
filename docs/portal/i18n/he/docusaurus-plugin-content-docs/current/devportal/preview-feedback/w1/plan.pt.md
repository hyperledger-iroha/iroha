---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
כותרת: Plano de preflight de parceiros W1
sidebar_label: Plano W1
תיאור: Tarefas, responsavis e checklist de evidencia para a coorte de preview de parceiros.
---

| פריט | פרטים |
| --- | --- |
| אונדה | W1 - Parceiros e integradores Torii |
| ג'נלה אלבו | Q2 2025 Semana 3 |
| Tag de artefato (planejado) | `preview-2025-04-12` |
| בעיה לעשות גשש | `DOCS-SORA-Preview-W1` |

## אובייקטיביות

1. Garantir aprovacoes legais e de governanca para os termos de preview de parceiros.
2. הכן פרוקסי נסה את זה ותמונות מצב של טלמטריה בארצות הברית ללא חבילה של זימון.
3. אטואליזר או תצוגה מקדימה אימות לבדיקה ותוצאות בדיקות.
4. Finalizar o roster de parceiros e os templates de solicitacao antes do envio dos convites.

## Desdobramento de tarefas

| תעודת זהות | טארפה | שו"ת | פראזו | סטטוס | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obter aprovacao legal para o adendo dos termos de preview | Docs/DevRel lead -> משפטי | 2025-04-05 | קונקלוידו | כרטיס חוקי `DOCS-SORA-Preview-W1-Legal` aprovado em 2025-04-05; PDF anexado ao tracker. |
| W1-P2 | Capturar Janela de staging do proxy נסה את זה (2025-04-10) e validar a saude do proxy | Docs/DevRel + Ops | 2025-04-06 | קונקלוידו | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` executado em 2025-04-06; transcricao de CLI e `.env.tryit-proxy.bak` arquivados. |
| W1-P3 | Construir artefato de preview (`preview-2025-04-12`), rodar `scripts/preview_verify.sh` + `npm run probe:portal`, arquivar descriptor/checksums | פורטל TL | 2025-04-08 | קונקלוידו | Artefato e logs de verificacao armazenados em `artifacts/docs_preview/W1/preview-2025-04-12/`; saida de probe anexada ao tracker. |
| W1-P4 | Revisar formularios de intake de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`), אישור תקנות e NDAs | קשר ממשל | 2025-04-07 | קונקלוידו | As oito solicitacoes aprovadas (as duas ultimas em 2025-04-11); aprovacoes linkadas no tracker. |
| W1-P5 | Redigir o convite (baseado em `docs/examples/docs_preview_invite_template.md`), definir `<preview_tag>` e `<request_ticket>` para cada parceiro | Docs/DevRel lead | 2025-04-08 | קונקלוידו | Rascunho do convite enviado em 2025-04-12 15:00 UTC junto com links de artefato. |

## רשימת בדיקה מוקדמת

> Dica: rode `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` עבור executar automaticamente os passos 1-5 (build, verificacao de checksum, probe do portal, checker link and atualizacao do proxy נסה זאת). רישום סקריפט ב-JSON יומן כדי להדגיש את נושא המעקב.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) עבור `build/checksums.sha256` ו-`build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e arquivar `build/link-report.json` ao lado do descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou passar o target adequado via `--tryit-target`); התחייב לעשות `.env.tryit-proxy` אטואליזדו ושמירה על `.bak` עבור החזרה לאחור.
6. התקנת בעיה W1 עם רישומי יומנים (בדיקת סכום לתיאור, בדיקה, בדיקה ללא פרוקסי נסה זאת ותמונות מצב Grafana).

## רשימת הוכחות- [x] Aprovacao legal assinada (PDF ou link do ticket) anexada ao `DOCS-SORA-Preview-W1`.
- [x] צילומי מסך של Grafana עבור `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor e log de checksum de `preview-2025-04-12` armazenados em `artifacts/docs_preview/W1/`.
- [x] Tabela de roster de convites com `invite_sent_at` preenchido (ver log W1 no tracker).
- [x] Artefatos de feedback refletidos em [`preview-feedback/w1/log.md`](./log.md) com uma linha por parceiro (atualizado em 2025-04-26 com dados de roster/telemetria/issuees).

להגדיר את este plano conforme כמו tarefas avancarem; o Tracker o Referencia para manter o Auditavel מפת דרכים.

## Fluxo de feedback

1. מבקר קודש, דופליקר או תבנית em
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   preencher os metadados e armazenar a copia completa em
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Resumir מזמין, מחסומים דה טלמטריה e issues abertos dentro do log vivo em
   [`preview-feedback/w1/log.md`](./log.md) para que reviewers de governanca possam rever toda a onda
   sem sair do repositorio.
3. Quando os exports de know-check ou סקרים chegarem, anexar no caminho de artefatos indicado no log
   e linkar o issue do tracker.