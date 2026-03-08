---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard de '@site/src/components/ExplorerAddressCard';

:::nota Fonte canônica
Esta página espelha `docs/source/sns/address_display_guidelines.md` e agora serve
como a copia canônica do portal. O arquivo fonte permanece para PRs de
tradução.
:::

Carteiras, exploradores e exemplos de SDK devem tratar enderecos de conta como
cargas imutáveis. O exemplo da carteira de varejo Android em
`examples/android/retail-wallet` agora demonstra o padrão de UX exigido:

- **Dois alvos de cópia.** Envie dois botões de cópia explicitos: IH58
  (preferido) e a forma comprimida somente Sora (`sora...`, segunda melhor opção). IH58 e sempre seguro para
  compartilhe externamente e alimente o payload do QR. A variante comprimida
  deve incluir um aviso inline porque funciona dentro de aplicativos compatíveis com
  Sora. O exemplo de carteira de varejo Android liga ambos os botoes Material e seus
  dicas de ferramentas em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, e a
  demo iOS SwiftUI espelha o mesmo UX via `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, texto selecionável.** Renderize ambas as strings com uma fonte
  monospace e `textIsSelectable="true"` para que os usuários possam funcionar
  sem valores invoca um IME. Evite campos editáveis: IMEs podem reescrever kana
  ou injetar pontos de código de largura zero.
- **Dicas de domínio padrão implícito.** Quando o seletor aponta para o domínio
  implicito `default`, mostre uma legenda lembrando os operadores de que nenhum
  sufixo e necessário. Exploradores também devem destacar o rótulo de domínio
  canônico quando o seletor codifica um resumo.
- **QR IH58.** Os códigos QR devem codificar a string IH58. Se a geração do QR
  falhar, mostrar um erro explícito em vez de uma imagem em branco.
- **Mensagem da área de transferência.** Depois de copiar a forma comprada,
  emita um brinde ou snackbar lembrando os usuários de que ela e somente Sora e
  propens uma distorção pelo IME.

Seguir esses guardrails evita a corrupção do Unicode/IME e atende aos critérios de
aceitação do roadmap ADDR-6 para UX de carteiras/exploradores.

## Capturas de tela de referência

Use como referências a seguir durante revisões de localização para garantir que os
rotulos de botoes, tooltips e avisos ficam alinhados entre plataformas:

- Referência Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android de dupla cópia](/img/sns/address_copy_android.svg)

- Referência iOS: `/img/sns/address_copy_ios.svg`

  ![Referência iOS de dupla cópia](/img/sns/address_copy_ios.svg)

## Ajudantes do SDK

Cada SDK expõe um auxiliar de conveniência que retorna as formas IH58 e comprimida
junto com uma string de aviso para que as camadas UI sejam consistentes:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspetor JavaScript: `inspectAccountId(...)` retorna uma string de aviso
  comprimida e a anexa a `warnings` quando chamadores fornecem um literal
  `sora...`, para que os dashboards da carteira/explorador possam exibir o aviso
  somente Sora durante fluxos de supervisão/validação em vez de apenas quando gerar
  a forma comprometida por conta própria.
-Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Use estes auxiliares em vez de reimplementar a lógica de codificação nas camadas da UI. Ó
helper JavaScript também expõe um payload `selector` em `domainSummary` (`tag`,
`digest_hex`, `registry_id`, `label`) para que UIs indiquem se um seletor e
Local-12 ou respaldado por registro sem reparar o payload bruto.

## Demo de instrumentação do explorador



Exploradores devem espelhar o trabalho de telemetria e acessibilidade da
carteira:

- Aplique `data-copy-mode="ih58|compressed|qr"` aos botões de cópia para que
  front-ends podem emitir contadores de uso junto com a métrica Torii
  `torii_address_format_total`. O componente demo acima é diferente de um evento
  `iroha:address-copy` com `{mode,timestamp}`; conecte isso ao seu pipeline de
  analitica/telemetria (por exemplo, envie para Segmento ou um coletor NORITO)
  para que dashboards possam correlacionar o uso de formatos de endereco do
  servidor com modos de cópia do cliente. Replique também os contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) no mesmo feed para
  que revisões de aposentadoria Local-12 podem exportar uma prova de 30 dias
  `domain_kind="local12"` diretamente do painel `address_ingest` do Grafana.
- Emparelhe cada controle com pistas `aria-label`/`aria-describedby` distintas
  que expliquem se um literal e seguro para compartilhar (IH58) ou somente Sora
  (comprimido). Inclui uma legenda do domínio implícita na descrição para que um
  tecnologia assistiva mostra o mesmo contexto visualmente.
- Exponha uma regiao viva (por exemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de cópia e avisos, alinhando o comportamento do
  VoiceOver/TalkBack já conectado a exemplos de Swift/Android.

Esta instrumentação satisfaz ADDR-6b ao provar que os operadores podem observar
tanto a ingestão Torii quanto os modos de cópia do cliente antes que os
seletores locais sejam desativados.

## Toolkit de migração Local -> Global

Use o [toolkit Local -> Global](local-to-global-toolkit.md) para automatizar um
revisão e conversa de seleções alternativas locais. Ó ajudante emite tanto o relatorio
JSON de auditoria quanto à lista convertida IH58/comprimida que operadores
anexa a tickets de readiness, enquanto o runbook associado vincula dashboards
Grafana e regras Alertmanager que controlam o corte no modo estrito.

## Referência rápida do layout binário (ADDR-1a)Quando SDKs expõem ferramentas avançadas de enderecos (inspetores, dicas de
validação, construtores de manifesto), aponte desenvolvedores para o formato wire
canônico em `docs/account_structure.md`. O layout sempre e
`header · selector · controller`, onde os bits do cabeçalho são:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) hoje; valores não zero são reservados e devem
  levantar `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distingue drivers simples (`0`) de multisig (`1`).
- `norm_version = 1` codifica as regras do seletor Norm v1. Normas futuras
  reutilização do mesmo campo de 2 bits.
- `ext_flag` e sempre `0`; bits ativos indicam extensões de payload nao
  suportadas.

O seletor segue imediatamente o cabeçalho:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIs e SDKs devem estar prontos para exibir o tipo de seletor:

- `0x00` = domínio padrão implícito (sem payload).
- `0x01` = resumo local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

Exemplos hex canonicos que ferramentas de carteira podem vincular ou embutir em
 documentos/testes:

| Tipo de seletor | Hex canônico |
|---------------|---------------|
| Padrão implícito | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumo local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` para a tabela completa de
seletor/estado e `docs/account_structure.md` para o diagrama completo de bytes.

## Importar formas canônicas

Operadores que convertem codificações Alternativas locais para IH58 canônico ou
strings comprimidas devem seguir a CLI do fluxo de trabalho documentada em ADDR-5:

1. `iroha tools address inspect` agora emite um resumo JSON estruturado com IH58,
   compactado e payloads hexadecimais canônicos. O resumo também inclui um objeto
   `domain` com campos `kind`/`warning` e ecoa qualquer domínio fornecido via o
   campo `input_domain`. Quando `kind` e `local12`, a CLI imprime um aviso em
   stderr e o resumo JSON ecoa a mesma orientação para que pipelines CI e SDKs
   posso exibi-la. Passe `legacy  suffix` sempre que quiser reproduzir a
   codificação convertida como `<ih58>@<domain>`.
2. SDKs podem exibir o mesmo aviso/resumo via JavaScript auxiliar:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  O helper preserva o prefixo IH58 detectado do literal a menos que você forneca
  explicitamente `networkPrefix`, entao resumos para redes não padrão não são
  re-renderizados silenciosamente com o prefixo padrão.3. Converta o payload canônico reutilizando os campos `ih58.value` ou
   `compressed` do resumo (ou solicitar outra codificação via `--format`). Esses
   strings ja são seguras para compartilhamento externo.
4. Atualizar manifestos, registros e documentos específicos ao cliente com a forma
   canonica e notifique as contrapartes de que seletores Locais serão rejeitados
   quando o corte for concluído.
5. Para conjuntos de dados em massa, execute
   `iroha tools address audit --input addresses.txt --network-prefix 753`. O comando
   le literais separados por nova linha (comentários iniciados com `#` sao
   ignorados, e `--input -` ou nenhuma flag usa STDIN), emite um relato JSON
   com resumos canonicos/IH58/comprimidos para cada entrada e conta erros de
   analisar e avisos de domínio local. Use `--allow-errors` ao auditar dumps alternativos
   com linhas lixo, e trave a automação com `strict CI post-check` quando os
   Os operadores estão prontos para bloquear seletores Local no CI.
6. Quando precisar de reescrita linha a linha, use
  Para planilhas de remediação de seletores locais, use
  para exportar um CSV `input,status,format,...` que destaca codificações
  canônicas, avisos e falhas de análise em uma única última.
   O helper ignora linhas nao Local por padrao, converte cada entrada restante
   para a codificação solicitada (IH58/comprimido/hex/JSON), e preservar o domínio
   original quando `legacy  suffix` e definido. Combine com `--allow-errors`
   para continuar a varredura mesmo quando um dump contém literais malformados.
7. A automação CI/lint pode executar `ci/check_address_normalize.sh`, que extrai
   seletores locais de `fixtures/account/address_vectors.json`, converter via
   `iroha tools address normalize`, e reexecutado
   `iroha tools address audit` para provar que libera não emitem
   mais resumos locais.

`torii_address_local8_total{endpoint}` junto com
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, e o painel Grafana
`dashboards/grafana/address_ingest.json` fornece o sinal de fiscalização: quando
os dashboards de produção mostram zero envios locais legítimos e zero colisoes
Local-12 por 30 dias consecutivos, Torii vai virar o gate Local-8 para hard-fail
na mainnet, seguida por Local-12 quando domínios globais tiverem entradas de
registro correspondente. Considere a saida do CLI como aviso ao operador para
esse congelamento - a mesma string de aviso e usada em tooltips de SDK e
automação para manter a paridade com os critérios de saída do roadmap. Torii agora
clusters dev/test para diagnosticar regressões. Continuar espelhando
`torii_address_domain_total{domain_kind}` não Grafana
(`dashboards/grafana/address_ingest.json`) para o pacote de evidências ADDR-7
prove que `domain_kind="local12"` ocorre em zero na janela requerida de 30
 dias antes de a mainnet desativar seleções alternativas. O pacote Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) adicionar três guarda-corpos:- `AddressLocal8Resurgence` página sempre que um contexto reporta um incremento
  Local-8 novo. Pare rollouts de modo estrito, localize um SDK superficial
  responsavel no dashboard e, se necessário, definir temporariamente
  padrão (`true`).
- `AddressLocal12Collision` dispara quando dois rótulos Local-12 fazem hash para
  o mesmo resumo. Pause promoções de manifesto, execute o toolkit Local -> Global
  para auditar o mapeamento de resumos e coordenadas com a governança Nexus antes
  de reemitir uma entrada de registro ou reativar lançamentos downstream.
- `AddressInvalidRatioSlo` avisa quando a proporção invalida em toda a frota
  (excluindo rejeições Local-8/strict-mode) excede o SLO de 0,1% por dez minutos.
  Use `torii_address_invalid_total` para localizar o contexto/razão responsavel
  e coordene com a equipe SDK proprietária antes de reativar o modo estrito.

### Trecho de nota de release (carteira e explorador)

Inclui o seguinte marcador nas notas de liberação da carteira/explorador ao enviar o
transição:

> **Enderecos:** Adicionado o helper `iroha tools address normalize`
> e conectado no CI (`ci/check_address_normalize.sh`) para que pipelines de
> carteira/explorador pode converter seletores locais alternativos para formas
> canônicas IH58/comprimidas antes de Local-8/Local-12 serem bloqueadas na
> rede principal. Atualizar quaisquer exportações personalizadas para rodar o comando e
> anexar a lista normalizada ao pacote de evidências de lançamento.