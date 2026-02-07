---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guia de exposição do hotel de visualização

A folha de rota DOCS-SORA exige que cada visualização pública seja pressionada no pacote de memes, verificando por soma de verificação que os reletores testaram a localização. Utilize este runbook após a integração dos usuários (e o ticket de aprovação dos convites) para receber on-line o hotel beta.

## Pré-requisito

- Vaga integração dos usuários aprovados e registrados na visualização do rastreador.
- A última construção do portal apresenta o nome `docs/portal/build/` e a verificação da soma de verificação (`build/checksums.sha256`).
- Identificadores SoraFS preview (URL Torii, autorite, cle privee, epoch soumis) armazenados em variáveis ​​de ambiente ou em uma configuração JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de mudança de DNS aberto com hotel souhaite (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) mais contatos de plantão.

## Etapa 1 - Construa e verifique o pacote

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

O script de verificação se recusa a continuar quando o manifesto da soma de verificação é alterado ou alterado, o que guarda cada artefato de visualização auditado.

## Etapa 2 - Empacotador de artefatos SoraFS

Converta a estatística do site em um par CAR/manifesto determinado. `ARTIFACT_DIR` é por padrão `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, o descritor e o manifesto de soma de verificação no ticket da visualização vaga.

## Etape 3 - Visualização do alias do editor

Relance o ajudante de pin **sans** `--skip-submit` quando você estiver prestes a expor o hotel. Insira a configuração JSON com os sinalizadores CLI explícitos:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

O comando escrito `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json`, que deve acompanhar o pacote de evidências de convites.

## Etapa 4 - Gerar o plano de DNS básico

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Compartilhe o JSON resultante com operações para que o DNS básico faça referência ao resumo exato do manifesto. Quando um descritor precedente é reutilizado como fonte de reversão, adicione `--previous-dns-plan path/to/previous.json`.

## Etapa 5 - Sondagem do hotel implantado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

A sonda confirma a etiqueta de serviço de lançamento, os cabeçalhos CSP e os metadonnees de assinatura. Relance o comando de duas regiões (ou junte um sortie curl) para que os auditores saibam que a borda do cache está quente.

## Pacote de evidências

Inclua os seguintes artefatos no ticket da visualização vaga e faça referência a eles no e-mail de convite:

| Artefato | Objetivo |
|----------|----------|
| `build/checksums.sha256` | Prove que o pacote corresponde ao build CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga útil SoraFS canônico + manifesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montre que la soumission du manifeste + le binding d'alias ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadonnees DNS (ticket, fentre, contatos), currículo de promoção de rota (`Sora-Route-Binding`), ponteiro `route_plan` (plano JSON + modelos de cabeçalho), informações de limpeza de cache e instruções de reversão para operações. |
| `artifacts/sorafs/preview-descriptor.json` | Descritor que assina o arquivo + soma de verificação. |
| Sortie `probe` | Confirme que o hotel on-line anuncia a tag de lançamento. |