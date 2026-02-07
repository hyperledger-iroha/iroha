---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: integração do operador nexus
título: Integração de operadores de espaço de dados Sora Nexus
description: Espelho de `docs/source/sora_nexus_operator_onboarding.md`, acompanhando um checklist de release end-to-end para operadores Nexus.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sora_nexus_operator_onboarding.md`. Mantenha as duas cópias homologadas até que as edicoes localizadas cheguem ao portal.
:::

# Integração de operadores de espaço de dados Sora Nexus

Este guia de captura do fluxo ponta a ponta que os operadores de espaço de dados Sora Nexus devem seguir quando um lançamento e anunciado. Ele complementa o runbook de dupla via (`docs/source/release_dual_track_runbook.md`) e uma nota de seleção de artistas (`docs/source/release_artifact_selection.md`) descrevendo como alinhar pacotes/imagens baixados, manifestos e modelos de configuração com as expectativas de pistas globais antes de colocar um no em produção.

## Audiência e pré-requisitos
- Você foi aprovado pelo Programa Nexus e recebeu sua atribuição de espaço de dados (índice de pista, ID/alias de espaço de dados e requisitos de política de roteamento).
- Você consegue acessar os contratos assinados do release publicado pela Release Engineering (tarballs, imagens, manifestos, assinaturas, chaves públicas).
- Você gerou ou recebeu material de chaves de produção para seu papel de validador/observador (identidade no Ed25519; chave de consenso BLS + PoP para validadores; mais quaisquer alternâncias de recursos disponíveis).
- Você consegue alcançar os peers Sora Nexus existentes que farao bootstrap do seu no.

## Etapa 1 - Confirmar o perfil de lançamento
1. Identificação ou apelido de rede ou ID de cadeia fornecido.
2. Execute `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) em um checkout deste repositório. O helper consulta `release/network_profiles.toml` e imprime o perfil para implantar. Para Sora Nexus a resposta deve ser `iroha3`. Para qualquer outro valor, pare e entre em contato com a Release Engineering.
3. Anote a tag de versão referenciada no anúncio do release (por exemplo `iroha3-v3.2.0`); voce o usara para buscar artefatos e manifestos.

## Etapa 2 - Recuperar e validar arquitetos
1. Baixe o pacote `iroha3` (`<profile>-<version>-<os>.tar.zst`) e seus arquivos companheiros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` se você fizer deploy com contenedores).
2. Valide a integridade antes de descompactar:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Substitua `openssl` pelo selecionado aprovado pela organização se você usar um KMS com hardware.
3. Inspecione `PROFILE.toml` dentro do tarball e os manifestos JSON para confirmar:
   -`profile = "iroha3"`
   - Os campos `version`, `commit` e `built_at` incluídos ao anúncio do release.
   - O SO/arquitetura específica ao alvo de implantação.
4. Se você usar a imagem do contêiner, repita a verificação de hash/assinatura para `<profile>-<version>-<os>-image.tar` e confirme o ID da imagem registrada em `<profile>-<version>-image.json`.

## Etapa 3 - Preparar configuração a partir dos templates
1. Extraia o pacote e copie `config/` para o local onde não vai ler sua configuração.
2. Trate os arquivos sob `config/` como templates:
   - Substitua `public_key`/`private_key` pelas suas chaves Ed25519 de produção. Remova as chaves privadas do disco se ou não forem obtidas de um HSM; atualize a configuração para ponta para o conector HSM.
   - Ajuste `trusted_peers`, `network.address` e `torii.address` para refletir suas interfaces alcancáveis ​​e os pares de bootstrap que foram atribuídos.
   - Atualizar `client.toml` com o endpoint Torii para operadoras (incluindo configuração TLS se aplicável) e as credenciais provisionadas para ferramentas operacionais.
3. Mantenha o ID da cadeia fornecido no pacote, a menos que as instruções de governança sejam explicitamente - a pista global espera um identificador único canônico de cadeia.
4. Planeje iniciar ou não com a bandeira do perfil Sora: `irohad --sora --config <path>`. O loader de configuração rejeitará configurações SoraFS ou multi-lane se o sinalizador estiver ausente.## Etapa 4 - Alinhar metadados de espaço de dados e roteamento
1. Edite `config/config.toml` para que a seção `[nexus]` corresponda ao catálogo de espaço de dados fornecido pelo Nexus Conselho:
   - `lane_count` deve ser igual ao total de pistas habilitadas na época atual.
   - Cada entrada em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` deve conter um `index`/`id` único e os aliases acordados. Não apague as entradas globais existentes; adicione seus pseudônimos delegados se o conselho atribuir espaços de dados adicionais.
   - Garantir que cada entrada de espaço de dados incluindo `fault_tolerance (f)`; comites lane-relay são dimensionados em `3f+1`.
2. Atualize `[[nexus.routing_policy.rules]]` para capturar a política que lhe foi dada. O modelo padrão roteia instruções de governança para a via `1` e implanta contratos para a via `2`; adicione ou modifique regras para que o tráfego destinado ao seu espaço de dados seja direcionado para a pista e o alias correto. Coordene com Engenharia de Liberação antes de alterar a ordem das regras.
3. Revisar os limites de `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. Espera-se que os operadores mantenham os valores aprovados pelo conselho; ajuste apenas se uma política atualizada foi ratificada.
4. Registre a configuração final no seu tracker de operações. O runbook de lançamento de dupla via exige fixação o `config.toml` efetivo (com segredos redigidos) ao ticket de onboarding.

## Etapa 5 - Validação pré-voo
1. Execute o validador de configuração embutido antes de entrar na rede:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Isso imprime uma configuração resolvida e falha precocemente se as entradas de catálogo/roteamento forem inconsistentes ou se a gênese e a configuração divergirem.
2. Se você fizer deploy com contenedores, execute o mesmo comando dentro da imagem após carregar com `docker load -i <profile>-<version>-<os>-image.tar` (lembre de incluir `--sora`).
3. Verifique os registros para avisos sobre identificadores de espaço reservado de pista/espaço de dados. Se houver, retorne à Etapa 4 - implantações de produção não devem depender dos IDs placeholder que acompanham os templates.
4. Execute seu procedimento local de fumaça (por exemplo, envie uma consulta `FindNetworkStatus` com `iroha_cli`, confirme que os endpoints de telemetria expoem `nexus_lane_state_total`, e verifique se as chaves de streaming foram rotacionadas ou importadas conforme necessário).

## Etapa 6 - Corte e entrega
1. Guarde o `manifest.json` selecionado e os artistas de assinatura no ticket de liberação para que os auditores possam reproduzir suas verificações.
2. Notifique Nexus Operações de que o não está pronto para ser introduzido; incluindo:
   - Identidade não (peer ID, nomes de host, endpoint Torii).
   - Valores eficazes do catálogo de lane/data-space e política de roteamento.
   - Hashes dos binários/imagens selecionadas.
3. Coordenação de admissão final de pares (gossip seed e atribuição de lane) com `@nexus-core`. Não entre na rede até receber aprovação; Sora Nexus aplicação de ocupação determinística de pistas e exige um manifesto de admissões atualizado.
4. Depois que o não estiver ativo, atualize seus runbooks com quaisquer overrides que você diz dinâmica e anote a tag de release para que a próxima iteração comece a partir desta linha de base.

## Checklist de referência
- [ ] Perfil de lançamento validado como `iroha3`.
- [ ] Hashes e assinaturas do pacote/imagem selecionadas.
- [ ] Chaves, endereços de peers e endpoints Torii atualizados para valores de produção.
- [ ] Catálogo de pistas/espaço de dados e política de roteamento do Nexus atribui a atribuição do conselho.
- [ ] Validador de configuração (`irohad --sora --config ... --trace-config`) passa sem avisos.
- [ ] Manifestos/assinaturas arquivadas no ticket de onboarding e Ops notificadas.

Para contexto mais amplo sobre fases de migração do Nexus e expectativas de telemetria, revise [Nexus notas de transição](./nexus-transition-notes).