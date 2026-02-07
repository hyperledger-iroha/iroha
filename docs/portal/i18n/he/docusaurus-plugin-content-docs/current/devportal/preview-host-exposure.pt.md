---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guia de exposicao do host de preview

מפת הדרכים DOCS-SORA יש צורך לעשות תצוגה מקדימה לציבור להשתמש ב-Mesmo Bundle אימות על ידי בדיקה עבור בודקים של בודקים מקומיים. השתמש ב-esta runbook apos o onboarding de revisores (eo ticket de aprovacao de convites) estarem completos para colocar או host beta online.

## דרישות מוקדמות

- Onda de onboarding de revisores aprovada e registrada no tracker de preview.
- Ultimo build do porte presente em `docs/portal/build/` e checksum verificado (`build/checksums.sha256`).
- Credenciais de preview SoraFS (URL Torii, autoridade, chave privada, epoch enviado) armazenadas em variaveis de ambiente ou em um config JSON como [SORA](0000001014X](0000001014X).
- Ticket de mudanca DNS aberto com o שם המארח desejado (`docs-preview.sora.link`, `docs.iroha.tech` וכו') mais contatos on-call.

## Passo 1 - Construir e Verificar o Bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

O script de verificacao se recusa a continuation quando o Manifesto de checksum esta ausente ou adulterado, mantendo cada artefato de preview auditado.

## Passo 2 - Empacotar os artefatos SoraFS

המרת האתר estatico em um par CAR/manifest deterministico. `ARTIFACT_DIR` padrao e `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

נספח `portal.car`, `portal.manifest.*`, או מתאר או מניפסט בדיקה וכרטיס תצוגה מקדימה.

## Passo 3 - Publicar או alias de preview

בצע מחדש o helper de pin **sem** `--skip-submit` quando estiver pronto para expor o host. פורנקה או תצורה של JSON או מסמנת פירושים של CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

O comando grava `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json`, que devem acompanhar o bundle de evidencia de convites.

## Passo 4 - Gerar o plano de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

חלק את תוצאות ה-JSON עם אופציות עבור הפניה ל-DNS מודפסת או לעכל את המניפסט. Ao reutilizar um descriptor anterior como origem de rollback, adicone `--previous-dns-plan path/to/previous.json`.

## Passo 5 - Testar o host implantado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

או בדיקה של אישור תג שחרור שירות, כותרות CSP ו-metadados de assinatura. Repita o comando a partir de duas regioes (ou anexe a saida de curl) para que auditores vejam que o edge cache esta quente.

## Bundle de Evidencia

כולל הוראות שימוש ללא כרטיס לתצוגה מקדימה והפניה ללא דוא"ל להזמנה:

| ארטפטו | פרופוזיטו |
|--------|--------|
| `build/checksums.sha256` | Prova que o bondle corresponde ao build de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | מטען canonico SoraFS + מניפסט. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Mostra que o envio do Manifesto + o alias מחייב foram concluidos. |
| `artifacts/sorafs/portal.dns-cutover.json` | מדדי DNS (כרטיס, ז'אנלה, תוכן), קורות חיים (`Sora-Route-Binding`), פונטיירו `route_plan` (פלוני JSON + תבניות כותרת), מידע על טיהור מטמון והוראות להחזרה לאופציות. |
| `artifacts/sorafs/preview-descriptor.json` | מתאר assinado que liga o archive + checksum. |
| Saida do `probe` | אישור מהי המארחים או הצהרת חיים או תג שחרור אספרדו. |