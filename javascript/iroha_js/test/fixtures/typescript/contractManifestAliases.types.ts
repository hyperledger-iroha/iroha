import type {
  ContractDynamicAccessHintInput,
  ContractEntrypointParamInput,
  ContractStateDescriptorInput,
} from "../../../index.js";
import type {
  KotodamaCompiledDynamicAccessHint,
} from "../../../kotodama-compiler.js";

const camelHint: ContractDynamicAccessHintInput = {
  baseKey: "state:amount",
  keyType: "quantity",
  boundKind: "take",
  maxKeys: 64,
};
const snakeHint: ContractDynamicAccessHintInput = {
  base_key: "state:Balances",
  key_type: "AccountId",
  bound_kind: "range",
  max_keys: 1,
};
const mixedHint: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  key_type: "Name",
  boundKind: "range",
  max_keys: 2,
};
const equalAliasesHint: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  base_key: "state:Balances",
  keyType: "Name",
  key_type: "Name",
  boundKind: "range",
  bound_kind: "range",
  maxKeys: 2,
  max_keys: 2,
};

// @ts-expect-error baseKey/base_key requires at least one spelling.
const missingBase: ContractDynamicAccessHintInput = {
  keyType: "Name",
  boundKind: "take",
  maxKeys: 1,
};
// @ts-expect-error keyType/key_type requires at least one spelling.
const missingKeyType: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  boundKind: "take",
  maxKeys: 1,
};
// @ts-expect-error boundKind/bound_kind requires at least one spelling.
const missingBoundKind: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  keyType: "Name",
  maxKeys: 1,
};
// @ts-expect-error maxKeys/max_keys requires at least one spelling.
const missingMaxKeys: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  keyType: "Name",
  boundKind: "take",
};
const invalidBoundKind: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  keyType: "Name",
  // @ts-expect-error dynamic bound kinds are a closed V1 union.
  boundKind: "prefix",
  maxKeys: 1,
};
const invalidKeyType: ContractDynamicAccessHintInput = {
  baseKey: "state:Balances",
  // @ts-expect-error dynamic key types are the closed V1 StateMap key-scalar set.
  keyType: "Json",
  boundKind: "take",
  maxKeys: 1,
};

const compiledHint: KotodamaCompiledDynamicAccessHint = {
  base_key: "state:amount",
  key_type: "quantity",
  bound_kind: "range",
  max_keys: 64,
};
const invalidCompiledHint: KotodamaCompiledDynamicAccessHint = {
  base_key: "state:Balances",
  // @ts-expect-error compiler outputs cannot advertise a non-StateMap key scalar.
  key_type: "Json",
  bound_kind: "take",
  max_keys: 1,
};

const camelParam: ContractEntrypointParamInput = {
  name: "amount",
  typeName: "quantity",
};
const snakeParam: ContractEntrypointParamInput = {
  name: "amount",
  type_name: "quantity",
};
// @ts-expect-error typeName/type_name requires at least one spelling.
const missingParamType: ContractEntrypointParamInput = { name: "amount" };

const camelState: ContractStateDescriptorInput = {
  name: "amount",
  typeName: "quantity",
};
const snakeState: ContractStateDescriptorInput = {
  name: "Balances",
  type_name: "StateMap<AccountId, quantity>",
};
// @ts-expect-error typeName/type_name requires at least one spelling.
const missingStateType: ContractStateDescriptorInput = { name: "amount" };

void camelHint;
void snakeHint;
void mixedHint;
void equalAliasesHint;
void missingBase;
void missingKeyType;
void missingBoundKind;
void missingMaxKeys;
void invalidBoundKind;
void invalidKeyType;
void compiledHint;
void invalidCompiledHint;
void camelParam;
void snakeParam;
void missingParamType;
void camelState;
void snakeState;
void missingStateType;
