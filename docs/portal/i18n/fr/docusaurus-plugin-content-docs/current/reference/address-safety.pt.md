---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Seguranca e acessibilidade de enderecos
description: Requisitos de UX para apresentar e compartilhar enderecos Iroha com seguranca (ADDR-6c).
---

Esta pagina captura o entregavel de documentacao ADDR-6c. Aplique estas restricoes a wallets, explorers, ferramentas de SDK e qualquer superficie do portal que renderize ou aceite enderecos voltados para pessoas. O modelo de dados canonico vive em `docs/account_structure.md`; a checklist abaixo explica como expor esses formatos sem comprometer seguranca ou acessibilidade.

## Fluxos seguros de compartilhamento

- Por padrao, toda acao de copiar/compartilhar deve usar o endereco IH58. Exiba o dominio resolvido como contexto de apoio para manter a string com checksum em destaque.
- Ofereca um atalho de "Compartilhar" que inclua o endereco em texto puro e um QR code derivados do mesmo payload. Permita que a pessoa usuaria inspecione ambos antes de confirmar.
- Quando o espaco exigir truncagem (cards pequenos, notificacoes), mantenha o prefixo legivel inicial, use reticencias e preserve os ultimos 4-6 caracteres para que a ancora do checksum sobreviva. Disponibilize um gesto de toque/atalho de teclado para copiar a string completa sem truncagem.
- Evite desincronizacao com o clipboard exibindo um toast de confirmacao que mostre exatamente a string IH58 copiada. Quando houver telemetria, conte tentativas de copy versus acoes de share para detectar regressoes de UX rapidamente.

## Salvaguardas para IME e entrada

- Rejeite entrada non-ASCII em campos de endereco. Quando surgirem artefatos de IME (full width, Kana, marcas de tom), mostre um aviso inline explicando como trocar o teclado para entrada latina antes de tentar novamente.
- Forneca uma area de paste em texto puro que remova marcas combinatorias e substitua espacos em branco por espacos ASCII antes da validacao. Isso evita perda de progresso quando o usuario desativa o IME no meio do fluxo.
- Fortaleca a validacao contra zero-width joiners, variation selectors e outros code points Unicode furtivos. Registre a categoria de code point rejeitada para que suites de fuzzing possam incorporar a telemetria.

## Expectativas para tecnologias assistivas

- Anote cada bloco de endereco com `aria-label` ou `aria-describedby` que detalhe o prefixo legivel e agrupe o payload em blocos de 4-8 caracteres ("ih dash b three two ..."). Isso impede que leitores de tela produzam um fluxo ininteligivel de caracteres.
- Anuncie eventos de copy/share bem-sucedidos por meio de uma live region "polite". Inclua o destino (clipboard, share sheet, QR) para que a pessoa usuaria saiba que a acao foi concluida sem mover o foco.
- Forneca texto `alt` descritivo para pre-visualizacoes de QR (por exemplo, "Endereco IH58 para `<account>` na chain `0x1234`"). Coloque ao lado do canvas do QR um botao "Copiar endereco em texto" para pessoas com baixa visao.

## Enderecos comprimidos somente Sora

- Gating: oculte a string comprimida `sora...` atras de uma confirmacao explicita. A confirmacao deve deixar claro que esse formato so funciona em chains Sora Nexus.
- Rotulagem: toda ocorrencia deve incluir um badge visivel "Somente Sora" e um tooltip explicando por que outras redes exigem o formato IH58.
- Protecoes: se o discriminante de chain ativo nao for a alocacao Nexus, recuse gerar o endereco comprimido e redirecione o usuario de volta para IH58.
- Telemetria: registre quantas vezes o formato comprimido e solicitado e copiado para que o playbook de incidentes consiga detectar picos de compartilhamento acidental.

## Gates de qualidade

- Estenda testes de UI automatizados (ou suites de acessibilidade no storybook) para garantir que os componentes de endereco exponham a metadata ARIA necessaria e que mensagens de rejeicao de IME aparecam.
- Inclua cenarios de QA manual para entrada via IME (kana, pinyin), passagem com leitor de tela (VoiceOver/NVDA) e copia via QR em temas de alto contraste antes de lancar.
- Torne esses checks visiveis nas checklists de release, juntamente com os testes de paridade IH58, para que regressoes continuem bloqueadas ate serem corrigidas.
