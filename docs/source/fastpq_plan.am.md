---
lang: am
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-17T04:50:15.304524+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# FASTPQ Prover የስራ መከፋፈል

ይህ ሰነድ ለምርት ዝግጁ የሆነ FASTPQ-ISI prover ለማድረስ እና በመረጃ-ቦታ መርሐግብር ቧንቧ መስመር ውስጥ ለማገናኘት የታቀደውን እቅድ ይይዛል። እንደ TODO ምልክት ካልተደረገበት በቀር ሁሉም ከዚህ በታች ያለው ትርጉም መደበኛ ነው። የተገመተው ጤናማነት የካይሮ አይነት DEEP-FRI ወሰኖችን ይጠቀማል። የሚለካው ወሰን ከ128 ቢት በታች ከወደቀ በCI ውስጥ ያሉ አውቶሜትድ ውድቅ የናሙና ሙከራዎች አይሳኩም።

## ደረጃ 0 — ሃሽ ቦታ ያዥ (ያረፈ)
- ቆራጥ Norito ኢንኮዲንግ ከ BLAKE2b ቁርጠኝነት ጋር።
- የቦታ ያዥ ጀርባ `BackendUnavailable` የሚመለስ።
- ቀኖናዊ መለኪያ ሰንጠረዥ በ `fastpq_isi` የቀረበ።

## ደረጃ 1 - የመከታተያ ገንቢ ፕሮቶታይፕ

> ** ሁኔታ (2025-11-09):** `fastpq_prover` አሁን ቀኖናዊ ማሸጊያዎችን አጋልጧል
> ረዳቶች (`pack_bytes`፣ `PackedBytes`) እና ወሳኙ Poseidon2
> በ Goldilocks ላይ ቁርጠኝነትን ማዘዝ. ቋሚዎች በ ላይ ተጣብቀዋል
> `ark-poseidon2` `3f2b7fe` መፈጸም፣ ጊዜያዊ BLAKE2ን ስለመቀየር ክትትሉን ይዘጋል።
> ቦታ ያዥ ተዘግቷል። ወርቃማ እቃዎች (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) አሁን የሪግሬሽን ስብስብን መልሕቅ ያድርጉ።

### አላማዎች
- ለKV-update AIR የ FASTPQ መከታተያ ገንቢን ይተግብሩ። እያንዳንዱ ረድፍ መመስጠር አለበት፡-
  - `key_limbs[i]`: መሠረት-256 እጅና እግር (7 ባይት, ትንሽ-endian) ቀኖናዊ ቁልፍ መንገድ.
  - `value_old_limbs[i]`፣ `value_new_limbs[i]`፡ ለቅድመ/ልጥፍ ዋጋዎች ተመሳሳይ ማሸግ።
  - መምረጫ አምዶች፡- `s_active`፣ `s_transfer`፣ `s_mint`፣ `s_burn`፣ `s_role_grant`፣ `s_role_revoke`፣ Prometheus `s_perm`.
  - ረዳት አምዶች: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - የንብረት አምዶች: `asset_id_limbs[i]` 7-ባይት እጅና እግር በመጠቀም.
  - SMT አምዶች በየደረጃው `ℓ`፡ `path_bit_ℓ`፣ `sibling_ℓ`፣ `node_in_ℓ`፣ `node_out_ℓ`፣ በተጨማሪም `neighbour_leaf` አባል ላልሆነ።
  - ሜታዳታ አምዶች፡ `dsid`፣ `slot`።
- ** ቆራጥ ቅደም ተከተል።** የተረጋጋ ዓይነት በመጠቀም ረድፎችን በመዝገበ ቃላት በ Prometheus ደርድር። `op_rank` ካርታ፡ `transfer=0`፣ `mint=1`፣ `burn=2`፣ `role_grant=3`፣ `role_revoke=4`፣ Prometheus `original_index` ከመደርደር በፊት 0 ላይ የተመሰረተ መረጃ ጠቋሚ ነው። የተገኘውን የPoseidon2 ማዘዣ ሃሽ (የጎራ መለያ `fastpq:v1:ordering`) ቀጥል። የ hash preimageን እንደ `[domain_len, domain_limbs…, payload_len, payload_limbs…]` ያንሱት ርዝመቶች u64 የመስክ አካላት ናቸው ስለዚህ ዜሮ ባይት ተከታይ ሆኖ ይቆያል።
- የፍተሻ ምስክር፡ `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` ያመርቱ የተከማቸ አምድ `s_perm` (ምክንያታዊ ወይም የ `s_role_grant` እና `s_role_revoke`) 1. የሚና/ፈቃድ መታወቂያዎች ቋሚ ስፋት 32-ባይት LE ሕብረቁምፊዎች ሲሆኑ; ዘመን 8-ባይት LE ነው።
- ከ AIR በፊትም ሆነ በውስጥም ያሉ ልዩነቶችን ያስገድዱ፡ መራጮች እርስ በርስ የሚስማሙ፣ በንብረት ላይ የሚጠበቁ፣ dsid/slot constants።
- `N_trace = 2^k` (`pow2_ceiling` የረድፍ ብዛት); `N_eval = N_trace * 2^b` `b` የፍንዳታ ገላጭ ነው።
- የቤት እቃዎችን እና የንብረት ሙከራዎችን ያቅርቡ;
  - የዙር ጉዞዎችን ማሸግ (`fastpq_prover/tests/packing.rs`፣ `tests/fixtures/packing_roundtrip.json`)።
  - የመረጋጋት ሃሽ ማዘዝ (`tests/fixtures/ordering_hash.json`)።
  - የቢች እቃዎች (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).### የአየር አምድ ንድፍ
| የአምድ ቡድን | ስሞች | መግለጫ |
| ----------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| እንቅስቃሴ | `s_active` | 1 ለትክክለኛ ረድፎች፣ 0 ለፓዲንግ።                                                                                       |
| ዋና | `key_limbs[i]`፣ `value_old_limbs[i]`፣ `value_new_limbs[i]` | የታሸጉ የጎልድሎክስ አካላት (ትንሽ-ኤንዲያን፣ 7-ባይት እግሮች)።                                                             |
| ንብረት | `asset_id_limbs[i]` | የታሸገ ቀኖናዊ ንብረት መለያ (ትንሽ-ኤንዲያን፣ 7-ባይት እግሮች)።                                                      |
| መራጮች | `s_transfer`፣ `s_mint`፣ `s_burn`፣ `s_role_grant`፣ `s_role_revoke`፣ `s_meta_set`፣ `s_perm` | 0/1. ገደብ፡ Σ መራጮች (`s_perm` ጨምሮ) = `s_active`; `s_perm` መስተዋቶች ሚና ስጦታ/ረድፎችን መሻር።              |
| ረዳት | `delta`፣ `running_asset_delta`፣ `metadata_hash`፣ `supply_counter` | ግዛት ለእገዳዎች፣ ለጥበቃ እና ለኦዲት መንገዶች ጥቅም ላይ ይውላል።                                                           |
| SMT | `path_bit_ℓ`፣ `sibling_ℓ`፣ `node_in_ℓ`፣ `node_out_ℓ`፣ `neighbour_leaf` | በየደረጃው Poseidon2 ግብዓቶች/ውጤቶች እና የጎረቤት ምስክርነት አባል ላልሆኑ።                                         |
| ፍለጋ | `perm_hash` | Poseidon2 hash ለፈቃድ ፍለጋ (የተገደበ `s_perm = 1` ብቻ)።                                            |
| ዲበ ውሂብ | `dsid`, `slot` | በመደዳዎች ላይ የማያቋርጥ።                                                                                                 |### ሂሳብ እና ገደቦች
- **የሜዳ ማሸግ፡** ባይት ወደ 7-ባይት እጅና እግር (ትንሽ-ኢንዲያን) ተቆራርጧል። እያንዳንዱ እጅና እግር `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; እጅና እግርን አለመቀበል ≥ Goldilocks modules.
- ** ሚዛን/መቆጠብ፡** `δ = value_new - value_old` ይሁን። የቡድን ረድፎች በ `asset_id`። በእያንዳንዱ የንብረት ቡድን የመጀመሪያ ረድፍ `r_asset_start = 1` ን ይግለጹ (0 ካልሆነ) እና ይገድቡ
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  በእያንዳንዱ የንብረት ቡድን ማረጋገጫ የመጨረሻ ረድፍ ላይ
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  ዝውውሮች ገደቡን በራስ-ሰር ያሟላሉ ምክንያቱም δ እሴቶቻቸው በቡድኑ ውስጥ ወደ ዜሮ ሲደመር። ምሳሌ፡ `value_old = 100` እና `value_new = 120` በአዝሙድ ረድፍ ላይ ከሆነ δ = 20 ስለዚህ ሚንት ድምር +20 ያበረክታል እና ምንም ቃጠሎ በማይኖርበት ጊዜ የመጨረሻው ቼክ ወደ ዜሮ ይቀየራል።
- ** ንጣፍ:** `s_active` ያስተዋውቁ። ሁሉንም የረድፍ ገደቦች በ`s_active` ማባዛት እና ተከታታይ ቅድመ ቅጥያ ያስፈጽሙ፡ `s_active[i] ≥ s_active[i+1]`። መደዳ ረድፎች (`s_active=0`) ቋሚ እሴቶችን ማቆየት አለባቸው ነገርግን አለበለዚያ ግን ያልተገደቡ ናቸው።
- ** ሃሽ ማዘዝ: ** Poseidon2 hash (ጎራ `fastpq:v1:ordering`) በረድፍ ኢንኮዲንግ; ለኦዲትነት በሕዝብ IO ውስጥ ተከማችቷል።

## ደረጃ 2 - STARK Prover ኮር

### አላማዎች
- የPoseidon2 Merkle ቁርጠኝነትን በክትትል እና ፍለጋ ግምገማ ቬክተሮች ላይ ይገንቡ። መለኪያዎች፡ ተመን=2፣ አቅም=1፣ ሙሉ ዙሮች=8፣ ከፊል ዙሮች=57፣ ቋሚዎች ከ`ark-poseidon2` ጋር ተያይዘው `3f2b7fe` (v0.3.0)።
ዝቅተኛ-ዲግሪ ማራዘሚያ፡ እያንዳንዱን አምድ በጎራ `D = { g^i | i = 0 .. N_eval-1 }` ላይ ይገምግሙ፣ `N_eval = 2^{k+b}` የጎልድሎክስ 2-adic አቅምን የሚከፍልበት። `g = ω^{(p-1)/N_eval}` ከ `ω` ቋሚ የጎልድሎክስ ሥር እና `p` ሞጁሉን እናድርግ። መሰረታዊ ንዑስ ቡድንን ተጠቀም (ኮሴት የለም)። በግልባጩ ውስጥ `g` ይቅረጹ (መለያ `fastpq:v1:lde`)።
- ፖሊኖሚሎች ቅንብር፡ ለእያንዳንዱ ገደብ `C_j`፣ ቅጽ `F_j(X) = C_j(X) / Z_N(X)` ከታች ከተዘረዘሩት የዲግሪ ህዳጎች ጋር።
- የመፈለጊያ ክርክር (ፍቃዶች): ናሙና `γ` ከግልባጭ. የክትትል ምርት `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. የሠንጠረዥ ምርት `T = ∏_j (table_perm_j - γ)`. የድንበር ገደብ: `Z_final / T = 1`.
- DEEP-FRI ከአሪቲ `r ∈ {8, 16}` ጋር፡ ለእያንዳንዱ ሽፋን ሥሩን በ `fastpq:v1:fri_layer_ℓ`፣ ናሙና `β_ℓ` (መለያ `fastpq:v1:beta_ℓ`)፣ እና በ`H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k` ማጠፍ።
- የማረጋገጫ ነገር (Norito-የተመሰጠረ)
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- አረጋጋጭ መስተዋቶች prover; በ1k/5k/20k-ረድፍ ዱካዎች ከወርቃማ ግልባጮች ጋር regression suite አሂድ።

### የዲግሪ ሂሳብ
| ገደብ | ከመከፋፈል በፊት ዲግሪ | ከመራጮች በኋላ ዲግሪ | ህዳግ vs `deg(Z_N)` |
|------------|--------
| የዝውውር / ሚንት / ማቃጠል ጥበቃ | ≤1 | ≤1 | `deg(Z_N) - 2` |
| የሚና ስጦታ/መሻር ፍለጋ | ≤2 | ≤2 | `deg(Z_N) - 3` |
| ዲበ ውሂብ ስብስብ | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT hash (በደረጃ) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| ፍለጋ ታላቁ ምርት | የምርት ግንኙነት | N/A | የድንበር ገደብ |
| የድንበር ሥሮች / አቅርቦት ድምር | 0 | 0 | ትክክለኛ |

የንጣፍ ረድፎች በ `s_active` በኩል ይያዛሉ; ደሞ ረድፎች ገደቦችን ሳይጥሱ ዱካውን ወደ `N_trace` ያራዝማሉ።## ኢንኮዲንግ እና ግልባጭ (አለምአቀፍ)
- ** ባይት ማሸግ: ** ቤዝ-256 (7-ባይት እጅና እግር, ትንሽ-ኢንዲያን). `fastpq_prover/tests/packing.rs` ውስጥ ሙከራዎች.
- ** የመስክ ኢንኮዲንግ: ** ቀኖናዊ Goldilocks (ትንሽ-ኤንዲያን 64-ቢት እጅና እግር, ውድቅ ≥ p); Poseidon2 ውጽዓቶች/SMT ሥሮች እንደ 32-ባይት ትንሽ-ኢንዲያን ድርድሮች ተከታታይ።
- ** ግልባጭ (ፊያት–ሻሚር):**
  1. BLAKE2b absorb `protocol_version`፣ `params_version`፣ `parameter_set`፣ `public_io`፣ እና Poseidon2 commit tag (`fastpq:v1:init`)።
  2. Absorb `trace_root`፣ `lookup_root` (`fastpq:v1:roots`)።
  3. የማግኘት ፈታኝ `γ` (`fastpq:v1:gamma`)።
  4. የቅንብር ተግዳሮቶች `α_j` (`fastpq:v1:alpha_j`)።
  5. ለእያንዳንዱ የ FRI ንብርብር ሥር፣ ከ`fastpq:v1:fri_layer_ℓ` ጋር ይምጡ፣ `β_ℓ` (`fastpq:v1:beta_ℓ`) ያግኙ።
  6. የመጠይቅ ኢንዴክሶችን ያግኙ (`fastpq:v1:query_index`)።

  መለያዎች ንዑስ ሆሄያት ASCII; አረጋጋጮች ፈተናዎችን ከመውሰዳቸው በፊት አለመመጣጠንን አይቀበሉም። ወርቃማው የጽሑፍ ግልባጭ: `tests/fixtures/transcript_v1.json`.
- ** ስሪት: *** `protocol_version = 1`፣ `params_version` ግጥሚያዎች `fastpq_isi` መለኪያ ስብስብ።

## የመፈለግ ክርክር (ፍቃዶች)
- ቃል ኪዳን የገባው ሠንጠረዥ በቃላታዊ መልኩ በ`(role_id_bytes, permission_id_bytes, epoch_le)` የተደረደረ እና በPoseidon2 Merkle tree (`perm_root` በ `PublicIO`) በኩል ተፈጽሟል።
- የመከታተያ ምስክር `perm_hash` እና መራጭ `s_perm` (ወይም ሚና መስጠት/መሻር) ይጠቀማል። ቱፕል እንደ `role_id_bytes || permission_id_bytes || epoch_u64_le` በቋሚ ስፋቶች (32፣ 32፣ 8 ባይት) ተቀምጧል።
- የምርት ግንኙነት;
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  የድንበር ማረጋገጫ፡ `Z_final / T = 1`። ለኮንክሪት ክምችት መራመጃ `examples/lookup_grand_product.md` ይመልከቱ።

## Sparse Merkle ዛፍ ገደቦች
- `SMT_HEIGHT` (የደረጃዎች ብዛት) ይግለጹ። አምዶች `path_bit_ℓ`፣ `sibling_ℓ`፣ `node_in_ℓ`፣ `node_out_ℓ`፣ `neighbour_leaf` ለሁሉም `ℓ ∈ [0, SMT_HEIGHT)` ይታያሉ።
- የPoseidon2 መለኪያዎች በ `ark-poseidon2` መፈጸም `3f2b7fe` (v0.3.0) ላይ ተሰክተዋል; የጎራ መለያ `fastpq:v1:poseidon_node`። ሁሉም አንጓዎች ትንሽ-የኤንዲያን መስክ ኢንኮዲንግ ይጠቀማሉ።
- በየደረጃው ደንቦችን ያዘምኑ፡-
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- ያስገባዋል `(node_in_0 = 0, node_out_0 = value_new)`; ይሰርዛል ስብስብ `(node_in_0 = value_old, node_out_0 = 0)`.
- የተጠየቀው የጊዜ ክፍተት ባዶ መሆኑን ለማሳየት አባል ያልሆኑ ማረጋገጫዎች `neighbour_leaf` ያቀርባሉ። ለተሰራ ምሳሌ እና JSON አቀማመጥ `examples/smt_update.md` ይመልከቱ።
- የድንበር ገደብ፡ የመጨረሻው ሃሽ ለቅድመ ረድፎች `old_root` እና ለድህረ ረድፎች `new_root` እኩል ነው።

## የድምፅ መለኪያዎች እና SLOs
| N_trace | ማፈንዳት | FRI arity | ንብርብሮች | ጥያቄዎች | est ቢት | የማረጋገጫ መጠን (≤) | RAM (≤) | P95 መዘግየት (≤) |
| ------- | ------ | -------- | ------ | ------- | -------- | ------------ | ------- | --- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 ኪባ | 1.5 ጊባ | 0.40 ሰ (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~ 132 | 420 ኪባ | 2.5 ጊባ | 0.75 ሰ (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~ 142 | 550 ኪባ | 3.5 ጊባ | 1.20 ሴ (A100) |

መነሾቹ አባሪ ሀን ይከተላሉ። CI መታጠቂያ የተበላሹ ማስረጃዎችን ያስገኛል እና ቢገመት <128 ከሆነ አይሳካም።## የህዝብ አይኦ እቅድ
| መስክ | ባይት | ኢንኮዲንግ | ማስታወሻ |
|-------------
| `dsid` | 16 | ትንሹ-ኤንዲያን UUID | የውሂብ ቦታ መታወቂያ ለመግቢያ መስመር (አለምአቀፍ ለነባሪ መስመር)፣ በ `fastpq:v1:dsid` መለያ የታሸገ። |
| `slot` | 8 | ትንሽ-endian u64 | ናኖሴኮንዶች ከዘመናት ጀምሮ።            |
| `old_root` | 32 | ትንሽ-endian Poseidon2 መስክ ባይት | የ SMT ስርወ ከመደቡ በፊት.              |
| `new_root` | 32 | ትንሽ-endian Poseidon2 መስክ ባይት | የ SMT ሥር ከቡድን በኋላ.               |
| `perm_root` | 32 | ትንሽ-endian Poseidon2 መስክ ባይት | ለ ማስገቢያ የፈቃድ ሰንጠረዥ ሥር. |
| `tx_set_hash` | 32 | BLAKE2b | የተደረደሩ መመሪያ መለያዎች።     |
| `parameter` | var | UTF-8 (ለምሳሌ፡ `fastpq-lane-balanced`) | የመለኪያ ስብስብ ስም።                 |
| `protocol_version`, `params_version` | 2 እያንዳንዳቸው | ትንሽ-endian u16 | የስሪት ዋጋዎች።                      |
| `ordering_hash` | 32 | Poseidon2 (ትንሽ-ኤንዲያን) | የተደረደሩ ረድፎች የተረጋጋ ሃሽ።         |

ስረዛ በዜሮ እሴት ክፍሎች የተመሰጠረ ነው; የማይገኙ ቁልፎች ዜሮ ቅጠል + የጎረቤት ምስክርን ይጠቀማሉ።

`FastpqTransitionBatch.public_inputs` ለ `dsid`፣ `slot`፣ እና የስር ቃል ኪዳኖች ቀኖናዊ አገልግሎት አቅራቢ ነው።
ባች ሜታዳታ ለሃሽ/ትራንስክሪፕት ብዛት የሂሳብ አያያዝ የተጠበቀ ነው።

## Hashes ኢንኮዲንግ
- ሃሽ በማዘዝ ላይ፡ Poseidon2 (መለያ `fastpq:v1:ordering`)።
- ባች የቅርስ ሃሽ፡ BLAKE2b ከ `PublicIO || proof.commitments` (መለያ `fastpq:v1:artifact`)።

## የተከናወነ (DoD) የመድረክ ፍቺዎች
- ** ደረጃ 1 ዶዲ ***
  - የድጋሚ ጉዞ ሙከራዎችን ማሸግ እና ዕቃዎች ተዋህደዋል።
  - AIR spec (`docs/source/fastpq_air.md`) `s_active`፣ የንብረት/SMT አምዶች፣ የመራጭ ፍቺዎች (`s_perm`ን ጨምሮ) እና ምሳሌያዊ ገደቦችን ያካትታል።
  - በ PublicIO ውስጥ የተቀዳ እና በቋሚ ዕቃዎች የተረጋገጠ ሃሽ ማዘዝ።
  - SMT/መፈለጊያ ምስክር ማመንጨት ከአባልነት እና ከአባልነት ውጪ በሆኑ ቬክተሮች ተተግብሯል።
  - የጥበቃ ሙከራዎች ሽግግርን፣ ሚንትን፣ ማቃጠልን እና የተቀላቀሉ ስብስቦችን ይሸፍናል።
- ** ደረጃ 2 ዶዲ ***
  - የትራንስክሪፕት ዝርዝር ተተግብሯል; ወርቃማ ግልባጭ (`tests/fixtures/transcript_v1.json`) እና የጎራ መለያዎች ተረጋግጠዋል።
  - የPoseidon2 ፓራሜትር መፈጸም `3f2b7fe` በኪነ-ህንፃዎች ላይ የፍቅራዊነት ፈተናዎች በ prover እና አረጋጋጭ ላይ ተሰክቷል።
  - ጤናማነት CI ጠባቂ ንቁ; የማረጋገጫ መጠን/ራም/የቆይታ ጊዜ SLOs ተመዝግቧል።
- ** ደረጃ 3 ዶዲ ***
  - የጊዜ መርሐግብር አድራጊ ኤፒአይ (`SubmitProofRequest`፣ `ProofResult`) በድብቅ ቁልፎች የተመዘገበ።
  - በይዘት የተከማቹ ቅርሶችን አረጋግጥ-በድጋሚ በመሞከር/በመመለስ።
  - ቴሌሜትሪ ለወረፋ ጥልቀት፣ ለወረፋ የሚቆይበት ጊዜ፣ የአፈጻጸም መዘግየት፣ የድጋሚ ሙከራ ቆጠራዎች፣ የድጋሚ ውድቀት ቆጠራዎች እና ጂፒዩ/ሲፒዩ አጠቃቀም፣ ከዳሽቦርድ እና ለእያንዳንዱ ሜትሪክ ማንቂያ ጣራዎች ወደ ውጭ ተልኳል።## ደረጃ 5 — ጂፒዩ ማጣደፍ እና ማሻሻል
- የዒላማ ፍሬዎች፡ LDE (NTT)፣ Poseidon2 hashing፣ Merkle ዛፍ ግንባታ፣ FRI መታጠፍ።
- ቆራጥነት፡- ፈጣን ሂሳብን ያሰናክሉ፣ በሲፒዩ፣ CUDA፣ ሜታል ላይ ቢት-ተመሳሳይ ውጤቶችን ያረጋግጡ። CI የማረጋገጫ ሥሮችን በመሳሪያዎች ላይ ማወዳደር አለበት።
- የቤንችማርክ ስብስብ ሲፒዩ ከጂፒዩ ጋር በማነፃፀር በማጣቀሻ ሃርድዌር (ለምሳሌ Nvidia A100፣ AMD MI210)።
- የብረት ጀርባ (አፕል ሲሊኮን)
  - የግንባታ ስክሪፕት የከርነል ስብስብን (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) ወደ `fastpq.metallib` በ `xcrun metal`/`xcrun metallib`; የማክኦኤስ ገንቢ መሳሪያዎች የብረታ ብረት መሳሪያ ሰንሰለትን (`xcode-select --install`፣ ከዚያም `xcodebuild -downloadComponent MetalToolchain` ከተፈለገ) እንደሚያካትቱ ያረጋግጡ።
  - ለ CI ማሞቂያዎች ወይም ቆራጥ ማሸጊያዎች በእጅ እንደገና መገንባት (መስታወቶች `build.rs`)
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    ስኬታማ ግንባታዎች `FASTPQ_METAL_LIB=<path>` ያመነጫሉ ስለዚህ የሩጫ ጊዜ ሜታልሊብ በቆራጥነት ሊጭን ይችላል ።
  - የኤልዲኢ ከርነል አሁን የግምገማ ቋት በአስተናጋጁ ላይ ዜሮ የተጀመረ ነው ብሎ ያስባል። ነባሩን የ`vec![0; ..]` ድልድል መንገድ ወይም በግልጽ ዜሮ ማቋረጫዎችን እንደገና ሲጠቀሙ ያቆዩ።【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141
  - ኮሴት ማባዛት ተጨማሪ ማለፊያን ለማስቀረት በመጨረሻው የኤፍኤፍቲ ደረጃ ላይ ተጣብቋል። በኤልዲኢ ዝግጅት ላይ የሚደረጉ ማናቸውም ለውጦች ያንን የማይለዋወጥ መጠበቅ አለባቸው።【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - የተጋራው ማህደረ ትውስታ FFT/LDE ከርነል አሁን በሰድር ጥልቀት ላይ ይቆማል እና የተቀሩትን ቢራቢሮዎች እና ማንኛውንም የተገላቢጦሽ ሚዛን ወደ ተወሰነ `fastpq_fft_post_tiling` ይለፍ። የ Rust አስተናጋጁ ተመሳሳይ የዓምድ ስብስቦችን በሁለቱም ከርነሎች ይከርክታል እና `log_len` የሰድር ገደቡን ሲያልፍ የድህረ ንጣፍ መላክን ብቻ ይጀምራል፣ስለዚህ የወረፋ ጥልቀት ቴሌሜትሪ፣ የከርነል ስታቲስቲክስ እና የመውደቅ ባህሪ ጂፒዩ ሰፊውን የመድረክ ስራ ሙሉ በሙሉ ሲቆጣጠር ይቆያሉ። በመሳሪያ ላይ።【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - የማስነሻ ቅርጾችን ለመሞከር, `FASTPQ_METAL_THREADGROUP=<width>` ያዘጋጁ; የመላኪያ ዱካ እሴቱን ከመሳሪያው ወሰን ጋር ያቆራኝ እና መሻሪያውን ይመዘግባል ስለዚህ የመገለጫ ሩጫዎች እንደገና ሳይሰበሰቡ የክር ቡድን መጠኖችን ይጥረጉ።【crates/fastpq_prover/src/metal.rs:321】- የኤፍኤፍቲ ንጣፍን በቀጥታ ያስተካክሉት፡ አስተናጋጁ አሁን የክር ቡድን መስመሮችን (16 ለአጭር መከታተያዎች፣ 32 አንድ ጊዜ `log_len ≥ 6`፣ 64 አንድ ጊዜ `log_len ≥ 10`፣ 128 አንዴ `log_len ≥ 14`፣ እና 256 በ `log_len ≥ 14`፣ እና 256 ለትንሽ ደረጃ) መከታተያዎች፣ 4 `log_len ≥ 12`፣ እና አንዴ ጎራው `log_len ≥ 18/20/22` ሲደርስ የጋራ ማህደረ ትውስታ ማለፊያ አሁን መቆጣጠሪያውን ለድህረ ንጣፍ ከርነል ከማስተላለፉ በፊት 12/14/16 ደረጃዎችን ይሰራል) ከተጠየቀው ጎራ እና የመሳሪያው የማስፈጸሚያ ስፋት/ከፍተኛ ክሮች። የተወሰኑ የማስጀመሪያ ቅርጾችን ለመሰካት በ`FASTPQ_METAL_FFT_LANES` (በ 8እና256 መካከል ያለው የሁለት ኃይል) እና `FASTPQ_METAL_FFT_TILE_STAGES` (1-16) መሻር; ሁለቱም እሴቶች በ`FftArgs` በኩል ይፈስሳሉ፣ ወደሚደገፍ መስኮት ይጣበቃሉ እና ለመገለጫ ገብተዋል sweeps.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT እና LDE አምድ ባንግ አሁን ከተፈታው የክር ቡድን ስፋት የተገኘ ነው፡ አስተናጋጁ በአንድ ትዕዛዝ ቋት በግምት 4096 አመክንዮአዊ ክሮች ያነጣጥራል፣ እስከ 64 አምዶችን በአንድ ጊዜ ከክብ ቋት ሰድር ጋር ያዋህዳል እና በ 64→32→16→8→4→2→1 ጎራውን ሲያቋርጥ በ64→32→16→8→4→2→1 2¹⁶/2¹⁸/2²⁰/2²² ገደቦች። ይህ ባለ 20 ኪ-ረድፍ ቀረጻ በአንድ መላክ ≥64 አምዶች ላይ ያስቀምጠዋል ረጅም ኮሴት አሁንም በቆራጥነት መጨረሱን ያረጋግጣል። መላኪያዎች ወደ ≈2ms ዒላማ እስኪጠጉ ድረስ የማስተካከያ መርሐግብር አስማሚው አሁንም የአምድ ስፋት በእጥፍ ይጨምራል እና አሁን አንድ ናሙና መላክ ከዚያ ዒላማ በላይ በሆነ ቁጥር ≥30% በሆነ ጊዜ ቡድኑን በራስ-ሰር በግማሽ ይቀንሳል። የPoseidon permutations ተመሳሳይ አስማሚ መርሐግብር ያጋራሉ እና በ`fastpq_metal_bench` ውስጥ ያለው የ`fastpq_metal_bench` ብሎክ አሁን የተፈታውን የግዛት ቆጠራ፣ ቆብ፣ የመጨረሻ ቆይታ እና የመሻር ባንዲራ ይመዘግባል ስለዚህ ወረፋ-ጥልቀት ያለው ቴሌሜትሪ በቀጥታ ከፖሲዶን ማስተካከያ ጋር ሊያያዝ ይችላል። የሚወስነውን የኤፍኤፍቲ ባች መጠን ለመሰካት በ`FASTPQ_METAL_FFT_COLUMNS` (1-64) ይሽሩት እና ቋሚ የአምድ ቆጠራን ለማክበር የኤልዲኢ አስተላላፊ ሲፈልጉ `FASTPQ_METAL_LDE_COLUMNS` (1-64) ይጠቀሙ። የብረታ ብረት አግዳሚ ወንበሩ በእያንዳንዱ ቀረጻ ውስጥ የተፈቱ `kernel_profiles.*.columns` ግቤቶችን ይለጥፋል ስለዚህ የማስተካከያ ሙከራዎች ይቆያሉ reproducible.【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:128- ባለብዙ ወረፋ መላክ አሁን በዲስክሪት ማክ አውቶማቲክ ነው፡ አስተናጋጁ `is_low_power`፣ `is_headless`ን ይመረምራል፣ እና የመሣሪያው ቦታ ሁለት የብረት ትዕዛዝ ወረፋ ይሽከረከራል የሚለውን ለመወሰን፣ የስራ ጫናው ቢያንስ 16 አምዶችን ሲሸከም (በአምዱ የሚለካው ጂፒዩ ረጅም ርቀትን ሲይዝ) ብቻ ነው። ቆራጥነትን ሳይከፍሉ ሥራ ይበዛሉ። የትዕዛዝ ቋት ሴማፎር አሁን "ሁለት በበረራ በአንድ ወረፋ" ወለል ላይ ያስፈጽማል፣ እና ወረፋ ቴሌሜትሪ አጠቃላይ የመለኪያ መስኮቱን (`window_ms`) እና መደበኛ ስራ የሚበዛባቸው ሬሾዎች (`busy_ratio`) ለአለም አቀፍ ሴማፎር እና እያንዳንዱ ወረፋ መግባቱን ያረጋግጣል ስለዚህ ሁለቱን % busy 5 ይለቀቁ artefacuts ተመሳሳይ ጊዜ. ነባሪዎችን በ`FASTPQ_METAL_QUEUE_FANOUT` (1-4 መስመሮች) እና `FASTPQ_METAL_COLUMN_THRESHOLD` (ከደጋፊ መውጣት በፊት በትንሹ ጠቅላላ አምዶች) ይሽሩ። የብረታ ብረት ፍተሻዎች የብዝሃ-ጂፒዩ ማክስ ሽፋን እንዲቆዩ ያስገድዳቸዋል፣ እና የተፈታው ፖሊሲ ከወረፋ ጥልቀት ቴሌሜትሪ እና ከአዲሱ `metal_dispatch_queue.queues[*]` ጋር ገብቷል። አግድ። s/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- የብረት ማወቂያ አሁን `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` በቀጥታ ወደ `system_profiler` ከመውደቁ በፊት CoreGraphicsን ይሞቃል እና `FASTPQ_DEBUG_METAL_ENUM`0 ለምን ጭንቅላት28 ሲሰራ የተዘረዘሩ መሳሪያዎችን ያትማል አሁንም ወደ ሲፒዩ መንገድ ወርዷል። መሻሩ ወደ `gpu` ሲዋቀር ነገር ግን ምንም አፋጣኝ ሳይገኝ ሲቀር፣ `fastpq_metal_bench` አሁን በሲፒዩ ላይ በፀጥታ ከመቀጠል ይልቅ የማረሚያ ቁልፍ ጠቋሚው ወዲያውኑ ይስታል። ይህ በWP2-E ውስጥ የተጠራውን “ዝምተኛ የሲፒዩ ውድቀት” ክፍልን ያጠባል እና ኦፕሬተሮች የተጠቀለሉ የቁጥር ምዝግብ ማስታወሻዎችን እንዲይዙ ቁልፍ ይሰጣል። ቤንችማርኮች።【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - የፖሲዶን ጂፒዩ ጊዜዎች አሁን የሲፒዩ ውድቀቶችን እንደ “ጂፒዩ” መረጃ አድርጎ ለመያዝ ፈቃደኛ አይደሉም። `hash_columns_gpu` የፍጥነት መቆጣጠሪያው በትክክል እየሰራ እንደሆነ ሪፖርት ያደርጋል፣ `measure_poseidon_gpu` ናሙናዎችን ይጥላል (እና ማስጠንቀቂያ ይመዘግባል) የቧንቧ መስመር ወደ ኋላ በወደቀ ቁጥር እና የPoseidon ማይክሮቤንች ልጅ ጂፒዩ ሃሽንግ ከሌለ በስህተት ይወጣል። በውጤቱም፣ `gpu_recorded=false` ሜታል አፈፃፀሙ ወደ ኋላ በተመለሰ ቁጥር፣ የወረፋ ማጠቃለያ አሁንም ያልተሳካውን የመላኪያ መስኮት ይመዘግባል እና የዳሽቦርድ ማጠቃለያዎች መመለሻውን ወዲያውኑ ይጠቁማሉ። መጠቅለያው (`scripts/fastpq/wrap_benchmark.py`) አሁን አይሳካም `metal_dispatch_queue.poseidon.dispatch_count == 0` ስለዚህ Stage7 ጥቅሎች ያለ እውነተኛ ጂፒዩ ፖሲዶን መላኪያ መፈረም አይችሉም ማስረጃ.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【ስክሪፕቶች/fastpq/wrap_bench.- Poseidon hashing አሁን ያንን የዝግጅት ውል ያንጸባርቃል። `PoseidonColumnBatch` ጠፍጣፋ የመጫኛ ቋት እና ማካካሻ/ርዝመት ገላጭዎችን ያመርታል፣ አስተናጋጁ እነዚያን ገላጭዎች በአንድ ባች እንደገና ይመሰርታል እና `COLUMN_STAGING_PIPE_DEPTH` ድርብ ቋት ያካሂዳል ስለዚህ ክፍያ + ገላጭ ሰቀላዎች ከጂፒዩ ስራ ጋር ይደራረባሉ እና ሁለቱም የብረት/CUDA ኮርነሎች እያንዳንዱን የተከፋፈለው መጠን ይበላሉ። የአምዱ መፍጨት ከመውጣቱ በፊት በመሣሪያው ላይ. `hash_columns_from_coefficients` አሁን 64+ አምዶችን በነባሪነት በበረራ ላይ ባሉ ልዩ ጂፒዩዎች (በ`FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH`) በማስቀመጥ እነዚያን ስብስቦች በጂፒዩ ሰራተኛ ክር ያሰራጫል። የብረታ ብረት አግዳሚ ወንበር በ`metal_dispatch_queue.poseidon_pipeline` ስር የተፈቱትን የቧንቧ መስመር ቅንጅቶች + ባች ቆጠራን ይመዘግባል እና `kernel_profiles.poseidon.bytes` ገላጭ ትራፊክን ያካትታል ስለዚህ Stage7 ቀረጻዎች አዲሱን ABI ያረጋግጣል ከጫፍ እስከ መጨረሻ።【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_51.
- Stage7-P2 የተዋሃደ Poseidon hashing አሁን በሁለቱም የጂፒዩ የኋላ ክፍሎች ውስጥ ይገኛል። የዥረት ሰራተኛው ተከታታይ `PoseidonColumnBatch::column_window()` ቁራጮችን ወደ `hash_columns_gpu_fused` ይመገባል፣ይህም ወደ `poseidon_hash_columns_fused` ያደርጓቸዋል ስለዚህ እያንዳንዱ መላኪያ `leaf_digests || parent_digests` ከቀኖናዊው `(⌈columns / 2⌉)` `ColumnDigests` ሁለቱንም ቁርጥራጮች ያቆያል እና `merkle_root_with_first_level` የወላጅ ንብርብርን ወዲያውኑ ይበላል፣ ስለዚህ ሲፒዩ ጥልቀት-1 ኖዶችን መቼም አያሰላም እና Stage7 ቴሌሜትሪ ጂፒዩ በተቀላቀለበት ከርነል በማንኛውም ጊዜ “ወደ ኋላ መውደቅ” ወላጆችን እንደሚይዝ ማረጋገጥ ይችላል። ተሳካ።【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】 【crates/ fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` አሁን `device_profile` ብሎክን በብረት መሣሪያ ስም ፣የመዝገብ መታወቂያ ፣`low_power`/`headless` ባንዲራዎች ፣ አካባቢ (የተሰራ ፣ ማስገቢያ ፣ ውጫዊ) ፣ ልዩ አመልካች ፣ I103000 (ለምሳሌ “M3 Max”)። የመድረክ7 ዳሽቦርዶች የአስተናጋጅ ስሞችን ሳይተነተኑ በM4/M3 vs discrete GPUs በባልዲ ለመቅረጽ ይህንን መስክ ይበላሉ፣ እና JSON መርከቦች ከወረፋው/የሂውሪስቲክ ማስረጃዎች ቀጥሎ እያንዳንዱ የተለቀቀው አርቲፊክስ ሩጫውን የፈጠረው የትኛውን የበረራ ክፍል ያረጋግጣል።【crates/fastpq_prover/src/bin/fastpq_metal:2_bench።- የኤፍኤፍቲ አስተናጋጅ/የመሳሪያ መደራረብ አሁን ባለ ሁለት ማቋረጫ መስኮት ይጠቀማል፡ ባች *n* በ`fastpq_fft_post_tiling` ውስጥ ሲያልቅ፣ አስተናጋጁ ባች *n+1* ወደ ሁለተኛው የማዘጋጃ ቋት ዘረጋ እና ቋት እንደገና ጥቅም ላይ ሲውል ብቻ ይቆማል። የኋለኛው ክፍል ምን ያህሉ ጠፍጣፋዎች እንደነበሩ እና ጂፒዩ መጠናቀቅን ከመጠበቅ አንጻር የፈጀውን ጊዜ በመዘርጋት ያሳለፈውን ጊዜ ይመዘግባል እና `fastpq_metal_bench` የተሰበሰበውን `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` ብሎክ ስለሚለቅ ቅርሶች ከፀጥታ አስተናጋጅ ድንኳኖች ይልቅ መደራረብን ማረጋገጥ ይችላሉ። የJSON ዘገባ አሁን ደግሞ በ`column_staging.phases.{fft,lde,poseidon}` ስር ያለውን አጠቃላይ ድምር በየደረጃው ይሰብራል፣ ይህም የStage7 ቀረጻዎች FFT/LDE/Poseidon ዝግጅት በአስተናጋጅ የታሰረ መሆኑን ወይም በጂፒዩ መጠናቀቅ ላይ በመጠባበቅ ላይ መሆኑን ያረጋግጣል። የPoseidon permutations ያው የተዋሃዱ የመድረክ ማቋረጫዎችን እንደገና ይጠቀማሉ፣ ስለዚህ `--operation poseidon_hash_columns` ቀረጻዎች አሁን በፖሲዶን የተወሰነ `column_staging` ዴልታዎችን ከወረፋ ጥልቅ ማስረጃዎች ጋር ያለ መሳሪያ መሳሪያ ይለቃሉ። አዲሱ የ`column_staging.samples.{fft,lde,poseidon}` ድርድሮች በአንድ ባች `batch/flatten_ms/wait_ms/wait_ratio` tuples ይመዘግባሉ፣ ይህም የ`COLUMN_STAGING_PIPE_DEPTH` መደራረብ መያዙን ማረጋገጥ (ወይም አስተናጋጁ ጂፒዩ መጠበቅ ሲጀምር ለመለየት ቀላል ያደርገዋል) ማጠናቀቅ)። tpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- Poseidon2 acceleration አሁን እንደ ከፍተኛ ሰፈር ይሰራል የብረት ከርነል፡ እያንዳንዱ ክር ቡድን ክብ ቋሚዎችን እና ኤምዲኤስ ረድፎችን ወደ ክር ቡድን ማህደረ ትውስታ ይገለበጣል፣ ሙሉ/ከፊል ዙሮችን ይከፍታል እና በአንድ መስመር ላይ ብዙ ግዛቶችን ይራመዳል ስለዚህ እያንዳንዱ መላኪያ ቢያንስ 4096 ምክንያታዊ ክሮች ይጀምራል። የማስጀመሪያውን ቅርፅ በ`FASTPQ_METAL_POSEIDON_LANES` (በ 32እና256 መካከል ያሉት የሁለት ሃይሎች፣ በመሳሪያው ገደብ ላይ የተጣበቁ) እና `FASTPQ_METAL_POSEIDON_BATCH` (1-32 ግዛቶች በአንድ ሌይን) በኩል የመገለጫ ሙከራዎችን `fastpq.metallib` እንደገና ሳይገነቡ ይሻሩ። የ Rust አስተናጋጁ ከመላኩ በፊት የተፈጠረውን ማስተካከያ በ`PoseidonArgs` በኩል ይከራል። አስተናጋጁ አሁን `MTLDevice::{is_low_power,is_headless,location}`ን በአንድ ቡት አንድ ጊዜ ያነሳል እና በራስ-ሰር የተለየ ጂፒዩዎችን ወደ VRAM-ደረጃ ጅምር ያዳላል (`256×24` በ≥48GiB ክፍሎች፣ `256×20` በ 32GiB፣ Prometheus ዝቅተኛ ኃይል) `256×8` (ለ 128/64 ሌይን ሃርድዌር ውድቀት 8/6 ግዛቶች በአንድ መስመር መጠቀማቸውን ቀጥለዋል) ስለዚህ ኦፕሬተሮች env vars ሳይነኩ> 16-ግዛት የቧንቧ መስመር ጥልቀት ያገኛሉ። `fastpq_metal_bench` ራሱን የቻለ `poseidon_microbench` ብሎክን ለመቅረጽ በ`fastpq_metal_bench` እራሱን እንደገና ይፈጽማል ስለዚህ የሚለቀቁት ቅርሶች የኮንክሪት ፍጥነትን ሊጠቅሱ ይችላሉ። ተመሳሳይ የገጽታ `poseidon_pipeline` ቴሌሜትሪ (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`) ስለዚህ Stage7 ማስረጃዎች በእያንዳንዱ ጂፒዩ ላይ መደራረብ መስኮት ያረጋግጣል. መከታተያ። tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:19
  LDE tile staging አሁን የኤፍኤፍቲ ሂዩሪስቲክስን ያንፀባርቃል፡ ከባድ ዱካዎች በጋራ ማህደረ ትውስታ ማለፊያ አንድ ጊዜ `log₂(len) ≥ 18` 12 ደረጃዎችን ብቻ ያከናውናሉ፣ በሎግ₂20 ወደ 10 ደረጃዎች ይወርዳሉ እና በሎግ₂22 ላይ ወደ ስምንት ደረጃዎች በማጣበቅ ሰፊዎቹ ቢራቢሮዎች ወደ ድህረ-ቲሊንግ ከርነል ይሸጋገራሉ። የሚወስን ጥልቀት በሚፈልጉበት ጊዜ በ`FASTPQ_METAL_LDE_TILE_STAGES` (1-32) ይሽሩ። አስተናጋጁ የድህረ ንጣፍ መላክን የሚጀምረው ሂዩሪስቲክ ቀደም ብሎ ሲቆም ብቻ ነው ስለዚህ ወረፋ-ጥልቀት እና የከርነል ቴሌሜትሪ የሚወስኑት።【crates/fastpq_prover/src/metal.rs:827】
  - የከርነል ማይክሮ ማመቻቸት፡ የጋራ ማህደረ ትውስታ FFT/LDE ንጣፎች አሁን ለእያንዳንዱ ቢራቢሮ `pow_mod*` ን እንደገና ከመገምገም ይልቅ የየሌይን ትዊድል እና የኮሴት ደረጃዎችን እንደገና ይጠቀማሉ። እያንዳንዱ መስመር Prometheus፣ `w_stride`፣ እና (ሲያስፈልግ) የኮሴት እርምጃውን በብሎኬት አንድ ጊዜ ያሰላል፣ ከዚያም በማካካሻዎቹ ውስጥ ይሰራጫል፣ በ `apply_stage_tile`/`apply_stage_tile`/`apply_stage_tile`/Prometheus ~ 1.55s ከቅርብ ጊዜ ሂውሪስቲክስ ጋር (አሁንም ከ950 ሚሴ ግብ በላይ ነው፣ ነገር ግን ተጨማሪ ~50 ሚሴ ማሻሻያ በባቺንግ-ብቻ tweak)።- የከርነል ስብስብ አሁን እያንዳንዱን የመግቢያ ነጥብ፣ በ`fastpq.metallib` ውስጥ የተተገበረውን የክር ቡድን/የጣሪያ ገደቦችን እና ሜታልሊብ በእጅ የማጠናቀር የመራቢያ ደረጃዎችን የሚመዘግብ ልዩ ማጣቀሻ (`docs/source/fastpq_metal_kernels.md`) አለው።
  - የቤንችማርክ ሪፖርቱ አሁን ምን ያህል FFT/IFFT/LDE ባች በተዘጋጀው የድህረ ንጣፍ ከርነል ውስጥ እንደሮጡ የሚመዘግብ የ`post_tile_dispatches` ነገርን ያወጣል (በየአይነት መላኪያ ብዛት እና የመድረክ/ሎግ₂ ድንበሮች)። `scripts/fastpq/wrap_benchmark.py` ብሎኩን ወደ `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary` ይገልብጣል፣ እና የገለፃው በር ማስረጃውን የሚተው የጂፒዩ ምስሎችን አይቀበልም ስለዚህ እያንዳንዱ ባለ 20k ረድፍ የባለብዙ ማለፊያ ከርነል መሄዱን ያረጋግጣል። በመሳሪያ ላይ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【ስክሪፕቶች/ fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - ለመሳሪያዎች/የብረት ዱካ ትስስሮች በአንድ-መላክ የማረም ምዝግብ ማስታወሻዎችን ለመልቀቅ `FASTPQ_METAL_TRACE=1` ያዘጋጁ (የቧንቧ መለያ፣ የክር ቡድን ስፋት፣ የማስጀመሪያ ቡድኖች፣ ያለፈ ጊዜ)።【crates/fastpq_prover/src/metal.rs:346】
- የመላኪያ ወረፋው አሁን በመሳሪያ ተዘጋጅቷል፡- `FASTPQ_METAL_MAX_IN_FLIGHT` caps concurrent Metal Command buffers (በራስ ነባሪ በ `system_profiler` በኩል ከተገኘው የጂፒዩ ኮር ቆጠራ የተገኘ፣ ቢያንስ ከወረፋው ደጋፊ መውጫ ወለል ጋር ተጣብቆ አስተናጋጅ-ትይዩው መሳሪያው ማክሮን ሪፖርት ለማድረግ ፈቃደኛ ካልሆነ)። አግዳሚ ወንበሩ ወረፋ ጥልቀት ያለው ናሙና ማድረግ ያስችላል ስለዚህ ወደ ውጭ የተላከው JSON `metal_dispatch_queue` ነገርን ከ `limit`፣ `dispatch_count`፣ `max_in_flight`፣ `busy_ms`፣000s ማስረጃ ጋር ይጨምራል። አንድ ጎጆ `metal_dispatch_queue.poseidon` ብሎክ በPoseidon ብቻ ቀረጻ (`--operation poseidon_hash_columns`) በሮጠ እና `metal_heuristics` ብሎክ ያመነጫል ፣የተወሰነውን የትዕዛዝ ቋት ወሰን እና የ FFT/LDE ባች አምዶችን (እሴቶቹን መሻርን ጨምሮ) የኦዲት አምዶችን ማስገደድ ይቻል እንደሆነ የሚገልጽ `--operation poseidon_hash_columns` ቴሌሜትሪ. Poseidon kernels እንዲሁ ባይት/ክር፣ መኖርያ እና መላኪያ ጂኦሜትሪ በቅርሶች ላይ ክትትል እንዲደረግበት የተወሰነ `poseidon_profiles` ብሎክን ከከርነል ናሙናዎች ይመግባል። ዋናው ሩጫ የወረፋውን ጥልቀት ወይም የኤልዲኢ ዜሮ ሙሌት ስታቲስቲክስን መሰብሰብ ካልቻለ (ለምሳሌ የጂፒዩ መላክ በፀጥታ ወደ ሲፒዩ ሲወድቅ) ማሰሪያው የጎደለውን ቴሌሜትሪ ለመሰብሰብ አንድ ነጠላ የመመርመሪያ መላክን በራስ-ሰር ያቃጥላል እና አሁን ጂፒዩ እነሱን ሪፖርት ለማድረግ ፈቃደኛ በማይሆንበት ጊዜ የዜሮ መሙላት ጊዜዎችን ያዋህዳል ፣ ስለሆነም የታተሙ ማስረጃዎች ሁል ጊዜ I100006NIን ያጠቃልላል። block.【crates/ fastpq_prover/src/metal.rs:2056 _prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - ያለ ብረት የመሳሪያ ሰንሰለት ሲሻገር `FASTPQ_SKIP_GPU_BUILD=1` አዘጋጅ; ማስጠንቀቂያው መዝለልን ይመዘግባል እና እቅድ አውጪው በሲፒዩ መንገድ ላይ ይቀጥላል።- የአሂድ ጊዜ ማወቂያ የብረት ድጋፍን ለማረጋገጥ `system_profiler` ይጠቀማል; ማዕቀፉ፣ መሳሪያው ወይም ሜታልሊብ ከጠፋ የግንባታው ስክሪፕት `FASTPQ_METAL_LIB` ያጸዳል እና እቅድ አውጪው በሚወስነው ሲፒዩ ላይ ይቆያል። መንገድ።【crates/ fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src4
  - የኦፕሬተር ማረጋገጫ ዝርዝር (የብረት አስተናጋጆች)
    1. የመሳሪያ ሰንሰለቱ መገኘቱን እና `FASTPQ_METAL_LIB` ነጥቦችን በተጠናቀረ `.metallib` (`echo $FASTPQ_METAL_LIB` ከ `cargo build --features fastpq-gpu` በኋላ ባዶ መሆን የለበትም)።【crates/fastpq_prors:/build.】
    2. የነቁ የጂፒዩ መስመሮችን በመጠቀም የተመጣጣኝነት ሙከራዎችን ያሂዱ፡ `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`። ይህ የብረታ ብረት ፍሬዎችን ይለማመዳል እና ማወቂያው ካልተሳካ በራስ-ሰር ወደ ኋላ ይመለሳል።
    3. ለዳሽቦርዶች የቤንችማርክ ናሙና ያንሱ፡ የተሰባሰበውን የብረታ ብረት ላይብረሪ ያግኙ
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`)፣ በ በኩል ይላኩት
       `FASTPQ_METAL_LIB`፣ እና አሂድ
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       ቀኖናዊው `fastpq-lane-balanced` አሁን እያንዳንዱን ምስል ወደ 32,768 ረድፎች ያዘጋጃል፣ ስለዚህ
       JSON ሁለቱንም የተጠየቁ 20k ረድፎችን እና ጂፒዩውን የሚነዳውን የታሸገ ጎራ ያንፀባርቃል
       አስኳሎች. JSON/ሎግ ወደ የማስረጃ ማከማቻዎ ይስቀሉ፤ የምሽት የማክሮስ የስራ ፍሰት መስተዋቶች
      ይህ አሂድ እና ለማጣቀሻ ቅርሶችን በማህደር ያስቀምጣል። ዘገባው ዘግቧል
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` ከእያንዳንዱ ቀዶ ጥገና `speedup` ጋር ፣
     የ LDE ክፍል `zero_fill.{bytes,ms,queue_delta}` ያክላል ስለዚህ ቅርሶችን መልቀቅ ቆራጥነትን ያረጋግጣሉ፣
     ዜሮ-ሙላ ከአናት አስተናጋጅ፣ እና ተጨማሪ የጂፒዩ ወረፋ አጠቃቀም (ገደብ፣ የመላኪያ ብዛት፣
     በበረራ ውስጥ ከፍተኛ፣ ስራ የበዛበት/የተደራራቢ ጊዜ) እና አዲሱ `kernel_profiles` ብሎክ በከርነል ይይዛል።
     ዳሽቦርዶች ጂፒዩ እንዲጠቁሙ የነዋሪነት ሬሾዎች፣ የተገመተው የመተላለፊያ ይዘት እና የቆይታ ጊዜ ክልሎች
       ጥሬ ናሙናዎችን እንደገና ሳይሰሩ መመለሻዎች።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       የብረት LDE መንገድ በ950ms (`<1 s` ኢላማ በአፕል ኤም-ተከታታይ ሃርድዌር ላይ) እንዲቆይ ይጠብቁ።
4. ዳሽቦርዶች የማስተላለፊያ መግብርን መቅረጽ እንዲችሉ የረድፍ አጠቃቀም ቴሌሜትሪ ከእውነተኛ ExecWitness ያንሱ
   ጉዲፈቻ. ምስክር ከTorii አምጡ
  (`iroha_cli audit witness --binary --out exec.witness`) እና በ መፍታት
  `iroha_cli audit witness --decode exec.witness` (በአማራጭ ያክሉ
  `--fastpq-parameter fastpq-lane-balanced` የሚጠበቀውን መለኪያ ስብስብ ለማረጋገጥ; FASTPQ ስብስቦች
  በነባሪ መልቀቅ; ውጤቱን መቁረጥ ካስፈለገዎት ብቻ `--no-fastpq-batches` ማለፍ።
   እያንዳንዱ ባች ግቤት አሁን `row_usage` ነገር (`total_rows`፣ `transfer_rows`፣
   `non_transfer_rows`፣ በመራጭ ቆጠራዎች እና `transfer_ratio`)። ያንን JSON ቅንጭብጭብ በማህደር ያስቀምጡ
   ጥሬ ቅጂዎችን እንደገና በማዘጋጀት ላይ።【crates/iroha_cli/src/audit.rs:209】 አዲሱን ቀረጻ ከ ጋር ያወዳድሩ።
   የቀደመው መነሻ ከ `scripts/fastpq/check_row_usage.py` ጋር ስለዚህ CI ከዝውውር ሬሾዎች ወይም
   ጠቅላላ ረድፎች ወደ ኋላ መመለስ

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```ለጭስ ምርመራዎች ናሙና JSON blobs በ `scripts/fastpq/examples/` ውስጥ ይኖራሉ። በአካባቢው `make check-fastpq-row-usage` ማሄድ ይችላሉ።
   (`ci/check_fastpq_row_usage.sh` ይጠቀለላል)፣ እና CI ቁርጠኛውን ለማነፃፀር በ`.github/workflows/fastpq-row-usage.yml` በኩል ተመሳሳይ ስክሪፕት ይሰራል።
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` ቅጽበታዊ ገጽ እይታዎች ስለዚህ የማስረጃው ጥቅል በማንኛውም ጊዜ በፍጥነት አይሳካም።
   የዝውውር ረድፎች ወደ ላይ ሾልከው ይመጣሉ። በማሽን የሚነበብ ልዩነት (CI job uploads `fastpq_row_usage_summary.json`) ከፈለጉ `--summary-out <path>` ይለፉ።
   አንድ ExecWitness ጠቃሚ ካልሆነ፣ የተሃድሶ ናሙናን ከ`fastpq_row_bench` ጋር ያዋህዱ።
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)፣ እሱም ትክክለኛውን `row_usage` ያወጣል።
   የሚዋቀር መራጭ ቆጠራ ነገር (ለምሳሌ፡ የ65536 ረድፍ የጭንቀት ሙከራ)

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```ደረጃ 7-3 መልቀቅ እሽጎች `scripts/fastpq/validate_row_usage_snapshot.py` ማለፍ አለባቸው፣ ይህም
   እያንዳንዱ የ`row_usage` ግቤት የመራጭ ቆጠራዎችን እና ያንን ይይዛል
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` ረዳቱን ይደውላል
   የጂፒዩ መስመሮች ከመስጠታቸው በፊት የጎደሉ ቅርቅቦች ወዲያውኑ ይከሽፋሉ።【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       የቤንች ማኒፌክት በር ይህንን በ`--max-operation-ms lde=950` ያስፈጽማል፣ ስለዚህ አድስ
       ማስረጃዎ ከዚያ ገደብ ባለፈ ቁጥር ይያዙ።
      እንዲሁም የመሣሪያዎች ማስረጃ ሲፈልጉ፣ መታጠቂያውን እንዲይዝ `--trace-dir <dir>` ይለፉ
      በ `xcrun xctrace record` (ነባሪ "የብረት ስርዓት መከታተያ" አብነት) እና እንደገና ይጀምራል
      በጊዜ ማህተም የተደረገ `.trace` ፋይል ከJSON ጋር ያከማቻል; አሁንም ቦታውን መሻር ይችላሉ /
      አብነት በእጅ ከ `--trace-output <path>` እና ከአማራጭ `--trace-template` / ጋር
      `--trace-seconds`. የተገኘው JSON `metal_trace_{template,seconds,output}` እንዲሁ ያስተዋውቃል
      artefact ጥቅሎች ሁልጊዜ የተያዙትን ዱካዎች ይለያሉ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      እያንዳንዱን ቀረጻ በመጠቅለል
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (የፊርማ መታወቂያን መሰካት ከፈለጉ `--gpg-key <fingerprint>` ይጨምሩ) ስለዚህ ጥቅሉ አልተሳካም
       በፍጥነት በማንኛውም ጊዜ የጂፒዩ LDE አማካይ የ950 ሚ.ኤስ ኢላማውን በጣሰ ጊዜ ፖሲዶን ከ1ሰ በላይ ወይም
       የፖሲዶን ቴሌሜትሪ ብሎኮች ጠፍተዋል፣ `row_usage_snapshot` ን ያካትታል
      ከJSON ቀጥሎ የPoseidon ማይክሮቤንች ማጠቃለያን በ`benchmarks.poseidon_microbench`፣
      እና አሁንም ለ runbooks እና ለGrafana ዳሽቦርድ ሜታዳታ ይይዛል
    (`dashboards/grafana/fastpq_acceleration.json`)። JSON አሁን `speedup.ratio` / ይለቃል
     `speedup.delta_ms` በአንድ ኦፕሬሽን ስለዚህ የተለቀቀው ማስረጃ ከጂፒዩ ጋር ሊወዳደር ይችላል።
     ሲፒዩ ጥሬውን ናሙናዎች እንደገና ሳይሠራበት ያገኛል፣ እና መጠቅለያው ሁለቱንም ይገለበጣል
     የዜሮ ሙላ ስታቲስቲክስ (ከ`queue_delta` በተጨማሪ) ወደ `zero_fill_hotspots` (ባይት ፣ መዘግየት ፣ የተገኘ)
     ጂቢ/ሰ))፣ በ`metadata.metal_trace` ስር ያሉትን የመሣሪያዎች ሜታዳታ ይመዘግባል፣ አማራጭውን ይከታል
     `metadata.row_usage_snapshot` ብሎክ `--row-usage <decoded witness>` ሲቀርብ እና ጠፍጣፋ
     የከርነል ቆጣሪዎች ወደ `benchmarks.kernel_summary` ስለዚህ ማነቆዎችን ይሸፍናሉ ፣ የብረት ወረፋ
     አጠቃቀም፣ የከርነል መኖር እና የመተላለፊያ ይዘት መመለሻዎች ያለ በጨረፍታ ይታያሉ
     ጥሬውን ስፔሉንግ ሪፖርት ያድርጉ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【ስክሪፕቶች/fastpq/wrap_benchmark .py:1】【አርቲፊክስ/ፈጣን ፒክ_ቤንችማርኮች/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     የረድፍ አጠቃቀሙ ቅጽበታዊ ገጽ እይታ አሁን ከታሸገው አርቲፊክስ ጋር ስለሚጓጓዝ፣ ትኬቶችን በቀላሉ ያውጡ
     ሁለተኛ JSON ቅንጣቢ ከማያያዝ ይልቅ ጥቅሉን ያጣቅሱ እና CI የተከተተውን ሊለያይ ይችላል።
    የStage7 ማቅረቢያዎችን ሲያረጋግጥ በቀጥታ ይቆጥራል። የማይክሮቤንች መረጃን በራሱ ለማስቀመጥ፣
    `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json` አሂድ
    እና የተገኘውን ፋይል በ `benchmarks/poseidon/` ስር ያከማቹ። የተዋሃደውን አንጸባራቂ ትኩስ አድርገው ያቆዩት።
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    ስለዚህ ዳሽቦርዶች/CI እያንዳንዱን ፋይል በእጅ ሳይራመዱ ሙሉውን ታሪክ ሊለያይ ይችላል።4. `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (Prometheus የመጨረሻ ነጥብ) በመጠቅለል ወይም `telemetry::fastpq.execution_mode` ሎግዎችን በመፈለግ ቴሌሜትሪ ያረጋግጡ; ያልተጠበቁ የ`resolved="cpu"` ግቤቶች የጂፒዩ ሃሳብ ቢኖርም አስተናጋጁ ወደ ኋላ መውረዱን ያመለክታሉ።【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. በጥገና ወቅት ሲፒዩ እንዲፈጽም ለማስገደድ `FASTPQ_GPU=cpu` (ወይም የማዋቀሪያ ቁልፍ) ይጠቀሙ እና የድጋፍ ምዝግብ ማስታወሻዎች አሁንም መኖራቸውን ያረጋግጡ። ይህ የSRE runbooksን ከመወሰኛ መንገድ ጋር እንዲጣጣሙ ያደርጋቸዋል።【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- ቴሌሜትሪ እና ውድቀት;
  - የማስፈጸሚያ-ሞድ ምዝግብ ማስታወሻዎች (`telemetry::fastpq.execution_mode`) እና ቆጣሪዎች (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) የተጠየቀውን እና የተፈታ ሁነታን ያጋልጣሉ ስለዚህ ጸጥ ያለ መውደቅ በ ውስጥ ይታያል። ዳሽቦርዶች።【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - የ`FASTPQ Acceleration Overview` Grafana ሰሌዳ (`dashboards/grafana/fastpq_acceleration.json`) የብረታ ብረት ጉዲፈቻ መጠንን በዓይነ ሕሊናህ በመመልከት ከቤንችማርክ ቅርሶች ጋር ያገናኛል፣ የተጣመሩ የማንቂያ ሕጎች (`dashboards/alerts/fastpq_acceleration_rules.yml`) ቀጣይነት ባለው የቁልቁለት ጉዞ ላይ።
  - `FASTPQ_GPU={auto,cpu,gpu}` መሻር ይደገፋል; ያልታወቁ እሴቶች ማስጠንቀቂያዎችን ያሳድጋሉ ነገር ግን አሁንም ለኦዲት ወደ ቴሌሜትሪ ይሰራጫሉ።【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - የጂፒዩ እኩልነት ፈተናዎች (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) ለ CUDA እና Metal ማለፍ አለባቸው; ሜታሊብ በማይኖርበት ጊዜ ወይም ማግኘቱ ሲቀር CI በጸጋ ይዘላል።
  - የብረታ ብረት ዝግጁነት ማስረጃ (የፍኖተ ካርታው ኦዲት ቆራጥነትን፣ የቴሌሜትሪ ሽፋንን እና የመውደቅ ባህሪን ማረጋገጥ እንዲችል ከእያንዳንዱ መልቀቅ ጋር ከታች ያሉትን ቅርሶች በማህደር ያስቀምጡ)።| ደረጃ | ግብ | ትዕዛዝ / ማስረጃ |
    | ---- | ---- | ------------------ |
    | metallib ይገንቡ | `xcrun metal`/`xcrun metallib` መገኘቱን ያረጋግጡ እና ለዚህ ቁርጠኝነት የሚወስነውን `.metallib` መልቀቅ | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; ወደ ውጪ ላክ `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | አረጋግጥ env var | በግንባታ ስክሪፕት የተቀዳውን env var በመፈተሽ የብረታ መቆየቱን ያረጋግጡ | `echo $FASTPQ_METAL_LIB` (ፍፁም መንገድ መመለስ አለበት፤ ባዶ ማለት የጀርባው አካል ተሰናክሏል)።
    | የጂፒዩ ተመሳሳይነት ስብስብ | ከመርከብዎ በፊት ከርነሎች መፈጸማቸውን ያረጋግጡ (ወይም ወሳኙን የመቀነስ ምዝግብ ማስታወሻዎችን ያመነጫሉ) | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` እና ውጤቱን ያከማቹ `backend="metal"` ወይም የመውደቅ ማስጠንቀቂያ።
    | የቤንችማርክ ናሙና | ዳሽቦርዶች የፍጥነት ማስረጃዎችን ወደ ውስጥ ማስገባት እንዲችሉ `speedup.*` እና FFT መስተካከልን የሚመዘግብ JSON/log ጥንድ ያንሱ | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; JSONን በማህደር ያስቀምጥ፣የጊዜ ማህተሙ `.trace` እና stdout ከመልቀቂያ ማስታወሻዎች ጋር በመሆን የGrafana ቦርድ የብረታ ብረት ሩጫውን ያነሳል (ሪፖርቱ የተጠየቁትን 20k ረድፎች እና የታሸጉ 32,768-ረድፎች ጎራውን ይመዘግባል ስለዚህ ገምጋሚዎች I180000401 ኢላማ)።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | ሪፖርት ጠቅልሎ ይፈርሙ | የጂፒዩ ኤልዲኢ ማለት 950ሚሴ ከጣሰ፣ Poseidon ከ1ሰ በላይ ከሆነ ወይም የፖሲዶን ቴሌሜትሪ ብሎኮች ጠፍተው ከሆነ እና የተፈረመ የእደ-ጥበብ ቅርቅብ ካዘጋጁ ልቀቱን ከሽፏል | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; ሁለቱንም ተጠቅልሎ JSON እና የመነጨውን `.json.asc` ፊርማ በመርከብ ኦዲተሮች የስራ ጫናውን ሳይደግሙ ንዑስ ሰከንድ መለኪያዎችን ማረጋገጥ ይችላሉ።【scripts/ fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark2【py:
    | የተፈረመ የቤንች መግለጫ | የ`<1 s` LDE ማስረጃን በብረታ ብረት/CUDA ቅርቅቦች ላይ ያስፈጽሙ እና ለመልቀቅ ፍቃድ የተፈረሙ ምግቦችን ይያዙ | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; የታችኛው ተፋሰስ አውቶማቲክ የንዑስ ሰከንድ የማረጋገጫ መለኪያዎችን ማረጋገጥ እንዲችል የማኒፌክት + ፊርማውን ከመልቀቂያ ትኬቱ ጋር አያይዘው ።
| CUDA ጥቅል | የSM80 CUDA ቀረጻን ከብረት ማስረጃ ጋር በደረጃ ያቆዩት ስለዚህ መግለጫዎች ሁለቱንም የጂፒዩ ክፍሎች ይሸፍናሉ። | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` በ Xeon + RTX አስተናጋጅ → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; የተጠቀለለውን መንገድ ወደ `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` ጨምር፣ `.json`/`.asc` ጥንድ ከብረት እሽግ አጠገብ አቆይ እና ኦዲተሮች ማጣቀሻ ሲፈልጉ የዘሩትን `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` ጥቀስ። አቀማመጥ።【ስክሪፕቶች/fastpq/wrap_benchmark.py:714】【አርቲፊክስ/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| ቴሌሜትሪ ቼክ | Prometheus/OTEL ንጣፎች `device_class="<matrix>", backend="metal"` ያንፀባርቃሉ (ወይንም የወረደውን ይመዝገቡ) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` እና ጅምር ላይ የሚወጣውን የ`telemetry::fastpq.execution_mode` መዝገብ ይቅዱ።【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| የግዳጅ ውድቀት ልምምድ | ለSRE የመጫወቻ መጽሐፍት ወሳኙን የሲፒዩ መንገድ ይመዝግቡ | ከ `FASTPQ_GPU=cpu` ወይም `zk.fastpq.execution_mode = "cpu"` ጋር አጭር የስራ ጫና ያሂዱ እና ኦፕሬተሮች የመመለሻ ሂደቱን እንዲለማመዱ የወረደውን ምዝግብ ይያዙ።【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src5【
    | መከታተያ ቀረጻ (አማራጭ) | መገለጫ በሚሰጡበት ጊዜ የከርነል መስመር/የጣሪያ መሻገሪያዎች በኋላ ሊገመገሙ የሚችሉ የላኪ ዱካዎችን ይያዙ | አንድ እኩልነት ፈተናን በ`FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` እንደገና ያሂዱ እና የተሰራውን የመከታተያ መዝገብ ከተለቀቁት ቅርሶችዎ ጋር ያያይዙ።【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    ማስረጃውን በመልቀቂያ ቲኬቱ በማህደር ያስቀምጡ እና በ`docs/source/fastpq_migration_guide.md` ውስጥ ያለውን ተመሳሳይ የፍተሻ ዝርዝር ያንጸባርቁ ስለዚህ የማዘጋጀት/የምርት ልቀቶች አንድ አይነት የመጫወቻ መጽሐፍ ይከተላሉ።【docs/source/fastpq_migration_guide.md:1】

### የመልቀቂያ ዝርዝር ማስፈጸሚያ

በእያንዳንዱ የFASTPQ የመልቀቂያ ትኬት ላይ የሚከተሉትን በሮች ያክሉ። ሁሉም ንጥሎች እስኪሆኑ ድረስ ልቀቶች ታግደዋል
የተሟሉ እና የተፈረሙ ቅርሶች ተያይዘዋል.

1. ** የንዑስ-ሁለተኛ ማረጋገጫ መለኪያዎች *** - ቀኖናዊው የብረት ማመሳከሪያ ቀረጻ
   (`fastpq_metal_bench_*.json`) ባለ 20000-ረድፍ የስራ ጫና (32768 የታሸጉ ረድፎች) መጠናቀቁን ማረጋገጥ አለበት
   <1ሰ. በትክክል፣ `benchmarks.operations` ግቤት `operation = "lde"` እና ተዛማጅ
   `report.operations` ናሙና `gpu_mean_ms ≤ 950` ማሳየት አለበት። ከጣሪያው በላይ የሆኑ ሩጫዎች ያስፈልጋሉ።
   የማረጋገጫ ዝርዝሩ ከመፈረሙ በፊት ምርመራ እና እንደገና መያዝ.
2. **የተፈረመ የቤንችማርክ መግለጫ** - ትኩስ የብረት + CUDA ቅርቅቦችን ከቀዳ በኋላ ያሂዱ
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` ለመልቀቅ
   `artifacts/fastpq_bench_manifest.json` እና የተነጠለ ፊርማ
   (`artifacts/fastpq_bench_manifest.sig`)። ሁለቱንም ፋይሎች እና የአደባባይ ቁልፍ አሻራ ከ. ጋር ያያይዙ
   ገምጋሚዎች መፈጫውን እና ፊርማውን በግል ማረጋገጥ እንዲችሉ ትኬት የመልቀቅ ትኬት።【xtask/src/fastpq.rs:1】
3. **የማስረጃ ማያያዣዎች** — ጥሬ ቤንችማርክ JSON፣ stdout log (ወይም Instruments trace፣ መቼ ነው) ያከማቹ።
   ተይዟል) እና አንጸባራቂው/ፊርማው ከመልቀቂያ ትኬት ጋር ተጣምሯል። የማረጋገጫ ዝርዝሩ ብቻ ነው።
   ትኬቱ ከእነዚያ ቅርሶች ጋር ሲገናኝ እና የጥሪ ገምጋሚው ሲያረጋግጥ እንደ አረንጓዴ ይቆጠራል
   በ`fastpq_bench_manifest.json` ውስጥ የተመዘገበው መፍጨት ከተሰቀሉት ፋይሎች ጋር ይዛመዳል።【አርቲፊክስ/fastpq_benchmarks/README.md:1】

## ደረጃ 6 - ማጠንከሪያ እና ሰነዶች
- የቦታ ያዥ ጀርባ ጡረታ ወጥቷል; የማምረቻ ቧንቧ መስመር በነባሪነት ያለ ምንም ባህሪ መቀያየር።
- ሊባዙ የሚችሉ ግንባታዎች (የፒን የመሳሪያ ሰንሰለት ፣ የእቃ መያዣ ምስሎች)።
- ለትራክ ፣ ኤስኤምቲ ፣ የመፈለጊያ መዋቅሮች ፊውዘር።
- የፕሮቨር-ደረጃ የጭስ ሙከራዎች የአስተዳደር የድምጽ መስጫ ድጋፎችን እና የገንዘብ ልውውጦችን ይሸፍናሉ ።
- የሩጫ መጽሐፍት የማንቂያ ገደቦች ፣ የማሻሻያ ሂደቶች ፣ የአቅም እቅድ መመሪያዎች።
- የአርክቴክቸር ማረጋገጫ ድጋሚ አጫውት (x86_64፣ ARM64) በCI.

### የቤንች መግለጫ እና የመልቀቂያ በር

የተለቀቀው ማስረጃ አሁን ሁለቱንም ብረት እና ሁለቱንም የሚሸፍን የመወሰን መግለጫን ያካትታል
CUDA የቤንችማርክ ቅርቅቦች። አሂድ፡

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```ትዕዛዙ የታሸጉትን እሽጎች ያረጋግጣል፣ የቆይታ/የፍጥነት ገደቦችን ያስፈጽማል፣
BLAKE3 + SHA-256 መፈጨትን ያመነጫል፣ እና (በአማራጭ) አንጸባራቂውን በ
የ Ed25519 ቁልፍ ስለዚህ የመልቀቂያ መሳሪያዎች ፕሮቬንሽን ማረጋገጥ ይችላል። ተመልከት
`xtask/src/fastpq.rs`/`xtask/src/main.rs` ለትግበራ እና
`artifacts/fastpq_benchmarks/README.md` ለተግባራዊ መመሪያ።

> **ማስታወሻ፡** `benchmarks.poseidon_microbench`ን የሚያስቀሩ የብረት ጥቅሎች አሁን መንስኤ ናቸው።
> አንጸባራቂው ትውልድ ይወድቃል። `scripts/fastpq/wrap_benchmark.py` እንደገና ያሂዱ
> (እና `scripts/fastpq/export_poseidon_microbench.py` ብቻውን ካስፈለገዎት
> ማጠቃለያ) የPoseidon ማስረጃ በማይኖርበት ጊዜ ሁሉ መለቀቅ ይታያል
> ሁልጊዜ scalar-vs-default ንፅፅርን ያንሱ።【xtask/src/fastpq.rs:409】

የ`--matrix` ባንዲራ (የ `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` ነባሪ
በሚኖርበት ጊዜ) የተያዙትን የመሣሪያ-አቋራጭ ሚዲያዎችን ይጭናል
`scripts/fastpq/capture_matrix.sh`. አንጸባራቂው ባለ 20000-ረድፍ ወለል እና
ለእያንዳንዱ የመሣሪያ ክፍል በእያንዳንዱ-ኦፕሬሽን መዘግየት/የፍጥነት ገደቦች
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` መሻር የለም
የተወሰነ መመለሻን ካላረሙ በስተቀር ረዘም ያለ ጊዜ ያስፈልጋል።

የታሸጉ የቤንችማርክ ዱካዎችን ወደ የ በማያያዝ ማትሪክስ ያድሱ
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` ዝርዝሮች እና ሩጫ
`scripts/fastpq/capture_matrix.sh`. ስክሪፕቱ በእያንዳንዱ መሣሪያ ሚዲያን ቅጽበተ-ፎቶዎችን ያሳያል፣
የተዋሃደውን `matrix_manifest.json` ያወጣል እና አንጻራዊውን መንገድ ያትማል።
`cargo xtask fastpq-bench-manifest` ይበላል. AppleM4፣ Xeon+RTX፣ እና
Neoverse+MI300 ቀረጻ ዝርዝሮች (`devices/apple-m4-metal.txt`፣
`devices/xeon-rtx-sm80.txt`፣ `devices/neoverse-mi300.txt`) በተጨማሪም ተጠቅልሎባቸዋል።
የቤንችማርክ ቅርቅቦች
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`፣
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`፣
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) አሁን ተረጋግጠዋል
ውስጥ፣ ስለዚህ እያንዳንዱ ልቀት ከማንፀባረቂያው በፊት አንድ አይነት መሳሪያ ተሻጋሪ ሚዲያዎችን ያስገድዳል
ነው። የተፈረመ።【አርቲፊክቶች/fastpq_benchmarks/matrix/መሳሪያዎች/አፕል-m4-ሜታል.txt:1】【አርቲፊክስ/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1】【አርቲፊክስ/ fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【አርቲፊክስ/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【አርቲፊክስ/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【አርቲፊክስ/ፈጣን pq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## የትችት ማጠቃለያ እና ክፍት ድርጊቶች

## ደረጃ 7 — ፍሊት ጉዲፈቻ እና ልቀት ማስረጃ

ደረጃ 7 ማረጋገጫውን ከ"ሰነድ እና ቤንችማርክድ" (ደረጃ 6) ይወስዳል
"ነባሪ-ለምርት መርከቦች ዝግጁ" ትኩረቱ በቴሌሜትሪ ወደ ውስጥ ማስገባት ነው.
የመሣሪያ ተሻጋሪ ቀረጻ እኩልነት፣ እና የኦፕሬተር ማስረጃ ቅርቅቦች የጂፒዩ ማጣደፍ
በቁርጠኝነት ሊታዘዝ ይችላል።- ** ደረጃ 7-1 — ፍሊት ቴሌሜትሪ ማስገቢያ እና SLOs።** የምርት ዳሽቦርዶች
  (`dashboards/grafana/fastpq_acceleration.json`) ለመኖር ሽቦ መደረግ አለበት።
  Prometheus/OTel ከአለርትማኔጀር ሽፋን ጋር ለወረፋ ጥልቀት ድንኳኖች ይመገባል።
  ዜሮ-ሙላ regressions, እና ጸጥታ የሲፒዩ ውድቀት. የማንቂያ ማሸጊያው ስር ይቆያል
  `dashboards/alerts/fastpq_acceleration_rules.yml` እና ተመሳሳይ ማስረጃዎችን ይመገባል።
  ጥቅል በደረጃ6 ያስፈልጋል።【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  ዳሽቦርዱ አሁን ለ`device_class`፣ `chip_family`፣ የአብነት ተለዋዋጮችን ያጋልጣል።
  እና `gpu_kind` ኦፕሬተሮች የብረታ ብረት ጉዲፈቻን በትክክለኛው ማትሪክስ እንዲመሩ ያስችላቸዋል።
  መለያ (ለምሳሌ፣ `apple-m4-max`)፣ በአፕል ቺፕ ቤተሰብ፣ ወይም በ discrete vs.
  መጠይቆችን ሳያርትዑ የተቀናጁ የጂፒዩ ክፍሎች።
  በ`irohad --features fastpq-gpu` የተገነቡ የማክሮስ ኖዶች አሁን ይለቃሉ
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`፣
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (የተጠመዱ/የተደራረቡ ሬሾዎች)፣ እና
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (ገደብ፣ ከፍተኛ_በረራ፣ መላኪያ_ቁጥር፣ መስኮት_ሰከንዶች) ስለዚህ ዳሽቦርዶች እና
  የአለርትማኔጀር ህጎች የብረታ ብረት ሴማፎር ተረኛ-ዑደት/ዋና ክፍል በቀጥታ ከ ማንበብ ይችላሉ።
  የቤንችማርክ ጥቅል ሳይጠብቅ Prometheus። አስተናጋጆች አሁን ወደ ውጭ ይላካሉ
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` እና
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` በማንኛውም ጊዜ
  የኤልዲኢ አጋዥ ዜሮ የጂፒዩ ግምገማ ማቋቋሚያ፣ እና Alertmanager ያገኘውን ነው።
  `FastpqQueueHeadroomLow` (ዋና ክፍል  0.40ሚሴ ከ 15 ሜትር በላይ) ደንቦች ስለዚህ የጭንቅላት ክፍል ወረፋ እና
  ዜሮ-ሙላ regressions ገጽ ኦፕሬተሮች ወዲያውኑ ይልቅ መጠበቅ
  ቀጣዩ የታሸገ ቤንችማርክ። አዲስ `FastpqCpuFallbackBurst` የገጽ ደረጃ ማንቂያ ትራኮች
  ጂፒዩ ከ5% በላይ የስራ ጫና በሲፒዩ ጀርባ ላይ እንዲያርፍ ይጠይቃል፣
  ኦፕሬተሮች ማስረጃዎችን እንዲይዙ ማስገደድ እና የስር-ምክንያት ጊዜያዊ የጂፒዩ ውድቀቶችን
  እንደገና ከመሞከርዎ በፊት መልቀቅ።【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【ዳሽቦርዶች/አል erts/fastpq_acceleration_rules.yml:1】【ዳሽቦርዶች/ማንቂያዎች/ፈተናዎች/fastpq_acceleration_rules.test.yml:1】
  የSLO ስብስብ አሁን ደግሞ የ≥50% የብረት ግዴታ-ዑደት ኢላማውን በ
  `FastpqQueueDutyCycleDrop` ደንብ፣ የትኛው አማካኝ ነው።
  `fastpq_metal_queue_ratio{metric="busy"}` በሚጠቀለል የ15 ደቂቃ መስኮት እና
  የጂፒዩ ስራ አሁንም በታቀደበት ጊዜ ሁሉ ያስጠነቅቃል ነገር ግን ወረፋውን ማቆየት አልቻለም
  የሚፈለግ የመኖሪያ ቦታ. ይህ የቀጥታ የቴሌሜትሪ ውል ከ ጋር እንዲጣጣም ያደርገዋል
  የጂፒዩ መስመሮች ከመስጠታቸው በፊት የማመሳከሪያ ማስረጃ።【dashboards/alerts/fastpq_acceleration_rules.yml:1】【ዳሽቦርዶች/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- ** ደረጃ 7-2 - የመሣሪያ ተሻጋሪ ማትሪክስ።** አዲሱ
  `scripts/fastpq/capture_matrix.sh` ይገነባል።
  `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` ከመሳሪያው
  የቀረጻ ዝርዝሮች በ `artifacts/fastpq_benchmarks/matrix/devices/`። አፕልኤም 4፣
  Xeon+RTX፣ እና Neoverse+MI300 medians አሁን ከነሱ ጋር በሪፖ ውስጥ ይኖራሉ።
  የታሸጉ ጥቅሎች፣ ስለዚህ `cargo xtask fastpq-bench-manifest` አንጸባራቂውን ይጭናል።
  በራስ-ሰር, የ 20000 ረድፎችን ወለል ያስፈጽማል እና በእያንዳንዱ መሳሪያ ይተገበራል
  የመልቀቂያ ቅርቅብ ከመምጣቱ በፊት የቆይታ/የፍጥነት ገደቦች ያለ የCLI ባንዲራዎችጸድቋል።【scripts/fastpq/capture_matrix.sh:1】【አርቲፊክስ/ fastpq_benchmarks/matrix/matrix_manifest.json:1】【አርቲፊክስ/fastpq_benchmarks/matrix/መሳሪያዎች/apple-m4-met al.txt:1】【አርቲፊክስ/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【አርቲፋ cts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
የተዋሃዱ አለመረጋጋት ምክንያቶች አሁን ከማትሪክስ ጎን ለጎን ይላካሉ፡ ማለፊያ
ለመልቀቅ `--reason-summary-out` ወደ `scripts/fastpq/geometry_matrix.py`
የJSON ሂስቶግራም ውድቀት/ማስጠንቀቂያ መንስኤዎች በአስተናጋጅ መለያ እና ምንጭ ተከፍተዋል።
ማጠቃለያ፣ ስለዚህ Stage7-2 ገምጋሚዎች የሲፒዩ ውድቀት ወይም የጠፋ ቴሌሜትሪ በ ላይ ማየት ይችላሉ።
ሙሉውን የ Markdown ሠንጠረዥ ሳይቃኙ በጨረፍታ። አሁን ያው ረዳት
`--host-label chip_family:Chip` ይቀበላል (ለብዙ ቁልፎች ይድገሙት) ስለዚህ የ
Markdown/JSON ውጽዓቶች ከመቅበር ይልቅ የተሰበሰቡ የአስተናጋጅ መለያ አምዶችን ያካትታሉ
ያንን ሜታዳታ በጥሬው ማጠቃለያ፣ የስርዓተ ክወና ግንባታዎችን ለማጣራት ቀላል ያደርገዋል
የመድረክ 7-2 የማስረጃ ጥቅል ሲያጠናቅቅ የብረታ ብረት ነጂ ስሪቶች።【scripts/fastpq/geometry_matrix.py:1】
ጂኦሜትሪ ጠረገ እንዲሁም ISO8601 `started_at`/`completed_at` መስኮችን ማህተም አድርጓል።
ማጠቃለያ፣ CSV እና Markdown ውጽዓቶች ስለዚህ ቅርቅቦች መቅረጽ መስኮቱን ማረጋገጥ ይችላል።
እያንዳንዱ አስተናጋጅ ደረጃ7-2 ማትሪክስ በርካታ የላቦራቶሪዎችን ሲቀላቀል።【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` አሁን የጂኦሜትሪ ማትሪክስ ከ ጋር ይሰፋል
`row_usage/*.json` ቅጽበተ-ፎቶዎች ወደ አንድ የደረጃ7 ጥቅል (`stage7_bundle.json`)
+ `stage7_geometry.md`)፣ የዝውውር ሬሾዎችን በማረጋገጥ
`validate_row_usage_snapshot.py` እና ቀጣይነት ያለው አስተናጋጅ/ኤንቪ/ምክንያት/ምንጭ ማጠቃለያዎች
ስለዚህ የመልቀቅ ትኬቶች ከጃጊንግ ይልቅ አንድ የሚወስን አርቲፊክስን ማያያዝ ይችላሉ።
የአስተናጋጅ ሠንጠረዦች።【ስክሪፕቶች/fastpq/stage7_bundle.py:1】【ስክሪፕቶች/fastpq/validate_row_usage_snapshot.py:1】
- ** ደረጃ 7-3 - የኦፕሬተር ጉዲፈቻ ማስረጃ እና የድጋሚ ልምምዶች *** አዲሱ
  `docs/source/fastpq_rollout_playbook.md` የአርቲፊክ ቅርቅቡን ይገልጻል
  (`fastpq_bench_manifest.json`፣ የታሸገ ብረት/CUDA ቀረጻዎች፣ Grafana ወደ ውጭ መላክ፣
  የማንቂያ አስተዳዳሪ ቅጽበታዊ ገጽ እይታ፣ የጥቅልል ምዝግብ ማስታወሻዎች) ከእያንዳንዱ የታቀደ ትኬት ጋር አብሮ መሆን አለበት።
  በተጨማሪም ደረጃውን የጠበቀ (አብራሪ → ራምፕ → ነባሪ) የጊዜ መስመር እና የግዳጅ ውድቀት ልምምዶች።
  `ci/check_fastpq_rollout.sh` እነዚህን ጥቅሎች ያጸድቃል ስለዚህ CI ደረጃ 7ን ያስፈጽማል
  ከመልቀቃቸው በፊት በር ወደፊት ይንቀሳቀሳሉ. የሚለቀቀው የቧንቧ መስመር አሁን ተመሳሳይ መጎተት ይችላል
  ጥቅል ወደ `artifacts/releases/<version>/fastpq_rollouts/…` በኩል
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, ማረጋገጥ
  የተፈረሙ ሰነዶች እና የታቀዱ ማስረጃዎች አብረው ይቆያሉ። የማጣቀሻ ጥቅል ይኖራል
  ለማቆየት `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/`
  የ GitHub የስራ ፍሰት (`.github/workflows/fastpq-rollout.yml`) አረንጓዴ እውን ሆኖ
  የታቀዱ ማቅረቢያዎች ተገምግመዋል።

### ደረጃ 7 FFT ወረፋ ደጋፊ-ውጭ`crates/fastpq_prover/src/metal.rs` አሁን `QueuePolicy` አፋጣኝ ያደርጋል
አስተናጋጁ ሀ ሪፖርት ባደረገ ቁጥር ብዙ የብረታ ብረት ትዕዛዝ ወረፋዎችን በራስ ሰር ያፈልቃል
discrete ጂፒዩ. የተዋሃዱ ጂፒዩዎች ነጠላ-ወረፋ መንገዱን ይጠብቃሉ።
(`MIN_QUEUE_FANOUT = 1`)፣ ልዩ የሆኑ መሣሪያዎች ነባሪ ለሁለት ወረፋ ብቻ ሲሆኑ
የስራ ጫና ቢያንስ 16 አምዶችን ሲሸፍን ማራገቢያ ያድርጉ። ሁለቱም ሂዩሪስቲክስ ሊስተካከል ይችላል
በአዲሱ `FASTPQ_METAL_QUEUE_FANOUT` እና `FASTPQ_METAL_COLUMN_THRESHOLD` በኩል
የአካባቢ ተለዋዋጮች፣ እና መርሐግብር አውጪው ክብ ሮቢን FFT/LDE ባች በመላ
በተመሳሳዩ ወረፋ ላይ የተጣመረውን የድህረ ንጣፍ መላኪያ ከመውጣቱ በፊት ንቁ ወረፋዎች
የትዕዛዝ ዋስትናዎችን ለመጠበቅ።【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
የመስቀለኛ መንገድ ኦፕሬተሮች ከአሁን በኋላ እነዚያን env vars በእጅ ወደ ውጭ መላክ አያስፈልጋቸውም፡ የ
`iroha_config` መገለጫ `fastpq.metal_queue_fanout` ያጋልጣል እና
`fastpq.metal_queue_column_threshold`፣ እና `irohad` እነሱን ይተገብራቸዋል።
`fastpq_prover::set_metal_queue_policy` የብረታ ብረት ጀርባ እንዲህ ከመጀመሩ በፊት
የፍሊት መገለጫዎች ከስፖክ ማስጀመሪያ መጠቅለያዎች ውጭ መባዛታቸው አይቀርም።【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
የተገላቢጦሽ የኤፍኤፍቲ ስብስቦች አሁን የስራ ጫናው ልክ በሆነ ቁጥር በአንድ ወረፋ ላይ ይጣበቃል
የደጋፊ መውጫ ጣራ (ለምሳሌ፡ ባለ 16-አምድ መስመር-ሚዛናዊ ቀረጻ) ይመታል፣ ይህም
ትልቅ-አምድ FFT/LDE/Poseidon ሲተው ≥1.0× እኩልነትን ለWP2-D ያድሳል
በባለብዙ ወረፋ መንገድ ላይ ይላካል።【crates/fastpq_prover/src/metal.rs:2018】

አጋዥ ሙከራዎች CI እንዲችል የወረፋ-ፖሊሲ ክላምፕስ እና የመተንተን ማረጋገጫን ይጠቀማሉ
በእያንዳንዱ ግንበኛ ላይ የጂፒዩ ሃርድዌር ሳያስፈልግ ደረጃ 7 ሂዩሪስቲክስን ያረጋግጡ፣
እና የጂፒዩ-ተኮር ሙከራዎች የድጋሚ አጫውት ሽፋን እንዲቀጥል የደጋፊ-ውጭ መሻሮችን ያስገድዳሉ
ከአዲሶቹ ነባሪዎች ጋር አስምር።【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### ደረጃ7-1 የመሣሪያ መለያዎች እና ማንቂያ ውል

`scripts/fastpq/wrap_benchmark.py` አሁን `system_profiler` በ macOS ቀረጻ ላይ ይመረምራል።
በእያንዳንዱ የተጠቀለለ ቤንችማርክ ውስጥ የሃርድዌር መለያዎችን ያስተናግዳል እና ይመዘግባል ስለዚህ ፍሊት ቴሌሜትሪ
እና የቀረጻው ማትሪክስ ያለ ተነባቢ የተመን ሉሆች በመሣሪያ መዞር ይችላል። ሀ
20000-ረድፍ ብረት ቀረጻ አሁን እንደዚህ ያሉ ግቤቶችን ይይዛል፡-

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

እነዚህ መለያዎች ከ`benchmarks.zero_fill_hotspots` እና ጋር አብረው ገብተዋል።
`benchmarks.metal_dispatch_queue` ስለዚህ የ Grafana ቅጽበታዊ ገጽ እይታ፣ ማትሪክስ መቅረጽ
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) እና የማስጠንቀቂያ አስተዳዳሪ
ሁሉም ማስረጃዎች መለኪያዎችን ባዘጋጀው የሃርድዌር ክፍል ይስማማሉ። የ
`--label` ባንዲራ አሁንም የላብራቶሪ አስተናጋጅ ሲጎድል በእጅ መሻርን ይፈቅዳል
`system_profiler`፣ ነገር ግን በራስ-የተፈተሸ መለያዎች አሁን አፕልኤም1-ኤም 4ን እና
discrete PCIe GPUs ከሳጥኑ ውጪ።【scripts/fastpq/wrap_benchmark.py:1】

የሊኑክስ ቀረጻዎች ተመሳሳይ ህክምና ያገኛሉ፡ `wrap_benchmark.py` አሁን ይመረምራል።
`/proc/cpuinfo`፣ `nvidia-smi`/`rocm-smi`፣ እና `lspci` ስለዚህ CUDA እና OpenCL ይሰራል
`cpu_model`፣ `gpu_model`፣ እና ቀኖናዊ `device_class` (`xeon-rtx-sm80`
ለStage7 CUDA አስተናጋጅ፣ `neoverse-mi300` ለ MI300A ቤተ ሙከራ)። ኦፕሬተሮች ይችላሉ።
አሁንም በራስ-የተገኙ እሴቶችን ይሽራል፣ ነገር ግን የStage7 ማስረጃ ቅርቅቦች ከአሁን በኋላ
Xeon/Neoverse ቀረጻዎችን በትክክለኛው መሳሪያ ለመሰየም በእጅ አርትዖቶችን ጠይቅ
ሜታዳታበሂደት ጊዜ፣ እያንዳንዱ አስተናጋጅ `fastpq.device_class`፣ `fastpq.chip_family` እና ያዘጋጃል።
`fastpq.gpu_kind` (ወይም ተዛማጅ `FASTPQ_*` አካባቢ ተለዋዋጮች) ወደ
በቀረጻ ጥቅል ውስጥ የሚታዩ ተመሳሳይ ማትሪክስ መለያዎች ስለዚህ Prometheus ወደ ውጭ መላክ
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` እና
የ FASTPQ Acceleration ዳሽቦርድ ከሶስቱ መጥረቢያዎች በማናቸውም ማጣራት ይችላል። የ
የአለርትማኔጀር ደንቦች ከተመሳሳዩ መለያ ስብስብ ጋር ተደምረዋል፣ ይህም ኦፕሬተሮችን እንዲቀርጹ ያስችላቸዋል
በአንድ ሃርድዌር ፕሮፋይል ከአንድ ሰው ይልቅ ጉዲፈቻ፣ ማዋረድ እና ውድቀት
ፍሊት-ሰፊ ውድር

የቴሌሜትሪ SLO/ማንቂያ ውል አሁን የተያዙትን መለኪያዎች ከደረጃ7 ጋር ያገናኛል።
በሮች ። ከዚህ በታች ያለው ሰንጠረዥ ምልክቶችን እና የማስፈጸሚያ ነጥቦችን ያጠቃልላል።

| ሲግናል | ምንጭ | ዒላማ / ቀስቅሴ | ማስፈጸም |
| ------ | ------ | --- | -------- |
| የጂፒዩ ጉዲፈቻ ጥምርታ | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95% የ-(መሣሪያ_ክፍል፣ ቺፕ_ቤተሰብ፣ ጂፑ_አይነት) ጥራቶች `resolved="gpu", backend="metal"` ላይ ማረፍ አለባቸው። ማንኛውም ሶስቴ ከ 50% በታች ከ 15m በላይ ሲወርድ | `FastpqMetalDowngrade` ማንቂያ (ገጽ)【ዳሽቦርዶች/ማንቂያዎች/ fastpq_acceleration_rules.yml:1】 |
| የኋላ ክፍተት | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | ለእያንዳንዱ ሶስት እጥፍ በ 0 መቆየት አለበት; ማንኛውም ቀጣይነት ያለው (>10m) ፍንጥቅ በኋላ አስጠንቅቅ | `FastpqBackendNoneBurst` ማንቂያ (ማስጠንቀቂያ)【ዳሽቦርዶች/ማንቂያዎች/ fastpq_acceleration_rules.yml:21】 |
| የሲፒዩ ውድቀት ውድር | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | ≤5% በጂፒዩ የተጠየቁ ማረጋገጫዎች ለማንኛውም ሶስት እጥፍ በሲፒዩ ጀርባ ላይ ሊያርፉ ይችላሉ። ገጽ ሶስት እጥፍ ከ 5% በላይ በ≥10m | `FastpqCpuFallbackBurst` ማንቂያ (ገጽ)【ዳሽቦርዶች/ማንቂያዎች/ fastpq_acceleration_rules.yml:32】 |
| የብረት ወረፋ ግዴታ ዑደት | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | የጂፒዩ ስራዎች በተሰለፉ ቁጥር የሚሽከረከር 15ሜ አማካኝ ≥50% መቆየት አለበት። የጂፒዩ ጥያቄዎች ሲቀጥሉ አጠቃቀሙ ከዒላማው በታች ሲወድቅ አስጠንቅቅ | `FastpqQueueDutyCycleDrop` ማንቂያ (ማስጠንቀቂያ)【ዳሽቦርዶች/ማንቂያዎች/ fastpq_acceleration_rules.yml:98】 |
| የወረፋ ጥልቀት እና ዜሮ መሙላት በጀት | የታሸገ ቤንችማርክ `metal_dispatch_queue` እና `zero_fill_hotspots` ብሎኮች | `max_in_flight` ቢያንስ አንድ ማስገቢያ በታች `limit` እና LDE ዜሮ-ሙላ አማካኝ ≤0.4ms (≈80GB/s) መቆየት አለበት ቀኖናዊ 20000-ረድፍ መከታተያ; ማንኛውም ማፈግፈግ የታቀደውን ጥቅል ያግዳል | በ`scripts/fastpq/wrap_benchmark.py` ውፅዓት የተገመገመ እና ከStage7 ማስረጃ ጥቅል (`docs/source/fastpq_rollout_playbook.md`) ጋር ተያይዟል። |
| የአሂድ ጊዜ ወረፋ ዋና ክፍል | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` ለእያንዳንዱ ሶስቴ; ያለ headroom 10m በኋላ አስጠንቅቅ | `FastpqQueueHeadroomLow` ማንቂያ (ማስጠንቀቂያ)【ዳሽቦርዶች/ማንቂያዎች/ fastpq_acceleration_rules.yml:41】 |
| የአሂድ ጊዜ ዜሮ-ሙላ መዘግየት | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | የቅርብ ጊዜ ዜሮ መሙላት ናሙና ≤0.40ms (ደረጃ 7 ገደብ) | `FastpqZeroFillRegression` ማንቂያ (ገጽ)【ዳሽቦርዶች/ማንቂያዎች/ fastpq_acceleration_rules.yml:58】 |መጠቅለያው የዜሮ መሙላት ረድፉን በቀጥታ ያስገድዳል። ማለፍ
`--require-zero-fill-max-ms 0.40` ወደ `scripts/fastpq/wrap_benchmark.py` እና እሱ
አግዳሚ ወንበር JSON ዜሮ ሙሌት ቴሌሜትሪ ሲጎድል ወይም በጣም ሞቃታማው በሚሆንበት ጊዜ አይሳካም።
የዜሮ ሙሌት ናሙና ከStage7 በጀት ይበልጣል፣የታቀፉ ቅርቅቦችን ይከላከላል
ያለ የታዘዘ ማስረጃ መላኪያ።【scripts/fastpq/wrap_benchmark.py:1008】

#### ደረጃ7-1 የማንቂያ-አያያዝ ማረጋገጫ ዝርዝር

ኦፕሬተሮች እንዲሰበሰቡ ከላይ የተዘረዘረው እያንዳንዱ ማንቂያ የተወሰነ የጥሪ መሰርሰሪያ ይመገባል።
የመልቀቂያ ጥቅል የሚያስፈልጋቸው ተመሳሳይ ቅርሶች፡-

1. **`FastpqQueueHeadroomLow` (ማስጠንቀቂያ)** ቅጽበታዊ የPrometheus መጠይቅ ያሂዱ
   ለ `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` እና
   የ Grafana "Queue headroom" ፓነልን ከ `fastpq-acceleration` ያንሱ
   ሰሌዳ. የጥያቄውን ውጤት ይመዝግቡ
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   ከማንቂያ መታወቂያው ጋር በመሆን የተለቀቀው ቅርቅብ ማስጠንቀቂያው መሆኑን ያረጋግጣል
   ወረፋው ከመራቡ በፊት እውቅና ተሰጥቶታል።【dashboards/grafana/fastpq_acceleration.json:1】
2. ** `FastpqZeroFillRegression` (ገጽ)።** መርምር
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` እና፣ ልኬቱ ከሆነ
   ጫጫታ፣ `scripts/fastpq/wrap_benchmark.py` በጣም በቅርብ ጊዜ በነበረው አግዳሚ ወንበር JSON ላይ በድጋሚ አሂድ
   `zero_fill_hotspots` ብሎክን ለማደስ። የ promQL ውጤቱን ያያይዙ ፣
   ቅጽበታዊ ገጽ እይታዎች፣ እና የታደሰ የቤንች ፋይል ወደ የታቀደው ማውጫ; ይህ ይፈጥራል
   በሚለቀቅበት ጊዜ `ci/check_fastpq_rollout.sh` የሚጠብቀው ተመሳሳይ ማስረጃ
   ማረጋገጥ።【ስክሪፕቶች/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. ** `FastpqCpuFallbackBurst` (ገጽ)።** ያረጋግጡ
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` ከ 5% ይበልጣል
   ወለል ፣ ከዚያ ለተዛማጅ የውርድ መልእክቶች የ `irohad` ምዝግብ ማስታወሻዎች ናሙና
   (`telemetry::fastpq.execution_mode resolved="cpu"`)። የ promQL መጣያ ያከማቹ
   በተጨማሪም የምዝግብ ማስታወሻዎች በ `metrics_cpu_fallback.prom`/`rollback_drill.log` ስለዚህ
   ጥቅል ተጽእኖውን እና የኦፕሬተሩን እውቅና ያሳያል።
4. **የማስረጃ ማሸግ።** ማንኛውም ማንቂያ ከተጣራ በኋላ፣የደረጃ7-3 ደረጃዎችን እንደገና አስጀምር
   የታቀዱ የመጫወቻ ደብተር (Grafana ወደ ውጭ መላክ ፣ የማንቂያ ቅጽበታዊ እይታ ፣ የድጋሚ መሰርሰሪያ) እና
   ጥቅሉን እንደገና ከማያያዝዎ በፊት በ`ci/check_fastpq_rollout.sh` በኩል እንደገና ያረጋግጡ
   ወደ መልቀቂያ ትኬት።【docs/source/fastpq_rollout_playbook.md:114】

አውቶማቲክን የሚመርጡ ኦፕሬተሮች ማሄድ ይችላሉ።
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
ለወረፋ ዋና ክፍል፣ ዜሮ ሙሌት እና የሲፒዩ ውድቀትን ለማግኘት Prometheus API ለመጠየቅ
ከላይ የተዘረዘሩት መለኪያዎች; ረዳቱ የተያዘውን JSON ይጽፋል (ቅድመ ቅጥያ በ
ኦሪጅናል ፕሮምQL) ወደ `metrics_headroom.prom`፣ `metrics_zero_fill.prom`፣ እና
`metrics_cpu_fallback.prom` በተመረጠው የታቀደ ልቀት ማውጫ ስር ስለዚህ እነዚያ ፋይሎች
ያለ በእጅ ጥምዝ ጥሪዎች ከጥቅሉ ጋር ማያያዝ ይቻላል.`ci/check_fastpq_rollout.sh` አሁን የወረፋውን ዋና ክፍል እና ዜሮ መሙላትን ያስገድዳል
በጀት በቀጥታ. እያንዳንዱን `metal` አግዳሚ ወንበሮችን በመጥቀስ ይተነትናል።
`fastpq_bench_manifest.json`፣ ይመረምራል።
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` እና
`benchmarks.zero_fill_hotspots[]`፣ እና የጭንቅላት ክፍል ሲወድቅ ጥቅሉን ይወድቃል
ከአንድ ማስገቢያ በታች ወይም ማንኛውም LDE መገናኛ ነጥብ `mean_ms > 0.40` ሲዘግብ። ይህ ያስቀምጣል
በ CI ውስጥ ያለው ደረጃ7 የቴሌሜትሪ ጥበቃ ፣ በ ላይ ከተሰራው በእጅ ግምገማ ጋር ይዛመዳል
Grafana ቅጽበታዊ እና የተለቀቀ ማስረጃ።【ci/check_fastpq_rollout.sh#L1】
እንደ ተመሳሳዩ ማረጋገጫ አካል ስክሪፕቱ አሁን እያንዳንዱ መጠቅለል እንዳለበት አጥብቆ ይናገራል
ቤንችማርክ በራስ-የተገኘ የሃርድዌር መለያዎችን (`metadata.labels.device_class`) ይይዛል
እና `metadata.labels.gpu_kind`)። እነዚያ መለያዎች የጠፉ ቅርቅቦች ወዲያውኑ አይሳኩም፣
ቅርሶችን፣ Stage7-2 ማትሪክስ እንደሚገለጥ እና የአሂድ ጊዜ እንደሚለቀቁ ዋስትና ይሰጣል
ዳሽቦርዶች ሁሉም ትክክለኛ ተመሳሳይ የመሣሪያ-መደብ ስሞችን ያመለክታሉ።

የGrafana “የቅርብ ጊዜ ቤንችማርክ” ፓኔል እና ተዛማጅ ልቀት ቅርቅብ አሁን ይህንን ይጠቅሳሉ።
`device_class`፣ ዜሮ-ሙላ በጀት እና የወረፋ-ጥልቀት ቅጽበታዊ ገጽ እይታ ስለዚህ የጥሪ መሐንዲሶች
የምርት ቴሌሜትሪ በምልክት ጊዜ ጥቅም ላይ ከዋለ ትክክለኛው የመያዣ ክፍል ጋር ማዛመድ ይችላል።
ጠፍቷል የወደፊት የማትሪክስ ግቤቶች ተመሳሳይ መለያዎችን ይወርሳሉ፣ ትርጉሙም የStage7-2 መሣሪያ
ዝርዝሮች እና የ Prometheus ዳሽቦርዶች ለ AppleM4 አንድ ነጠላ የስም ዘዴ ይጋራሉ ፣
M3 ማክስ፣ እና መጪ MI300/RTX ቀረጻዎች።

### ደረጃ7-1 ፍሊት ቴሌሜትሪ runbook

የጂፒዩ መስመሮችን በነባሪነት ከማንቃትዎ በፊት ይህንን የፍተሻ ዝርዝር ይከተሉ
እና የማስጠንቀቂያ አስተዳዳሪ ደንቦች በመለቀቅ ዝግጅት ወቅት የተያዙትን ተመሳሳይ ማስረጃዎች ያንፀባርቃሉ፡

1. ** ቀረጻ እና የሩጫ ጊዜ አስተናጋጆችን ይሰይሙ።** `python3 scripts/fastpq/wrap_benchmark.py`
   አስቀድሞ `metadata.labels.device_class`፣ `chip_family`፣ እና `gpu_kind` ያወጣል
   ለእያንዳንዱ ተጠቅልሎ JSON. እነዚያን መለያዎች ከ ጋር በማመሳሰል ያቆዩዋቸው
   `fastpq.{device_class,chip_family,gpu_kind}` (ወይም የ
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) በ `iroha_config` ውስጥ
   ስለዚህ የሩጫ ጊዜ መለኪያዎች ያትማሉ
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   እና የ `fastpq_metal_queue_*` መለኪያዎች ከሚታዩ ተመሳሳይ መለያዎች ጋር
   በ `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. አዲስ ሲያዘጋጁ
   ክፍል፣ የማትሪክስ አንጸባራቂውን በ በኩል ያድሱ
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   ስለዚህ CI እና ዳሽቦርዶች ተጨማሪ መለያውን ይረዳሉ።
2. ** የወረፋ መለኪያዎችን እና የጉዲፈቻ መለኪያዎችን ያረጋግጡ።** `irohad --features fastpq-gpu` ን ያሂዱ።
   የቀጥታ ወረፋውን ለማረጋገጥ በብረታ ብረት አስተናጋጆች ላይ እና የቴሌሜትሪ መጨረሻ ነጥብን ይቧጩ
   መለኪያዎች ወደ ውጭ በመላክ ላይ ናቸው

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```የመጀመሪያው ትዕዛዝ ሴማፎር ናሙና `busy` እየለቀቀ መሆኑን ያረጋግጣል፣
   `overlap`፣ `limit`፣ እና `max_in_flight` ተከታታይ እና ሁለተኛው የሚያሳየው
   እያንዳንዱ የመሳሪያ ክፍል ለ`backend="metal"` እየፈታ ነው ወይም ወደ ኋላ እየወደቀ ነው።
   `backend="cpu"`. ከዚህ በፊት የቧጨራውን ኢላማ በPrometheus/OTel በኩል ሽቦ ያድርጉ
   Grafana የመርከቦቹን እይታ ወዲያውኑ ማቀድ እንዲችል ዳሽቦርዱን ማስመጣት ።
3. **የዳሽቦርድ + ማንቂያ ጥቅልን ጫን።** አስመጣ
   `dashboards/grafana/fastpq_acceleration.json` ወደ Grafana (ያቆየው)
   አብሮ የተሰራ የመሣሪያ ክፍል፣ ቺፕ ቤተሰብ እና የጂፒዩ ዓይነት አብነት ተለዋዋጮች) እና ጭነት
   `dashboards/alerts/fastpq_acceleration_rules.yml` ወደ Alertmanager አንድ ላይ
   ከእሱ አሃድ የሙከራ መሣሪያ ጋር። ደንቡ ጥቅል `promtool` መታጠቂያ ይልካል; መሮጥ
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   ደንቦቹ ሲቀየሩ `FastpqMetalDowngrade` እና
   `FastpqBackendNoneBurst` አሁንም በሰነድ በተቀመጡት ገደቦች ላይ ይቃጠላል።
4. **ጌት ከማስረጃ ጥቅል ጋር ይለቃል።** አቆይ
   ልቀትን በማመንጨት ላይ `docs/source/fastpq_rollout_playbook.md` ምቹ
   ማስረከብ እያንዳንዱ ጥቅል የታሸጉትን መለኪያዎችን፣ Grafana ወደ ውጭ መላክ፣
   የማንቂያ ጥቅል፣ የቴሌሜትሪ ወረፋ ማረጋገጫ እና የመመለሻ ምዝግብ ማስታወሻዎች። CI ቀድሞውንም ያስፈጽማል
   ውል: `make check-fastpq-rollout` (ወይም በመጥራት
   `ci/check_fastpq_rollout.sh --bundle <path>`) ጥቅሉን ያረጋግጣል፣ እንደገና ይሰራል
   ማንቂያው ይፈትሻል፣ እና የጭንቅላት ክፍል ወይም ዜሮ ሲሞሉ ለመፈረም ፈቃደኛ አይሆንም
   በጀቶች ወደ ኋላ መመለስ.
5. ** ማንቂያዎችን ወደ ማሻሻያ መልሰው ያስሩ።** የአለርትማኔጀር ገፆች ሲሆኑ፣ Grafana ይጠቀሙ።
   ቦርድ እና ጥሬው Prometheus ቆጣሪዎች ከደረጃ 2 ለማረጋገጥ
   ከወረፋ ረሃብ፣ ከሲፒዩ ውድቀት፣ ወይም ከኋላ = ምንም አይፈነዳም።
የ runbook ይኖራል
ይህ ሰነድ በተጨማሪ `docs/source/fastpq_rollout_playbook.md`; አዘምን
የመልቀቅ ትኬት ከሚመለከተው `fastpq_execution_mode_total`፣
`fastpq_metal_queue_ratio`፣ እና `fastpq_metal_queue_depth` ጥቅሶች አንድ ላይ
ወደ Grafana ፓነል አገናኞች እና ገምጋሚዎች ማየት እንዲችሉ የማንቂያ ቅጽበታዊ ገጽ እይታ
በትክክል የትኛው SLO እንደቀሰቀሰ።

### WP2-E - ደረጃ በደረጃ የብረት መገለጫ ቅጽበታዊ እይታ

`scripts/fastpq/src/bin/metal_profile.rs` የታሸጉትን የብረት ቀረጻዎችን ያጠቃልላል
ስለዚህ የ900ms ዒላማው በጊዜ ሂደት መከታተል ይቻላል (አሂድ
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`)።
አዲሱ Markdown አጋዥ
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
ከታች ያሉትን የመድረክ ሰንጠረዦች ያመነጫል (Markdown ን ከጽሑፍ ጋር ያትማል
ማጠቃለያ ስለዚህ WP2-E ትኬቶች ማስረጃውን በቃላት መካተት ይችላሉ። ሁለት ቀረጻዎች ተከታትለዋል
አሁን፡

> ** አዲስ የ WP2-E መሣሪያ፡** `fastpq_metal_bench --gpu-probe ...` አሁን አንድን ያወጣል።
> የማወቂያ ቅጽበታዊ እይታ (የተጠየቀ/የተፈታ የማስፈጸሚያ ሁነታ፣ `FASTPQ_GPU`
> ይሽራል፣ የተገኘ የኋላ ክፍል እና የተዘረዘሩ የብረታ ብረት መሳሪያዎች/የመዝገብ መታወቂያዎች)
> ማንኛውም ከርነሎች ከመሮጥ በፊት። የግዳጅ ጂፒዩ በቆመበት ጊዜ ይህን ምዝግብ ያንሱት።
> ወደ ሲፒዩ መንገድ ስለሚወድቅ ማስረጃዎቹ የትኞቹን አስተናጋጆች እንደሚያዩ ይመዘግባል
> `MTLCopyAllDevices` መመለሻ ዜሮ እና የትኛዎቹ መሻሮች በነበሩበት ጊዜ ተፈጻሚ ነበሩ።
> ቤንችማርክ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> ** የመድረክ ቀረጻ አጋዥ:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> አሁን `fastpq_metal_bench`ን ለFFT፣ LDE እና Poseidon በተናጠል ያንቀሳቅሳል፣
> ጥሬውን የJSON ውጤቶች በየደረጃው ማውጫዎች ስር ያከማቻል እና አንድ ነጠላ ያወጣል።
> `stage_profile_summary.json` ጥቅል ሲፒዩ/ጂፒዩ ጊዜዎችን የሚመዘግብ፣ የወረፋ ጥልቀት
> ቴሌሜትሪ፣ የአምድ-ማስተዳደሪያ ስታቲስቲክስ፣ የከርነል መገለጫዎች እና ተያያዥ ዱካ
> ቅርሶች። ንዑስ ስብስብን ለማነጣጠር `--stage fft --stage lde --stage poseidon` ይለፉ፣
> `--trace-template "Metal System Trace"` የተወሰነ የ xctrace አብነት ለመምረጥ፣
> እና `--trace-dir` ወደ `.trace` ቅርቅቦችን ወደ የጋራ ቦታ ለመምራት። ያያይዙት።
> ማጠቃለያ JSON እና የተፈጠሩ የመከታተያ ፋይሎች ለእያንዳንዱ WP2-E እትም ስለዚህ ገምጋሚዎች
> የወረፋ መኖርን (`metal_dispatch_queue.*`)፣ መደራረብ ሬሾን እና
> በእጅ ብዜትን ሳያስገቡ የተቀረጸ የማስጀመሪያ ጂኦሜትሪ በሩጫ ላይ
> `fastpq_metal_bench` ጥሪዎች።【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> ** ወረፋ/ማስረጃ ረዳት (2026-05-09):** `scripts/fastpq/profile_queue.py` አሁን
> አንድ ወይም ከዚያ በላይ `fastpq_metal_bench` JSON ያስገባ እና ሁለቱንም የማርክዳውን ሰንጠረዥ ያወጣል እና
> በማሽን ሊነበብ የሚችል ማጠቃለያ (`--markdown-out/--json-out`) ስለዚህ ወረፋ ጥልቀት፣ መደራረብ ሬሾ እና
> አስተናጋጅ-ጎን ስቴጅንግ ቴሌሜትሪ ከእያንዳንዱ WP2-E አርቲፊክስ ጋር አብሮ መጓዝ ይችላል። መሮጥ
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` ከዚህ በታች ያለውን ሰንጠረዥ አዘጋጅቶ በማህደር የተቀመጠ ሜታል ቀረጻ አሁንም እንደሚዘግብ ጠቁሟል።
> `dispatch_count = 0` እና `column_staging.batches = 0`—WP2-E.1 እስከ ብረት ድረስ ክፍት እንደሆኑ ይቆያሉ።
> በቴሌሜትሪ የነቃ መሣሪያ እንደገና ይገነባል። የፈጠረው JSON/Markdown ቅርሶች በቀጥታ ስርጭት
> ለኦዲት በ `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` ስር።
> ረዳቱ አሁን (2026-05-19) እንዲሁም የፖሲዶን ቧንቧ መስመር ቴሌሜትሪ (`pipe_depth`፣
> `batches`፣ `chunk_columns`፣ እና `fallbacks`) በሁለቱም የMarkdown ሠንጠረዥ እና የJSON ማጠቃለያ ውስጥ፣
> ስለዚህ WP2-E.4/6 ገምጋሚዎች ጂፒዩ በቧንቧ መስመር ላይ መቆየቱን እና አለመኖሩን ማረጋገጥ ይችላሉ።
> ጥሬ ቀረጻውን ሳይከፍቱ መውደቅ ተከስቷል።【scripts/fastpq/profile_queue.py:1】> ** የመድረክ መገለጫ ማጠቃለያ (2026-05-30):** `scripts/fastpq/stage_profile_report.py` ይበላል
> በ `cargo xtask fastpq-stage-profile` የወጣው `stage_profile_summary.json` ጥቅል እና
> የ WP2-E ገምጋሚዎች ማስረጃን ወደ ቲኬቶች መቅዳት እንዲችሉ ሁለቱንም Markdown እና JSON ማጠቃለያ ይሰጣል
> ጊዜዎችን በእጅ ሳይገለብጥ። ጥራ
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> ጂፒዩ/ሲፒዩ ማለት፣ ፈጣን ዴልታዎች፣ የመከታተያ ሽፋን፣ እና የሚዘረዝሩ ቆራጥ ሠንጠረዦችን ለማምረት
> የቴሌሜትሪ ክፍተቶች በየደረጃው። የJSON ውፅዓት ሠንጠረዡን ያንፀባርቃል እና በየደረጃው የችግር መለያዎችን ይመዘግባል
> (`trace missing`፣ `queue telemetry missing`፣ ወዘተ) ስለዚህ የአስተዳደር አውቶማቲክ አስተናጋጁን ሊለያይ ይችላል።
> ከ WP2-E.1 እስከ WP2-E.6 ድረስ ተጠቅሷል።
> ** አስተናጋጅ/የመሳሪያ መደራረብ ጠባቂ (2026-06-04):** `scripts/fastpq/profile_queue.py` አሁን ማብራሪያ ይሰጣል
> FFT/LDE/Poseidon የጥበቃ ሬሾ በየደረጃው ጠፍጣፋ/ቆይ ሚሊሰከንድ ድምር እና ልቀት
> ችግር `--max-wait-ratio <threshold>` ደካማ መደራረብን ሲያገኝ። ተጠቀም
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> ሁለቱንም የማርክ ታች ሠንጠረዥ እና የJSON ቅርቅብ ከ WP2-E.5 ትኬቶች ጋር ግልጽ በሆነ የጥበቃ መጠን ለመያዝ
> ድርብ-ማቋረጫ መስኮቱ ጂፒዩ መመገቡን እንደያዘ ያሳያል። ግልጽ-ጽሑፍ ኮንሶል ውፅዓት እንዲሁ
> የጥሪ ላይ ምርመራዎችን ቀላል ለማድረግ በየደረጃው ያለውን ሬሾ ይዘረዝራል።
> ** የቴሌሜትሪ ጠባቂ + የሩጫ ሁኔታ (2026-06-09):** `fastpq_metal_bench` አሁን `run_status` ብሎክ ያወጣል
> (የኋላ መለያ፣ የመላኪያ ብዛት፣ ምክንያቶች) እና አዲሱ `--require-telemetry` ባንዲራ ሩጫውን ወድቋል።
> በማንኛውም ጊዜ የጂፒዩ ጊዜዎች ወይም ወረፋ/የቴሌሜትሪ ዝግጅት በሚጠፋበት ጊዜ። `profile_queue.py` ሩጫውን ያቀርባል
> ሁኔታ እንደ አንድ የተወሰነ አምድ እና ወለል ላይ `ok` ያልሆኑ ሁኔታዎች እትም ዝርዝር ውስጥ, እና
> `launch_geometry_sweep.py` ተመሳሳይ ሁኔታን ወደ ማስጠንቀቂያዎች/መፈረጅ ስለሚያስገባ ማትሪክስ ምንም አይችልም
> በፀጥታ ወደ ሲፒዩ የወደቁ ወይም የወረፋ መሣሪያን የተዘለሉ ቀረጻዎችን አምኗል።
> **Poseidon/LDE ራስ-ማስተካከል (2026-06-12):** `metal_config::poseidon_batch_multiplier()` አሁን ሚዛኖች
> በብረታ ብረት የሚሰሩ ፍንጮች እና `lde_tile_stage_target()` የሰድር ጥልቀትን በልዩ ጂፒዩዎች ላይ ያሳድጋል።
> የተተገበረው ብዜት እና ንጣፍ ገደብ በ `metal_heuristics` ብሎክ ውስጥ ተካትቷል።
> `fastpq_metal_bench` ውፅዓቶች እና በ `scripts/fastpq/metal_capture_summary.py` የተሰራ፣ ስለዚህ WP2-E
> ጥቅሎች በጥሬው JSON ሳይቆፍሩ በእያንዳንዱ ቀረጻ ውስጥ ጥቅም ላይ የሚውሉትን ትክክለኛ የቧንቧ መስመሮች ይመዘግባሉ::

| መለያ | መላኪያ | ስራ የበዛበት | መደራረብ | ከፍተኛ ጥልቀት | FFT ጠፍጣፋ | FFT መጠበቅ | FFT መጠበቅ % | LDE ጠፍጣፋ | LDE ይጠብቁ | LDE መጠበቅ % | ፖሲዶን ጠፍጣፋ | ፖሲዶን መጠበቅ | Poseidon መጠበቅ % | የቧንቧ ጥልቀት | የቧንቧ ስብስቦች | የቧንቧ ውድቀት |
|---|---|---|---|---|---|---|---|---|--|
| fastpq_metal_bench_poseidon | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_አዲስ | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20k ቅጽበታዊ እይታ (ቅድመ-መሻር)

`fastpq_metal_bench_20k_latest.json`| መድረክ | አምዶች | የግቤት ሌን | ጂፒዩ አማካኝ (ms) | ሲፒዩ አማካኝ (ms) | የጂፒዩ ድርሻ | ፍጥነት | Δ ሲፒዩ (ሚሴ) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 130.986ms (115.761–167.755) | 112.616ms (95.335–132.929) | 2.4% | 0.860× | -18.370 |
| IFFT | 16 | 32768 | 129.296ms (111.127–142.955) | 158.144ms (126.847–237.887) | 2.4% | 1.223× | +28.848 |
| LDE | 16 | 262144 | 1570.656ms (1544.397–1584.502) | 1752.523ms (1548.807–2191.930) | 29.2% | 1.116× | +181.867 |
| ፖሲዶን | 16 | 524288 | 3548.329ms (3519.881–3576.041) | 3642.706ms (3539.055–3758.279) | 66.0% | 1.027× | +94.377 |

ቁልፍ ምልከታዎች፡-1. የጂፒዩ ድምር 5.379s ነው፣ ይህም ከ900ሚሴ ግብ **4.48 ሴ በላይ ነው። ፖሲዶን
   hashing አሁንም የሩጫ ሰዓቱን (≈66%) በ LDE ከርነል በሰከንድ ይቆጣጠራል
   ቦታ (≈29%)፣ ስለዚህ WP2-E ሁለቱንም የፖሲዶን ቧንቧ መስመር ጥልቀት እና ማጥቃት ያስፈልገዋል።
   የሲፒዩ ውድቀት ከመጥፋቱ በፊት የ LDE ማህደረ ትውስታ ነዋሪነት / ንጣፍ እቅድ።
2. ምንም እንኳን IFFT>1.22× ከስካላር በላይ ቢሆንም FFT ሪግሬሽን (0.86×) ሆኖ ይቆያል።
   መንገድ. የማስጀመሪያ-ጂኦሜትሪ መጥረግ ያስፈልገናል
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) ለመረዳት
   የኤፍኤፍቲ መኖርያ ቀድሞውንም የተሻለውን ሳይጎዳ መዳን ይቻል እንደሆነ
   የIFFT ጊዜዎች። የ `scripts/fastpq/launch_geometry_sweep.py` አጋዥ አሁን ያሽከረክራል።
   እነዚህ ሙከራዎች ከጫፍ እስከ ጫፍ፡ በነጠላ ሰረዝ የተለዩ መሻሪያዎችን ማለፍ (ለምሳሌ፣
   `--fft-columns 16,32 --queue-fanout 1,2` እና
   `--poseidon-lanes auto,256`) እና ይጠራል
   `fastpq_metal_bench` ለእያንዳንዱ ጥምረት፣ የJSON ጭነት ጭነቶችን ስር ያከማቹ
   `artifacts/fastpq_geometry/<timestamp>/`፣ እና የ`summary.json` ጥቅል ቀጥል
   የእያንዳንዱን ሩጫ ወረፋ ሬሾዎች፣ የኤፍኤፍቲ/ኤልዲኢ ማስጀመሪያ ምርጫዎችን፣ ጂፒዩ እና ሲፒዩ ጊዜዎችን መግለጽ፣
   እና የአስተናጋጁ ሜታዳታ (የአስተናጋጅ ስም/መለያ፣ የመድረክ ሶስት እጥፍ፣ የተገኘ መሳሪያ
   ክፍል፣ ጂፒዩ አቅራቢ/ሞዴል) ስለዚህ የመሣሪያ ተሻጋሪ ንጽጽሮች ቆራጥነት አላቸው።
   ፕሮቬንሽን. ረዳቱ አሁን ደግሞ `reason_summary.json` ቀጥሎ ይጽፋል
   ማጠቃለያ በነባሪ፣ ለመንከባለል ከጂኦሜትሪ ማትሪክስ ጋር ተመሳሳይ ክላሲፋየር በመጠቀም
   የሲፒዩ ውድቀት እና የጠፋ ቴሌሜትሪ። መለያ ለመስጠት `--host-label staging-m3` ይጠቀሙ
   ከጋራ ቤተ-ሙከራዎች ይቀርጻል.
   ተጓዳኝ `scripts/fastpq/geometry_matrix.py` መሳሪያ አሁን አንድ ወይም ያስገባል።
   ተጨማሪ ማጠቃለያ ጥቅሎች (`--summary hostA/summary.json --summary hostB/summary.json`)
   እና እያንዳንዱን የማስጀመሪያ ቅርጽ * የተረጋጋ* ብለው የሚሰይሙ Markdown/JSON ሰንጠረዦችን ያወጣል።
   (FFT/LDE/Poseidon GPU ጊዜዎች ተይዘዋል) ወይም *ያልተረጋጋ* (የጊዜ ማብቂያ፣ የሲፒዩ ውድቀት፣
   ሜታል ያልሆነ ጀርባ፣ ወይም የጠፋ ቴሌሜትሪ) ከአስተናጋጅ አምዶች ጋር። የ
   ሠንጠረዦች አሁን የተፈታውን `execution_mode`/`gpu_backend` እና ሀን ያካትታሉ
   `Reason` አምድ ስለዚህ የሲፒዩ ውድቀት እና የጎደሉ የጂፒዩ ጊዜዎች በ ውስጥ ግልጽ ናቸው
   ደረጃ 7 ማትሪክስ የጊዜ እገዳዎች በሚኖሩበት ጊዜ እንኳን; ማጠቃለያ መስመር ይቆጠራል
   የተረጋጋ vs አጠቃላይ ሩጫዎች። ማለፊያ `--operation fft|lde|poseidon_hash_columns`
   መጥረጊያው አንድ ነጠላ ደረጃን ማግለል ሲያስፈልግ (ለምሳሌ፣ ወደ መገለጫ
   Poseidon ለየብቻ) እና `--extra-args` ለቤንች-ተኮር ባንዲራዎች ነፃ ያድርጉት።
   ረዳቱ ማንኛውንም ይቀበላል
   የትእዛዝ ቅድመ ቅጥያ (ወደ `cargo run … fastpq_metal_bench` ነባሪ) እና አማራጭ
   የአፈጻጸም መሐንዲሶች እንዲችሉ `--halt-on-error` / `--timeout-seconds` ጠባቂዎች
   ንፅፅር በሚሰበስቡበት ጊዜ መጥረጊያውን በተለያዩ ማሽኖች ላይ ማባዛት ፣
   ለደረጃ7 የብዝሃ-መሳሪያ ማስረጃ ቅርቅቦች።
3. `metal_dispatch_queue` `dispatch_count = 0` ሪፖርት አድርጓል፣ ስለዚህ ወረፋ መያዝ
   የጂፒዩ ኮርነሎች ቢሄዱም ቴሌሜትሪ ጠፍቷል። የብረታ ብረት ሩጫ አሁን ይጠቀማል
   ለወረፋ/የአምድ ማስተናገጃ መቀያየርን አጥር ማግኘት/መልቀቅ የሰራተኛ ክሮች
   የመሳሪያውን ባንዲራዎች ይመልከቱ፣ እና የጂኦሜትሪ ማትሪክስ ሪፖርቱ ይጠራል
   FFT/LDE/Poseidon ጂፒዩ ጊዜ አቆጣጠር በማይኖርበት ጊዜ ያልተረጋጋ የማስጀመሪያ ቅርጾች። አቆይ
   ገምጋሚዎች ማየት እንዲችሉ Markdown/JSON ማትሪክስ ከ WP2-E ትኬቶች ጋር በማያያዝ
   ወረፋ ቴሌሜትሪ ከተገኘ በኋላ የትኞቹ ጥምሮች አሁንም አልተሳኩም።የ `run_status` ጠባቂ እና `--require-telemetry` ባንዲራ አሁን መቅረጽ አልተሳካም
   የጂፒዩ ጊዜ ሲጠፋ ወይም ቴሌሜትሪ ወረፋ/ማዘጋጀት በማይኖርበት ጊዜ፣ ስለዚህ
   dispatch_count=0 ሩጫዎች ሳያውቁ ወደ WP2-E ቅርቅቦች ሊገቡ አይችሉም።
   `fastpq_metal_bench` አሁን `--require-gpu` ያጋልጣል፣ እና
   `launch_geometry_sweep.py` በነባሪነት ያነቃዋል (መርጠው ይውጡ
   `--allow-cpu-fallback`) ስለዚህ የሲፒዩ ውድቀት እና የብረት ማወቂያ ውድቀቶች ይቋረጣሉ
   ወዲያውኑ Stage7 ማትሪክስ ጂፒዩ ባልሆኑ ቴሌሜትሪዎች ከመበከል ይልቅ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【ስክሪፕቶች/fastpq/launch_geometry_sweep.py】
4. ዜሮ-ሙሌት መለኪያዎች ቀደም ሲል በተመሳሳይ ምክንያት ጠፍተዋል; የአጥር ማስተካከያ
   የአስተናጋጁን መሳሪያ በቀጥታ ያቆያል፣ ስለዚህ ቀጣዩ ቀረጻ የሚከተሉትን ማካተት አለበት።
   `zero_fill` አግድ ያለ ሰው ሠራሽ ጊዜዎች።

#### 20k ቅጽበተ-ፎቶ ከ `FASTPQ_GPU=gpu` ጋር

`fastpq_metal_bench_20k_refresh.json`

| መድረክ | አምዶች | የግቤት ሌን | ጂፒዩ አማካኝ (ms) | ሲፒዩ አማካኝ (ms) | የጂፒዩ ድርሻ | ፍጥነት | Δ ሲፒዩ (ሚሴ) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 79.951ms (65.645–93.193) | 83.289ms (59.956–107.585) | 0.3% | 1.042× | +3.338 |
| IFFT | 16 | 32768 | 78.605ms (69.986–83.726) | 93.898ms (80.656–119.625) | 0.3% | 1.195× | +15.293 |
| LDE | 16 | 262144 | 657.673ms (619.219–712.367) | 669.537ms (619.716–723.285) | 2.1% | 1.018× | +11.864 |
| ፖሲዶን | 16 | 524288 | 30004.898ms (27284.117–32945.253) | 29087.532ms (24969.810–33020.517) | 97.4% | 0.969× | -917.366 |

ምልከታዎች፡-

1. በ`FASTPQ_GPU=gpu` እንኳን ይህ ቀረጻ አሁንም የሲፒዩ ውድቀትን ያንጸባርቃል፡-
   ~ 30ዎች በአንድ ድግግሞሽ `metal_dispatch_queue` በዜሮ ተጣብቋል። መቼ
   መሻር ተዘጋጅቷል ነገር ግን አስተናጋጁ የብረት መሣሪያ ማግኘት አልቻለም፣ CLI አሁን ይወጣል
   ማንኛውንም ከርነሎች ከማስኬድዎ በፊት እና የተጠየቀውን/የተፈታውን ሁነታ እና እንዲሁም የ
   መሐንዲሶች ማግኘቱን፣መብቶችን ወይም ን ማወቅ እንዲችሉ የጀርባ መለያ
   የሜታሊብ ፍለጋ ማሽቆልቆሉን አስከትሏል። `fastpq_metal_bench --gpu-probeን ያሂዱ
   --ረድፎች …` with `FASTPQ_DEBUG_METAL_ENUM=1` የመቁጠሪያ ምዝግብ ማስታወሻውን ለመያዝ እና
   ፕሮፋይሉን እንደገና ከማስኬዱ በፊት ያለውን የማወቅ ችግር ያስተካክሉ።【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. ዜሮ ሙሌት ቴሌሜትሪ አሁን እውነተኛ ናሙና መዝግቧል (18.66 ሚሴ ከ32ሚቢ በላይ)፣ ይህም ያረጋግጣል።
   የአጥር ጥገናው ይሰራል፣ ግን ወረፋ ዴልታዎች ጂፒዩ እስኪልክ ድረስ አይቀሩም።
   ተሳካለት ።
3. የጀርባው ደረጃ እየቀነሰ ስለሚሄድ፣ የStage7 ቴሌሜትሪ በር አሁንም አለ።
   ታግዷል፡ የወረፋ ዋና ክፍል ማስረጃ እና የፖሲዶን መደራረብ እውነተኛ ጂፒዩ ያስፈልጋቸዋል
   መሮጥ

እነዚህ ምስሎች አሁን የ WP2-E የኋላ መዝገብን መልሕቅ አድርገውታል። ቀጣይ ድርጊቶች፡ ፕሮፋይለርን ሰብስቡ
flamecharts እና የወረፋ ምዝግብ ማስታወሻዎች (አንዴ የጀርባው በጂፒዩ ላይ ሲሰራ)፣ ኢላማውን ያድርጉ
FFT ን እንደገና ከመጎብኘትዎ በፊት የPoseidon/LDE ማነቆዎች እና የጀርባውን ውድቀት ያንሱ
ስለዚህ Stage7 ቴሌሜትሪ ትክክለኛ የጂፒዩ ዳታ አለው።

### ጥንካሬዎች
- የመጨመሪያ ደረጃ አሰጣጥ፣ የመከታተያ-የመጀመሪያ ንድፍ፣ ግልጽ የSTARK ቁልል።### ከፍተኛ ቅድሚያ የሚሰጣቸው የድርጊት እቃዎች
1. ማሸግ / እቃዎችን ማዘዝ እና የ AIR ዝርዝርን ማዘመን.
2. Poseidon2ን ያጠናቅቁ `3f2b7fe` እና ምሳሌ SMT/lookup vectors ያትሙ።
3. የተሰሩ ምሳሌዎችን (`lookup_grand_product.md`, `smt_update.md`) ከመሳሪያዎች ጋር ይያዙ.
4. አባሪ ጨምር ጤናማነት መነጨ እና CI ውድቅ የተደረገ ዘዴ።

### የተፈቱ የንድፍ ውሳኔዎች
- ZK ተሰናክሏል (ትክክለኝነት-ብቻ) በ P1; ወደፊት ደረጃ እንደገና ይጎብኙ.
- ከአስተዳደር ግዛት የተገኘ የፍቃድ ሰንጠረዥ ሥር; ባችዎች ጠረጴዛን እንደ ተነባቢ-ብቻ ይመለከቱታል እና አባልነታቸውን በፍለጋ ያረጋግጡ።
- የሌሉ ቁልፍ ማረጋገጫዎች ዜሮ ቅጠል እና የጎረቤት ምስክር ከቀኖናዊ ኢንኮዲንግ ጋር ይጠቀማሉ።
- ትርጉም ሰርዝ = የቅጠል ዋጋ በቀኖናዊ ቁልፍ ቦታ ውስጥ ወደ ዜሮ ተቀናብሯል።

ይህንን ሰነድ እንደ ቀኖናዊ ማጣቀሻ ይጠቀሙ; መንሸራተትን ለማስወገድ ከምንጩ ኮድ፣ መጫዎቻዎች እና ተጨማሪዎች ጋር ያዘምኑት።

## አባሪ ሀ - የድምፅ አመጣጥ

ይህ አባሪ የ"Soundness & SLOs" ሰንጠረዥ እንዴት እንደሚመረት እና CI ቀደም ሲል የተጠቀሰውን ≥128-ቢት ወለል እንዴት እንደሚያስፈጽም ያብራራል።

### ማስታወሻ
- `N_trace = 2^k` - ለሁለት ኃይል ከተደረደሩ እና ከተጣበቀ በኋላ የመከታተያ ርዝመት።
- `b` - የማፈንዳት ምክንያት (`N_eval = N_trace × b`)።
- `r` - FRI arity (8 ወይም 16 ለቀኖናዊ ስብስቦች).
- `ℓ` - የFRI ቅነሳዎች ቁጥር (`layers` አምድ)።
- `q` — የማረጋገጫ መጠይቆች በማረጋገጫ (`queries` አምድ)።
- `ρ` - ውጤታማ የኮድ መጠን በአምዱ እቅድ አውጪ የተዘገበው፡ `ρ = max_i(degree_i / domain_i)` ከመጀመሪያው FRI ዙር የተረፉ ገደቦች ላይ።

የጎልድሎክስ ቤዝ መስክ `|F| = 2^64 - 2^32 + 1` አለው፣ ስለዚህ የFiat–Shamir ግጭቶች በ`q / 2^64` የተገደቡ ናቸው። መፍጨት አንድ orthogonal `2^{-g}` ፋክተር ያክላል፣ ከ `g = 23` ለ `fastpq-lane-balanced` እና `g = 21` ለላቲነት መገለጫ።【crates/fastpq_isi/src/params.rs:65【crates/fastpq_isi/src/params.rs:65

### የትንታኔ ትስስር

በቋሚ-ተመን DEEP-FRI የስታቲስቲክስ ውድቀት እድል ያሟላል።

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

ምክንያቱም እያንዳንዱ ሽፋን `ρ` ቋሚ እንዲሆን በማድረግ የፖሊኖሚል ዲግሪ እና የጎራ ስፋትን በተመሳሳይ ሁኔታ `r` ይቀንሳል። የሠንጠረዡ `est bits` አምድ `⌊-log₂ p_fri⌋`; Fiat–Shamir እና መፍጨት እንደ ተጨማሪ የደህንነት ህዳግ ሆነው ያገለግላሉ።

### እቅድ አውጪ ውፅዓት እና የሚሰራ ስሌት

የደረጃ 1 አምድ እቅድ አውጪን በተወካይ ስብስቦች ላይ ማስኬድ፡-

| መለኪያ ስብስብ | `N_trace` | `b` | `N_eval` | `ρ` (እቅድ) | ውጤታማ ዲግሪ (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------ | -------- | --- | -------- | ------------ | ---------------------------------- | --- | --- | ------------------ |
| ሚዛናዊ 20k ባች | `2^15` | 8 | 262144 | 0.077026 | 20192 | 5 | 52 | 190ቢት |
| ማስተላለፍ 65k ባች | `2^16` | 8 | 524288 | 0.200208 | 104967 | 6 | 58 | 132ቢት |
| Latency 131k ባች | `2^17` | 16 | 2097152 | 0.209492 | 439337 | 5 | 64 | 142ቢት |ምሳሌ (የተመጣጠነ 20k ባች)
1. `N_trace = 2^15`, ስለዚህ `N_eval = 2^15 × 8 = 2^18`.
2. የፕላነር መሳሪያዎች `ρ = 0.077026` ሪፖርቶች, ስለዚህ `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, ከጠረጴዛው ግቤት ጋር ይዛመዳል.
4. Fiat–Shamir ግጭቶች ቢበዛ `2^{-58.3}` ይጨምራሉ፣ እና መፍጨት (`g = 23`) ሌላ `2^{-23}` በመቀነስ አጠቃላይ ድምፁ ከ160bits በላይ በምቾት እንዲቆይ ያደርጋል።

### CI ውድቅ-ናሙና መታጠቂያ

እያንዳንዱ የCI አሂድ የሞንቴ ካርሎ ትጥቆችን ያከናውናል፣ ይህም የተጨባጭ መለኪያዎች በትንታኔ ገደብ ±0.6bits ውስጥ መቆየታቸውን ለማረጋገጥ፡-
1. ቀኖናዊ መለኪያ ስብስብ ይሳሉ እና `TransitionBatch` ከተዛማጅ የረድፍ ብዛት ጋር ያዋህዱ።
2. ዱካውን ይገንቡ፣ በዘፈቀደ የተመረጠውን ገደብ ገልብጡ (ለምሳሌ፣ የፍለጋ ታላቁን ምርት ወይም የSMT ወንድም እህት) እና ማስረጃን ለማምጣት ይሞክሩ።
3. አረጋጋጩን እንደገና ያሂዱ፣ የFiat–Shamir ፈተናዎችን እንደገና በማንሳት (መፍጨትን ጨምሮ) እና የተበላሸው ማስረጃ ውድቅ መደረጉን ይመዝግቡ።
4. ለ 16384 ዘሮች በአንድ ፓራሜትር ይድገሙ እና 99% ክሎፐር-ፒርሰን ዝቅተኛውን ገደብ ወደ ቢትስ ይለውጡ።

የሚለካው የታችኛው ወሰን ከ128ቢቶች በታች ከተንሸራተቱ ስራው ወዲያውኑ አይሳካለትም፣ ስለዚህ በእቅድ አውጪው ውስጥ ያሉ ለውጦች፣ ፎልዲንግ ሉፕ ወይም የጽሁፍ ግልባጭ ሽቦ ከመቀላቀል በፊት ይያዛሉ።

## አባሪ ለ - የጎራ-ሥር አመጣጥ

Stage0 የመከታተያ እና የግምገማ ጀነሬተሮችን ከፖሲዶን የተገኘ ቋሚ ቋሚዎች ይሰካል ስለዚህ ሁሉም ትግበራዎች ተመሳሳይ ንዑስ ቡድኖችን ይጋራሉ።

## አሰራር
1. **የዘር ምርጫ።** UTF‑8 መለያ `fastpq:v1:domain_roots` ወደ Poseidon2 ስፖንጅ በFASTPQ ውስጥ ሌላ ቦታ ይቅቡት (የግዛት ስፋት=3፣ ተመን=2፣አራት ሙሉ + 57 ከፊል ዙሮች)። ግብዓቶች `[len, limbs…]` ኢንኮዲንግ ከ`pack_bytes` እንደገና ጥቅም ላይ ይውላሉ፣ ይህም የመሠረት ጀነሬተር `g_base = 7` ይሰጣል።
2. **መከታተያ ጀነሬተር።** `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` አስላ እና `trace_root^{2^{trace_log_size}} = 1` አረጋግጥ የግማሽ ሃይል 1 ካልሆነ።
3. ** LDE ጄኔሬተር።** `lde_root` ለማግኘት ከ`lde_log_size` ጋር ተመሳሳይ አገላለጽ ይድገሙት።
4. ** ኮሴት ምርጫ።** ደረጃ 0 መሰረታዊ ንዑስ ቡድንን (`omega_coset = 1`) ይጠቀማል። የወደፊት ኮሴቶች እንደ `fastpq:v1:domain_roots:coset` ያለ ተጨማሪ መለያ ሊወስዱ ይችላሉ።
5. **የፔርሙቴሽን መጠን።** `permutation_size` በግልጽ ቀጥል ስለዚህ መርሐግብር አውጪዎች የማቀፊያ ሕጎችን ከሁለት ስውር ኃይል ፈጽሞ አይሰጡም።

### ማባዛትና ማረጋገጥ
- Tooling: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` ዝገት ቅንጣቢዎችን ወይም ማርክዳውን ሠንጠረዥን ያወጣል (`--format table`፣ `--seed`፣ `--filter` ይመልከቱ)።
- ሙከራዎች፡ `canonical_sets_meet_security_target` ቀኖናዊ መለኪያ ስብስቦችን ከታተሙ ቋሚዎች (ዜሮ ያልሆኑ ሥሮች፣ ንፋስ/አሪቲ ማጣመር፣ የፐርሙቴሽን መጠን) ጋር እንዲጣጣሙ ያቆያል፣ ስለዚህ `cargo test -p fastpq_isi` ወዲያውኑ ተንሳፋፊን ይይዛል።【crates/fastpq_isi/rsrc/src/params.】
- የእውነት ምንጭ፡ የStage0 ሰንጠረዡን እና `fastpq_isi/src/params.rs`ን አንድ ላይ ያዘምኑ አዲስ የመለኪያ ጥቅሎች በገቡ ቁጥር።

## አባሪ ሐ - የቁርጠኝነት ቧንቧ ዝርዝሮች### የPoseidon ቁርጠኝነት ፍሰት ፍሰት
ደረጃ 2 በአረጋጋጭ እና አረጋጋጭ የተካፈለውን የመወሰን ቁርጠኝነት ይገልጻል፡-
1. ** መደበኛ ሽግግሮች።** `trace::build_trace` እያንዳንዷን ባች በመደርደር ወደ `N_trace = 2^{⌈log₂ rows⌉}` በመጠቅለል የአምድ ቬክተሮችን ከታች በቅደም ተከተል ያወጣል።【crates/fastpq_prover/src/trace.rs:123】
2. ** ሃሽ አምዶች።** `trace::column_hashes` ዓምዶቹን `fastpq:v1:trace:column:<name>` በተሰየመ Poseidon2 ስፖንጅ ያሰራጫል። የ`fastpq-prover-preview` ባህሪው ገባሪ በሆነበት ጊዜ ተመሳሳዩ የመተላለፊያ መንገድ በኋለኛው የሚፈለጉትን የIFFT/LDE ኮፊሸን ይለውጣል፣ስለዚህ ምንም ተጨማሪ ማትሪክስ ቅጂዎች አልተመደቡም።【crates/fastpq_prover/src/trace.rs:474】
3. **ወደ መርክል ዛፍ ውሰዱ።** `trace::merkle_root` ልዩ ጉዳዮችን ለማስወገድ ደረጃው ያልተለመደ ደጋፊ በሚወጣበት ጊዜ የመጨረሻውን ቅጠል በፖሲዶን ኖዶች በፖሲዶን ኖዶች በማጠፍጠፍ።【crates/fastpq_prover./rsrc/
4. **መፍጨትን ጨርስ `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

አረጋጋጩ የFiat–Shamir ፈተናዎችን ከመውሰዱ በፊት ያንኑ መፈጨት እንደገና ያሰላል፣ ስለዚህ ከማናቸውም ክፍት ቦታዎች በፊት ማረጋገጫዎችን ያቋርጣል።

### የፖሲዶን ውድቀት መቆጣጠሪያዎች- ፕሮፌሰሩ አሁን የተወሰነውን የፖሲዶን ቧንቧ መሻር (`zk.fastpq.poseidon_mode`፣ env `FASTPQ_POSEIDON_MODE`፣ CLI `--fastpq-poseidon-mode`) ኦፕሬተሮች ጂፒዩ FFT/LDE ከሲፒዩ ፖሲዶን ሃሺንግ ጋር በመደባለቃቸው የመድረክ 7.900ms ኢላማ ላይ መድረስ በማይችሉ መሳሪያዎች ላይ ያጋልጣል። የሚደገፉ እሴቶች የማስፈጸሚያ-ሞድ ቁልፍን (`auto`፣ `cpu`፣ `gpu`) ያንፀባርቃሉ፣ ይህም ሳይገለጽ ከዓለም አቀፋዊ ሁኔታ ጋር ነው። የሩጫ ሰዓቱ ይህንን እሴት በሌይን ውቅረት (`FastpqPoseidonMode`) ይከራል እና ወደ prover (`Prover::canonical_with_modes`) ያሰራጫል ስለዚህ መሻሪያዎች በውቅረት ውስጥ የሚወሰኑ እና ሊመረመሩ የሚችሉ ናቸው። ቆሻሻዎች።【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- ቴሌሜትሪ የተፈታውን የቧንቧ መስመር ሁኔታ በአዲሱ `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` ቆጣሪ (እና OTLP መንትያ `fastpq.poseidon_pipeline_resolutions_total`) ወደ ውጭ ይልካል። ስለዚህ `sorafs`/ኦፕሬተር ዳሽቦርዶች መልቀቅ በጂፒዩ ውህድ/በፓይፕላይድ ሀሺንግ ከግዳጅ የሲፒዩ ውድቀት (`path="cpu_forced"`) ወይም የሩጫ ጊዜ ዝቅታዎች (`path="cpu_fallback"`) ሲሄድ ማረጋገጥ ይችላሉ። የCLI ፍተሻ በ`irohad` ውስጥ በራስ-ሰር ይጫናል፣ስለዚህ ጥቅሎችን እና የቀጥታ ቴሌሜትሪዎችን መልቀቅ ተመሳሳይ የማስረጃ ዥረት ይጋራሉ።【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- የድብልቅ ሁነታ ማስረጃዎች በነባሩ የማደጎ በር በኩል በእያንዳንዱ የውጤት ሰሌዳ ላይ ታትመዋል፡- ማረጋገጫው ለእያንዳንዱ ባች የተፈታ ሁነታን + የመንገድ መለያን ያወጣል እና ማረጋገጫው በደረሰ ቁጥር `fastpq_poseidon_pipeline_total` ቆጣሪ ይጨምራል። ይህ ቡኒ መውጣቶች እንዲታዩ በማድረግ እና ማመቻቸት በሚቀጥልበት ጊዜ ንፁህ ማብሪያ / ማጥፊያን በማቅረብ WP2-E.6ን ያረካል።【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` አሁን Prometheus ቧጨራዎችን (ሜታል ወይም CUDA) ይተነትናል እና የ`poseidon_metrics` ማጠቃለያ በእያንዳንዱ የተጠቀለለ ጥቅል ውስጥ አካቷል። ረዳቱ የቆጣሪ ረድፎችን በ`metadata.labels.device_class` ያጣራል፣ ተዛማጅ የ`fastpq_execution_mode_total` ናሙናዎችን ይይዛል እና የ`fastpq_poseidon_pipeline_total` ግቤቶች ሲጎድሉ መጠቅለሉ ስለማይሳካ WP2-E.6 ቅርቅቦች ሁል ጊዜ ሊባዙ የሚችሉ CUDA/Metal ማስረጃዎችን ይላካሉ። ማስታወሻዎች።【ስክሪፕቶች/fastpq/wrap_benchmark.py:1】【ስክሪፕቶች/fastpq/tests/test_wrap_benchmark.py:1】

#### ቆራጥ ድብልቅ ሁነታ ፖሊሲ (WP2-E.6)1. **የጂፒዩ እጥረትን እወቅ።** Stage7 ቀረጻ ወይም ቀጥታ ስርጭት Grafana ቅጽበተ ፎቶ የሚያሳየው የPoseidon መዘግየት አጠቃላይ የማረጋገጫ ሰዓቱን>900 ሚ.ኤስ FFT/LDE ከዒላማ በታች ሲቆይ ይጠቁሙ። ኦፕሬተሮች የቀረጻውን ማትሪክስ (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) ያብራራሉ እና `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` ሲዘገይ `fastpq_execution_mode_total{backend="metal"}` አሁንም ጂፒዩ FFT/LDE ይመዘግባል መላኪያዎች።【ስክሪፕቶች/fastpq/wrap_benchmark.py:1】【ዳሽቦርዶች/grafana/fastpq_acceleration.json:1】
2. ** ለተጎዱ አስተናጋጆች ብቻ ወደ ሲፒዩ ፖሲዶን ገልብጡ።** `zk.fastpq.poseidon_mode = "cpu"` (ወይም `FASTPQ_POSEIDON_MODE=cpu`) በአስተናጋጅ-አካባቢው ውቅረት ውስጥ ከበረራ መለያዎች ጋር ያዋቅሩ፣ `zk.fastpq.execution_mode = "gpu"` ን በማቆየት FFT/LDE አፋጣኝ መጠቀሙን ይቀጥላል። ገምጋሚዎች ለውጡን በቁርጠኝነት እንደገና ማጫወት እንዲችሉ የውቅረት ልዩነትን በታቀደው ትኬቱ ​​ውስጥ ይቅዱ እና የአስተናጋጁ መሻርን ወደ ቅርቅቡ እንደ `poseidon_fallback.patch` ያክሉ።
3. ** ማሽቆልቆሉን ያረጋግጡ።** መስቀለኛ መንገዱን እንደገና ከጀመሩ በኋላ ወዲያውኑ የፖሲዶን ቆጣሪውን ያፅዱ፡-
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   ቆሻሻው `path="cpu_forced"` በመቆለፊያ ደረጃ እያደገ ከጂፒዩ ማስፈጸሚያ ቆጣሪ ጋር ማሳየት አለበት። ከነባሩ `metrics_cpu_fallback.prom` ቅጽበታዊ ገጽ እይታ አጠገብ እንደ `metrics_poseidon.prom` ጥራጊውን ያከማቹ እና ተዛማጅ `telemetry::fastpq.poseidon` የምዝግብ ማስታወሻ መስመሮችን በ `poseidon_fallback.log` ውስጥ ይቅረጹ።
4. ** ተቆጣጠር እና ውጣ።** የማመቻቸት ስራ በሚቀጥልበት ጊዜ በ`fastpq_poseidon_pipeline_total{path="cpu_forced"}` ላይ ማንቂያዎን ይቀጥሉ። አንዴ patch የየማስረጃ ጊዜውን በሙከራ አስተናጋጁ ላይ በ900ms ስር ካመጣ በኋላ አወቃቀሩን ወደ `auto` ያንከባልልልናል፣መቧጨሩን እንደገና ያስኪዱ (በድጋሚ `path="gpu"` ያሳያል) እና የድብልቅ ሞድ መሰርሰሪያውን ለመዝጋት ቀዳሚ/በኋላ ሜትሪክስን ያያይዙ።

**የቴሌሜትሪ ውል::

| ሲግናል | PromQL / ምንጭ | ዓላማ |
|--------|-------|-------|
| Poseidon ሁነታ ቆጣሪ | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | የሲፒዩ ሃሺንግ ሆን ተብሎ እና በተጠቆመው የመሳሪያ ክፍል የተዘረጋ መሆኑን ያረጋግጣል። |
| የማስፈጸሚያ ሁነታ ቆጣሪ | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | ኤፍኤፍቲ/ኤልዲኢ አሁንም በጂፒዩ እንደሚሰራ ያረጋግጣል ፖሲዶን ደረጃውን ስታወርድም። |
| የምዝግብ ማስታወሻ | `telemetry::fastpq.poseidon` ግቤቶች `poseidon_fallback.log` ውስጥ ተይዘዋል | አስተናጋጁ በምክንያት `cpu_forced` ሲፒዩ ሃሺንግ መፍታት እንደቻለ በየማስረጃ ያቀርባል። |

የልቀት ቅርቅቡ አሁን `metrics_poseidon.prom`፣ የውቅረት ልዩነት እና ቅይጥ ሁነታ ንቁ በሆነ ቁጥር የምዝግብ ማስታወሻው ማካተት አለበት ስለዚህ አስተዳደር ከFFT/LDE ቴሌሜትሪ ጎን ለጎን የሚወስን የውድቀት ፖሊሲን ኦዲት ማድረግ ይችላል። `ci/check_fastpq_rollout.sh` ወረፋውን/ ዜሮ መሙላትን አስቀድሞ ያስፈጽማል; ተከታዩ በር ንጽህና ይሆናል - Poseidon ቆጣሪ አንዴ ድብልቅ ሁነታ መሬቶች ልቀት አውቶማቲክ ውስጥ.

የStage7 ቀረጻ መሳሪያ አስቀድሞ CUDAን ይይዛል፡ እያንዳንዱን የ`fastpq_cuda_bench` ጥቅል ከ`--poseidon-metrics` ጋር (የተፋቀውን `metrics_poseidon.prom` በመጠቆም) እና ውጤቱ አሁን በብረት ላይ ጥቅም ላይ የዋለውን የቧንቧ መስመር ቆጣሪ/የመፍትሄ ማጠቃለያ ይይዛል ስለዚህ አስተዳደር ያለ CUDA ውድቀት ሊረጋገጥ ይችላል መሳሪያ ማድረግ።【ስክሪፕቶች/fastpq/wrap_benchmark.py:1】### የአምድ ቅደም ተከተል
የሃሺንግ ቧንቧ መስመር በዚህ የመወሰኛ ቅደም ተከተል አምዶችን ይበላል፡
1. መራጭ ባንዲራዎች፡- `s_active`፣ `s_transfer`፣ `s_mint`፣ `s_burn`፣ `s_role_grant`፣ `s_role_revoke`፣ I10800 `s_perm`.
2. የታሸጉ የእጅ አንጓዎች አምዶች (እያንዳንዱ ዜሮ-የተጣበቀ በክትትል ርዝመት): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. ረዳት ሚዛኖች፡- `delta`፣ `running_asset_delta`፣ `metadata_hash`፣ `supply_counter`፣ `perm_hash`፣ Prometheus፣ Prometheus `slot`.
4. Sparse Merkle ምስክሮች ለእያንዳንዱ ደረጃ `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` አምዶቹን በትክክል በዚህ ቅደም ተከተል ይራመዳል፣ ስለዚህ የቦታ ያዢው የኋላ ክፍል እና የStage2 STARK ትግበራ በሁሉም ልቀቶች ላይ ተረጋግተው ይቆያሉ።【crates/fastpq_prover/src/trace.rs:474】

### የጎራ መለያዎች ግልባጭ
ደረጃ 2 ፈታኝ ትውልድን ለመወሰን ከዚህ በታች ያለውን የFiat–Shamir ካታሎግ ያስተካክላል፡-

| መለያ | ዓላማ |
| --- | ------- |
| `fastpq:v1:init` | የፕሮቶኮል ሥሪትን፣ የመለኪያ ስብስብን እና `PublicIO`ን ይምጡ። |
| `fastpq:v1:roots` | ፈለጉን ይወስኑ እና የ Merkle ሥሮችን ይፈልጉ። |
| `fastpq:v1:gamma` | የፍለጋ ግራንድ-ምርት ፈተናን ናሙና። |
| `fastpq:v1:alpha:<i>` | የናሙና ቅንብር-ፖሊኖሚል ተግዳሮቶች (`i = 0, 1`)። |
| `fastpq:v1:lookup:product` | የተገመገመውን ፍለጋ ታላቅ ምርት ይምጡ። |
| `fastpq:v1:beta:<round>` | ለእያንዳንዱ የFRI ዙር የመታጠፍ ፈተናን ናሙና። |
| `fastpq:v1:fri_layer:<round>` | ለእያንዳንዱ የFRI ንብርብር የ Merkle ሥርን ይሥሩ። |
| `fastpq:v1:fri:final` | መጠይቆችን ከመክፈትዎ በፊት የመጨረሻውን የFRI ንብርብር ይቅረጹ። |
| `fastpq:v1:query_index:0` | አረጋጋጭ መጠይቅ ኢንዴክሶችን በቆራጥነት ያውጡ። |