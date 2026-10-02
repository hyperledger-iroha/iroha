# X509 joint original-polynomial relation

This source contract describes the first-release MAIN/CA continuation in
`crates/iroha_core_privacy/src/privacy_engines/zk_x509`. The required nonce-scoped
framing needs current native controls, authentic profile fixtures and a complete
maximum proof with fresh verification. Earlier source12 evidence predates this
framing and failed time and literal address-space limits. Independent relation,
transcript and zero-knowledge qualification remain required; activation stays
unavailable. Current evidence is recorded in [the goal tracker](zk_first_release_goals.md).

The credential has no public intermediate terminal products. MAIN uses the local
SHA relation with 564 constraints and the local RFC relation with 1,650
constraints. CA uses 1,363 local constraints. The full reference evaluators remain
algebraic test oracles; they are not independent proof acceptance paths.
The omitted terminal equations are replaced by 108 quotients in the original
MAIN composition. Those quotients bind all thirteen SHA input/digest endpoints
and the RFC governed-root consumer to the original masked CA auxiliary columns.
They use multiplication identities without division by a private bus product.

The consuming protocol stages enforce this order:

1. After successful preflight, sample the required public 32-byte nonce from the
   existing checked RNG, before any mask or base commitment. Bind original MAIN and CA base roots through the shared X5B1 pre-auxiliary
   challenge schedule, then commit both original auxiliary roots.
2. Bind both auxiliary roots before the active local and cross-relation
   composition coefficients. Construct the original MAIN composition including
   the 108 cross quotients and the original CA local composition.
3. Bind both transcript checkpoints, both composition roots and both independent
   FRI-mask roots before deriving one point admissible for every original
   polynomial, native translation and key/digest power map.
4. Bind both current/next local DEEP records and MAIN's 31 key/digest values.
   Bind the canonical ordered 24 MAIN and 108 CA original auxiliary values into
   both local transcripts before sampling either local DEEP mix.
5. Include the 24 and 108 supplemental divided differences in each respective
   original FRI relation. Both joint verifiers must finish successfully before
   credential acceptance. The producer self-verifies the complete credential.

The canonical version-one envelope remains exact and rejects trailing bytes,
truncations, noncanonical Fp4 coordinates and retired terminal frames. X5M1 is
4 magic bytes, 2 version bytes, 31 Fp4 values and one 4-byte inner length: 1,002
framing bytes. X5C1 has 4 magic bytes, 2 version bytes and one 4-byte inner
length: 10 framing bytes. X5S1 has 108 public-header bytes, 132 Fp4 values
(4,224 bytes) and two 8-byte section headers: 4,348 framing bytes. The header is
`X5S1 || version:u16be || count:u16be || nonce[32] || consensus_context[32] ||
governed_ca_root[32] || channel:u32be`. MAIN and CA inner maxima remain 7,908,768
and 1,498,816 bytes, yielding 9,412,944 bytes overall with 24,240 bytes remaining
under the unchanged 9,437,184-byte cap. The partitioned MAIN section cap is
7,934,010 bytes; CA keeps its original independent cap. No legacy decoder is
retained.

CA pads 104 active rows to 4,096 native rows (log 12), uses a log-16 LDE and
2,100 hiding coefficients. MAIN retains its six native groups, original masks,
joined roots and log-22 LDE. Both retain 136 distinct post-grinding queries,
20 grinding bits, the existing FRI caps and 137 independent adjacent Fp4
composition-mask coefficients. Mask-count arithmetic alone does not establish
joint hiding or transcript simulation.

The public preallocation forecast narrows each ordinary MAIN quotient cache
before admission. It reserves the retained original CA coefficients and metadata,
charges the late MAIN24 owner only after ordinary registration caches are dropped,
and reserves the 640 MiB CA working ceiling. Actual allocation capacities, Merkle
trees and FRI owners are checked in addition to this forecast. Auxiliary replay
holds private CA boundary products under clearing owners across errors and
unwinding; boundary storage is reserved before private copies are written.

Validation order is compilation, genuine compiled-profile field capture and
independent digest recomputation, native profile pin adoption, genuine IO and
projection proof capture, native control/mutation tests, then an isolated maximum
credential proof and fresh verification. Capture prints are observations, not
permission to update a pin without reviewing the changed protocol. The maintained
`scripts/check_zk_x509_proof_geometry.py` checks source-derived framing and bounds.
Neither source geometry nor CA component proofs establish full credential
soundness, zero knowledge, 300-second performance, 12 GiB RSS or literal 32 GiB
address-space compliance.

## Candidate interactive algebraic ledger

The following argument is under independent review. It does not install a soundness certificate or establish the round-by-round/Fiat–Shamir lift. The activation pin remains zero.

Primary references: Haböck, *A summary on the FRI low degree test*, December 17 2024, https://eprint.iacr.org/2022/1216.pdf, Theorem 2/Remark 3 for affine FRI and Section 5.2 for the DEEP-ALI configuration method; Ben-Sasson et al., *DEEP-FRI*, https://arxiv.org/abs/1903.12243. The former theorem uses L−1/2, replaced by 3/2 for affine batching. Its reduction arities are those of every FRI fold. These references motivate the proof method; the mixed-domain and power-map argument below is a new application under review, not a theorem quoted from either source.

## Actual stages and distributions

One immutable public proof instance scopes every dynamic commitment and transcript
hash. Its effective length-prefixed profile label is
`base_profile || b"\0X5I1" || family:u8 || nonce[32]`, with verifier-fixed
`Joint=0`, `MAIN=1` and `CA=2`; a proof cannot supply a family tag. Static compiled
profile and fixed-schedule identities keep their deterministic unscoped frames.
The nonce is separate from the statement-derived public binding, so changing it
leaves that binding unchanged but changes the dynamic verification context.
All 32-byte values, including zero, are canonical. There is no absent-nonce
fallback. Honest generation consumes 32 disjoint checked-stream bytes before
any masks, preserves the RNG's existing health-failure coupling, and never
serializes or replays private entropy. Original phase owners retain and compare
the instance; CA receives it from the original MAIN pre-auxiliary token.

This framing separates distinct honest nonce/family functions in the concrete
multi-target mask search. It adds no secret entropy to a disclosed leaf, does not
prevent malicious nonce reuse or a failed RNG, and does not establish a general
quantum random-oracle privacy theorem. Earlier transcript and zero-knowledge
arguments require review against this changed source and framing.

Both base roots precede the shared bus challenges. Both original auxiliary roots precede all active local/link alphas. Both local checkpoints and both composition/FRI-mask roots precede the shared z. Both complete local DEEP records, MAIN31 key/digest values and all canonical MAIN24/CA108 values precede either local batching vector. Engine acceptance requires both original FRI verifiers to finish.

Every Fp4 alpha/mix uses four canonical big-endian u64 coefficients from a uniform 384-bit ideal-oracle response, with rejection instead of reduction; zero is allowed. Attempt identifiers distinguish each of at most sixteen draws. For z the predicate is deterministic and public at the sampling stage. Conditional on success the accepted value is uniform on the admitted set. Exhaustion rejects. This local distribution statement does not treat adaptive Fiat–Shamir calls as fresh interactive coins.

The rejected z count is at most E=1+693*(2^19+2^22)+110*(2^12+2^16)=3,277,643,777. MAIN native subgroups and all 24 translated MAIN multipliers lie in H19, preserving both H19 and generator·H22. The original point and six power2, five power8 and twenty power32 maps have summed degree693. A nonzero scale times X^k has at most k preimages of any forbidden point. CA has two local and 108 translated linear maps. Collisions only reduce the union. SDK independent review confirmed this census. Therefore the sampling support has size at least |Fp4|−E.

## Restored degree and paired configurations

The original local affine batch contains each current and next divided difference, every composition divided difference, and every supplemental divided difference; the independent mask oracle is its affine offset. MAIN has 5,811 original columns, six composition chunks and 55 extra values (31+24), yielding 11,683 independently weighted functions. CA has 823 original columns, four composition chunks and 108 extras, yielding 1,758 independently weighted functions. The mask is one affine offset per batch. These are independent weights, not powers of one batching scalar. The affine FRI commitment term accounts for this batch without a width-dependent scalar-power factor; adding a separate 1/|F| per extra opening would double-count that batching step.

The accepted FRI coefficient caps are K_MAIN=144*2^12=589,824 and K_CA=144*2^6=9,216. A candidate divided-difference polynomial has degree at most K−1; multiplying its linear denominator and adding its claim restores a polynomial of degree at most K. This is the adversarial recovered degree, not the smaller honest masked-column degree. The actual query evaluator uses the same original f(x) value for both (f(x)−v_current)/(x−z) and (f(x)−v_next)/(x−omega*z), and for every supplemental denominator. It does not query an independently shifted f(omega*x) codeword. On the single common correlated-agreement set, all restored representatives of a given original column therefore coincide; no intersection of shifted agreement sets is assumed. Since that set has more than K points, their differences vanish identically, including repeated or coinciding supplemental points. All fields of each original trace/composition configuration are thus consistent. An original trace codeword is Fp-valued; its recovered polynomial is also over Fp because coefficientwise Frobenius difference vanishes on more than K base-field points.

For each domain, take agreement a>7/16 and pairwise intersection bound r=K/N=9/64. For L distinct complete vector configurations, any pair agrees on at most K points (choose one differing component). L=1 is trivial. For L>=2, the normalized incidence mean is at least L*a>=7/8, where the function mu^2−mu is increasing, so replacing the mean by its lower bound in the next inequality is valid. If s_x counts configurations agreeing with the oracle vector at x, Cauchy gives sum s_x(s_x−1) >= N*(L^2*a^2−L*a), whereas pairwise intersection gives <=L(L−1)K. Hence L <= (a−r)/(a^2−r). At a=7/16, r=9/64 this is76/13<6. Using eight per domain and64 pairs is conservative. This is a list of complete jointly agreeing vector configurations, not independent choices per column. If using only the coarser r<=1/7, the ratio is <=528/87≈6.07, still below eight; the '<6' statement specifically uses9/64.

For fixed base oracles, the projected base configurations also form such lists before bus challenges. For fixed base+aux oracles, their paired configurations form such lists before alphas. For fixed base+aux+composition+mask oracles, they form such lists before z. Taking Cartesian products of the two local lists gives at most 64 candidate pairs in each stage; it does not require the two query sets or their agreement sets to intersect. The common z imposes the cross identity on candidate polynomials, not on intersecting native domains. The lists here are the entire lists defined by the already committed oracle words, never chosen post-challenge extractions. Immediately after paired openings define Bad_MAIN and Bad_CA as the fixed predicates that the respective batch lacks correlated agreement. At every prefix, each local FRI theorem bounds Pr[joint acceptance AND Bad_i] by its local error. For CA, condition additionally on the completed MAIN transcript, which supplies no future CA verifier coins. Adding these two bounds neither conditions on eventual acceptance nor assumes independent acceptance events. Outside these bad events, the restoration/projection argument applies to both domains. Independent review found no contradiction in this fresh-coin interactive argument; it supplies no round-by-round or Fiat–Shamir lift.

## Explicit degree census

Let D=589,824 and C=9,216 be the restored degree bounds. All public fixed polynomial degrees are below their native row count, hence below D/C. All translations below are nonzero constants from native root groups.

| Contribution | Actual form | Numerator degree bound | Divisor roots |
|---|---|---:|---|
|49 MAIN local AIR registrations|fixed-aware degree at most7, only X and native-root*X|7D|native subgroup in H19|
|192 terminal links|f(X)−g(X), or f(X)−1|D|one point in H19|
|12 key blocks|f(X)−g(scale*X^k), k=1,2,8|8D|at most65 distinct points in H19|
|5 digest blocks|f(X)−sum of four public weights*g(scale_j*X^32)|32D|eight distinct points in H19|
|20 SHA union links|difference of products with at most four unpowered factors|4D|one point in H19|
|108 CA cross links|f_MAIN(X)−public*g_MAIN(gamma*X)*h_CA(eta*X)|D+C|one point in H19|
|MAIN six-chunk recomposition|sum X^(j*(D−137))*q_j(X)|D+5(D−137)|none|
|CA local AIR|fixed-aware degree at most3|3C|H12|
|CA four-chunk recomposition|sum X^(j*(C−137))*q_j(X)|C+3(C−137)|none|

No power32 evaluation occurs inside a product. Every active MAIN divisor is square-free and divides V_MAIN=X^(2^19)−1; the CA divisor is V_CA=X^(2^12)−1. Consequently conservative cleared relation degrees are B_MAIN=32D+2^19=19,398,656 and B_CA=4C+2^12=40,960. Honest cross quotient degree 532,297 is deliberately not used as an adversarial extraction bound.

For a fixed invalid paired configuration, an active independent alpha corresponding to a nonzero residue makes cancellation modulo V possible: for its active divisor d, the polynomial N*(V/d) is nonzero modulo V because d is square-free and divides the square-free V. Cancellation is possible for at most one field value after the other alphas are fixed. Union over at most 64 pairs gives 64/|F|. If that event does not occur, at least one cleared relation is nonzero. Its shared-z root event is bounded by 64*(B_MAIN+B_CA)/(|F|−E). No independence between the two relation checks is assumed. Using |F|>2^252, E<2^64, and64*(B_MAIN+B_CA)=1,244,135,424<2^31 gives alpha<2^-246 and point<2^-220; their sum is<2^-219.

## Error accounting

Base configuration selection can depend on later bus challenges, so apply the same factor 64 to the existing RFC/P256/SHA collision bounds. Keep each SHA term separate before exact dyadic addition: two memory terms<2^-172, one call term<2^-200, one base-fold term<2^-165. Their sum is<2^-164; after the factor 64 it is<2^-158. RFC becomes164 bits and P256165 bits.

The actual complete fold schedules are MAIN twelve binary folds (sum24) and CA six (sum12). At rate>=1/8, 7/sqrt(rho)*sum(arities)<7*3*24=504<512. Therefore the existing loose FRI terms remain valid: MAIN first<2^-188, second<2^-220; CA combined<2^-199. The new joint<2^-219 term fits with the first two inside MAIN's existing<2^-187 allowance. The final seven exponents are [160,187,160,199,164,165,158]. Their exact dyadic sum is below 2^-157. A minimum-exponent-minus-log(number-of-terms) estimate would unnecessarily lose precision; the held checker performs checked integer addition instead.

This ledger is a candidate interactive algebraic argument. Installing a nonzero certificate pin still requires independent acceptance of the mixed-domain conditional argument and actual round-by-round/ROM proof; none is supplied by native tests or these integer inequalities. The separate joint zero-knowledge proof must compose per-column mask closure,137-coefficient adjacent chunk masks, independent FRI-mask decoupling, adaptive transcript simulation and failure behavior.
