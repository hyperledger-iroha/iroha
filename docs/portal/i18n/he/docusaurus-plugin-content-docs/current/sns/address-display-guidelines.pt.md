---
lang: he
direction: rtl
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

לייבא את ExplorerAddressCard מ-'@site/src/components/ExplorerAddressCard';

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sns/address_display_guidelines.md` e agora serve
como a copia canonica do portal. O arquivo fonte permanece para PRs de
traducao.
:::

Carteiras, exploradores and exemplos de SDK devem tratar enderecos de conta como
מטענים imutavis. O דוגמה ל-Android קמעונאי
`examples/android/retail-wallet` הדגמה דוגמת UX:

- **Dois alvos de copia.** Envie dois botoes de copia explicitos: I105
  (preferido) e a forma comprimida somente Sora (`sora...`, segunda melhor opcao). I105 e semper seguro para
  חיבור חיצוני ואוכל או מטען ל-QR. וריאנטה קומפרימידה
  deve כולל את aviso inline porque, אז פונקציונליות של אפליקציות compatveis com
  סורה. O דוגמה לקמעונאי אנדרואיד ליגה אמבוs os botoes חומר e seus
  עצות כלים em
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, e a
  הדגמה iOS SwiftUI espelha o mesmo UX באמצעות `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **מונוספייס, texto selecionavel.** עיבוד אמבס כמחרוזות com uma fonte
  monospace e `textIsSelectable="true"` עבור שימוש ב-possam inspecionar
  valores sem invocar um IME. Evite campos editaveis: IMEs podem reescrever kana
  ou injetar pontos de codigo de largura zero.
- **Dicas de dominio padrao implicito.** Quando o seletor aponta para o dominio
  implicito `default`, mostre uma legenda lembrando os operadores de que nenhum
  סופיקס והכרחי. Exploradores tambem devem destacar או label de dominio
  canonico quando o seletor codifica um digest.
- **QR I105.** Codigos QR פיתחה קודי למחרוזת I105. Se a geracao do QR
  falhar, mostre um erro explicito em vez de uma imagem em branco.
- **Mensageria da area de transferencia.** Depois de copiar a forma comprimida,
  emita um toast ou snackbar lembrando os usuarios de que ela e somente Sora e
  propensa a distorcao por IME.

Seguir essas מעקות בטיחות evita corrupcao Unicode/IME e atende aos criterios de
aceitacao לעשות מפת הדרכים ADDR-6 עבור UX de carteiras/exploradores.

## Capturas de tela de referencia

השתמש כמתייחסים ל-seguir durante revisoes de localizacao para garantir que os
rotulos de botoes, טיפים כלים ומאפיינים אחרים של פלטפורמות:

- Referencia Android: `/img/sns/address_copy_android.svg`

  ![Referencia Android de dupla copia](/img/sns/address_copy_android.svg)

- Referencia iOS: `/img/sns/address_copy_ios.svg`

  ![Referencia iOS de dupla copia](/img/sns/address_copy_ios.svg)

## Helpers de SDK

Cada SDK expoe um helper de conveniencia que retorna as formas I105 e comprimida
junto com a string de aviso para que כמו camadas UI fiquem consistentes:- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- מפקח JavaScript: `inspectAccountId(...)` חוזר על מחרוזת
  comprimida e a anexa a `warnings` quando chamadores fornecem um מילולית
  `sora...`, עבור לוחות מחוונים de carteira/explorador possam exibir o aviso
  somente Sora durante fluxos de colagem/validacao em vez de apenas quando geram
  a forma comprimida por conta propria.
- פייתון: `AccountAddress.display_formats(network_prefix: int = 753)`
- סוויפט: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

השתמש בעוזרי אסטרטגיים בהם ניתן ליישם מחדש את הלוגיקה של קידוד ממשק המשתמש. O
עוזר JavaScript tambem expoe umloadload `selector` em `domainSummary` (`tag`,
`digest_hex`, `registry_id`, `label`) para que UIs indiquem se um seletor e
Local-12 או מחזיר את הרישום לתיקון או מטען מטען.

## Demo de instrumentacao do explorador

<ExplorerAddressCard />

Exploradores devem espelhar o trabalho de telemetria e acessibilidade da
carteira:

- Aplique `data-copy-mode="i105|i105_default|qr"` aos botoes de copia para que
  חזיתות possam emitir contadores de uso junto com a metrica Torii
  `torii_address_format_total`. O componente demo acima dispara um evento
  `iroha:address-copy` com `{mode,timestamp}`; conecte isso ao seu pipeline de
  analitica/telemetria (לדוגמה, envie para Segment ou um coletor NORITO)
  para que לוחות מחוונים possam correlacionar o uso de formatos de endereco לעשות
  servidor com modos de copia do cliente. Replique tambem os contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) ללא הזנת mesmo para
  que revisoes de aposentadoria Local-12 possam exportar uma prova de 30 dias
  `domain_kind="local12"` diretamente do painel `address_ingest` do Grafana.
- Emparelhe cada control com pistas `aria-label`/`aria-describedby` הבדלים
  que expliquem se um literal e seguro para compartilhar (I105) ou somente Sora
  (קומפרימידו). כולל א אגדה דומיניו מרומזת נה תיאור פארה א
  עזרת טכנולוגיה או גישה חזותית.
- Exponha uma regiao viva (por exemplo, `<output aria-live="polite">...</output>`)
  anunciando resultados de copia e avisos, alinhando או comportamento do
  VoiceOver/TalkBack וחיבור לדוגמאות של Swift/Android.

Esta instrumentacao satisfaz ADDR-6b ao provar que operadores podem observar
tanto a ingestao Torii quanto os modos de copia do cliente antes que os
seletores Local sejam desativados.

## ערכת כלים de migracao מקומי -> גלובלי

השתמש ב-[ערכת כלים מקומית -> גלובלי](local-to-global-toolkit.md) עבור אוטומטיזר א
revisao e conversao de seletores אלטרנטיבות מקומיות. O helper emite tanto o relatorio
JSON de auditoria quanto a list convertida I105/comprimida que operators
בחינת כרטיסים למוכנות, בדיקת לוחות מחוונים או ריבוק associado vincula
Grafana e regras Alertmanager que controlam o cutover em modo estrito.

## Referencia rapida do layout binario (ADDR-1a)Quando SDKs expuserem tooling avancado de enderecos (מפקחים, דיאס דה
validacao, builders de manifest), aponte desenvolvedores para o formato wire
canonico em `docs/account_structure.md`. O layout semper ה
`header · selector · controller`, סיביות מערכת הפעלה אחרות עושות כותרת עליונה:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (סיביות 7-5) hoje; valores nao zero sao reservados e devem
  levantar `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` מבדיל את השליטה הפשוטה (`0`) de multisig (`1`).
- `norm_version = 1` codifica as regras do seletor Norm v1. נורמות עתידיות
  reutilizarao או mesmo campo de 2 ביטים.
- `ext_flag` e semper `0`; bits ativos indicam extensoes de payload nao
  suportadas.

הבורר או הכותרת המיידית:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIs e SDKs devem estar prontas para exibir or tipo deseltor:

- `0x00` = dominio padrao implicito (מטען סם).
- `0x01` = תקציר מקומי (12-בייט `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = entrada de registro global (`registry_id:u32` big-endian).

דוגמאות hex canonicos que ferramentas de carteira podem linkar ou embutir em
 מסמכים/בדיקות:

| טיפו דה סלטור | Hex canonico |
|--------------|--------------|
| Implicito padrao | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| תקציר מקומי (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Registro global (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` para a tabela completa de
seletor/estado e `docs/account_structure.md` לדיאגרמה מלאה של בתים.

## Impor formas canonicas

Operadores que convertem codificacoes חלופות מקומיות עבור I105 canonico ou
מחרוזות מתאימות להפרדה ולתיעוד CLI של זרימת עבודה ב-ADDR-5:

1. `iroha tools address inspect` agora emite um resumo JSON estruturado com I105,
   קומפרימידו ומטעני hex canonicos. O resumo tambem inclui um objeto
   `domain` com campos `kind`/`warning` e ecoa qualquer dominio fornecido via o
   campo `input_domain`. Quando `kind` e `local12`, CLI imprime um aviso em
   stderr e o resumo JSON ecoa a mesma orientacao para que pipelines CI e SDKs
   possam exibi-la. Passe `legacy  suffix` semper que quiser reproduzir a
   codificacao convertida como `<i105>@<domain>`.
2. SDKs podem exibir o mesmo aviso/resumo באמצעות o Helper JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  O helper preserva o prefixo I105 detectado do letteral a menos que voce forneca
  מפורש `networkPrefix`, קורות חיים מפורטים עבור נאו פדראו נאו סאו
  re-renderizados silenciosamente com o prefixo padrao.3. המרת מטען שימוש canonico reutilizando os campos `i105.value` ou
   `i105_default` לעשות רזומה (אתה מבקש קוד קוד דרך `--format`). Essas
   מיתרים וסאו סגוראס עבור קומפרטילהמנטו חיצוני.
4. אטואלי מניפסטים, רישום ומסמכים וולטאדוס ולקוחות com a forma
   canonica e notifique as contrapartes de que seletores Local serao rejeitados
   quando o cutover for concluido.
5. Para conjuntos de dados em massa, הוצא להורג
   `iroha tools address audit --input addresses.txt --network-prefix 753`. הו קומנדו
   le literais separados por nova linha (commentarios iniciados com `#` sao
   ignorados, e `--input -` ou nenhum flag usa STDIN), emite um relatorio JSON
   com resumos canonicos/I105/comprimidos para cada entrada e conta erros de
   לנתח e avisos de dominio מקומי. השתמש ב-`--allow-errors` ובנוסף אלטרנטיבות של אודיטר dumps
   com linhas lixo, e trave a automacao com `strict CI post-check` quando os
   operadores estiverem prontos para bloquear seletores Local no CI.
6. Quando precisar de reescrita linha a linha, השתמש
  Para planilhas de remediacao de seletores מקומי, השתמש
  para exportar um CSV `input,status,format,...` que destaca codificacoes
  canonicas, avisos e falhas de parse ema unica passada.
   O helper ignora linhas nao Local por padrao, converte cada entrada restante
   para a codificacao solicitada (I105/comprimido/hex/JSON), eserva o dominio
   quando מקורי `legacy  suffix` e definido. Combine com `--allow-errors`
   para continuar a varredura mesmo quando um dump contem literais malformados.
7. מפעל CI/lint pode automacao `ci/check_address_normalize.sh`, que extrai
   seletores Local de `fixtures/account/address_vectors.json`, המרה באמצעות
   `iroha tools address normalize`, וביצוע מחדש
   `iroha tools address audit` למען הוכחה משחררת נאו emitem
   mais digests מקומי.

`torii_address_local8_total{endpoint}` junto com
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, e o painel Grafana
`dashboards/grafana/address_ingest.json` פורנcem o sinal de enforcement: quando
לוחות המחוונים של מערכת ההפעלה מייצרת רוב מערכות אפס סביבה חוקיות מקומיות ואפס קוליסות
Local-12 פור 30 dias consecutivos, Torii או שער Local-8 עבור hard-fail
ב-mainnet, seguido por Local-12 quando dominios globais tiverem entradas de
כתבי רישום. שקול את המילה CLI כמו אביו או מפעיל עבור
esse congelamento - מחרוזת mesma de aviso e usada em tooltips de SDK e
automacao para manter paridade com os criterios de saida do מפת הדרכים. Torii אגורה
clusters dev/test או רגרסיות אבחנתיות. המשך espelhando
`torii_address_domain_total{domain_kind}` לא Grafana
(`dashboards/grafana/address_ingest.json`) para que o pacote de evidencia ADDR-7
להוכיח que `domain_kind="local12"` permaneceu em zero na Janela Requerida de 30
 dias antes de a mainnet desativar seletores alternativos. O Pacote Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) מעקות בטיחות של adiciona tres:- `AddressLocal8Resurgence` עמוד ספר que um contexto reporta um incremento
  מקומי-8 נובו. הפוך את ההשקה של modo estrito, התמקם SDK שטחי
  responsavel ללא לוח מחוונים e, Se necessario, defina temporariamente
  padrao (`true`).
- תוויות `AddressLocal12Collision` dispara quando dois Local-12 fazem hash para
  o mesmo digest. השהה את פרסומות המניפסט, הפעל או ערכת כלים מקומית -> גלובלית
  para auditar o mapeamento de digests e coordene com a governanca Nexus antes
  de reemitir a entrada de registro או reativar השקות במורד הזרם.
- `AddressInvalidRatioSlo` avisa quando a proporcao invalida em toda a frota
  (excluindo rejeicoes Local-8/strict-mode) עולים על SLO של 0.1% לפי דקות.
  השתמש ב-`torii_address_invalid_total` עבור לוקליזר או בהקשר של תגובה/razao
  e coordene com a equipe SDK proprietaria antes de reativar o modo estrito.

### Trecho de not de release (carteira e explorador)

כולל o seguinte bullet nas notas de release de carteira/explorador ao enviar o
חתך:

> **Enderecos:** Adicionado o helper `iroha tools address normalize`
> e conectado no CI (`ci/check_address_normalize.sh`) para que pipelines de
> ממיר carteira/explorador possam מבחר חלופות מקומיות לפורמטים
> canonicas I105/comprimidas antes de Local-8/Local-12 serem bloqueados na
> רשת מרכזית. Atualize quaisquer exports personalizados para Rodar o comando e
> הוספה לרשימת נורמליזדה או צרור הוכחות לשחרור.