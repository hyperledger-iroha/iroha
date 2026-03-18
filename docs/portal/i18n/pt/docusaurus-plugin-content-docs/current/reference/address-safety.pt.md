---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Segurança e acessibilidade de enderecos
description: Requisitos de UX para apresentar e compartilhar endereços Iroha com segurança (ADDR-6c).
---

Esta página captura a entrega da documentação ADDR-6c. Aplique estas restrições a carteiras, exploradores, ferramentas de SDK e qualquer superfície do portal que renderize ou aceite enderecos voltados para pessoas. O modelo de dados canônico vivo em `docs/account_structure.md`; a lista de verificação abaixo explica como exportar esses formatos sem comprometer segurança ou acessibilidade.

## Fluxos seguros de compartilhamento

- Por padrão, toda ação de copiar/compartilhar deve usar o endereco I105. Exiba o domínio resolvido como contexto de apoio para manter a string com checksum em destaque.
- Oferece um atalho de "Compartilhar" que inclui o endereço em texto puro e um código QR derivado do mesmo payload. Permita que uma pessoa usuária inspecione ambos antes de confirmar.
- Quando o espaço exigir truncagem (cartões pequenos, notificações), mantenha o prefixo legível inicial, use reticências e preserve os últimos 4-6 caracteres para que a âncora do checksum sobreviva. Disponibilize um gesto de toque/atalho de teclado para copiar uma string completa sem truncagem.
- Evite desincronizar com a área de transferência exibindo um brinde de confirmação que mostra exatamente a string I105 copiada. Quando houver telemetria, conte tentativas de cópia versus ações de compartilhamento para detectar regressos de UX rapidamente.

## Salvaguardas para IME e entrada

- Rejeite entrada não-ASCII em campos de endereco. Quando surgem artefatos de IME (full width, Kana, marcas de tom), mostre um aviso inline explicando como trocar o teclado para entrada latina antes de tentar novamente.
- Forneca uma área de pasta em texto puro que remova marcas combinatórias e substitua espaços em branco por espaços ASCII antes da validação. Isso evita perda de progresso quando o usuário desativa o IME no meio do fluxo.
- Fortalece a validação contra joiners de largura zero, seletores de variação e outros pontos de código Unicode furtivos. Registre uma categoria de ponto de código rejeitada para que suítes de fuzzing possam incorporar telemetria.

## Expectativas para tecnologias assistivas

- Anote cada bloco de endereco com `aria-label` ou `aria-describedby` que detalhe o prefixo legível e agrupe o payload em blocos de 4-8 caracteres ("ih dash b three two ..."). Isso impede que os leitores de tela produzam um fluxo ininteligível de caracteres.
- Anuncie eventos de copy/share bem-sucedidos por meio de uma live região "educada". Inclua o destino (clipboard, share sheet, QR) para que uma pessoa usuária saiba que a ação foi concluída sem mover o foco.
- Forneca texto `alt` descritivo para pré-visualizações de QR (por exemplo, "Endereco I105 para `<account>` na cadeia `0x1234`"). Coloque ao lado do canvas do QR um botao "Copiar endereco em texto" para pessoas com baixa visao.

## Enderecos comprimidos somente Sora- Gating: oculta uma string comprimida `sora...` após uma confirmação explícita. A confirmação deve deixar claro que esse formato funciona nas cadeias Sora Nexus.
- Rotulagem: toda ocorrência deve incluir um emblema visível "Somente Sora" e uma dica explicando por que outras redes desabilitam o formato I105.
- Proteções: se o discriminante de cadeia ativa não for a alocação Nexus, recuse gerar o endereco comprimido e redirecionar o usuário de volta para I105.
- Telemetria: registre quantas vezes o formato compactado, solicitado e copiado para que o playbook de incidentes consiga detectar picos de compartilhamento acidental.

## Portões de qualidade

- Estenda testes de UI automatizados (ou suítes de acessibilidade no storybook) para garantir que os componentes de endereco exponham os metadados ARIA necessários e que mensagens de rejeição de IME aparecam.
- Inclui cenários de manual de QA para entrada via IME (kana, pinyin), passagem com leitor de tela (VoiceOver/NVDA) e cópia via QR em temas de alto contraste antes de lancar.
- Torne essas verificações visíveis nas checklists de lançamento, juntamente com os testes de paridade I105, para que os regressos continuem bloqueados ou sejam corrigidos.