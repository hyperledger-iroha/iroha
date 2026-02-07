---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Planejando a soma de verificação do controle

Este plano descreve o trabalho, não é necessário para isso, é um artefacto de produção de portal Você pode precisar de mais informações antes de publicar. Цель — гарантировать, что рецензенты скачивают точный снимок, собранный в CI, что манифест checksum неизменяем, и Este produto é fornecido pelo SoraFS com o metadado Norito.

##Céli

- **Детерминированные сборки:** обеспечить, что `npm run build` дает воспроизводимый результат и всегда создает `build/checksums.sha256`.
- **Proverенные предпросмотры:** требовать, чтобы каждый артефакт предпросмотра включал манифест checksum e запрещать публикацию при провале проверки.
- **Метаданные, публикуемые через Norito:** сохранять дескрипторы предпросмотра (метаданные коммита, digest checksum, CID SoraFS) como Norito JSON, esses instrumentos podem ser acessados com privilégios de áudio.
- **Instrumentos para operadores:** предоставить одношаговый скрипт проверки, который потребители могут запускать local (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); скрипт теперь оборачивает поток проверки checksum + дескриптора целиком. Стандартная команда предпросмотра (`npm run serve`) теперь автоматически вызывает этот помощник перед `docusaurus serve`, чтобы локальные снимки оставались под контролем checksum (por este `npm run serve:verified` сохранен как явный алиас).

## Fase 1 — Controle no CI

1. Verifique `.github/workflows/docs-portal-preview.yml`, resultados:
   - Selecione `node docs/portal/scripts/write-checksums.mjs` para usar o Docusaurus (você pode localizá-lo).
   - Verifique `cd build && sha256sum -c checksums.sha256` e prove o trabalho antes de terminar.
   - Упаковывать директорию build em `artifacts/preview-site.tar.gz`, copiando checksum de gerenciamento, вызывать `scripts/generate-preview-descriptor.mjs` e выполнять `scripts/sorafs-package-preview.sh` com JSON-конфигурацией (como `docs/examples/sorafs_preview_publish.json`), quais fluxos de trabalho são visualizados e metadados, e determinados SoraFS-бандл.
   - Загружать статический сайт, артефакты метаданных (`docs-portal-preview`, `docs-portal-preview-metadata`) e SoraFS-bandl (`docs-portal-preview-sorafs`), este manual, seu carro e o plano podem ser fornecidos sem problemas.
2. Faça comentários no CI-бейджем, definindo a solução de verificação da soma de verificação na solicitação pull (realizando a verificação comentários GitHub Script em `docs-portal-preview.yml`).
3. Verifique o fluxo de trabalho em `docs/portal/README.md` (criado CI) e verifique os dados na lista de verificação pública.

## Provérbio de script

`docs/portal/scripts/preview_verify.sh` fornece vários artefatos que não são usados no `sha256sum`. Use `npm run serve` (ou seu nome alternativo `npm run serve:verified`), este script é salvo e `docusaurus serve` para o mesmo nome при распространении локальных снимков. Provérbios lógicos:

1. Abra o utilitário SHA (`sha256sum` ou `shasum -a 256`) pelo `build/checksums.sha256`.
2. При необходимости сравнивает digest/имя файла дескриптора предпросмотра `checksums_manifest` и, если указан, digest/имя файла архива предпросмотра.
3. Abra um código inválido para um local sem problemas, com os quais os projetos podem ser bloqueados предпросмотры.

Exemplo de utilização (por meio de artefactos CI):```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Os engenheiros de CI e os repositórios de software usam scripts de código de barras, para que o pacote de pacotes seja pré-processado ou configurado артефакты к релизному тикету.

## Fase 2 — Publicação SoraFS

1. O fluxo de trabalho de fluxo de trabalho é definido como:
   - Faça o backup no staging-шлюз SoraFS com o `sorafs_cli car pack` e `manifest submit`.
   - Abra o manual de resumo e o CID SoraFS.
   - Сериализует `{ commit, branch, checksum_manifest, cid }` em Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Gerencie o descritor do site do artista e abra o CID no pull request.
3. Faça testes de integração, usando o `sorafs_cli` em regime de simulação, para obter uma boa configuração сохраняли совместимость схемы метаданных.

## Fase 3 — Atualização e auditoria

1. Abra Norito-схему (`PreviewDescriptorV1`), consulte a descrição da estrutura em `docs/portal/schemas/`.
2. Verifique a lista de verificação pública DOCS-SORA, operação:
   - Verifique `sorafs_cli manifest verify` para proteger o CID.
   - Фиксировать digest манифеста checksum и CID в описании релизного PR.
3. Faça a atualização automática do descritor de tela com a soma de verificação do manipulador no momento da conclusão релизам.

## Результаты e владельцы

| Etap | Владелец(ы) | Cel | Nomeação |
|------|------------|------|--------|
| Soma de verificação de controle no CI externo | Documentos de infraestrutura | Nedelia 1 | Construa o portão e os artefatos de construção. |
| Publicação de vendas em SoraFS | Infraestrutura Docs / Commanda Storage | Nedelia 2 | Требует доступа к staging-учетным данным e обновлений схемы Norito. |
| Atualização de integração | Lide Docs/DevRel / WG para atualização | Nedelia 3 | Publique o esquema e obtenha instruções e roteiro. |

## Открытые вопросы

- Какая среда SoraFS должна хранить артефакты предпросмотра (staging vs. выделенная pista de visualização)?
- Não há nenhuma fonte de dados (Ed25519 + ML-DSA) para definir o procedimento antes de ser publicado?
- O fluxo de trabalho CI закреплять конфигурацию orquestrador (`orchestrator_tuning.json`) при запуске `sorafs_cli`, чтобы сохранять воспроизводимость манифестов?

Verifique a configuração em `docs/portal/docs/reference/publishing-checklist.md` e atualize este plano para uma operação nova.