---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Endereço de segurança e acessibilidade
description: Endereços Iroha کو محفوظ انداز میں پیش کرنے اور شیئر کرنے کیلئے Requisitos UX (ADDR-6c).
---

یہ صفحہ Documentação ADDR-6c entregue کو ظاہر کرتا ہے۔ Você pode usar carteiras, exploradores, ferramentas SDK, e usar a superfície do portal para usar a superfície do portal e endereços voltados para humanos, renderizar, aceitar e aceitar modelo de dados canônico `docs/account_structure.md` میں ہے؛ Lista de verificação بتاتا ہے کہ ان formatos کو segurança یا acessibilidade کو متاثر کئے بغیر کیسے expor کیا جائے۔

## Fluxos de compartilhamento seguros

- ہر ação de copiar/compartilhar کی padrão کو endereço I105 بنائیں۔ domínio resolvido کو contexto de suporte کے طور پر دکھائیں تاکہ string com soma de verificação سامنے رہے۔
- ایک affordance “Compartilhar” فراہم کریں جو endereço completo em texto simples اور اسی carga útil سے بنے código QR کو pacote کرے۔ usuários کو commit کرنے سے پہلے دونوں inspecionar کرنے دیں۔
- جب جگہ کم ہو (pequenos cartões, notificações), prefixo legível por humanos رکھیں, reticências دکھائیں, اور آخری 4–6 caracteres برقرار رکھیں تاکہ âncora de soma de verificação قائم رہے۔ truncamento کے بغیر cópia completa da string کرنے کیلئے toque/atalho de teclado دیں۔
- dessincronização da área de transferência کو روکنے کیلئے brinde de confirmação دیں جو بالکل وہی Visualização de string I105 کرے جو copiar ہوئی۔ جہاں telemetria دستیاب ہو، tentativas de cópia کو ações de compartilhamento کے مقابل contagem کریں تاکہ regressões UX جلد سامنے آئیں۔

## IME e proteções de entrada

- campos de endereço میں entrada não ASCII rejeitada کریں۔ جب Artefatos de composição IME (largura total, Kana, marcas de tons) ظاہر ہوں تو aviso inline دکھائیں جو بتائے کہ دوبارہ کوشش سے پہلے teclado کو entrada latina پر کیسے لایا جائے۔
- ایک zona de colagem de texto simples فراہم کریں جو marcas de combinação ہٹائے اور espaço em branco کو espaços ASCII سے بدل دے, پھر validação کرے۔ O IME dos usuários é definido como progresso do progresso
- joiners de largura zero, seletores de variação, e pontos de código Unicode furtivos کے خلاف validação سخت کریں۔ log de categoria de ponto de código rejeitado کریں تاکہ importação de telemetria de fuzzing suites کر سکیں۔

## Expectativas de tecnologia assistiva

- ہر bloco de endereço کو `aria-label` یا `aria-describedby` سے anotar کریں جو feitiço de prefixo legível por humanos کرے اور carga útil کو 4–8 grupos de caracteres میں pedaço کرے (“ih traço b três dois…”). Existem leitores de tela بے معنی کرداروں کی لڑی نہیں بولتے۔
- eventos de cópia/compartilhamento bem-sucedidos کو atualização educada da região ao vivo کے ذریعے anunciar کریں۔ destino (área de transferência, planilha de compartilhamento, QR) شامل کریں تاکہ usuário کو foco بدلے بغیر ação مکمل ہونے کا پتا ہو۔
- Visualizações QR کیلئے texto descritivo `alt` دیں (exemplo: “Endereço I105 para `<account>` na cadeia `0x1234`”). کم بصارت والے usuários کیلئے QR canvas کے ساتھ “Copiar endereço como texto” substituto دیں۔

## Endereços compactados somente Sora- Gating: string compactada `i105` کو confirmação explícita کے پیچھے چھپائیں۔ confirmação میں دہرائیں کہ یہ formulário صرف Sora Nexus cadeias پر کام کرتی ہے۔
- Rotulagem: ہر ocorrência میں واضح emblema “somente Sora” اور dica de ferramenta دیں جو بتائے کہ دوسری redes کو formulário I105 کیوں چاہیے۔
- Guardrails: اگر discriminante de cadeia ativa Nexus alocação نہ ہو تو endereço compactado gerar کرنے سے مکمل انکار کریں اور usuário کو I105 پر واپس بھیجیں۔
- Telemetria: formulário compactado کے solicitação / cópia کی frequência ریکارڈ کریں تاکہ manual de incidentes picos de compartilhamento acidental کو detectar کر سکے۔

## Portões de qualidade

- testes automatizados de UI (یا storybook a11y suites) کو بڑھائیں تاکہ componentes de endereço مطلوبہ exposição de metadados ARIA کریں اور mensagens de rejeição de IME ظاہر ہوں۔
- cenários de controle de qualidade manual میں entrada IME (kana, pinyin), passagem do leitor de tela (VoiceOver/NVDA), اور temas de alto contraste پر cópia QR شامل کریں, liberação سے پہلے۔
- ان verificações کو listas de verificação de liberação میں testes de paridade I105 کے ساتھ شامل کریں تاکہ regressões درست ہونے تک bloqueado رہیں۔