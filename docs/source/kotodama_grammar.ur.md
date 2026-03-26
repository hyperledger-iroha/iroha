---
lang: ur
direction: rtl
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9d64b88546c924258ef054d8071b38230f3f19c8a7d920f9594b0ecb84252ce
source_last_modified: "2025-12-04T09:32:10.286919+00:00"
translation_last_reviewed: 2026-01-05
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/kotodama_grammar.md -->

# Kotodama زبان کی گرامر اور معنویت

یہ دستاویز Kotodama زبان کی نحو (لیکسر، گرامر)، ٹائپنگ قواعد، تعیناتی معنویت، اور اس بات کی وضاحت کرتی ہے کہ پروگرام Norito کی pointer‑ABI روایات کے ساتھ IVM بائٹ کوڈ (`.to`) میں کیسے لوئر ہوتے ہیں۔ Kotodama کے سورس `.ko` ایکسٹینشن استعمال کرتے ہیں۔ کمپائلر IVM بائٹ کوڈ (`.to`) خارج کرتا ہے اور اختیاری طور پر مینی فیسٹ بھی واپس کر سکتا ہے۔

مضامین
- جائزہ اور مقاصد
- لغوی ساخت
- اقسام اور لٹریلز
- اعلانات اور ماڈیولز
- کنٹریکٹ کنٹینر اور میٹاڈیٹا
- فنکشنز اور پیرامیٹرز
- بیانات
- اظہار
- Builtins اور pointer‑ABI کنسٹرکٹرز
- کلیکشنز اور میپس
- تعینی تکرار اور حدود
- غلطیاں اور تشخیصات
- IVM میں کوڈ جنریشن میپنگ
- ABI، ہیڈر، اور مینی فیسٹ
- روڈ میپ

## جائزہ اور مقاصد

- تعینیت: پروگرامز کو ہر ہارڈویئر پر ایک جیسا نتیجہ دینا چاہیے؛ نہ فلوٹنگ پوائنٹ اور نہ ہی غیر تعینی ذرائع۔ تمام میزبان تعاملات Norito‑انکوڈڈ آرگومنٹس کے ساتھ syscalls کے ذریعے ہوتے ہیں۔
- قابلِ نقل: ہدف Iroha Virtual Machine ‏(IVM) بائٹ کوڈ ہے، کوئی فزیکل ISA نہیں۔ ریپو میں نظر آنے والی RISC‑V جیسے انکوڈنگز IVM ڈی کوڈر کی عمل درآمدی تفصیل ہیں اور قابلِ مشاہدہ رویے کو تبدیل نہیں کرنی چاہئیں۔
- قابلِ آڈٹ: چھوٹی اور واضح معنویت؛ نحو سے IVM اوپ کوڈز اور میزبان syscalls تک واضح میپنگ۔
- حد بندی: غیر محدود ڈیٹا پر لوپس کو واضح حدود کے ساتھ آنا چاہیے۔ میپ iteration کے سخت اصول determinism کی ضمانت دیتے ہیں۔

## لغوی ساخت

Whitespace اور تبصرے
- whitespace ٹوکنز کو جدا کرتا ہے اور بصورتِ دیگر غیر اہم ہے۔
- لائن تبصرے `//` سے شروع ہوتے ہیں اور سطر کے آخر تک جاتے ہیں۔
- بلاک تبصرے `/* ... */` nested نہیں ہوتے۔

Identifiers
- آغاز: `[A-Za-z_]` اور پھر `[A-Za-z0-9_]*`۔
- case-sensitive؛ `_` ایک درست identifier ہے مگر حوصلہ شکنی کی جاتی ہے۔

کلیدی الفاظ (محفوظ)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`.

آپریٹرز اور علامات
- حسابی: `+ - * / %`
- بٹ وائز: `& | ^ ~`، شفٹس `<< >>`
- تقابل: `== != < <= > >=`
- منطقی: `&& || !`
- اسائن: `= += -= *= /= %= &= |= ^= <<= >>=`
- متفرق: `: , ; . :: ->`
- بریکٹس: `() [] {}`

لٹریلز
- Integer: decimal (`123`)، hex (`0x2A`)، binary (`0b1010`)۔ تمام اعدادِ صحیح runtime میں 64‑bit signed ہیں؛ suffix کے بغیر لٹریلز inference سے یا بطور `int` طے ہوتے ہیں۔
- String: double quotes اور `\` escapes کے ساتھ؛ UTF‑8۔
- Boolean: `true`, `false`۔

## اقسام اور لٹریلز

Scalar اقسام
- `int`: 64‑bit two’s‑complement؛ add/sub/mul کے لیے 2^64 modulo wrap؛ division کی signed/unsigned variants IVM میں متعین ہیں؛ کمپائلر معنویت کے مطابق op منتخب کرتا ہے۔
- `bool`: منطقی قدر؛ `0`/`1` میں لوئر ہوتی ہے۔
- `string`: immutable UTF‑8 string؛ syscalls کو دیتے وقت Norito TLV میں نمائندگی؛ VM میں bytes slices اور length سے کام۔
- `bytes`: خام Norito payload؛ pointer‑ABI کے `Blob` ٹائپ کا alias، hash/crypto/proof inputs اور durable overlays کے لیے۔

Composite اقسام
- `struct Name { field: Type, ... }` صارف کی تعریف کردہ product types۔ constructors expression میں `Name(a, b, ...)` کال سِنٹیکس استعمال کرتے ہیں۔ `obj.field` سپورٹ ہے اور اندرونی طور پر tuple‑style positional fields میں لوئر ہوتا ہے۔ durable state ABI on‑chain Norito‑encoded ہے؛ کمپائلر struct order کو منعکس کرنے والے overlays بناتا ہے اور حالیہ ٹیسٹ (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) layout کو ریلیزز میں مقفل رکھتے ہیں۔
- `Map<K, V>`: deterministic associative map؛ semantics میں iteration اور iteration کے دوران mutation پر پابندیاں ہیں (نیچے دیکھیں)۔
- `Tuple (T1, T2, ...)`: anonymous product type؛ multi‑return کے لیے استعمال۔

خصوصی pointer‑ABI اقسام (host‑facing)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` وغیرہ runtime کی first‑class types نہیں ہیں۔ یہ constructors ہیں جو INPUT region میں typed, immutable pointers (Norito TLV envelopes) بناتے ہیں اور صرف syscall arguments کے طور پر یا variables کے درمیان بغیر mutation کے منتقل کیے جا سکتے ہیں۔

Type inference
- مقامی `let` bindings initializer سے type infer کرتے ہیں۔ function parameters کو واضح طور پر type کرنا ضروری ہے۔ return types مبہم نہ ہوں تو infer ہو سکتے ہیں۔

## اعلانات اور ماڈیولز

Top‑level items
- Contracts: `seiyaku Name { ... }` میں functions، state، structs اور metadata شامل ہوتے ہیں۔
- ایک فائل میں متعدد contracts کی اجازت ہے مگر حوصلہ شکنی کی جاتی ہے؛ ایک بنیادی `seiyaku` manifests میں default entry کے طور پر استعمال ہوتا ہے۔
- `struct` declarations contract کے اندر user types متعین کرتے ہیں۔

Visibility
- `kotoage fn` پبلک entrypoint کو ظاہر کرتا ہے؛ visibility dispatcher permissions پر اثر انداز ہوتی ہے، codegen پر نہیں۔

## کنٹریکٹ کنٹینر اور میٹاڈیٹا

Syntax
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

Semantics
- `meta { ... }` emitted IVM header کے لیے compiler defaults کو override کرتا ہے: `abi_version`, `vector_length` (0 کا مطلب unset)، `max_cycles` (0 کا مطلب compiler default)، `features` header feature bits (ZK tracing, vector announce) کو toggle کرتا ہے۔ unsupported features warning کے ساتھ ignore ہوتے ہیں۔ جب `meta {}` موجود نہ ہو تو compiler `abi_version = 1` emit کرتا ہے اور باقی fields کے لیے option defaults استعمال کرتا ہے۔
- `features: ["zk", "simd"]` (aliases: `"vector"`) متعلقہ header bits کی صریح درخواست ہے۔ نامعلوم feature strings اب ignore ہونے کے بجائے parser error دیتی ہیں۔
- `state` contract variables durable declare کرتا ہے۔ compiler accesses کو `STATE_GET/STATE_SET/STATE_DEL` syscalls میں lower کرتا ہے اور host انہیں per-transaction overlay میں stage کرتا ہے (rollback کیلئے checkpoint/restore، اور commit پر WSV میں flush). literal paths کیلئے access hints emit ہوتے ہیں؛ dynamic keys map-level conflict keys پر fall back ہوتے ہیں۔ explicit host reads/writes کیلئے `state_get/state_set/state_del` اور map helpers `get_or_insert_default` استعمال کریں؛ یہ Norito TLVs سے گزرتے ہیں اور نام/فیلڈ آرڈر کو stable رکھتے ہیں۔
- `state` identifiers reserved ہیں؛ parameters یا `let` bindings میں `state` نام کو shadow کرنا مسترد ہے (`E_STATE_SHADOWED`)۔
- State map values first‑class نہیں ہیں: map operations اور iteration کے لیے state identifier کو براہِ راست استعمال کریں۔ user‑defined functions کو state maps bind یا pass کرنا مسترد ہے (`E_STATE_MAP_ALIAS`)۔
- Durable state maps فی الحال صرف `int` اور pointer‑ABI key types کو سپورٹ کرتے ہیں؛ دیگر key types compile‑time پر reject ہوتے ہیں۔
- Durable state fields کو `int`, `bool`, `Json`, `Blob`/`bytes`, یا pointer‑ABI types ہونا چاہیے (ان سے بنی structs/tuples سمیت)؛ durable state کے لیے `string` سپورٹ نہیں۔

## ٹرگر ڈیکلیریشنز

ٹرگر ڈیکلیریشنز entrypoint manifests میں scheduling metadata شامل کرتی ہیں اور contract instance
کے activate ہونے پر خودکار طور پر register ہوتی ہیں (deactivate ہونے پر ہٹا دی جاتی ہیں)۔
انہیں `seiyaku` بلاک کے اندر parse کیا جاتا ہے۔

Syntax
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Notes
- `call` کو اسی contract میں public `kotoage fn` entrypoint کی طرف اشارہ کرنا چاہیے؛
  optional `namespace::entrypoint` manifest میں record ہوتا ہے مگر cross‑contract callbacks
  فی الحال runtime میں reject ہوتے ہیں (صرف local callbacks)۔
- Supported filters: `time pre_commit` اور `time schedule(start_ms, period_ms?)`, نیز
  by-call triggers کے لیے `execute trigger <name>`, `data any` ڈیٹا ایونٹس کے لیے، اور
  pipeline filters (`pipeline transaction`, `pipeline block`, `pipeline merge`, `pipeline witness`)۔
- Metadata values JSON literals (`string`, `number`, `bool`, `null`) یا `json!(...)` ہونی چاہئیں۔
- Runtime-injected metadata keys: `contract_namespace`, `contract_id`, `contract_entrypoint`,
  `contract_code_hash`, `contract_trigger_id`۔

## فنکشنز اور پیرامیٹرز

Syntax
- Declaration: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- Public: `kotoage fn name(...) { ... }`
- Initializer: `hajimari() { ... }` (runtime کے ذریعے deploy پر invoke ہوتا ہے، VM خود نہیں).
- Upgrade hook: `kaizen(args...) permission(Role) { ... }`.

Parameters اور returns
- Arguments `r10..r22` registers میں values یا INPUT pointers (Norito TLV) کے طور پر ABI کے مطابق pass ہوتے ہیں؛ اضافی args stack پر spill ہوتے ہیں۔
- Functions صفر یا ایک scalar یا tuple return کرتی ہیں۔ primary return value scalar کے لیے `r10` میں؛ tuples convention کے مطابق stack/OUTPUT میں materialize ہوتے ہیں۔

## بیانات

- Variable bindings: `let x = expr;`, `let mut x = expr;` (mutability compile‑time چیک ہے؛ runtime mutation صرف locals کے لیے مجاز).
- Assignment: `x = expr;` اور compound forms `x += 1;` وغیرہ۔ Targets variables یا map indices ہونے چاہئیں؛ tuple/struct fields immutable ہیں۔
- Control: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, C‑style `for (init; cond; step) { ... }`.
  - `for` initializers اور steps سادہ `let name = expr` یا expression statements ہونے چاہئیں؛ پیچیدہ destructuring مسترد (`E0005`, `E0006`).
  - `for` scoping: init clause bindings loop میں اور بعد میں visible ہیں؛ body یا step میں بننے والی bindings loop سے باہر نہیں جاتیں۔
- Equality (`==`, `!=`) `int`, `bool`, `string`, pointer‑ABI scalars (مثلاً `AccountId`, `Name`, `Blob`/`bytes`, `Json`) کے لیے سپورٹ ہے؛ tuples, structs, اور maps comparable نہیں۔
- Map loop: `for (k, v) in map { ... }` (deterministic؛ نیچے دیکھیں).
- Flow: `return expr;`, `break;`, `continue;`.
- Call: `name(args...);` یا `call name(args...);` (دونوں قابلِ قبول؛ compiler انہیں call statements میں normalize کرتا ہے).
- Assertions: `assert(cond);`, `assert_eq(a, b);` non‑ZK builds میں IVM `ASSERT*` یا ZK mode میں constraints پر map ہوتے ہیں۔

## اظہار

Precedence (اعلیٰ → کم)
1. Member/index: `a.b`, `a[b]`
2. Unary: `! ~ -`
3. Multiplicative: `* / %`
4. Additive: `+ -`
5. Shifts: `<< >>`
6. Relational: `< <= > >=`
7. Equality: `== !=`
8. Bitwise AND/XOR/OR: `& ^ |`
9. Logical AND/OR: `&& ||`
10. Ternary: `cond ? a : b`

Calls اور tuples
- Calls positional arguments استعمال کرتے ہیں: `f(a, b, c)`.
- Tuple literal: `(a, b, c)` اور destructure: `let (x, y) = pair;`.
- Tuple destructuring کو matching arity والی tuple/struct types درکار ہیں؛ mismatch مسترد۔

Strings اور bytes
- Strings UTF‑8 ہیں؛ raw bytes کی ضرورت والی functions `Blob` pointers کو constructors کے ذریعے قبول کرتی ہیں (Builtins دیکھیں).

## Builtins اور pointer‑ABI کنسٹرکٹرز

Pointer constructors (INPUT میں Norito TLV emit کر کے typed pointer واپس کرتے ہیں)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`

Prelude macros ان constructors کے لیے مختصر aliases اور inline validation فراہم کرتے ہیں:
- `account!("soraカタカナ...")`, `account_id!("soraカタカナ...")`
- `asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`, `asset_id!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` یا structured literals جیسے `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`, `blob!("bytes")`, `norito_bytes!("...")`

Macros اوپر والے constructors میں expand ہوتے ہیں اور غلط literals کو compile‑time پر reject کرتے ہیں۔

Implementation status
- Implemented: اوپر والے constructors string literal arguments قبول کرتے ہیں اور typed Norito TLV envelopes میں لوئر ہو کر INPUT region میں رکھے جاتے ہیں۔ یہ immutable typed pointers واپس دیتے ہیں جو syscall arguments کے طور پر استعمال ہو سکتے ہیں۔ non‑literal string expressions مسترد ہیں؛ dynamic inputs کے لیے `Blob`/`bytes` استعمال کریں۔ `blob`/`norito_bytes` runtime میں `bytes`‑typed values بھی قبول کرتے ہیں بغیر macro shims کے۔
- Extended forms:
  - `json(Blob[NoritoBytes]) -> Json*` syscall `JSON_DECODE` کے ذریعے۔
  - `name(Blob[NoritoBytes]) -> Name*` syscall `NAME_DECODE` کے ذریعے۔
  - Pointer decode from Blob/NoritoBytes: کوئی بھی pointer constructor (AXT types سمیت) `Blob`/`NoritoBytes` payload قبول کرتا ہے اور متوقع type id کے ساتھ `POINTER_FROM_NORITO` میں لوئر ہوتا ہے۔
  - Pass‑through for pointer forms: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Method sugar سپورٹ ہے: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Host/syscall builtins (SCALL میں map ہوتے ہیں؛ درست نمبرز ivm.md میں)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

Utility builtins
- `info(string|int)`: OUTPUT کے ذریعے structured event/message emit کرتا ہے۔
- `hash(blob) -> Blob*`: Norito‑encoded hash بطور Blob واپس دیتا ہے۔
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` اور `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: inline ISI builders؛ تمام آرگومنٹس compile‑time literals ہونے چاہئیں (string literals یا literal‑based pointer constructors)۔ `nullifier32` اور `inputs32` بالکل 32 bytes ہونے چاہئیں (raw string یا `0x` hex)، اور `amount` غیر منفی ہونا چاہیے۔
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `pointer_to_norito(ptr) -> NoritoBytes*`: موجودہ pointer‑ABI TLV کو NoritoBytes کے طور پر wrap کرتا ہے تاکہ ذخیرہ/منتقلی ہو سکے۔
- `isqrt(int) -> int`: integer square root (`floor(sqrt(x))`) بطور IVM opcode۔
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — native IVM opcodes پر مبنی fused arithmetic helpers (ceil division divide‑by‑zero پر trap کرتی ہے)۔

Notes
- Builtins پتلی شِمز ہیں؛ compiler انہیں register moves اور `SCALL` میں لوئر کرتا ہے۔
- Pointer constructors خالص ہیں: VM یقینی بناتی ہے کہ INPUT میں Norito TLV کال کی مدت تک immutable رہے۔
 - pointer‑ABI fields والے structs (مثلاً `DomainId`, `AccountId`) syscall arguments کو آسانی سے گروپ کرنے کے لیے استعمال کیے جا سکتے ہیں۔ compiler `obj.field` کو درست register/value پر map کرتا ہے بغیر اضافی allocations کے۔

## کلیکشنز اور میپس

Type: `Map<K, V>`
- In‑memory maps (heap پر `Map::new()` سے یا parameters میں) ایک ہی key/value pair رکھتے ہیں؛ keys اور values word‑sized types ہونے چاہئیں: `int`, `bool`, `string`, `Blob`, `bytes`, `Json`, یا pointer types (مثلاً `AccountId`, `Name`)۔
- Durable state maps (`state Map<...>`) Norito‑encoded keys/values استعمال کرتے ہیں۔ Supported keys: `int` یا pointer types۔ Supported values: `int`, `bool`, `Json`, `Blob`/`bytes`, یا pointer types۔
- `Map::new()` in‑memory single entry کو allocate اور zero‑initialize کرتا ہے (key/value = 0)؛ non‑`Map<int,int>` maps کے لیے explicit type annotation یا return type دیں۔
- State maps first‑class values نہیں: آپ انہیں reassign نہیں کر سکتے (مثلاً `M = Map::new()`); entries کو indexing سے اپڈیٹ کریں (`M[key] = value`)۔
- Operations:
  - Indexing: `map[key]` get/set value (set host syscall سے ہوتا ہے؛ runtime API mapping دیکھیں).
  - Existence: `contains(map, key) -> bool` (lowered helper؛ ممکنہ intrinsic syscall).
  - Iteration: `for (k, v) in map { ... }` deterministic order اور mutation rules کے ساتھ۔

Deterministic iteration rules
- Iteration set loop entry پر keys کا snapshot ہے۔
- ترتیب Norito‑encoded keys کے bytes کی سخت ascending lexicographic ترتیب ہے۔
- Iterated map میں loop کے دوران ساختی تبدیلیاں (insert/remove/clear) deterministic `E_ITER_MUTATION` trap پیدا کرتی ہیں۔
- Bound لازمی ہے: یا map پر declared max (`@max_len`)، explicit attribute `#[bounded(n)]`، یا `.take(n)`/`.range(..)` کے ذریعے explicit bound؛ ورنہ compiler `E_UNBOUNDED_ITERATION` emit کرتا ہے۔

Bounds helpers
- `#[bounded(n)]`: map expression پر اختیاری attribute، مثلاً `for (k, v) in my_map #[bounded(2)] { ... }`۔
- `.take(n)`: آغاز سے پہلی `n` entries iterate کرتا ہے۔
- `.range(start, end)`: نصف‑کھلے وقفے `[start, end)` میں entries iterate کرتا ہے۔ semantics `start` اور `n = end - start` کے برابر ہیں۔

Dynamic bounds پر نوٹس
- Literal bounds: `n`, `start`, `end` اگر integer literals ہوں تو پوری طرح سپورٹ ہیں اور fixed iterations میں compile ہوتے ہیں۔
- Non‑literal bounds: جب `kotodama_dynamic_bounds` فیچر `ivm` crate میں فعال ہو، compiler dynamic `n`, `start`, `end` expressions قبول کرتا ہے اور runtime assertions (non‑negative, `end >= start`) داخل کرتا ہے۔ Lowering زیادہ سے زیادہ K guarded iterations emit کرتا ہے `if (i < n)` کے ساتھ تاکہ اضافی body executions نہ ہوں (default K=2)۔ آپ K کو `CompilerOptions { dynamic_iter_cap, .. }` سے programmatically tune کر سکتے ہیں۔
- `koto_lint` چلائیں تاکہ compile سے پہلے Kotodama lint warnings دیکھ سکیں؛ main compiler parsing اور type‑checking کے بعد lowering جاری رکھتا ہے۔
- Error codes کی دستاویز [Kotodama Compiler Error Codes](./kotodama_error_codes.md) میں ہے؛ فوری وضاحت کے لیے `koto_compile --explain <code>` استعمال کریں۔

## غلطیاں اور تشخیصات

Compile‑time تشخیصات (مثالیں)
- `E_UNBOUNDED_ITERATION`: map loop کے لیے bound نہیں۔
- `E_MUT_DURING_ITER`: loop body میں iterated map کی ساختی mutation۔
- `E_STATE_SHADOWED`: local bindings `state` declarations کو shadow نہیں کر سکتیں۔
- `E_BREAK_OUTSIDE_LOOP`: `break` loop کے باہر استعمال ہوا۔
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` loop کے باہر استعمال ہوا۔
- `E0005`: for‑loop initializer supported حد سے پیچیدہ ہے۔
- `E0006`: for‑loop step clause supported حد سے پیچیدہ ہے۔
- `E_BAD_POINTER_USE`: pointer‑ABI constructor کے نتیجے کو جہاں first‑class type چاہیے ہو وہاں استعمال کرنا۔
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`۔
- Tooling: `koto_compile` bytecode emit کرنے سے پہلے lint چلاتا ہے؛ `--no-lint` کے ذریعے skip کریں یا `--deny-lint-warnings` سے lint output پر build fail کرائیں۔

Runtime VM errors (منتخب؛ مکمل فہرست ivm.md میں)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`۔

Error messages
- Diagnostics میں مستحکم `msg_id`s ہوتے ہیں جو دستیاب ہونے پر `kotoba {}` ترجمہ ٹیبلز سے map ہوتے ہیں۔

## IVM میں کوڈ جنریشن میپنگ

Pipeline
1. Lexer/Parser AST تیار کرتے ہیں۔
2. Semantic analysis نام resolve کرتا ہے، types check کرتا ہے، اور symbol tables بھرتا ہے۔
3. IR lowering ایک سادہ SSA‑style فارم میں۔
4. Register allocation IVM GPRs میں (`r10+` args/ret کے لیے)؛ stack spills۔
5. Bytecode emission: IVM‑native اور RV‑compatible encodings کا mix؛ metadata header `abi_version`, features, vector length، اور `max_cycles` کے ساتھ emit۔

Mapping highlights
- Arithmetic اور logic IVM ALU ops پر map ہوتے ہیں۔
- Branching اور control conditional branches اور jumps پر map ہوتے ہیں؛ compiler فائدہ ہو تو compressed forms استعمال کرتا ہے۔
- Locals کی memory VM stack پر spill ہوتی ہے؛ alignment enforce ہوتا ہے۔
- Builtins register moves اور 8‑bit نمبر والے `SCALL` میں لوئر ہوتے ہیں۔
- Pointer constructors Norito TLVs کو INPUT region میں رکھتے ہیں اور ان کے addresses دیتے ہیں۔
- Assertions `ASSERT`/`ASSERT_EQ` میں map ہوتی ہیں جو non‑ZK execution میں trap اور ZK builds میں constraints emit کرتی ہیں۔

Determinism constraints
- FP نہیں؛ non‑deterministic syscalls نہیں۔
- SIMD/GPU acceleration bytecode سے invisible ہے اور bit‑identical ہونی چاہیے؛ compiler hardware‑specific ops emit نہیں کرتا۔

## ABI، ہیڈر، اور مینی فیسٹ

IVM header fields جو compiler سیٹ کرتا ہے
- `version`: IVM bytecode format version (major.minor)۔
- `abi_version`: syscall table اور pointer‑ABI schema version۔
- `feature_bits`: feature flags (مثلاً `ZK`, `VECTOR`)۔
- `vector_len`: منطقی vector length (0 → unset)۔
- `max_cycles`: admission bound اور ZK padding hint۔

Manifest (اختیاری sidecar)
- `code_hash`, `abi_hash`, `meta {}` بلاک سے metadata، compiler version، اور reproducibility کے لیے build hints۔

## روڈ میپ

- **KD-231 (اپریل 2026):** iteration bounds کے لیے compile‑time range analysis شامل کرنا تاکہ loops scheduler کو bounded access sets فراہم کریں۔
- **KD-235 (مئی 2026):** pointer constructors اور ABI وضاحت کے لیے `string` سے الگ first‑class `bytes` scalar متعارف کرانا۔
- **KD-242 (جون 2026):** builtin opcode set (hash / signature verification) کو feature flags کے پیچھے deterministic fallbacks کے ساتھ بڑھانا۔
- **KD-247 (جون 2026):** error `msg_id`s کو مستحکم کرنا اور `kotoba {}` tables میں mapping برقرار رکھنا۔
### Manifest Emission

- Kotodama compiler API کمپائل شدہ `.to` کے ساتھ `ContractManifest` واپس کر سکتی ہے، بذریعہ `ivm::kotodama::compiler::Compiler::compile_source_with_manifest`۔
- Fields:
  - `code_hash`: code bytes کا hash (IVM header اور literals کے بغیر) جو compiler artefact کو bind کرنے کے لیے نکالتا ہے۔
  - `abi_hash`: پروگرام کے `abi_version` کے لیے allowed syscall surface کا stable digest (دیکھیں `ivm.md` اور `ivm::syscalls::compute_abi_hash`)۔
- اختیاری `compiler_fingerprint` اور `features_bitmap` toolchains کے لیے reserved ہیں۔
- `entrypoints`: exported entrypoints کی ordered list (public, `hajimari`, `kaizen`) جس میں مطلوبہ `permission(...)` strings اور compiler کی best‑effort read/write key hints شامل ہیں تاکہ admission logic اور schedulers متوقع WSV access پر غور کر سکیں۔
- Manifest admission‑time checks اور registries کے لیے ہے؛ lifecycle کے لیے `docs/source/new_pipeline.md` دیکھیں۔

</div>
