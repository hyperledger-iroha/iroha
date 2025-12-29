<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
translator: manual
source_hash: 582a75b7b68e86acd82b36ccacec2691d6552d45bb00e2f6fe5bed1424f2842a
source_last_modified: "2025-11-06T19:39:18.688464+00:00"
translation_last_reviewed: 2025-11-14
---

---
title: Segurança e acessibilidade de endereços
description: Requisitos de UX para exibir e compartilhar endereços Iroha com segurança (ADDR-6c).
---

Esta página documenta o entregável ADDR‑6c. Aplique estas restrições a wallets,
exploradores, tooling de SDK e qualquer superfície do portal que renderize ou aceite
endereços voltados para pessoas. O modelo de dados canônico está em
`docs/account_structure.md`; a checklist abaixo explica como expor esses formatos sem
comprometer segurança ou acessibilidade.

## Fluxos seguros de compartilhamento

- Faça com que toda ação de copiar/compartilhar use por padrão o endereço IH‑B32. Exiba o
  domínio resolvido como contexto de suporte para manter a string com checksum em
  destaque.
- Ofereça um atalho de “Compartilhar” que inclua o endereço em texto puro e um QR code
  derivados do mesmo payload. Permita que a pessoa usuária inspecione ambos antes de
  confirmar.
- Quando o espaço exigir truncagem (cards pequenos, notificações), mantenha o prefixo
  legível inicial, use reticências e preserve os últimos 4–6 caracteres para que o
  “âncora” do checksum sobreviva. Disponibilize um gesto de toque/atalho de teclado para
  copiar a string completa sem truncagem.
- Evite dessincronização com o clipboard exibindo um toast de confirmação que mostre
  exatamente a string IH‑B32 copiada. Quando houver telemetria, conte tentativas de copy
  versus ações de share para detectar regressões de UX rapidamente.

## Salvaguardas para IME e entrada

- Rejeite entrada non‑ASCII em campos de endereço. Quando surgirem artefatos de IME
  (full‑width, Kana, marcas de tom), mostre um aviso inline explicando como trocar o
  teclado para entrada latina antes de tentar novamente.
- Forneça uma área de paste em texto puro que remova marcas combinatórias e substitua
  espaços em branco por espaços ASCII antes da validação. Isso evita perda de progresso
  quando o usuário desativa o IME no meio do fluxo.
- Fortaleça a validação contra zero‑width joiners, variation selectors e outros code
  points Unicode “furtivos”. Registre a categoria de code point rejeitada para que suites
  de fuzzing possam incorporar a telemetria.

## Expectativas para tecnologias assistivas

- Anote cada bloco de endereço com `aria-label` ou `aria-describedby` que detailhe o
  prefixo legível e agrupe o payload em blocos de 4–8 caracteres
  (“ih dash b three two …”). Isso impede que leitores de tela produzam um fluxo
  ininteligível de caracteres.
- Anuncie eventos de copy/share bem‑sucedidos por meio de uma live region “polite”.
  Inclua o destino (clipboard, share sheet, QR) para que a pessoa usuária saiba que a
  ação foi concluída sem mover o foco.
- Forneça texto `alt` descritivo para pré‑visualizações de QR (por exemplo,
  “Endereço IH‑B32 para `<alias>@<domain>` na chain `0x1234`”). Coloque ao lado do canvas
  do QR um botão “Copiar endereço em texto” para pessoas com baixa visão.

## Endereços comprimidos exclusivos Sora

- Gating: oculte a string comprimida `snx1…` atrás de uma confirmação explícita. A
  confirmação deve deixar claro que esse formato só funciona em chains Sora Nexus.
- Rotulagem: toda ocorrência deve incluir um badge visível “Somente Sora” e um tooltip
  explicando por que outras redes exigem o formato IH‑B32.
- Proteções: se o discriminante de chain ativo não for a alocação Nexus, recuse gerar o
  endereço comprimido e redirecione o usuário de volta para IH‑B32.
- Telemetria: registre quantas vezes o formato comprimido é solicitado e copiado para que
  o playbook de incidentes consiga detectar picos de compartilhamento acidental.

## Gates de qualidade

- Estenda testes de UI automatizados (ou suites de acessibilidade em storybook) para
  garantir que os componentes de endereço exponham o metadata ARIA necessário e que
  mensagens de rejeição de IME apareçam.
- Inclua cenários de QA manual para entrada via IME (kana, pinyin), passagem com leitor
  de tela (VoiceOver/NVDA) e cópia via QR em temas de alto contraste antes de lançar.
- Torne esses checks visíveis nas checklists de release, juntamente com os testes de
  paridade IH‑B32, para que regressões continuem bloqueadas até serem corrigidas.
