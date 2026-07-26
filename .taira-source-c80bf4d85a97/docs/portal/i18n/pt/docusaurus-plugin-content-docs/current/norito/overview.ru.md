---
lang: pt
direction: ltr
source: docs/portal/docs/norito/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Обзор Norito

Norito — бинарный сериализации, usado no nosso Iroha: он определяет, как structуры É necessário codificar uma unidade, conectar-se ao disco e obter o contrato e a localização. Ao colocar a caixa na área de trabalho em Norito em vez de `serde`, você pode fazer isso no local de trabalho desejado идентичные байты.

Isso deve ser feito com materiais canônicos.

## Arquiteto no site oficial

- **Заголовок + payload** – Каждое сообщение Norito начинается согласования recursos (sinalizadores, soma de verificação), за которым следует голый carga útil. Упакованные раскладки e сжатие согласуются через биты заголовка.
- **Definir codificação** – `norito::codec::{Encode, Decode}` realiza a codificação de base. Este layout é usado para gerar cargas úteis em uma configuração, configurar e configurar детерминированными.
- **Схема + deriva** – `norito_derive` gera as realizações `Encode`, `Decode` e `IntoSchema`. Упакованные структуры/последовательности включены по умолчанию и описаны в `norito.md`.
- **Реестр multicodec** – Идентификаторы хешей, типов ключей e описателей payload находятся в `norito::multicodec`. A tabela automática pode ser encontrada em `multicodec.md`.

##Instrumentos

| Bem | Comando / API | Nomeação |
| --- | --- | --- |
| Fornecer serviços/serviços | `ivm_tool inspect <file>.to` | Selecione a versão ABI, sinalizadores e pontos de entrada. |
| Кодировать/декодировать em Rust | `norito::codec::{Encode, Decode}` | Realizado para todos os nossos tipos de modelo de dados. |
| JSON de interoperabilidade | `norito::json::{to_json_pretty, from_json}` | Determinar JSON no valor Norito. |
| Gerar documentos/especificações | `norito.md`, `multicodec.md` | Listas de documentação no repositório de documentos. |

##Process разработки

1. **Deriva deriva** – Insira `#[derive(Encode, Decode, IntoSchema)]` para uma nova estrutura de dados. Избегайте ручных сериализаторов, mas isso não é absolutamente necessário.
2. **Proveritь упакованные layouts** – Use `cargo test -p norito` (e recursos empacotados de matriz em `scripts/run_norito_feature_matrix.sh`), чтобы убедиться, что novos layouts estão estáveis.
3. **Documentos de segurança** – Como codificar um arquivo, instalar `norito.md` e tabela multicodec, atualizá-lo porta de entrada (`/reference/norito-codec` e este arquivo).
4. **Testes de teste Norito-first** – Testes de integração usando JSON ajuda Norito `serde_json`, isso é fornecido para você, é e é vendido.

## Быстрые ссылки

-Especificação: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec definido: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Recursos da matriz de script: `scripts/run_norito_feature_matrix.sh`
- Layouts compactados de exemplos: `crates/norito/tests/`

Verifique este item com uma inicialização simples (`/norito/getting-started`) para uma implementação prática компиляции и запуска байткода, использующего payloads Norito.