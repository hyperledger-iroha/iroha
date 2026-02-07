---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard de '@site/src/components/ExplorerAddressCard';

:::nota História Canônica
Esta página contém `docs/source/sns/address_display_guidelines.md` e temperatura
служит канонической копией портала. Este arquivo foi criado para PR переводов.
:::

Кошельки, обозреватели e примеры SDK должны относиться к адресам аккаунтов как
não é uma carga útil. Exemplo de aplicativo Android em
`examples/android/retail-wallet` método de demonstração do padrão UX:

- **Dве цели копирования.** Предоставьте две явные кнопки copia: IH58
  (предпочтительно) e use a forma somente Sora (`sora...`, второй по предпочтению). IH58 é melhorado
  делиться наружу e он используется na carga útil QR. Сжатая форма должна включать
  встроенное предупреждение, потому что работает только в Sora-aware приложениях.
  Exemplo de aplicativo Android para usar os botões Material e suas dicas de ferramentas em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, um
  O programa iOS SwiftUI funciona para o UX por meio de `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Моноширинный, выбираемый текст.** Отрисовывайте обе строки моноширинным
  modelo e `textIsSelectable="true"`, este produto pode ser testado
  Você não pode usar o IME. Selecione a opção de edição: IME pode ser configurado
  kana ou внедрить нулевой ширины кодовые точки.
- **Подсказки для неявного домена по умолчанию.** Когда селектор указывает на
  Novo domínio `default`, ajuste de posição, ajuste de operação, isso
  суффикс не требуется. Обозреватели также должны выделять канонический доменный
  ярлык, когда селектор кодирует resumo.
- **QR para IH58.** QR-коды должны кодировать строку IH58. Если генерация QR
  провалилась, покажите явную ошибку вместо пустого изображения.
- **Сообщение буфера обмена.** После копирования сжатой формы отправьте brinde
  ou snackbar, напоминающий пользователям, что она Sora-only e подвержена
  искажению IME.

A configuração dos guardrails permite que você use Unicode/IME e удовлетворяет
Critérios de configuração de cartões ADDR-6 para UX кошельков/обозревателей.

## Скриншоты для сверки

Use a configuração correta de acordo com a prova local, essas informações
botão, dicas de ferramentas e sugestões de configuração da plataforma:

- Atualização Android: `/img/sns/address_copy_android.svg`

  ![Copiar Android automaticamente](/img/sns/address_copy_android.svg)

- Atualização iOS: `/img/sns/address_copy_ios.svg`

  ![Copiar iOS automaticamente](/img/sns/address_copy_ios.svg)

## SDK SDK

O SDK do SDK fornece um auxiliar útil, configurando os formatos IH58 e сжатую, e
также строку предупреждения, чтобы UI-слои оставались согласованными:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspetor JavaScript: `inspectAccountId(...)` возвращает предупреждение о
  сжатой форме и добавляет его em `warnings`, que é literalmente traduzido
  `sora...`, чтобы обозреватели/panelи кошельков могли показывать предупреждение
  Somente Sora está disponível/validado, e não há nenhuma forma de geração de formulários.
-Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)Use estes ajudantes para codificar lógica na interface da interface do usuário.
Auxiliar de JavaScript para processar a carga útil `selector` em `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`), essas UIs podem ser usadas, является
No seletor Local-12 ou no registro, não é possível analisar a carga útil necessária.

## Instalação de demonstração



Обозреватели должны отражать телеметрию и доступность кошелька:

- Coloque `data-copy-mode="ih58|compressed|qr"` em um botão de cópia, чтобы
  Os frontais podem ser usados ​​para obter uma conexão de rede com sistema métrico Torii
  `torii_address_format_total`. O componente de demonstração será ativado
  событие `iroha:address-copy` com `{mode,timestamp}` - подключите его к своему
  pacote de análise / pacote de análise (por exemplo, no segmento ou NORITO
  coletor), painéis de controle com formatos de uso
  адресов на сервере с режимами копирования cliente. Aqui estão as configurações corretas
  Domínio Torii (`torii_address_domain_total{domain_kind}`) em que você está seguro, чтобы
  provерки вывода Local-12 могли экспортировать 30-дневное доказательство
  `domain_kind="local12"` é colocado no painel Grafana `address_ingest`.
- Сопоставляйте каждому контролу отдельные `aria-label`/`aria-describedby`,
  объясняющие, безопасен ли literal para обмена (IH58) ou только para Sora
  (сжатый). Включайте подпись неявного домена описание, чтобы вспомогательные
  A tecnologia está disponível para esse contato.
- Exibição de região ao vivo (por exemplo, `<output aria-live="polite">...</output>`),
  который объявляет результаты копирования и предупреждения, совпадая с
  usado VoiceOver/TalkBack, usado no exemplo Swift/Android.

Esta ferramenta é usada para ADDR-6b, que permite que o operador possa
наблюдать e прием Torii, e режимы копирования клиента до отключения Local
selecionador.

## Migração local -> Global

Use [Local -> Global toolkit](local-to-global-toolkit.md) para
автоматизации аудита и конверсии устаревших Seletores locais. Ajudante выводит и
JSON-отчет аудита, e конвертированный список IH58/сжатых значений, который
операторы прикладывают к prontidão тикетам, а сопутствующий runbook связывает
no painel Grafana e no Alertmanager, que executa a transição estrita.

## Быстрый справочник бинарной раскладки (ADDR-1a)

Когда SDK показывают fornece instrumentos de endereço (inspetores, подсказки
validação, construtores de manifesto), направляйте разработчиков к каноническому
formato de fio de `docs/account_structure.md`. Раскладка всегда
`header · selector · controller`, esta é a seguinte configuração de cabeçalho:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) segundo; ненулевые значения зарезервированы e
  fornecido com `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` possui controladores únicos (`0`) e multisig (`1`).
- `norm_version = 1` código de seleção padrão Norm v1. Будущие нормы будут
  переиспользовать para este pólo de 2 bits.
- `ext_flag` em vez de `0`; установленные биты означают неподдерживаемые расширения
  carga útil.

O seletor está localizado no cabeçalho do cabeçalho:```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Interface do usuário e SDK são usados ​​para selecionar o tipo de seleção:

- `0x00` = novo domínio para умолчанию (sem carga útil).
- `0x01` = resumo local (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = registro global (`registry_id:u32` big-endian).

Exemplos hexadecimais canônicos, instrumentos de cotação que podem ser usados ​​ou
ver em documentos/testes:

| Tipo de seleção | hexadecimal canônico |
|---------------|---------------|
| Novos para умолчанию | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumo local (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Sim. `docs/source/references/address_norm_v1.md` para seletor de tabelas de estado/estado
e `docs/account_structure.md` para diagramas de rede.

## Formato canônico padrão

Operadores, conversão de configuração de código local no canônico IH58 ou
Selecione o ícone, use a interface CLI do ADDR-5:

1. `iroha tools address inspect` теперь выдает структурированное JSON-резюме с IH58,
   carga útil hexadecimal compactada e canônica. Selecione o item `domain`
   no modelo `kind`/`warning` e no local de trabalho mais difícil
   `input_domain`. O produto `kind` é `local12`, CLI está pronto para uso
   stderr, um JSON-резюме отражает ту же подсказку, contém pacotes CI e SDK moгли
   ее показывать. Instale `--append-domain`, exceto o que você deseja
   codificação convertível como `<ih58>@<domain>`.
2. O SDK pode ter um aviso/resumo para o auxiliar JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  Helper сохраняет IH58 префикс, извлеченный из literal, если только вы явно не
  указали `networkPrefix`, поэтому резюме para не-default сетей не перерисовываются
  Este é o perfil padrão.3. Converta a carga útil canônica, usando `ih58.value` ou
   `compressed` из резюме (или запросите другой encoding через `--format`). Эти
   As barras são adequadas para o seu trabalho.
4. Abra manifestos, registros e documentos de clientes, formulários canônicos e
   уведомите контрагентов, что Local селекторы будут отклоняться после cutover.
5. Para o melhor ajuste possível
   `iroha tools address audit --input addresses.txt --network-prefix 753`. Comando
   literais читает, разделенные переводом строки (comentários com `#` игнорируются,
   em `--input -` ou em uma bandeira usando STDIN), você precisa de um JSON
   каноническими/IH58/сжатыми резюме для каждой записи и считает ошибки парсинга
   e предупреждения Domínio local. Use `--allow-errors` por auditoria
   `--fail-on-warning`, o operador de operação seleciona o seletor local no CI.
6. Não há necessidade de testar, usar
  Para a correção da tabela Local селекторов используйте
  para transporte CSV `input,status,format,...`, que pode ser canônico
  кодировки, предупреждения и ошибки парсинга один проход.
   Helper по умолчанию пропускает не-Local строки, конвертирует каждую оставшуюся
   Use a codificação codificada (IH58/сжатый/hex/JSON) e administre o domínio
   por `--append-domain`. Совмещайте с `--allow-errors`, чтобы продолжать скан даже
   exceto dump que contém literais.
7. CI/lint automático pode ser usado para substituir `ci/check_address_normalize.sh`, caixa
   selecione Seletores locais de `fixtures/account/address_vectors.json`,
   converta-o em `iroha tools address normalize` e feche-o perfeitamente
   `iroha tools address audit --fail-on-warning`, isso é necessário, mas isso não é possível
   эмитят Resumos locais.

`torii_address_local8_total{endpoint}` em vez de
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` e painel Grafana
`dashboards/grafana/address_ingest.json` aplicação do sinal de dados: cogda
продакшн-дашборды показывают ноль легитимных Local отправок e ноль Local-12
Colisão na técnica em 30 de dezembro, Torii substitui o portão local-8 em hard-fail
mainnet, no local-12, que é um domínio global que contém nomes de domínio
registro. Считайте вывод CLI операторским уведомлением об этом замораживании -
Essa é a solução usada nas dicas de ferramentas do SDK e automaticamente, aqui
сохранять паритет с критериями выхода дорожной карты. Torii operação para uso
classe de registro de diagnóstico. Продолжайте зеркалировать
`torii_address_domain_total{domain_kind}` e Grafana
(`dashboards/grafana/address_ingest.json`), pacote completo ADDR-7
Posso dizer que este `domain_kind="local12"` é novo na tecnologia
Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) instalado três
guarda-corpos:- `AddressLocal8Resurgence` permite que o contato seja solicitado em qualquer outro fator
  Local-8. Execute rollouts em modo estrito, instale o SDK problemático no servidor e,
  возврата сигнала к нулю - затем восстановите дефолт (`true`).
- `AddressLocal12Collision` срабатывает, когда два Local-12 лейбла хэшируются в
  um resumo. Приостановите manifestos de promoções, запустите Local -> Global
  kit de ferramentas para resumos de mapeamento de auditoria e coordenação com governança Nexus
  перевыпуском entrada de registro ou повторным включением implementações downstream.
- `AddressInvalidRatioSlo` предупреждает, когда флотский proporção inválida (исключая
  отказы Local-8/strict-mode) aumenta SLO 0,1% em um minuto mínimo. Usar
  `torii_address_invalid_total` para conexão de contato/exposição de dados e
  O SDK instalado no SDK permite que você abra a janela de modo estrito.

### Fragment релизной заметки (кошелек и обозреватель)

Ative o marcador nas notas de lançamento do código/recuperação do cutover:

> **Адреса:** Auxiliar auxiliar `iroha tools address normalize --only-local --append-domain`
> e colocado em CI (`ci/check_address_normalize.sh`), este pacote de cola/
> обозревателя могли конвертировать устаревшие Seletores locais em канонические
> IH58/formas de configuração para blocos Local-8/Local-12 na mainnet. Обновите любые
> кастомные экспорты, чтобы запускать команду и прикладывать нормализованный
> список к pacote de evidências релиза.