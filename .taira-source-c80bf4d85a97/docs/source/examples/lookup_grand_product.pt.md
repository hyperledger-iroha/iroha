---
lang: pt
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2026-01-03T18:08:00.673232+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Exemplo de pesquisa de grande produto

Este exemplo expande o argumento de pesquisa de permissão FASTPQ mencionado em
`fastpq_plan.md`.  No pipeline do Estágio 2, o provador avalia o seletor
Colunas (`s_perm`) e testemunha (`perm_hash`) na extensão de baixo grau (LDE)
domínio, atualiza um grande produto em execução `Z_i` e, finalmente, confirma todo o
sequência com Poseidon.  O acumulador de hash é anexado à transcrição
sob o domínio `fastpq:v1:lookup:product`, enquanto o `Z_i` final ainda corresponde
o produto da tabela de permissão confirmada `T`.

Consideramos um lote pequeno com os seguintes valores de seletor:

| linha | `s_perm` | `perm_hash` |
| --- | -------- | -------------------------------------------------------- |
| 0 | 1 | `0x019a...` (função de concessão = auditor, permissão = transfer_asset) |
| 1 | 0 | `0xabcd...` (sem alteração de permissão) |
| 2 | 1 | `0x42ff...` (revogar função = auditor, perm = burn_asset) |

Seja `gamma = 0xdead...` o desafio de pesquisa Fiat-Shamir derivado do
transcrição.  O provador inicializa `Z_0 = 1` e dobra cada linha:

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

Linhas onde `s_perm = 0` não alteram o acumulador.  Depois de processar o
rastreamento, o provador Poseidon faz hash da sequência `[Z_1, Z_2, ...]` para a transcrição
mas também publica `Z_final = Z_3` (o produto final em execução) para corresponder à tabela
condição de limite.

No lado da tabela, a árvore Merkle de permissão confirmada codifica o determinístico
conjunto de permissões ativas para o slot.  O verificador (ou o provador durante
geração de testemunha) calcula

```
T = product over entries: (entry.hash + gamma)
```

O protocolo impõe a restrição de limite `Z_final / T = 1`.  Se o traço
introduziu uma permissão que não está presente na tabela (ou omitiu uma que
é), a grande proporção do produto diverge de 1 e o verificador rejeita.  Porque
ambos os lados multiplicam por `(value + gamma)` dentro do campo Cachinhos Dourados, a proporção
permanece estável em back-ends de CPU/GPU.

Para serializar o exemplo como Norito JSON para fixtures, registre a tupla de
`perm_hash`, seletor e acumulador após cada linha, por exemplo:

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

Os espaços reservados hexadecimais (`0x...`) podem ser substituídos por Cachinhos Dourados de concreto
elementos de campo ao gerar testes automatizados.  Jogos da fase 2 adicionalmente
registre o hash Poseidon do acumulador em execução, mas mantenha o mesmo formato JSON,
portanto, o exemplo pode funcionar como modelo para vetores de teste futuros.