/** Canonical Kotodama V1 lexical keywords generated from `grammar/v1.lex`. */
export const KOTODAMA_V1_KEYWORDS = Object.freeze([
  "authorize",
  "break",
  "const",
  "continue",
  "else",
  "enum",
  "error",
  "false",
  "fn",
  "for",
  "hajimari",
  "始まり",
  "if",
  "in",
  "kaizen",
  "改善",
  "kotoage",
  "言挙げ",
  "let",
  "match",
  "module",
  "return",
  "seiyaku",
  "誓約",
  "state",
  "struct",
  "trigger",
  "true",
  "var",
  "view",
]);

/** Source declaration names reserved by canonical V1 types and compiler intrinsics. */
export const KOTODAMA_V1_DECLARATION_RESERVED = Object.freeze([
  "i64",
  "u128",
  "bool",
  "string",
  "bytes",
  "Amount",
  "Json",
  "AccountId",
  "AssetDefinitionId",
  "AssetId",
  "DomainId",
  "Name",
  "NftId",
  "DataSpaceId",
  "Option",
  "Result",
  "List",
  "StateMap",
  "Secret",
  "AccountView",
  "AssetView",
  "AssetDefinitionView",
  "DomainView",
  "NftView",
  "QueryPage",
  "AxtDescriptor",
  "AssetHandle",
  "ProofBlob",
  "SoracloudRequest",
  "SoracloudResponse",
  "state_map_get",
]);

const KEYWORD_SET = new Set(KOTODAMA_V1_KEYWORDS);
const DECLARATION_RESERVED_SET = new Set(KOTODAMA_V1_DECLARATION_RESERVED);

/** Return whether a string is one exact source identifier under V1 resolution rules. */
export function isCanonicalKotodamaIdentifier(value, { declaration = false } = {}) {
  return (
    typeof value === "string" &&
    /^[A-Za-z_][A-Za-z0-9_]*$/u.test(value) &&
    !KEYWORD_SET.has(value) &&
    !value.startsWith("__kotodama_link_") &&
    (!declaration || !DECLARATION_RESERVED_SET.has(value))
  );
}

/** Return whether a string is a normal V1 entrypoint name or branded lifecycle selector. */
export function isCanonicalKotodamaEntrypoint(value) {
  return (
    value === "hajimari" ||
    value === "始まり" ||
    value === "kaizen" ||
    value === "改善" ||
    isCanonicalKotodamaIdentifier(value, { declaration: true })
  );
}
