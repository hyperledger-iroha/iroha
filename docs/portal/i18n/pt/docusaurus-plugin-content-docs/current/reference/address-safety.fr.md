---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Segurança e acessibilidade de endereços
descrição: Exigências UX para apresentar e compartilhar endereços Iroha em segurança (ADDR-6c).
---

Esta captura de página está disponível na documentação ADDR-6c. Aplique essas restrições a carteiras, exploradores, ferramentas SDK e toda a superfície do portal que renderiza ou aceita endereços destinados a seres humanos. O modelo de donnees canônico é encontrado em `docs/account_structure.md`; a lista de verificação contém comentários explícitos, expõe esses formatos sem comprometer a segurança ou a acessibilidade.

## Flux de partage surs

- Por padrão, toda ação de cópia/partilha deve utilizar o endereço IH58. Exiba o domínio resolvido como o contexto do aplicativo para que a cadeia com a soma de verificação fique no centro.
- Proponha uma ação "Compartilhar" que reagrupe o endereço em texto bruto e um QR derivado da carga útil do meme. Permita que os usuários inspecionem os dois antes de confirmar.
- Ao impor o intervalo (cartões minúsculos, notificações), manter o prefixo visível, exibir reticências e guardar os 4-6 caracteres mais recentes para que o espaço da soma de verificação sobreviva. Oferece um teclado de gesto/raccourci para copiar a cadeia completa sem troncatura.
- Execute a dessincronização dos papéis de impressão e emita um brinde de confirmação que pré-visualiza a cadeia IH58 exata. Se a telemetria existir, compare as tentativas de cópia versus as ações de compartilhamento para detectar rapidamente as regressões UX.

## IME e proteções de entrada

- Rejeite todas as entradas não ASCII nos campos de endereço. Quando os artefatos de composição IME (largura total, Kana, marca de tonalite) forem exibidos, exiba um aviso inline explícito comente passer le clavier en saisie latine avant de reensayer.
- Forneça uma zona de colagem em texto bruto que suprima as marcas combinadas e substitua os espaços pelos espaços ASCII antes da validação. Isso evita que você perca a progressão se o usuário desativar o IME durante o fluxo.
- Execute a validação com joiners de largura zero, seletores de variação e outros pontos de código furtivos Unicode. Divulgue a categoria do ponto de código rejeitado para que as suítes de fuzzing possam importar a telemetria.

## Atentes para as tecnologias de assistência- Anote todo bloco de endereço com `aria-label` ou `aria-describedby` que forma o prefixo lisível e segmenta a carga útil em grupos de 4 a 8 caracteres ("ih traço b três dois ..."). Isso evita que os leitores de tela produzam um fluxo ininteligível de caracteres.
- Anuncie ações de cópia/compartilhamento reutilizáveis ​​por meio de uma região ao vivo no modo educado. Inclua o destino (papéis de impressão, partilha, QR) para que o usuário sache que a ação termina sem substituir o foco.
- Forneça um texto `alt` descritivo para abrir o QR (por exemplo, "Endereço IH58 para `<account>` na cadeia `0x1234`"). Oferece um substituto "Copiar o endereço em texto" na parte inferior da tela QR para usuários mal-intencionados.

## Endereços somente para Sora

- Gating: armazena em cache a cadeia compactada `sora...` após uma confirmação explícita. A confirmação é reiterar que o formato não funciona nas cadeias Sora Nexus.
- Etiqueta: cada ocorrência deve incluir um emblema visível "Somente Sora" e uma dica de ferramenta explícita para outras pesquisas que exijam o formato IH58.
- Guardrails: se o discriminante de cadeia ativa não for a alocação Nexus, recuse a geração do endereço comprimido e redirecione-o para IH58.
- Telemetria: registre a frequência de demanda e a cópia da forma compactada para que o manual de incidentes detecte as fotos de uma partida acidental.

## Portões de qualidade

- Mantenha os testes UI automatizados (ou os pacotes a11y do storybook) para verificar se os componentes do endereço expõem os metadonnees ARIA necessários e se as mensagens de IME rejeitadas são exibidas.
- Inclui cenários de manuais de controle de qualidade para entrada IME (kana, pinyin), um leitor de tela (VoiceOver/NVDA) e uma cópia de QR em temas com forte contraste antes do lançamento.
- Faites remonter essas verificações nas listas de verificação de lançamento nas costas dos testes de parite IH58 para que as regressões permaneçam bloqueadas apenas com a correção.