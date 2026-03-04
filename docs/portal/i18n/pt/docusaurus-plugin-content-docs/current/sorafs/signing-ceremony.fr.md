---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: cerimônia de assinatura
título: Substituição da cerimônia de assinatura
descrição: Comente o Parlamento Sora aprova e distribui os equipamentos do bloco SoraFS (SF-1b).
sidebar_label: Cerimônia de assinatura
---

> Roteiro: **SF-1b — aprovações de luminárias do Parlamento Sora.**
> O fluxo de trabalho do Parlamento substitui a antiga «cerimônia de assinatura do conselho» fora da linha.

Le rituel manuel de assinatura des fixtures du chunker SoraFS está retirado. Todos os
aprovações passam desormais par le **Parlement Sora**, la DAO basee sur le tirage
au sort qui gouverne Nexus. Os membros do Parlamento bloquearam o XOR para obter
citoyennete, torneio entre painéis e voto na cadeia para aprovar, rejeitar ou
revenir sur des releases de fixtures. Este guia explica o processo e as ferramentas
para os desenvolvedores.

## Vista do Conjunto do Parlamento

- **Citoyennete** — Os operadores imobilizam o XOR necessário para inscrever-se como
  citoyens et devenir elegíveis au tirage au sort.
- **Painéis** — As responsabilidades são repartidas entre os painéis rotativos
  (Infraestrutura, Moderação, Tresorerie, ...). Le Panel Infraestrutura deficiente
  as aprovações de luminárias SoraFS.
- **Tirage au sort et rotation** — Les sieges de panel são reatribuídos de acordo com la
  cadência especificada na constituição do Parlamento afin qu'aucun groupe ne
  monopolizar as aprovações.

## Fluxo de aprovação de luminárias

1. **Envio de proposta**
   - Le Tooling WG televerse le bundle candidato `manifest_blake3.json` et le diff
     de fixture no registro on-chain via `sorafs.fixtureProposal`.
   - A proposta registra o resumo BLAKE3, a versão semântica e as notas
     de mudança.
2. **Revista e vote**
   - Le Panel Infrastructure recupera a afetação através do arquivo de taches du Parlement.
   - Os membros inspecionam os artefatos CI, executam testes de paridade e
     emettent des votes ponderes on-chain.
3. **Finalização**
   - Uma vez que o quorum foi apurado, o tempo de execução gerou um evento de aprovação incluindo
     o resumo canônico do manifesto e o engajamento Merkle da carga útil do dispositivo.
   - O evento é duplicado no registro SoraFS para que os clientes possam
     recuperar o último manifesto aprovado pelo Parlamento.
4. **Distribuição**
   - Les helpers CLI (`cargo xtask sorafs-fetch-fixture`) recupera o manifesto
     aprovação via Nexus RPC. As constantes JSON/TS/Go do depósito permanecem sincronizadas
     um relancante `export_vectors` e um resumo válido para relacionamento com o registro
     na cadeia.

## Desenvolvedor de fluxo de trabalho

- Regenerar os equipamentos com:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Utilize o auxiliar de busca do Parlamento para carregar o envelope aprovado,
  verifique as assinaturas e rafraichir os locais dos fixtures. Ponteiro `--signatures`
  vers o envelope publicado pelo Parlamento; le helper resout le manifest associe,
  recalcular o resumo BLAKE3 e impor o perfil canônico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```Passe `--manifest` se o manifesto encontrar outro URL. Os envelopes não
os signatários são recusados, exceto se `--allow-unsigned` está ativo para a fumaça correr localmente.

- Para validar um manifesto através de um gateway de teste, cibler Torii tanto quanto des
  cargas úteis locaux:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI local n'exige mais uma lista `signer.json`.
  `ci/check_sorafs_fixtures.sh` compare o estado do repo com o último noivado
  on-chain et echoue lorsqu'ils divergentes.

## Notas de governo

- A constituição do Parlamento governa o quórum, a rotação e a escalada;
  Nenhuma configuração no nível da caixa não é necessária.
- As reversões de urgência são geradas por meio do painel de moderação do Parlamento. Le
  Infraestrutura do painel apresenta uma proposta de reversão que faz referência ao resumo
  precedente du manifest, et la release est substitue une fois approuvee.
- As aprovações históricas restantes estão disponíveis no registro SoraFS para
  uma repetição forense.

## Perguntas frequentes

- **Ou est passe `signer.json` ?**  
  Il a ete supprime. Toda atribuição de assinatura na cadeia; `manifest_signatures.json`
  no depósito não há um dispositivo de desenvolvimento que corresponda ao último
  evento de aprovação.

- **Faut-il encore des subscriptions Ed25519 locales ?**  
  Não. As aprovações do Parlamento são estocadas como artefatos na rede. Os jogos
  locais existentes para a reprodução mais são válidos em relação ao resumo do Parlamento.

- **Comment les equipes surveillent-elles les approbations ?**  
  Desligue o evento `ParliamentFixtureApproved` ou interrogue o registro via
  Nexus RPC para obter o resumo atual do manifesto e a lista de membros do painel.