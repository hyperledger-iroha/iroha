---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Безопасность и доступность адресов
description: UX-требования para безопасного отображения и передачи адресов Iroha (ADDR-6c).
---

Este é o documento físico ADDR-6c. Verifique a configuração do navegador, exploradores, SDK de instalação e biblioteca de suporte ao portal, download gratuito ou forneça um endereço para a cidade. O modelo canônico foi instalado em `docs/account_structure.md`; Verifique se a lista não está correta, pois você pode usar essas informações para obter mais informações e entregas.

## O cenário perfeito

- Para usar o endereço I105, use o endereço I105. Ao configurar o domínio de controle para obter o contato, a linha de verificação com soma de verificação será exibida na tela.
- Предлагайте действие “Compartilhar”, которое включает полный адрес e QR-code, полученный из того же payload. Deixe que o processo seja realizado antes de você poder usá-lo.
- Если места мало (маленькие карточки, уведомления), сохраняйте читаемый префикс, показывайте многоточие и оставляйте последние 4–6 símbolos, чтобы сохранить якорь checksum. Pressione/Goryaчую chave para copiar um conjunto de dados sem uso.
- Предотвращайте рассинхронизацию буфера обмена, показывая brinde подтверждения с точной I105 строкой, которая была скопирована. Então, se você tiver telemetria, selecione a cópia e o destino “поделиться”, чтобы быстро выявлять registro UX.

## IME e salve sua tela

- Não use ASCII no seu endereço. Você pode usar artefatos IME (largura total, Kana, tamanho grande), definir a capacidade inline, объясняющее, как переключить клавиатуру на латинский ввод перед повтором.
- Дайте texto simples зону вставки, которая удаляет комбинируемые знаки и заменяет пробелы на ASCII пробелы перед валидацией. Este processo não permite o progresso da abertura do IME na rede local.
- Use métodos de validação de marceneiros de largura zero, seletores de variação e pontos de código Unicode. Логируйте категорию отклоненного кода, чтобы fuzzing suites pode importar telemetria.

## Otimização para tecnologia assistiva

- Аннотируйте каждый блок адреса с `aria-label` ou `aria-describedby`, который проговаривает читаемый префикс и distribuir carga útil em grupos de 4 a 8 símbolos (“ih traço b três dois…”). Este não é um leitor de tela que contém símbolos.
- Solicite uma cópia atualizada da região ao vivo educadamente. Указывайте пункт назначения (área de transferência, planilha de compartilhamento, QR), чтобы пользователь понимал, что действие завершено без переноса foco.
- Crie o texto `alt` para o QR-превью (por exemplo, “Endereço I105 para `<account>` na cadeia `0x1234`”). O ambiente do QR-Kanvas fornece o substituto “Copiar endereço como texto” para ser usado com segurança.

## Сжатые адреса только для Sora- Gating: скрывайте сжатую строку `i105` para obter mais informações. Para obter mais informações, esta forma será executada no conjunto Sora Nexus.
- Rotulagem: каждое появление должно включать видимый бейдж “somente Sora” e dica de ferramenta com объяснением, почему другим сетям требуется forma I105.
- Guardrails: если активный discriminante цепочки не соответствует выделению Nexus, полностью отказывайтесь генерировать сжатый адрес и направляйте пользователя обратно к I105.
- Telemetria: записывайте частоту запросов e копирования сжатой формы, чтобы playbook инцидентов мог обнаруживать всплески случайного шаринга.

##Caso

- Расширьте автоматизированные UI-tests (ou storybook a11y suites), чтобы подтверждать наличие необходимых ARIA метаданных и появление сообщений об отказе IME.
- Crie um cenário de controle de qualidade para seu IME (kana, pinyin), leitor de tela padrão (VoiceOver/NVDA) e cópia de QR no seu tema antes do lançamento.
- Verifique este teste na lista de verificação de lançamento no teste de paridade I105, чтобы регрессии оставались заблокированными до expansão.