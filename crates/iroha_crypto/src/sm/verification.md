# SM2 verification ownership

Compact SM2 keys contain the exact UTF-8 distinguishing identifier and canonical
uncompressed SEC1 point. `verification::BorrowedKey` borrows that envelope and
keeps the parsed point and 32-byte identity hash inline. Compact verification,
including typed signatures and admission, never copies the identifier into an
owned verifying key or consults the generic decoded-key cache.

Explicit `Sm2PublicKey` values retain their owned identifier. Both representations
use `verification::verify`: canonical nonzero r/s parsing, SM3 over ZA and the
message, nonzero r+s, the existing fixed point combination, and scalar reduction.
Signing and public key/signature wire encodings are unchanged. OpenSSL selection
for SM3/SM4 does not change this SM2 relation.

The bounded parser reports fixed internal reasons. Public ParseError adapters
still allocate their original diagnostic String. Neither those errors nor an
explicit owned identifier are advertised as funded State-pool allocations.
Borrowed verification does not itself establish complete State-reader custody.

Required controls cover empty, Unicode and maximum-length identities; exact
canonical bytes; malformed-key diagnostic precedence; positive shared vectors;
unsupported alternate-curve rejection; nonzero scalars whose sum is zero; all
message/hash mutations; and zero-allocation cold/repeated ordinary, typed and
admission verification. Differential tests use the pinned native primitive only
as a test oracle; there is one production verification relation.

The combined verification point must be finite. The B6/B7 relation uses affine
coordinates; infinity has no such coordinates. The Iroha owner explicitly
rejects the identity before requesting x. This is an intentional correctness
change from pinned `sm2` 0.13.3, whose `primeorder` 0.13.6 identity sentinel
exposes x=0. For a known key d, r=e and s=−r·d/(1+d) produce nonzero r, s, and
t while the combined point is infinity. The regression proves that geometry
and requires owned, borrowed, ordinary, and admission verification to reject.
This construction uses a known private key; it is an invalid-signature
acceptance counterexample, not evidence of forgery against unknown keys.

The finite-point requirement follows from the affine-coordinate operation in
[the published SM2 algorithm, B6/B7](https://datatracker.ietf.org/doc/html/draft-shen-sm2-ecdsa-02#section-5.3.1).
The draft does not spell out a separate infinity check; this is the domain of
that operation. [OpenSSL 3.6 SM2 verification](https://github.com/openssl/openssl/blob/openssl-3.6.0/crypto/sm2/sm2_sign.c#L367)
rejects failed affine-coordinate extraction before comparing r. Pinned
primitive mutation-agreement tests cover their named inputs only and do not
make the dependency a correctness oracle for all inputs.
