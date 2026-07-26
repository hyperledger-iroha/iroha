---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: integração do operador nexus
título: Incorporação de operadores de espaço de dados de Sora Nexus
description: Espejo de `docs/source/sora_nexus_operator_onboarding.md`, que segue o checklist de release end-to-end para operadores de Nexus.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sora_nexus_operator_onboarding.md`. Mantenha ambas as cópias alinhadas até que as edições localizadas cheguem ao portal.
:::

# Incorporação de operadores de espaço de dados de Sora Nexus

Este guia captura o fluxo de ponta a ponta que deve seguir os operadores de espaço de dados de Sora Nexus uma vez se anuncia um lançamento. Complementa o runbook de duplo via (`docs/source/release_dual_track_runbook.md`) e a nota de seleção de artefatos (`docs/source/release_artifact_selection.md`) ao descrever como pacotes / imagens lineares baixados, manifestos e configurações de planta com as expectativas de pistas globais antes de colocar um nó online.

## Audiência e pré-requisitos
- Foi aprovado pelo Programa Nexus e recebeu sua atribuição de espaço de dados (índice de pista, ID/alias do espaço de dados e requisitos de política de roteamento).
- Você pode acessar os artefatos firmados do release publicado pela Release Engineering (tarballs, imagens, manifestos, firmas, folhas públicas).
- Gerou ou recebeu material de folhas de produção para sua função de validador/observador (identidade do nó Ed25519; chave de consenso BLS + PoP para validadores; mas qualquer alternância de funções confidenciais).
- Você pode alcançar os pares existentes do Sora Nexus que inicializam seu nodo.

## Paso 1 - Confirmar o perfil de lançamento
1. Identifique o alias de red ou chain ID que você morrerá.
2. Execute `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) em um checkout deste repositório. O ajudante consulta `release/network_profiles.toml` e imprime o perfil para desplegar. Para Sora Nexus a resposta deve ser `iroha3`. Para qualquer outro valor, detecte e entre em contato com a Engenharia de Liberação.
3. Anote a tag de versão que faz referência ao anúncio de lançamento (por exemplo, `iroha3-v3.2.0`); ele é usado para baixar artefatos e manifestos.

## Paso 2 - Recuperar e validar artefatos
1. Descarregue o pacote `iroha3` (`<profile>-<version>-<os>.tar.zst`) e seus arquivos companheiros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` se despliegas contenedores).
2. Valide a integridade antes de descomprimir:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Substitua `openssl` com o selecionador aprovado pela organização se usar um KMS com respaldo de hardware.
3. Inspecione `PROFILE.toml` dentro do tarball e manifeste JSON para confirmar:
   -`profile = "iroha3"`
   - Os campos `version`, `commit` e `built_at` coincidem com o anúncio do lançamento.
   - El OS/arquitectura coincide com seu objetivo de despliegue.
4. Se usar a imagem do contêiner, repita a verificação de hash/firma para `<profile>-<version>-<os>-image.tar` e confirme o ID da imagem registrada em `<profile>-<version>-image.json`.

## Paso 3 - Preparar configuração desde plantas
1. Extraia o pacote e copie `config/` para o local onde o nó leu sua configuração.
2. Trate os arquivos inferiores `config/` como plantas:
   - Substituição `public_key`/`private_key` com suas folhas Ed25519 de produção. Elimine as chaves privadas do disco se você obtiver um HSM; atualiza a configuração para conectar o conector HSM.
   - Ajusta `trusted_peers`, `network.address` e `torii.address` para refletir suas interfaces acessíveis e os pares de bootstrap atribuídos.
   - Atualize `client.toml` com o endpoint Torii do próprio operador (incluindo configuração TLS e aplicativo) e as credenciais fornecidas para ferramentas operacionais.
3. Mantenha o ID da cadeia fornecido no pacote, a menos que o Governance indique explicitamente: a faixa global espera um identificador de cadeia canônica única.
4. Planeje iniciar o nó com a bandeira do perfil Sora: `irohad --sora --config <path>`. O carregador de configuração rechazará as configurações de SoraFS ou multi-lane se a bandeira estiver ausente.## Paso 4 - Metadados lineares de espaço de dados e roteamento
1. Edite `config/config.toml` para que a seção `[nexus]` coincida com o catálogo de espaços de dados fornecido pelo Conselho Nexus:
   - `lane_count` deve ser igual ao total de pistas habilitadas na época atual.
   - Cada entrada em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` deve conter um `index`/`id` único e o alias acordado. Não elimina as entradas globais existentes; adicione seu pseudônimo delegado se o conselho atribuir espaços de dados adicionais.
   - Certifique-se de que cada entrada de espaço de dados inclua `fault_tolerance (f)`; os comitês lane-relay são dimensionados em `3f+1`.
2. Atualize `[[nexus.routing_policy.rules]]` para capturar a política que você atribuiu. A planta por defeito enruta instruções de governança na pista `1` e despliegues de contratos na pista `2`; adicione ou modifique as regras para que o tráfego destinado ao seu espaço de dados vá para a pista e alias correto. Coordenação com Engenharia de Liberação antes de alterar a ordem de regras.
3. Revise os umbrais de `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. Espera-se que os operadores mantenham os valores aprovados pelo conselho; ajuste-os apenas se for ratificado uma política atualizada.
4. Registre a configuração final no seu rastreador de operações. O runbook de liberação dupla via requer a adição do `config.toml` efetivo (com segredos redigidos) ao ticket de integração.

## Paso 5 - Validação prévia
1. Execute o validador de configuração integrado antes de conectar-se à rede:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Isso imprime o resultado da configuração e falha se as entradas do catálogo/roteamento forem inconsistentes ou se a gênese e a configuração não coincidirem.
2. Se você remover o conteúdo, execute o mesmo comando dentro da imagem após carregar com `docker load -i <profile>-<version>-<os>-image.tar` (a recuperação inclui `--sora`).
3. Revisa logs para advertências sobre identificadores placeholder de lane/data-space. Se aparecer, volte para a Etapa 4: os despliegues de produção não dependem dos IDs placeholder que vêm com as plantas.
4. Execute seu procedimento local de fumaça (p. ej., envie uma consulta `FindNetworkStatus` com `iroha_cli`, confirme se os endpoints de telemetria estão expostos `nexus_lane_state_total` e verifique se as folhas de streaming giram ou importam conforme correspondente).

## Paso 6 - Corte e transferência
1. Guarde o `manifest.json` selecionado e os artefatos da firma no ticket de liberação para que os auditores possam reproduzir suas verificações.
2. Notifique a Nexus Operations que o nodo está listado para ser introduzido; inclui:
   - Identidade do nó (ID de peer, nomes de host, endpoint Torii).
   - Valores efetivos de catálogo de pista/espaço de dados e política de roteamento.
   - Hashes dos binários/imagens que você verifica.
3. Coordenar a admissão final de pares (sementes de fofoca e atribuição de pista) com `@nexus-core`. Não te unas a la red até receber aprovação; Sora Nexus aplica ocupação determinista de pistas e requer um manifesto de admissão atualizado.
4. Depois que o nó for ao vivo, atualize seus runbooks com qualquer substituição que você inserir e anote a tag de lançamento para que a iteração seguinte seja iniciada a partir desta linha de base.

## Checklist de referência
- [ ] Perfil de lançamento validado como `iroha3`.
- [ ] Hashes e firmas do pacote/imagem selecionadas.
- [ ] Chaves, direções de peers e endpoints Torii atualizados com valores de produção.
- [] Catálogo de pistas/espaço de dados e política de roteamento de Nexus coincidem com a atribuição do conselho.
- [ ] Validador de configuração (`irohad --sora --config ... --trace-config`) sem avisos.
- [ ] Manifestos/firmas arquivados no ticket de onboarding e Ops notificado.

Para contexto adicional sobre fases de migração de Nexus e expectativas de telemetria, revise [Nexus notas de transição](./nexus-transition-notes).