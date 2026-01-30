---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Guia de exposicao do host de preview

O roadmap DOCS-SORA exige que todo preview publico use o mesmo bundle verificado por checksum que os revisores exercitam localmente. Use este runbook apos o onboarding de revisores (e o ticket de aprovacao de convites) estarem completos para colocar o host beta online.

## Pre-requisitos

- Onda de onboarding de revisores aprovada e registrada no tracker de preview.
- Ultimo build do portal presente em `docs/portal/build/` e checksum verificado (`build/checksums.sha256`).
- Credenciais de preview SoraFS (URL Torii, autoridade, chave privada, epoch enviado) armazenadas em variaveis de ambiente ou em um config JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de mudanca DNS aberto com o hostname desejado (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) mais contatos on-call.

## Passo 1 - Construir e verificar o bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

O script de verificacao se recusa a continuar quando o manifesto de checksum esta ausente ou adulterado, mantendo cada artefato de preview auditado.

## Passo 2 - Empacotar os artefatos SoraFS

Converta o site estatico em um par CAR/manifest deterministico. `ARTIFACT_DIR` padrao e `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Anexe `portal.car`, `portal.manifest.*`, o descriptor e o manifesto de checksum ao ticket da onda de preview.

## Passo 3 - Publicar o alias de preview

Reexecute o helper de pin **sem** `--skip-submit` quando estiver pronto para expor o host. Forneca o config JSON ou flags CLI explicitos:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

O comando grava `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json`, que devem acompanhar o bundle de evidencia de convites.

## Passo 4 - Gerar o plano de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Compartilhe o JSON resultante com Ops para que a mudanca DNS referencie o digest exato do manifesto. Ao reutilizar um descriptor anterior como origem de rollback, adicione `--previous-dns-plan path/to/previous.json`.

## Passo 5 - Testar o host implantado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

O probe confirma o tag de release servido, headers CSP e metadados de assinatura. Repita o comando a partir de duas regioes (ou anexe a saida de curl) para que auditores vejam que o edge cache esta quente.

## Bundle de evidencia

Inclua os seguintes artefatos no ticket da onda de preview e referencie-os no email de convite:

| Artefato | Proposito |
|----------|-----------|
| `build/checksums.sha256` | Prova que o bundle corresponde ao build de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Payload canonico SoraFS + manifesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Mostra que o envio do manifesto + o alias binding foram concluidos. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (ticket, janela, contatos), resumo de promocao de rota (`Sora-Route-Binding`), ponteiro `route_plan` (plano JSON + templates de header), info de purge de cache e instrucoes de rollback para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descriptor assinado que liga o archive + checksum. |
| Saida do `probe` | Confirma que o host ao vivo anuncia o tag de release esperado. |
