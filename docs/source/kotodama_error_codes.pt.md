---
lang: pt
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2026-01-03T18:08:01.373878+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Códigos de erro do compilador Kotodama

O compilador Kotodama emite códigos de erro estáveis para que os usuários de ferramentas e CLI possam
compreender rapidamente a causa de uma falha. Usar`koto_compile --explain <code>`
para imprimir a dica correspondente.

| Código | Descrição | Correção típica |
|-------|------------|-------------|
| `E0001` | O destino da ramificação está fora do intervalo para a codificação de salto IVM. | Divida funções muito grandes ou reduza o inlining para que as distâncias básicas dos blocos permaneçam dentro de ± 1 MiB. |
| `E0002` | Os sites de chamada fazem referência a uma função que nunca foi definida. | Verifique se há erros de digitação, modificadores de visibilidade ou sinalizadores de recursos que removeram o receptor. |
| `E0003` | Syscalls de estado durável foram emitidas sem a ABI v1 habilitada. | Defina `CompilerOptions::abi_version = 1` ou adicione `meta { abi_version: 1 }` dentro do contrato `seiyaku`. |
| `E0004` | Syscalls relacionados a ativos receberam ponteiros não literais. | Use `account_id(...)`, `asset_definition(...)`, etc., ou passe 0 sentinelas para padrões de host. |
| `E0005` | O inicializador de loop `for` é mais complexo do que o suportado atualmente. | Mova configurações complexas antes do loop; apenas inicializadores simples `let`/expressão são aceitos atualmente. |
| `E0006` | A cláusula de etapa do loop `for` é mais complexa do que a suportada atualmente. | Atualize o contador de loop com uma expressão simples (por exemplo, `i = i + 1`). |