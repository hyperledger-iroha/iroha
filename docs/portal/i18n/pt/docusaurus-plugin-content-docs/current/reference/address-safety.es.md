---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Segurança e acessibilidade de direções
description: Requisitos de UX para apresentar e compartilhar direções de Iroha com segurança (ADDR-6c).
---

Esta página captura a entrega da documentação ADDR-6c. Aplique essas restrições a carteiras, exploradores, ferramentas de SDK e qualquer superfície do portal que renderize ou aceite direções orientadas a pessoas. O modelo de dados canônico vive em `docs/account_structure.md`; a lista de verificação abaixo explica como expor esses formatos sem comprometer segurança ou acessibilidade.

## Fluxos seguros de partição

- Por defeito, cada ação de copiar/compartilhar deve usar a direção I105. Exibe o domínio resultante como contexto de apoio para que a cadeia com checksum permaneça na frente.
- Oferece uma ação "Compartilhar" que inclui a direção em texto plano e um QR derivado do mesmo payload. Permite que as pessoas inspecionem ambos antes de confirmar.
- Quando o espaço obriga a truncar (cartões pequenos, notificações), mantenha o prefixo legível, mostre pontos suspensivos e retenha os últimos 4-6 caracteres para que sobreviva a ancla do checksum. Experimente um toque/ataque de teclado para copiar a cadeia completa sem truncamento.
- Evite a desincronização dos porta-papéis emitindo um brinde de confirmação que pré-visualiza a cadeia I105 exata que se copio. Onde há telemetria, existem intenções de cópia versus ações de partição para detectar regressões de UX rapidamente.

## IME e segurança de entrada

- Rechaza entradas no ASCII em campos de direção. Quando aparecem artefatos de composição IME (largura total, Kana, marcas de tom), mostra uma advertência inline que explica como alterar o teclado para entrada em latim antes de tentar novamente.
- Prove uma zona de texto plano que elimine marcas combinadas e substitua espaços em branco por espaços ASCII antes de validar. Isso evita que a pessoa perca o progresso quando o IME é desativado no meio do fluxo.
- Suportar a validação contra marceneiros de largura zero, seletores de variação e outros pontos de código Unicode sigilosos. Registre a categoria do ponto de código rechazado para que as suítes fuzzing possam importar a telemetria.

## Expectativas de tecnologia assistiva

- Anote cada bloco de direção com `aria-label` ou `aria-describedby` que exclui o prefixo legível e agrupa a carga útil em blocos de 4 a 8 caracteres ("ih traço b três dois ..."). Isso evita que os leitores de tela produzam um fluxo de caracteres ininteligíveis.
- Anuncia os eventos de cópia/compartição exitosos por meio de uma atualização da região ao vivo no modo educado. Inclua o destino (porta-papeles, hoja de compartir, QR) para que a pessoa separe que a ação se completa sem mover o foco.
- Fornecer texto `alt` descritivo para as vistas anteriores de QR (p. ej., "Direção I105 para `<account>` na cadeia `0x1234`"). Inclui um substituto "Copiar direção como texto" junto com a tela de QR para pessoas com baixa visão.## Direções comprimidas solo Sora

- Gating: oculta a cadeia comprimida `sora...` após uma confirmação explícita. A confirmação deve reiterar que o formato só funciona nas cadeias Sora Nexus.
- Etiquetado: cada aparição deve incluir uma insígnia visível "Solo Sora" e uma dica de ferramenta que explica que outras redes exigem o formato I105.
- Guardrails: se o discriminante de cadeia ativo não for a atribuição de Nexus, rechaza gerar a direção comprimida e dirigir a pessoa de volta para I105.
- Telemetria: registre-se com a frequência solicitada e copie o formulário comprometido para que o manual de incidentes detecte picos de participação acidental.

## Portões de qualidade

- Estenda as verificações UI automatizadas (ou suítes de a11y em storybook) para afirmar que os componentes de direções expõem os metadados ARIA requeridos e que as mensagens de rechazo por IME aparecem.
- Inclui cenários de manual de controle de qualidade para entrada IME (kana, pinyin), leitor de tela (VoiceOver/NVDA) e cópia de QR em temas de alto contraste antes do lançamento.
- Reflita sobre essas verificações nas listas de verificação de liberação junto com as verificações de paridade I105 para que as regiões sigam bloqueadas até serem corrigidas.