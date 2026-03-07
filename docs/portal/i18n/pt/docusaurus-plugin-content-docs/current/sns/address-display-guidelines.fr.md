---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard de '@site/src/components/ExplorerAddressCard';

:::nota Fonte canônica
Esta página reflete `docs/source/sns/address_display_guidelines.md` e sert
manutenção da cópia canônica do portal. O arquivo fonte restante para os PRs
de tradução.
:::

Portefeuilles, exploradores e exemplos de SDK devem trair os endereços
de compte como des payloads immuables. Exemplo de portefeuille de varejo Android
em `examples/android/retail-wallet`, você deve manter o padrão UX necessário:

- **Deux cibles de copie.** Fournissez deux boutons de copie explicita: IH58
  (preferir) e a forma compactada somente Sora (`sora...`, segunda escolha). IH58 está hoje
  certifique-se de compartilhar externamente e manter a carga útil do QR. A variante comprimida
  inclua um aviso inline parcialmente que não funcione naquele
  aplicativos com preços gratuitos por Sora. O exemplo do Android ramifica os dois botões Material e
  leia suas dicas de ferramentas em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, e outros
  demonstração iOS SwiftUI reflete o meme UX via `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monoespaçado, seleção de texto habilitável.** Exiba as duas cadeias com uma polícia
  monospace e `textIsSelectable="true"` para que os usuários possam usar
  inspeciona os valores sem invocar um IME. Evitez les champs editáveis: les
  O IME pode recriar o kana ou injetar pontos de código maiores que zero.
- **Indicações de domínio por padrão implícito.** Quando o seletor aponta para
  le domaine implicite `default`, exibindo uma legenda rappelant aux operadores
  qu'aucun sufixo n'est requis. Os exploradores também devem se preparar para a frente
  o rótulo do domínio canônico quando o seletor codifica um resumo.
- **QR IH58.** Os códigos QR devem codificar a cadeia IH58. Si la geração du
  QR echoue, exibe um erro explícito no lugar de uma imagem de vídeo.
- **Message presse-papiers.** Depois de copiar a forma compactada, emita um
  brinde ou snackbar rappelant aux utilisateurs qu'elle est Sora-only et sujette
  a la distorção IME.

Siga esse cuidado, evite a corrupção do Unicode/IME e satisfaça os critérios
aceitação do roteiro ADDR-6 para o UX portefeuille/explorateur.

## Capturas de referência

Utilize as seguintes referências nas revistas de localização para garantir
que os símbolos dos botões, dicas de ferramentas e avisos permanecem alinhados entre
formas de placa:

- Referência Android: `/img/sns/address_copy_android.svg`

  ![Cópia dupla do Android de referência](/img/sns/address_copy_android.svg)

- Referência iOS: `/img/sns/address_copy_ios.svg`

  ![Cópia dupla do iOS de referência](/img/sns/address_copy_ios.svg)

## SDK de ajudantes

Cada SDK expõe um auxiliar de prática que retorna as formas IH58 et
comprimir também a cadeia de aviso para que os sofás da interface do usuário permaneçam
coerentes:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspetor JavaScript: `inspectAccountId(...)` retornar a cadeia
  d'avertissement comprimir et l'ajoute a `warnings` quand les recorrentes
  forneceu um literal `sora...`, para que exploradores/tabelas de borda de
  portefeuille pode exibir a advertência Sora-only pendente les flux de
  colagem/validação plutot que seulement lorsqu'ils generent eux-memes la forme
  comprimido.
-Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Utilize estes auxiliares em vez de reimplementar a lógica de codificação nos
UI de sofás. O JavaScript auxiliar expõe também uma carga útil `selector` em
`domainSummary` (`tag`, `digest_hex`, `registry_id`, `label`) para as UIs
pode indicar se um seletor é Local-12 ou adicionar um registro sem
reanalisa a carga útil bruta.

## Demonstração de instrumentação exploratória



Os exploradores devem reproduzir o trabalho de telemetria e acessibilidade
fato para a carteira:

- Aplique `data-copy-mode="ih58|compressed|qr"` nos botões de cópia para que
  os front-ends podem emetre des compteurs d'usage em paralelo com o
  métrica Torii `torii_address_format_total`. Le composant demo ci-dessus
  enviar um evento `iroha:address-copy` com `{mode,timestamp}` - reliez cela
  seu pipeline de análise/telemetria (por exemplo, enviar um segmento ou um
  coletor base no NORITO) para que os painéis possam correlacionar o uso
  o formato do endereço do servidor com os modos de cópia do cliente. Refletir
  também os computadores de domínio Torii (`torii_address_domain_total{domain_kind}`)
  no fluxo de meme para que as revistas de retrato Local-12 possam exportar um
  teste de 30 dias `domain_kind="local12"` diretamente do quadro
  `address_ingest` de Grafana.
- Associe cada controle às indicações `aria-label`/`aria-describedby`
  distintos que são explícitos se um literal está sur a partager (IH58) ou Sora-only
  (comprimir). Inclua a legenda do domínio implícita na descrição para
  que as tecnologias de assistência refletem o meme contexto da exibição.
- Exponha uma região ao vivo (ex. `<output aria-live="polite">...</output>`) aqui
  anuncie os resultados da cópia e os avisos, alinhados
  Comporte o cabo VoiceOver/TalkBack nos exemplos Swift/Android.

Esta instrumentação ADDR-6b satisfatória e prova que os operadores podem
observe a ingestão Torii e os modos de cópia do cliente antes de
selecteurs Local soient desactives.

## Toolkit de migração Local -> GlobalUse o [kit de ferramentas Local -> Global](local-to-global-toolkit.md) para
automatiza a auditoria e converte os seletores hereditários locais. O ajudante
emita para o relatório de auditoria JSON e a lista convertida IH58/compressee que
os operadores joignent aux tickets de readiness, portanto, o runbook associado
encontram-se os painéis Grafana e as regras Alertmanager que os controlam
cutover em modo estrito.

## Referência rápida do layout binário (ADDR-1a)

Quando o SDK expõe uma visita antecipada ao endereço (inspetores, indicações de
validação, construtores de manifesto), indique os desenvolvedores para o formato
captura canônica de fio em `docs/account_structure.md`. Le layout está sempre
`header · selector · controller`, ou os bits do cabeçalho são:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) hoje; os valores não zero são reservados
  e alavanca dupla `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue os controladores simples (`0`) de multisig (`1`).
- `norm_version = 1` codifica as regras do seletor Norm v1. Futuros Les Norm
  reutilizou o meme campeão 2 bits.
- `ext_flag` vaut toujours `0`; os bits ativos indicam as extensões de
  carga útil não cobrada.

O seletor atende imediatamente ao cabeçalho:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

As UIs e o SDK devem exibir o tipo de seleção:

- `0x00` = domínio por padrão implícito (sem carga útil).
- `0x01` = resumo local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Exemplos hexadecimais canônicos de que as ferramentas de carteira podem ser ou inteiras
documentos/testes auxiliares:

| Digite o seletor | Hex canônico |
|---------------|---------------|
| Implícito por padrão | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumo local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` para a mesa completa
seletor/estado e `docs/account_structure.md` para o diagrama completo
bytes.

## Imposer as formas canônicas

Os operadores que convertem as codificações do patrimônio local em IH58 canônico
Ou em cadeias compactadas, você deve seguir o documento CLI do fluxo de trabalho sob ADDR-5:

1. `iroha tools address inspect` emite manter uma estrutura JSON de currículo com IH58,
   compresse e payloads hexadecimais canônicos. O currículo inclui também um objeto
   `domain` com os campos `kind`/`warning` e reflete todos os domínios fornecidos por
   o campeão `input_domain`. Quando `kind` vai `local12`, a CLI imprime um
   anúncio em stderr e currículo reflete JSON no meme enviado para isso
   Os pipelines CI e o SDK podem ser exibidos. Passez `legacy  suffix`
   Quando você quiser, refazer a codificação convertida para a forma `<ih58>@<domain>`.
2. O SDK pode exibir o aviso/currículo do meme por meio do auxiliar
   JavaScript:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Le helper preserve le prefixe IH58 detecte depuis le literal sauf si vous
  forneça explicitação `networkPrefix`, fornecendo currículos para pesquisas
  non defaut ne sont pas re-rendus silencieusement com o prefixo par defaut.

3. Converta a carga útil canônica e utilize os campos `ih58.value` ou
   `compressed` você continua (ou exige outra codificação via `--format`). Ces
   chaines sont deja sures a partager en externe.
4. Mantenha atualizados os manifestos, registros e documentos orientados pelo cliente com
   forme canônico e notifique os contrapartes que os seletores locais serão
   recusa une fois le cutover termine.
5. Para jogos de donnees em massa, executez
   `iroha tools address audit --input addresses.txt --network-prefix 753`. O comando
   lit des literaux separes par nouvelle ligne (les commentaires commencant par
   `#` é ignorado, e `--input -` ou qualquer sinalizador utiliza STDIN), emite um relacionamento
   JSON com currículos canônicos/IH58/compresse para cada entrada, e conta
   erros de análise assim como anúncios de domínio local. Utilizar
   `--allow-errors` para auditoria de dumps herites contendo linhas
   parasitas, e bloqueie a automação via `strict CI post-check` quando
   Os operadores pretendem bloquear os seletores locais no CI.
6. Quando você precisar de uma reescrita online, use
  Para as folhas de cálculo de remediação dos seletores locais, use
  pour exporter un CSV `input,status,format,...` qui met en avant les encodages
  canônicos, avisos e verificações de análise em um único passe.
   Le helper ignora as linhas não locais por padrão, converte cada entrada
   restante na codificação exigida (IH58/compresse/hex/JSON), e preserve o
   domínio original quando `legacy  suffix` está ativo. Associez-le a
   `--allow-errors` para continuar a análise do meme quando um dump contém o mesmo
   literatura malformada.
7. A automação CI/lint pode executar o `ci/check_address_normalize.sh`, aqui
   extraia os seletores locais de `fixtures/account/address_vectors.json`,
   converta-o via `iroha tools address normalize` e feliz
   `iroha tools address audit` para provar que os lançamentos
   n'emetent plus de digests Local.`torii_address_local8_total{endpoint}` mais
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, e o quadro Grafana
`dashboards/grafana/address_ingest.json` fornece sinal de aplicação:
uma vez que os painéis de produção foram montados com zero soumissões locais
legítimos e zero colisões Local-12 pendente 30 dias consecutivos, Torii
basculera a porta Local-8 em echec strict sur mainnet, seguindo por Local-12 une
foi que os domínios globais tenham entradas de registro correspondentes.
Consideraz la sortie CLI como l'avis operaur pour ce gel - la meme chaine
 A advertência é usada nas dicas de ferramentas do SDK e na automação para
mantenha a partida com os critérios de sortie du roadmap. Torii utilizar
que aparece nos clusters de desenvolvimento/teste para o diagnóstico de regressões. Continuar a
espelho `torii_address_domain_total{domain_kind}` em Grafana
(`dashboards/grafana/address_ingest.json`) para o pacote de teste ADDR-7
Puisse mostrar que `domain_kind="local12"` está restante a zero pendente da janela
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) ajoute trois
barreiras:

- `AddressLocal8Resurgence` página cada vez que um contexto sinaliza uma nova
  incrementação Local-8. Interrompa as implementações no modo estrito e localize-as
  superfície SDK fautive no painel e, se necessário, definido temporariamente
  padrão (`true`).
- `AddressLocal12Collision` se declina quando dois rótulos Hashent Local-12
  ver o resumo do meme. Pare as promoções do manifesto e execute-as
  kit de ferramentas Local -> Global para auditar o mapeamento de resumos e coordenadas
  com o governo Nexus antes de reemitar a entrada de registro ou de
  reinicialize as implementações e aval.
- `AddressInvalidRatioSlo` evita que a proporção seja invalidada para a echalle de la
  flotte (excluindo rejeições Local-8/strict-mode) despassou o SLO de 0,1%
  pendente dix minutos. Use `torii_address_invalid_total` para identificar o arquivo
  contexto/razão responsável e coordenação com a equipe SDK proprietária antes
  de reativar o modo estrito.

### Extrait de note de release (portefeuille et explorateur)

Inclua a bala a seguir nas notas de lançamento portefeuille/explorateur
Senhor do corte:

> **Endereços:** Adicione o ajudante `iroha tools address normalize`
> e uma ramificação no CI (`ci/check_address_normalize.sh`) para os pipelines
> portefeuille/explorateur pode converter os seletores de heranças locais vers
> des formes canonices IH58/compressées avant que Local-8/Local-12 soient
> blocos na rede principal. Faça um dia de exportações personalizadas para executar
> comande e junte a lista normalizada no pacote de pré-lançamento.