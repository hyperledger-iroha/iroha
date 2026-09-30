# FASTPQ offline wire budget owner review

The source-size policy audit identified no code line-count gate in this owner. Its source-derived canonical Norito byte geometry, fixed first-release proof ceilings, diagnostic classification and production resource preflight remain enforced.

The current fixed `FriValues` enum serializes its arity through the exhaustive `arity_byte` projection: Four → 4, Eight → 8 and Sixteen → 16. This produces the same single leading byte followed by exactly that many 32-byte extension values. The previous guard still expected `self.len() as u8`. The guard now checks the exact projection and single-byte writer, with tag-map and tag-writer refusal controls added to every existing geometry, decoder, encoder and producer-bound mutation.

Exact previous guard/test bytes and the reviewed canonical codec are preserved in [the preimage inventory](preimages.json). Current codec unit tests check exact raw bytes under every Norito layout and reject malformed tags, lengths, retired vector framing and noncanonical limbs. No Rust codec, wire bytes, resource ceilings or qualification claim changed during this repair.
