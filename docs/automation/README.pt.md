---
lang: pt
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Bases de automação da documentação

Este diretório captura as superfícies de automação mencionadas nos itens do
roadmap AND5/AND6 (Android Developer Experience + Release Readiness) e DA-1
(automação do modelo de ameaças de Disponibilidade de Dados) quando exigem
evidências documentais auditáveis. Manter as referências de comandos e os
artefatos esperados no repositório garante que os pré-requisitos de revisões de
conformidade fiquem disponíveis mesmo quando pipelines de CI ou painéis estiverem
offline.

## Estrutura do diretório

| Caminho | Propósito |
|---------|-----------|
| `docs/automation/android/` | Baselines de automação para documentação e localização Android (AND5), incluindo logs de sincronização de stubs i18n, resumos de paridade e evidências de publicação do SDK exigidas antes do aceite AND6. |
| `docs/automation/da/` | Saídas de automação do modelo de ameaças de Data Availability referenciadas por `cargo xtask da-threat-model-report` e pela atualização noturna da documentação. |

Cada subdiretório documenta os comandos que produzem a evidência junto com o
layout de arquivos esperado (normalmente resumos JSON, logs ou manifestos). As
equipes depositam novos artefatos na pasta correspondente sempre que uma execução
de automação altera materialmente os docs publicados e, em seguida, vinculam o
commit na entrada relevante de status/roadmap.

## Uso

1. **Execute a automação** usando os comandos descritos no README do subdiretório
   (por exemplo, `ci/check_android_fixtures.sh` ou
   `cargo xtask da-threat-model-report`).
2. **Copie os artefatos JSON/log resultantes** de `artifacts/…` para a pasta
   `docs/automation/<program>/…` correspondente, com um carimbo de data/hora
   ISO-8601 no nome do arquivo para que os auditores possam correlacionar a
   evidência com as atas de governança.
3. **Referencie o commit** em `status.md`/`roadmap.md` ao fechar um gate do
   roadmap, para que os revisores confirmem a baseline de automação usada na
   decisão.
4. **Mantenha os arquivos leves.** A expectativa é de metadados estruturados,
   manifestos ou resumos - não blobs binários grandes. Dumps maiores devem ficar
   em object storage, com a referência assinada registrada aqui.

Ao centralizar estas notas de automação, destravamos o pré-requisito de
“baselines de docs/automation disponíveis para auditoria” citado por AND6 e
damos ao fluxo do modelo de ameaças de DA um ponto determinístico para os
relatórios noturnos e verificações pontuais manuais.
