---
lang: pt
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2026-01-03T18:08:00.438859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Exemplo de atualização esparsa do Merkle

Este exemplo prático ilustra como o rastreamento do FASTPQ Estágio 2 codifica um
testemunha não membro usando a coluna `neighbour_leaf`. A esparsa árvore Merkle
é binário nos elementos do campo Poseidon2. As chaves são convertidas em canônicas
Strings little-endian de 32 bytes, com hash para um elemento de campo e o mais
bits significativos selecionam o ramo em cada nível.

## Cenário

- Folhas pré-estaduais
  - `asset::alice::rose` -> chave hash `0x12b7...` com valor `0x0000_0000_0000_0005`.
  - `asset::bob::rose` -> chave hash `0x1321...` com valor `0x0000_0000_0000_0003`.
- Solicitação de atualização: inserir `asset::carol::rose` com valor 2.
- O hash da chave canônica para Carol se expande para o prefixo de 5 bits `0b01011`. O
  vizinhos existentes têm prefixos `0b01010` (Alice) e `0b01101` (Bob).

Como não existe nenhuma folha cujo prefixo corresponda a `0b01011`, o provador deve fornecer
evidência adicional de que o intervalo `(alice, bob)` está vazio. O estágio 2 é preenchido
a linha de rastreamento nas colunas `path_bit_{level}`, `sibling_{level}`,
`node_in_{level}` e `node_out_{level}` (com `level` em `[0, 31]`). Todos os valores
são elementos de campo Poseidon2 codificados na forma little-endian:

| nível | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | Notas |
| ----- | ---------------- | --------------------------- | ------------------------------------ | ------------------------------------ | ----- |
| 0 | 1 | `0x241f...` (hash de folha de Alice) | `0x0000...` | `0x4b12...` (`value_2 = 2`) | Inserir: comece do zero, armazene o novo valor. |
| 1 | 1 | `0x7d45...` (vazio à direita) | Poseidon2(`node_out_0`, `sibling_0`) | Poseidon2(`sibling_1`, `node_out_1`) | Siga o prefixo bit 1. |
| 2 | 0 | `0x03ae...` (ramo Bob) | Poseidon2(`node_out_1`, `sibling_1`) | Poseidon2(`node_in_2`, `sibling_2`) | A ramificação é invertida porque bit = 0. |
| 3 | 1 | `0x9bc4...` | Poseidon2(`node_out_2`, `sibling_2`) | Poseidon2(`sibling_3`, `node_out_3`) | Níveis mais altos continuam subindo. |
| 4 | 0 | `0xe112...` | Poseidon2(`node_out_3`, `sibling_3`) | Poseidon2(`node_in_4`, `sibling_4`) | Nível raiz; o resultado é a raiz pós-estado. |

A coluna `neighbour_leaf` desta linha é preenchida com a folha de Bob
(`key = 0x1321...`, `value = 3`, `hash = Poseidon2(key, value) = 0x03ae...`). Quando
verificando, o AIR verifica se:

1. O vizinho fornecido corresponde ao irmão utilizado no nível 2.
2. A chave vizinha é lexicograficamente maior que a chave inserida e a
   o irmão esquerdo (Alice) é lexicograficamente menor.
3. Substituir a folha inserida pela vizinha reproduz a raiz pré-estado.Juntas, essas verificações provam que não existia nenhuma folha para o intervalo `(0b01010,
0b01101)` antes da atualização. Implementações que geram rastreios FASTPQ podem usar
este layout literalmente; as constantes numéricas acima são ilustrativas. Para um completo
Testemunha JSON, emite as colunas exatamente como aparecem na tabela acima (com
sufixos numéricos por nível), usando strings de bytes little-endian serializadas com
Auxiliares JSON Norito.