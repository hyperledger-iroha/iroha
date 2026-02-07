---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plano de pré-visualização com checksum

Este plano descreve o trabalho restante necessário para tornar cada artefato de pré-visualização do portal verificado antes da publicação. O objetivo é garantir que os revisores baixem exatamente o snapshot construído em CI, que o manifesto de checksum seja imutavel e que a pré-visualização seja descoberta via SoraFS com metadados Norito.

## Objetivos

- **Construções determinísticas:** Garantir que `npm run build` produza dita reproduzível e sempre gere `build/checksums.sha256`.
- **Pré-visualizações verificadas:** Exigir que cada artefacto de pré-visualização inclua um manifesto de checksum e recusar a publicação quando uma verificação falhar.
- **Metados publicados via Norito:** Persistir descritores de pré-visualização (metadados de commit, digest de checksum, CID SoraFS) como JSON Norito para que as ferramentas de governança possam auditar releases.
- **Ferramentas para operadores:** Fornecer um script de verificação de um passo que os consumidores possam executar localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); o script agora envolve o fluxo de validação de checksum + descritor de ponta a ponta. O comando padrão de pré-visualização (`npm run serve`) agora invoca esse helper automaticamente antes de `docusaurus serve` para que snapshots locais permaneçam protegidos por checksum (com `npm run serve:verified` mantidos como alias explícito).

## Fase 1 - Aplicação em CI

1. Atualizar `.github/workflows/docs-portal-preview.yml` para:
   - Executar `node docs/portal/scripts/write-checksums.mjs` após o build do Docusaurus (já invocado localmente).
   - Executar `cd build && sha256sum -c checksums.sha256` e falhar no job em caso de divergência.
   - Empacotar o diretório build como `artifacts/preview-site.tar.gz`, copiar o manifesto de checksum, executar `scripts/generate-preview-descriptor.mjs` e executar `scripts/sorafs-package-preview.sh` com uma configuração JSON (ver `docs/examples/sorafs_preview_publish.json`) para que o fluxo de trabalho emita tantos metadados quanto um pacote determinístico SoraFS.
   - Enviar o site estatístico, os artefatos de metadados (`docs-portal-preview`, `docs-portal-preview-metadata`) e o pacote SoraFS (`docs-portal-preview-sorafs`) para que o manifesto, o resumo CAR e o plano possam ser executados sem refazer o build.
2. Adicione um comentário de crachá CI resumindo o resultado da verificação de checksum em pull requests (implementado por meio do passo de comentário GitHub Script de `docs-portal-preview.yml`).
3. Documentar o fluxo de trabalho em `docs/portal/README.md` (seção CI) e vincular as etapas de verificação na checklist de publicação.

## Script de verificação

`docs/portal/scripts/preview_verify.sh` valida os artefatos de pré-visualização baixados sem as invocações manuais de `sha256sum`. Use `npm run serve` (ou o alias explícito `npm run serve:verified`) para executar o script e iniciar `docusaurus serve` em um único passo ao compartilhar snapshots locais. A lógica de verificação:

1. Execute a ferramenta SHA correspondente (`sha256sum` ou `shasum -a 256`) contra `build/checksums.sha256`.
2. Opcionalmente compare o digest/nome do arquivo do descritor de pré-visualização `checksums_manifest` e, quando fornecido, o digest/nome do arquivo do arquivo de pré-visualização.
3. Sai com código não zero quando alguma divergência e bloqueada para que os revisores possam bloquear pré-visualizações adulteradas.

Exemplo de uso (após extrair os artefatos de CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Engenheiros de CI e release devem executar o script sempre que baixarem um pacote de pré-visualização ou fixação em artefatos a um ticket de release.

## Fase 2 - Publicação em SoraFS

1. Estender o fluxo de trabalho de pré-visualização com um trabalho que:
   - Envie o site construído para o gateway de staging do SoraFS usando `sorafs_cli car pack` e `manifest submit`.
   - Capture o digest do manifesto retornado e o CID do SoraFS.
   - Serializa `{ commit, branch, checksum_manifest, cid }` em JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Armazenar o descritor junto com os artistas de build e exportar o CID sem comentários ao pull request.
3. Adicione testes de integração que exercitem `sorafs_cli` em modo dry-run para garantir que mudanças futuras atualizem a consistência do esquema de metadados.

## Fase 3 – Governança e auditoria

1. Publicar um esquema Norito (`PreviewDescriptorV1`) descrevendo uma estrutura do descritor em `docs/portal/schemas/`.
2. Atualizar um checklist de publicação DOCS-SORA para exigir:
   - Rodar `sorafs_cli manifest verify` contra o CID enviado.
   - Registrador do resumo do manifesto de checksum e do CID na descrição do PR de release.
3. Ligar a automação de governança para cruzar o descritor com o manifesto de checksum durante votos de liberação.

## Entregas e responsabilidades| Marco | Proprietário(s) | Alvo | Notas |
|-------|-----------------|------|-------|
| Aplicação de checksum em CI concluída | Infraestrutura de Documentos | Semana 1 | Adicionado portão de falha e upload de artefatos. |
| Publicação de pré-visualização no SoraFS | Infraestrutura de Docs / Equipe de Storage | Semana 2 | Solicite acesso a credenciais de staging e atualizações do esquema Norito. |
| Integração de governança | Líder de Docs/DevRel / GT de Governança | Semana 3 | Publica o esquema e atualiza checklists e entradas do roadmap. |

## Perguntas em aberto

- Qual ambiente do SoraFS deve apresentar artefatos de pré-visualização (staging vs. pista de pré-visualização dedicada)?
- precisamos de assinaturas duplas (Ed25519 + ML-DSA) no descritor de pré-visualização antes da publicação?
- O fluxo de trabalho de CI deve fixar a configuração do orquestrador (`orchestrator_tuning.json`) ao executar `sorafs_cli` para manter manifestos reproduzíveis?

Registre as decisões em `docs/portal/docs/reference/publishing-checklist.md` e atualize este plano quando as dúvidas forem resolvidas.