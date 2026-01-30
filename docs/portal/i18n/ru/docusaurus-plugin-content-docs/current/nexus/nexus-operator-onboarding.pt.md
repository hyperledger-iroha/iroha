---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operator-onboarding
title: Integracao de operadores de data-space Sora Nexus
description: Espelho de `docs/source/sora_nexus_operator_onboarding.md`, acompanhando a checklist de release end-to-end para operadores Nexus.
---

:::note Fonte canonica
Esta pagina reflete `docs/source/sora_nexus_operator_onboarding.md`. Mantenha as duas copias alinhadas ate que as edicoes localizadas cheguem ao portal.
:::

# Integracao de operadores de data-space Sora Nexus

Este guia captura o fluxo end-to-end que operadores de data-space Sora Nexus devem seguir quando um release e anunciado. Ele complementa o runbook de dupla via (`docs/source/release_dual_track_runbook.md`) e a nota de selecao de artefatos (`docs/source/release_artifact_selection.md`) descrevendo como alinhar bundles/imagens baixados, manifests e templates de configuracao com as expectativas de lanes globais antes de colocar um no em producao.

## Audiencia e prerequisitos
- Voce foi aprovado pelo Programa Nexus e recebeu sua atribuicao de data-space (indice de lane, data-space ID/alias e requisitos de politica de routing).
- Voce consegue acessar os artefatos assinados do release publicados pelo Release Engineering (tarballs, imagens, manifests, assinaturas, chaves publicas).
- Voce gerou ou recebeu material de chaves de producao para seu papel de validator/observer (identidade de no Ed25519; chave de consenso BLS + PoP para validators; mais quaisquer toggles de recursos confidenciais).
- Voce consegue alcancar os peers Sora Nexus existentes que farao bootstrap do seu no.

## Etapa 1 - Confirmar o perfil de release
1. Identifique o alias de rede ou chain ID fornecido.
2. Execute `scripts/select_release_profile.py --network <alias>` (ou `--chain-id <id>`) em um checkout deste repositorio. O helper consulta `release/network_profiles.toml` e imprime o perfil para deploy. Para Sora Nexus a resposta deve ser `iroha3`. Para qualquer outro valor, pare e contate Release Engineering.
3. Anote o tag de versao referenciado no anuncio do release (por exemplo `iroha3-v3.2.0`); voce o usara para buscar artefatos e manifests.

## Etapa 2 - Recuperar e validar artefatos
1. Baixe o bundle `iroha3` (`<profile>-<version>-<os>.tar.zst`) e seus arquivos companheiros (`.sha256`, opcional `.sig/.pub`, `<profile>-<version>-manifest.json`, e `<profile>-<version>-image.json` se voce fizer deploy com contenedores).
2. Valide a integridade antes de descompactar:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   Substitua `openssl` pelo verificador aprovado pela organizacao se voce usa um KMS com hardware.
3. Inspecione `PROFILE.toml` dentro do tarball e os manifests JSON para confirmar:
   - `profile = "iroha3"`
   - Os campos `version`, `commit` e `built_at` correspondem ao anuncio do release.
   - O OS/arquitetura correspondem ao alvo de deploy.
4. Se voce usa a imagem de contenedor, repita a verificacao de hash/assinatura para `<profile>-<version>-<os>-image.tar` e confirme o image ID registrado em `<profile>-<version>-image.json`.

## Etapa 3 - Preparar configuracao a partir dos templates
1. Extraia o bundle e copie `config/` para o local onde o no vai ler sua configuracao.
2. Trate os arquivos sob `config/` como templates:
   - Substitua `public_key`/`private_key` pelas suas chaves Ed25519 de producao. Remova chaves privadas do disco se o no vai obtelas a partir de um HSM; atualize a configuracao para apontar para o conector HSM.
   - Ajuste `trusted_peers`, `network.address` e `torii.address` para refletir suas interfaces alcancaveis e os peers de bootstrap que lhe foram atribuidos.
   - Atualize `client.toml` com o endpoint Torii para operadores (incluindo configuracao TLS se aplicavel) e as credenciais provisionadas para tooling operacional.
3. Mantenha o chain ID fornecido no bundle a menos que Governance instrua explicitamente - a lane global espera um unico identificador canonico de cadeia.
4. Planeje iniciar o no com o flag de perfil Sora: `irohad --sora --config <path>`. O loader de configuracao rejeitara configuracoes SoraFS ou multi-lane se o flag estiver ausente.

## Etapa 4 - Alinhar metadata de data-space e routing
1. Edite `config/config.toml` para que a secao `[nexus]` corresponda ao catalogo de data-space fornecido pelo Nexus Council:
   - `lane_count` deve ser igual ao total de lanes habilitadas na epoca atual.
   - Cada entrada em `[[nexus.lane_catalog]]` e `[[nexus.dataspace_catalog]]` deve conter um `index`/`id` unico e os aliases acordados. Nao apague as entradas globais existentes; adicione seus aliases delegados se o conselho atribuiu data-spaces adicionais.
   - Garanta que cada entrada de dataspace inclua `fault_tolerance (f)`; comites lane-relay sao dimensionados em `3f+1`.
2. Atualize `[[nexus.routing_policy.rules]]` para capturar a politica que lhe foi dada. O template padrao roteia instrucoes de governanca para a lane `1` e deploys de contratos para a lane `2`; adicione ou modifique regras para que o trafego destinado ao seu data-space seja encaminhado para a lane e o alias corretos. Coordene com Release Engineering antes de alterar a ordem das regras.
3. Revise os limites de `[nexus.da]`, `[nexus.da.audit]` e `[nexus.da.recovery]`. Espera-se que os operadores mantenham os valores aprovados pelo conselho; ajuste apenas se uma politica atualizada foi ratificada.
4. Registre a configuracao final no seu tracker de operacoes. O runbook de release de dupla via exige anexar o `config.toml` efetivo (com segredos redigidos) ao ticket de onboarding.

## Etapa 5 - Validacao pre-flight
1. Execute o validador de configuracao embutido antes de entrar na rede:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   Isso imprime a configuracao resolvida e falha cedo se as entradas de catalogo/routing forem inconsistentes ou se genesis e config divergirem.
2. Se voce fizer deploy com contenedores, execute o mesmo comando dentro da imagem apos carregala com `docker load -i <profile>-<version>-<os>-image.tar` (lembre de incluir `--sora`).
3. Verifique logs para avisos sobre identificadores placeholder de lane/data-space. Se houver, retorne a Etapa 4 - deploys de producao nao devem depender dos IDs placeholder que acompanham os templates.
4. Execute seu procedimento local de smoke (por exemplo, enviar uma query `FindNetworkStatus` com `iroha_cli`, confirmar que endpoints de telemetria expoem `nexus_lane_state_total`, e verificar que as chaves de streaming foram rotacionadas ou importadas conforme necessario).

## Etapa 6 - Cutover e hand-off
1. Guarde o `manifest.json` verificado e os artefatos de assinatura no ticket de release para que auditores possam reproduzir suas verificacoes.
2. Notifique Nexus Operations de que o no esta pronto para ser introduzido; inclua:
   - Identidade do no (peer ID, hostnames, endpoint Torii).
   - Valores efetivos do catalogo de lane/data-space e politica de routing.
   - Hashes dos binarios/imagens verificados.
3. Coordene a admissao final de peers (gossip seeds e atribuicao de lane) com `@nexus-core`. Nao entre na rede ate receber aprovacao; Sora Nexus aplica ocupacao deterministica de lanes e exige um manifest de admissoes atualizado.
4. Depois que o no estiver ativo, atualize seus runbooks com quaisquer overrides que voce introduziu e anote o tag de release para que a proxima iteracao comece a partir desta baseline.

## Checklist de referencia
- [ ] Perfil de release validado como `iroha3`.
- [ ] Hashes e assinaturas do bundle/imagem verificados.
- [ ] Chaves, enderecos de peers e endpoints Torii atualizados para valores de producao.
- [ ] Catalogo de lanes/dataspace e politica de routing do Nexus correspondem a atribuicao do conselho.
- [ ] Validador de configuracao (`irohad --sora --config ... --trace-config`) passa sem avisos.
- [ ] Manifests/assinaturas arquivados no ticket de onboarding e Ops notificado.

Para contexto mais amplo sobre fases de migracao do Nexus e expectativas de telemetria, revise [Nexus transition notes](./nexus-transition-notes).
