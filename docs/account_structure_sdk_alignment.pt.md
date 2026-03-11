# Nota de implantação do I105 para responsáveis por SDK e codecs

Equipes: SDK Rust, SDK TypeScript/JavaScript, SDK Python, SDK Kotlin, ferramentas de codec

Contexto: `docs/account_structure.md` agora reflete a implementação de AccountId I105 em produção.
Alinhem o comportamento e os testes dos SDKs com a especificação canônica.

Referências-chave:
- Codec de endereço + layout do cabeçalho — `docs/account_structure.md` §2
- Registro de curvas — `docs/source/references/address_curve_registry.md`
- Tratamento de domínio Norm v1 — `docs/source/references/address_norm_v1.md`
- Vetores de fixtures — `fixtures/account/address_vectors.json`

Ações:
1. **Saída canônica:** `AccountId::to_string()`/Display DEVE emitir apenas I105
   (sem sufixo `@domain`). O hex canônico é para depuração (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical I105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **Checksum I105:** use Blake2b-512 sobre `I105PRE || prefix || payload`,
   pegue os primeiros 2 bytes. A base do alfabeto comprimido é **105**.
5. **Gate de curvas:** SDKs são Ed25519-only por padrão. Forneça opt-in explícito
   para ML‑DSA/GOST/SM (flags de build no Swift; `configureCurveSupport` em JS/Android).
   Não assuma secp256k1 habilitado por padrão fora de Rust.
6. **Sem CAIP-10:** ainda não há mapeamento CAIP‑10 entregue; não exponha nem
   dependa de conversões CAIP‑10.

Por favor confirmem quando codecs/testes estiverem atualizados; dúvidas podem
ser acompanhadas no tópico do RFC de endereçamento de contas.
