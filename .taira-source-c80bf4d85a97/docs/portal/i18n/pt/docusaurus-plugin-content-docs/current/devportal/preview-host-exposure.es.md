---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guia de exposição do host de visualização

O roteiro DOCS-SORA exige que cada visualização pública use o mesmo pacote selecionado por soma de verificação que os revisores verificam localmente. Use este runbook após concluir a integração dos revisores (e o ticket de aprovação de convites) para colocar online o host da versão beta de visualização.

## Requisitos anteriores

- Ola de onboarding de revisores aprovados e registrados no tracker de visualização.
- Última compilação do portal presente em `docs/portal/build/` e checksum selecionado (`build/checksums.sha256`).
- Credenciais de visualização SoraFS (URL de Torii, autoridade, chave privada, época enviada) armazenadas em variáveis ​​de ambiente ou em uma configuração JSON como [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de mudança DNS aberto com o nome de host desejado (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) mas contatos de plantão.

## Paso 1 - Construir e verificar o pacote

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

O script de verificação não será continuado quando a manifestação de checksum estiver faltando ou for manipulada, mantendo auditado cada artefato de visualização.

## Paso 2 - Empaquetar os artefatos SoraFS

Converta o site estatístico em um CAR/manifesto determinista. `ARTIFACT_DIR` por defeito é `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Adjunta `portal.car`, `portal.manifest.*`, o descritor e o manifesto de checksum no ticket da tela de visualização.

## Paso 3 - Publicar o alias de visualização

Repete o ajudante do pino **sin** `--skip-submit` quando esta lista é exibida para expor o host. Proporciona a configuração JSON ou flags CLI explícitos:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

O comando descreve `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json`, que devem viajar com o pacote de evidências de convites.

## Passo 4 - Gerar o plano de corte DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Compare o JSON resultante com Ops para que a mudança de DNS faça referência ao resumo da manifestação exata. Ao reutilizar um descritor anterior como fonte de rollback, adicione `--previous-dns-plan path/to/previous.json`.

## Paso 5 - Probar el host desplegado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

A sonda confirma a tag de lançamento servida, cabeçalhos CSP e metadados de empresa. Repita o comando de duas regiões (ou adicione a saída de curl) para que os auditores façam com que o cache do edge esteja quente.

## Pacote de evidências

Inclua os seguintes artefatos no ticket da data de pré-visualização e referências no e-mail de convite:

| Artefato | Proposta |
|----------|-----------|
| `build/checksums.sha256` | Demonstra que o pacote coincide com a construção do CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga útil canônico SoraFS + manifesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Mostra que o envio do manifesto + o alias vinculativo foi concluído. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (ticket, janela, contatos), resumo de promoção de rota (`Sora-Route-Binding`), o puntero `route_plan` (plano JSON + planta de cabeçalho), informações de limpeza de cache e instruções de reversão para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descritor firmado que coloca o arquivo + checksum. |
| Saída de `probe` | Confirme que o host ao vivo anuncia a etiqueta de lançamento esperada. |