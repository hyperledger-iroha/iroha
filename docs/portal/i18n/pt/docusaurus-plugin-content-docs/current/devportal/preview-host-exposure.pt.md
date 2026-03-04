---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guia de exposição do host de preview

O roadmap DOCS-SORA exige que toda visualização pública use o mesmo pacote selecionado por checksum que os revisores exercem localmente. Use este runbook após o onboarding de revisores (e o ticket de aprovação de convites) que estiverem completos para colocar o host beta online.

## Pré-requisitos

- Onda de onboarding de revisores aprovados e registrados no tracker de preview.
- Ultimo build do portal presente em `docs/portal/build/` e checksum verificado (`build/checksums.sha256`).
- Credenciais de visualização SoraFS (URL Torii, autoridade, chave privada, época enviada) armazenados em variáveis ​​de ambiente ou em uma configuração JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de mudanca DNS aberto com o nome de host desejado (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) mais contatos de plantão.

## Passo 1 - Construir e verificar o pacote

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

O script de verificação se recusa a continuar quando o manifesto de checksum está ausente ou adulterado, mantendo cada arte de visualização auditada.

## Passo 2 - Empacotar os artistas SoraFS

Converta o site estatístico em um par CAR/manifesto determinístico. `ARTIFACT_DIR` padrão e `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Anexo `portal.car`, `portal.manifest.*`, o descritor e o manifesto de checksum ao ticket da onda de visualização.

## Passo 3 - Publicar ou alias de visualização

Execute novamente o helper de pin **sem** `--skip-submit` quando estiver pronto para exportar o host. Forneça a configuração JSON ou sinalizadores CLI explícitos:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

O comando grave `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json`, que deve acompanhar o pacote de provas de convites.

## Passo 4 - Gerar o plano de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Compartilhe o JSON resultante com Ops para que a mudanca DNS referencie o resumo exato do manifesto. Ao reutilizar um descritor anterior como origem de rollback, acréscimo `--previous-dns-plan path/to/previous.json`.

## Passo 5 - Testar o host implantado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

A sonda confirma a tag de liberação servida, cabeçalhos CSP e metadados de assinatura. Repita o comando a partir de duas regiões (ou anexe a saida de curl) para que os auditores vejam que o edge cache está quente.

## Pacote de evidências

Inclui os seguintes artistas no ticket da onda de preview e referência-os no email de convite:

| Artefato | Proposta |
|----------|-----------|
| `build/checksums.sha256` | Prova que o bundle corresponde ao build do CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga canônica SoraFS + manifesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Mostra que o envio do manifesto + o alias vinculativo foram concluídos. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (ticket, janela, contatos), resumo de promoção de rota (`Sora-Route-Binding`), ponteiro `route_plan` (plano JSON + templates de cabeçalho), informações de limpeza de cache e instruções de rollback para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descritor assinado que liga o arquivo + checksum. |
| Saida do `probe` | Confirma que o host ao vivo anuncia o tag de lançamento esperado. |