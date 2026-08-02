"""pyvega -- an executable reference specification of the Vega-MC proof system.

This package is a small, unoptimized second implementation of the
Vega-MC prover and verifier for the canonical engine (T256 curve, Keccak-256
transcript, Hyrax PCS). It exists to *define* the protocol precisely and to
cross-conform against the Rust library: proofs it accepts are exactly the proofs
the Rust verifier accepts, and vice versa.

Layout:
  params      T256 curve and field integer constants
  codec       bincode little-endian / fixint decoder
  field       scalar / base field byte encodings
  curve       33-byte compressed group-element (de)compression
  proof       structural parser for the VegaMcZkSNARK proof object
"""
