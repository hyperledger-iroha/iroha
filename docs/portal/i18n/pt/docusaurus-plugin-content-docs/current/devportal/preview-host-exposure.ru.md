---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Руководство по экспозиции preview-хоста

Дорожная карта DOCS-SORA требует, чтобы каждый публичный preview использовал тот же bundle, проверенный checksum, который ревьюеры проверяют локально. Utilize este runbook para obter atualizações on-line (e obtenha uma licença de uso), para obter o host de visualização beta em sim.

## Предварительные требования

- Você pode atualizar a versão e registrar-se no rastreador de visualização.
- Você pode inserir um portal em `docs/portal/build/` e verificar a soma de verificação (`build/checksums.sha256`).
- Учётные данные SoraFS preview (Torii URL, Authority, Private Key, отправленный época) сохранены в переменных окружения или JSON Verifique, por exemplo [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Abra um tíquete de configuração de DNS com um nome de host (`docs-preview.sora.link`, `docs.iroha.tech` e etc.) e contatos de plantão.

## Passo 1 - Pacote de entrega e fornecimento

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

O script fornece uma verificação de soma de verificação ou uma verificação de soma de verificação, ou uma visualização prévia artefactos.

## Passo 2 - Artefatos de expansão SoraFS

Verifique a situação do carro/manifesto. `ARTIFACT_DIR` para uso `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Use `portal.car`, `portal.manifest.*`, descritor e manifeste a soma de verificação para a onda de visualização do ticket.

## Passo 3 - Alias de visualização do Опубликовать

Запустите pin helper **без** `--skip-submit`, когда будете готовы открыть хост. Transfira a configuração JSON ou suas bandeiras CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

O comando `portal.pin.report.json`, `portal.manifest.submit.summary.json` e `portal.submit.response.json` está disponível no pacote de evidências.

## Passo 4 - Plano de transferência de DNS do gerenciador

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Usando JSON com Ops, essa configuração de DNS está configurada para o manifesto de resumo. Para usar o descritor de reversão anterior, use `--previous-dns-plan path/to/previous.json`.

## Passo 5 - Проверить развернутый хост

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Probe pode identificar a tag de lançamento, CSP заголовки e метаданные подписи. Ao usar o comando de duas regiões (ou usar o curl), esses auditores serão exibidos, esse programa de cache de borda.

## Pacote de evidências

Включите следующие артефакты в тикет preview wave e укажите их в письме-приглашении:

| Artefato | Atualizado |
|----------|------------|
| `build/checksums.sha256` | Observe que este pacote contém a compilação do CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Carga útil SoraFS + manifesto. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Показывает, что отправка manifest e привязка alias успешны. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS metadaнные (тикет, окно, контакты), сводка продвижения маршрута (`Sora-Route-Binding`), указатель `route_plan` (JSON plan + cabeçalho de cabeçalho), sucessivamente a limpeza de cache e a reversão de instruções para Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descritor Подписанный, arquivo связывающий + soma de verificação. |
| Chave `probe` | É claro que o host ao vivo publica a tag de lançamento. |