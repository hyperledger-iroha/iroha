---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plano de pré-visualização com checksum

Este plano detalha o trabalho restante necessário para que cada artefato de pré-visualização do portal seja verificado antes da publicação. O objetivo é garantir que os revisores baixem exatamente o instante construído no CI, que o manifesto de checksum seja imutável e que a pré-visualização seja descubrível através de SoraFS com metadados Norito.

## Objetivos

- **Compilações determinísticas:** Certifique-se de que `npm run build` produz saída reproduzível e sempre emite `build/checksums.sha256`.
- **Pré-visualizações verificadas:** Verifique se cada artefato de pré-visualização inclui uma manifestação de soma de verificação e recarregue a publicação quando a verificação falhar.
- **Metadados publicados com Norito:** Persista os descritores de pré-visualização (metadados de commit, resumo de checksum, CID de SoraFS) como JSON de Norito para que as ferramentas de governança possam auditar os lançamentos.
- **Ferramentas para operadores:** Prove um script de verificação de uma única etapa que os consumidores possam executar localmente (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); o script agora envolve o fluxo de validação de checksum + descritor de ponto a ponto. O comando padrão de pré-visualização (`npm run serve`) agora chama este helper automaticamente antes de `docusaurus serve` para que as localidades instantâneas permaneçam protegidas por checksum (com `npm run serve:verified` mantido como alias explícito).

## Fase 1 - Aplicação em CI

1. Atualizar `.github/workflows/docs-portal-preview.yml` para:
   - Execute `node docs/portal/scripts/write-checksums.mjs` após a compilação de Docusaurus (você será chamado localmente).
   - Executar `cd build && sha256sum -c checksums.sha256` e falhar no trabalho se houver discrepâncias.
   - Empaque o diretório build como `artifacts/preview-site.tar.gz`, copie o manifesto de checksum, execute `scripts/generate-preview-descriptor.mjs` e execute `scripts/sorafs-package-preview.sh` com uma configuração JSON (ver `docs/examples/sorafs_preview_publish.json`) para que o fluxo de trabalho emita tantos metadados como um pacote SoraFS determinístico.
   - Subir o site estatístico, os artefatos de metadados (`docs-portal-preview`, `docs-portal-preview-metadata`) e o pacote SoraFS (`docs-portal-preview-sorafs`) para que o manifesto, o currículo CAR e o plano possam ser inspecionados sem voltar a executar o construir.
2. Agregar um comentário com insígnia de CI que resume o resultado da verificação de soma de verificação nas solicitações pull (implementado por meio da etapa de comentário GitHub Script de `docs-portal-preview.yml`).
3. Documente o fluxo de trabalho em `docs/portal/README.md` (seção CI) e siga as etapas de verificação na lista de verificação de publicação.

## Script de verificação

`docs/portal/scripts/preview_verify.sh` valida os artefatos de pré-visualização baixados sem exigir invocações manuais de `sha256sum`. Use `npm run serve` (ou o alias explícito `npm run serve:verified`) para executar o script e lançar `docusaurus serve` em um único passo ao compartilhar localidades instantâneas. A lógica de verificação:

1. Execute a ferramenta SHA adequada (`sha256sum` ou `shasum -a 256`) contra `build/checksums.sha256`.
2. Opcionalmente compare o resumo/nome do arquivo do descritor de pré-visualização `checksums_manifest` e, quando fornecido, o resumo/nome do arquivo do arquivo de pré-visualização.
3. Venda com código diferente de zero quando for detectada qualquer discrepância para que os revisores possam bloquear pré-visualizações manipuladas.

Exemplo de uso (depois de extrair os artefatos de CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Os engenheiros de CI e de lançamento devem executar o script sempre que descarregarem um pacote de pré-visualização ou artefatos adicionais a um ticket de lançamento.

## Fase 2 - Publicação em SoraFS

1. Estenda o fluxo de trabalho de pré-visualização com um trabalho que:
   - Suba o site construído no gateway de teste de SoraFS usando `sorafs_cli car pack` e `manifest submit`.
   - Capture o resumo do manifesto enviado e o CID de SoraFS.
   - Serialice `{ commit, branch, checksum_manifest, cid }` e JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Armazene o descritor junto com o artefato de construção e exponha o CID no comentário do pull request.
3. Agregar testes de integração que executam `sorafs_cli` em modo dry-run para garantir que mudanças futuras mantenham a coerência do esquema de metadados.

## Fase 3 - Governança e auditoria1. Publicar um esquema Norito (`PreviewDescriptorV1`) que descreve a estrutura do descritor em `docs/portal/schemas/`.
2. Atualize a lista de verificação da publicação DOCS-SORA para exigir:
   - Execute `sorafs_cli manifest verify` contra o CID carregado.
   - Registrar o resumo do manifesto de checksum e o CID na descrição do PR de lançamento.
3. Conecte a automação de governança para cruzar o descritor com o manifesto de checksum durante as votações de liberação.

## Entregáveis e responsáveis

| Hito | Responsáveis(es) | Objetivo | Notas |
|------|----------------|----------|-------|
| Aplicação de checksum em CI completada | Infraestrutura de Documentos | Semana 1 | Agregar um portão de queda e cargas de artefatos. |
| Publicação de pré-visualizações em SoraFS | Infraestrutura de Documentação / Equipamento de Armazenamento | Semana 2 | Requer acesso a credenciais de preparação e atualizações do esquema Norito. |
| Integração de governança | Líder de Docs/DevRel / WG de Gobernanza | Semana 3 | Publica o esquema e atualiza listas de verificação e entradas do roteiro. |

## Perguntas abertas

- Que ambiente de SoraFS deve ser colocado nos artefatos de pré-visualização (staging vs. carril de pré-visualização dedicado)?
- Necessitamos firmas duales (Ed25519 + ML-DSA) no descritor de pré-visualização antes da publicação?
- O fluxo de trabalho do CI deve definir a configuração do orquestrador (`orchestrator_tuning.json`) para executar `sorafs_cli` para manter as manifestações reproduzíveis?

Registre as decisões em `docs/portal/docs/reference/publishing-checklist.md` e atualize este plano quando as dúvidas forem resolvidas.