---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Revue de sécurité SF-6
resumo: Constatações e ações de acompanhamento da avaliação independente da assinatura sans clé, do streaming de prova e dos pipelines de envio de manifestos.
---

# Revue de segurança SF-6

**Fenêtre d'évaluation :** 10/02/2026 → 18/02/2026  
**Líderes de revisão:** Guilda de Engenharia de Segurança (`@sec-eng`), Grupo de Trabalho de Ferramentas (`@tooling-wg`)  
**Perímetro:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de streaming de prova, gerenciamento de manifestos em Torii, integração Sigstore/OIDC, ganchos de liberação CI.  
**Artefatos:**  
- Fonte CLI e testes (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifesto/prova de manipuladores Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Liberação de automação (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harness de parité déterminista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Rapport de parité GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Metodologia

1. **Ateliers de modelagem de ameaças** cartografaram as capacidades de ataque para os postos de desenvolvimento, os sistemas CI e os nós Torii.  
2. **Revisão de código** fornece superfícies de identificação (troca de tokens OIDC, assinatura sem clé), validação de manifestos Norito e pressão de retorno do streaming de prova.  
3. **Testes dinâmicos** reiniciam manifestos de fixtures e simulam modos de panne (replay de token, adulteração de manifesto, fluxos de prova encerrados) por meio do chicote de paridade e dos fuzz drives dedicados.  
4. **Inspeção de configuração** valida os padrões `iroha_config`, gerencia os sinalizadores CLI e os scripts de lançamento para garantir execuções determinadas e auditáveis.  
5. **Entretien de processus** para confirmar o fluxo de remediação, os caminhos da escalada e a captura de evidências de auditoria com os proprietários de liberação do Tooling WG.

## Currículo de constantes| ID | Sévérité | Zona | Constante | Resolução |
|----|----------|------|---------|------------|
| SF6-SR-01 | Elevado | Assinatura sem clé | Os padrões de audiência dos tokens OIDC estão implícitos nos modelos CI, com risco de reprodução entre locatários. | Adicionada uma exigência explícita `--identity-token-audience` nos ganchos de lançamento e modelos CI ([processo de lançamento](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI échoue désormais si l'audience est omise. |
| SF6-SR-02 | Moyenne | Transmissão de prova | Os caminhos de contrapressão aceitam buffers de assinantes sem limite, permitindo o armazenamento de memória. | `sorafs_cli proof stream` impõe as caudas do canal nascidas com um truncamento determinado, publica os currículos Norito e aborta a transmissão; O espelho Torii foi criado recentemente para gerar blocos de resposta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Moyenne | Envio de manifestos | A CLI aceita os manifestos sem verificar os planos de blocos embarcados quando `--plan` está ausente. | `sorafs_cli manifest submit` recalcula e compara os resumos CAR, mesmo que `--expect-plan-digest` seja fornecido, rejeitando incompatibilidades e expondo dicas de correção. Os testes foram bem-sucedidos/é verificados (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Faível | Trilha de auditoria | A lista de verificação de liberação não foi disponibilizada antes do registro de aprovação assinado para a revista de segurança. | Adicionar uma seção [processo de liberação] (../developer-releases.md) exige o anexo de hashes do memorando de revisão e o URL do ticket de assinatura antes do GA. |

Todas as estatísticas altas/médias foram corrigidas durante a abertura da revista e validadas pelo equipamento de paridade existente. Aucun questão crítica latente ne reste.

## Validação de controles

- **Portée des identifiants :** Os modelos CI impõem a classificação do público e do emissor explícitos ; a CLI e o auxiliar de liberação são ouvidos rapidamente se `--identity-token-audience` não acompanham `--identity-token-provider`.  
- **Replay déterministe :** Os testes do dia atual cobrem os fluxos positivos/negativos do envio de manifestos, garantindo que os resumos incompatíveis permaneçam com os cheques não determinados e sejam sinalizados antes de tocar na rede.  
- **Streaming à prova de contrapressão:** Torii difunde os itens PoR/PoTR através dos canais nascidos, e o CLI não conserva que os níveis de latência tronqués + cinco exemplos de cheque, evitando o croissance sans limit tout en gardant des résumés déterministes.  
- **Observabilidade:** O streaming de prova de computadores (`torii_sorafs_proof_stream_*`) e os currículos CLI capturam as razões de aborto, oferecendo migalhas de auditoria aos operadores.  
- **Documentação :** Os guias devs ([índice do desenvolvedor](../developer-index.md), [referência CLI](../developer-cli.md)) sinalizam os sinalizadores sensíveis e os fluxos de trabalho em escala.

## Adicionado à lista de verificação de lançamento

Os gerentes de lançamento **doivent** juntam-se às seguintes sugestões para a promoção de um candidato GA:1. Hash du memorando de revista de segurança mais recente (este documento).  
2. Garantia do bilhete de remediação seguinte (ex. `governance/tickets/SF6-SR-2026.md`).  
3. A saída de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostra os argumentos público/emissor explícitos.  
4. Registros capturados do arnês de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmação de que as notas de lançamento Torii incluem os computadores de transmissão de prova de streaming nascidos.

Não colecione os artefatos ci-dessus bloqueando a assinatura do GA.

**Hashes dos artefatos de referência (assinatura em 20/02/2026):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Suivis attente

- **Mise à jour du Threat Model:** Répéter cette revue chaque trimestre ou avant des ajouts majeurs de flags CLI.  
- **Couverture fuzzing:** As codificações de streaming à prova de transporte são distorcidas via `fuzz/proof_stream_transport`, identidade couvrant, gzip, deflate e zstd.  
- **Repetição de incidente:** Planeje um operador de exercício simulando um comprometimento de token e uma reversão de manifesto, para garantir que a documentação reflita os procedimentos praticados.

## Aprovação

- Representante da Guilda de Engenharia de Segurança: @sec-eng (2026/02/20)  
- Grupo de Trabalho de Ferramentas Representante: @tooling-wg (2026-02-20)

Guarde as aprovações assinadas com o pacote de artefatos de lançamento.