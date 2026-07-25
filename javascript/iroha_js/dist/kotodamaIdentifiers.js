// BEGIN GENERATED: kotodama-v1-validator-policy
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

/** Names reserved for non-type source declarations. */
export const KOTODAMA_V1_DECLARATION_RESERVED = Object.freeze([
  "int",
  "decimal",
  "quantity",
  "bool",
  "string",
  "bytes",
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
  "__kotodama_list_len",
  "__kotodama_list_get",
  "__kotodama_list_try_set",
  "__kotodama_list_try_push",
  "__kotodama_list_pop",
  "__kotodama_list_contains",
  "__kotodama_list_take",
  "__kotodama_list_enumerate",
  "__kotodama_decimal_div_round",
  "__kotodama_quantity_div_round",
  "__kotodama_quantity_ratio_round",
  "__kotodama_decimal_to_int_trunc",
  "__kotodama_decimal_to_int_round",
]);

/** Retired numeric spellings reserved only for types and source units. */
export const KOTODAMA_V1_RETIRED_TYPE_NAMES = Object.freeze([
  "i8",
  "i16",
  "i32",
  "i64",
  "i128",
  "isize",
  "u8",
  "u16",
  "u32",
  "u64",
  "u128",
  "usize",
  "num",
  "Int",
  "Integer",
  "float",
  "f32",
  "f64",
  "Decimal",
  "Fixed",
  "FixedPoint",
  "Amount",
  "amount",
  "money",
  "Quantity",
  "number",
]);

const KEYWORD_SET = new Set(KOTODAMA_V1_KEYWORDS);
const DECLARATION_RESERVED_SET = new Set(KOTODAMA_V1_DECLARATION_RESERVED);
const RETIRED_TYPE_SET = new Set(KOTODAMA_V1_RETIRED_TYPE_NAMES);
// END GENERATED: kotodama-v1-validator-policy

/** Return whether a string is one exact source identifier under V1 resolution rules. */
export function isCanonicalKotodamaIdentifier(
  value,
  { declaration = false, typeDeclaration = false } = {},
) {
  return (
    typeof value === "string" &&
    /^[A-Za-z_][A-Za-z0-9_]*$/u.test(value) &&
    !KEYWORD_SET.has(value) &&
    !value.startsWith("__kotodama_link_") &&
    (!declaration || !DECLARATION_RESERVED_SET.has(value)) &&
    (!typeDeclaration || (
      !DECLARATION_RESERVED_SET.has(value) &&
      !RETIRED_TYPE_SET.has(value)
    ))
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
