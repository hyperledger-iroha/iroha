"""Structural parser for the Vega-MC proof object ``VegaMcZkSNARK``.

This mirrors, field for field, the byte layout fixed in
``book/src/spec/proof-object.md``. Each ``parse_*`` function consumes exactly the
bytes of one Rust type, so parsing the whole proof and finding the cursor exactly
at end-of-input is a strong structural conformance check.

Encoding primitives come from :mod:`pyvega.codec`; scalars from
:mod:`pyvega.field`; compressed points from :mod:`pyvega.curve`.

A ``Commitment`` is a single-field struct wrapping ``Vec<GE>``, so on the wire it
is just a length-prefixed list of compressed points.
"""

from dataclasses import dataclass
from typing import List, Optional, Tuple

from .codec import Reader
from .field import read_scalar
from .curve import read_point, WirePoint

Commitment = List[WirePoint]  # single-field struct: Vec<GE>


def parse_commitment(r: Reader) -> Commitment:
  return r.vec(read_point)


@dataclass
class SplitR1CSInstance:
  comm_W_shared: Optional[Commitment]
  comm_W_precommitted: Optional[Commitment]
  comm_W_rest: Commitment
  public_values: List[int]
  challenges: List[int]


def parse_split_r1cs_instance(r: Reader) -> SplitR1CSInstance:
  return SplitR1CSInstance(
    comm_W_shared=r.option(parse_commitment),
    comm_W_precommitted=r.option(parse_commitment),
    comm_W_rest=parse_commitment(r),
    public_values=r.vec(read_scalar),
    challenges=r.vec(read_scalar),
  )


@dataclass
class SplitMultiRoundR1CSInstance:
  comm_w_per_round: List[Commitment]
  public_values: List[int]
  challenges_per_round: List[List[int]]


def parse_split_multiround_instance(r: Reader) -> SplitMultiRoundR1CSInstance:
  return SplitMultiRoundR1CSInstance(
    comm_w_per_round=r.vec(parse_commitment),
    public_values=r.vec(read_scalar),
    challenges_per_round=r.vec(lambda rr: rr.vec(read_scalar)),
  )


@dataclass
class RelaxedR1CSInstance:
  comm_W: Commitment
  comm_E: Commitment
  X: List[int]
  u: int


def parse_relaxed_instance(r: Reader) -> RelaxedR1CSInstance:
  return RelaxedR1CSInstance(
    comm_W=parse_commitment(r),
    comm_E=parse_commitment(r),
    X=r.vec(read_scalar),
    u=read_scalar(r),
  )


@dataclass
class NovaNIFS:
  comm_T: Commitment


def parse_nova_nifs(r: Reader) -> NovaNIFS:
  return NovaNIFS(comm_T=parse_commitment(r))


@dataclass
class InnerProductArgumentLinear:
  delta: WirePoint
  beta: WirePoint
  z_vec: List[int]
  z_delta: int
  z_beta: int


def parse_ipa(r: Reader) -> InnerProductArgumentLinear:
  return InnerProductArgumentLinear(
    delta=read_point(r),
    beta=read_point(r),
    z_vec=r.vec(read_scalar),
    z_delta=read_scalar(r),
    z_beta=read_scalar(r),
  )


@dataclass
class HyraxEvaluationArgument:
  ipa: InnerProductArgumentLinear  # single-field struct


def parse_hyrax_eval_arg(r: Reader) -> HyraxEvaluationArgument:
  return HyraxEvaluationArgument(ipa=parse_ipa(r))


  # SumcheckProof and CompressedUniPoly are both single-field structs wrapping a
  # Vec, so each is just a length-prefixed list on the wire.
CompressedUniPoly = List[int]  # coeffs_except_linear_term


def parse_compressed_uni_poly(r: Reader) -> CompressedUniPoly:
  return r.vec(read_scalar)


SumcheckProof = List[CompressedUniPoly]  # compressed_polys


def parse_sumcheck_proof(r: Reader) -> SumcheckProof:
  return r.vec(parse_compressed_uni_poly)


@dataclass
class RelaxedR1CSSpartanProof:
  sc_proof_outer: SumcheckProof
  claims_outer: Tuple[int, int, int]
  sc_proof_inner: SumcheckProof
  v_W: List[int]
  blind_W: int
  v_E: List[int]
  blind_E: int


def parse_relaxed_spartan_proof(r: Reader) -> RelaxedR1CSSpartanProof:
  return RelaxedR1CSSpartanProof(
    sc_proof_outer=parse_sumcheck_proof(r),
    claims_outer=r.tuple(read_scalar, read_scalar, read_scalar),
    sc_proof_inner=parse_sumcheck_proof(r),
    v_W=r.vec(read_scalar),
    blind_W=read_scalar(r),
    v_E=r.vec(read_scalar),
    blind_E=read_scalar(r),
  )


@dataclass
class VegaMcZkSNARK:
  comm_W_shared: Optional[Commitment]
  step_instances: List[SplitR1CSInstance]
  core_instance: SplitR1CSInstance
  eval_arg: HyraxEvaluationArgument
  U_verifier: SplitMultiRoundR1CSInstance
  nifs: NovaNIFS
  random_U: RelaxedR1CSInstance
  relaxed_snark: RelaxedR1CSSpartanProof


def parse_proof(r: Reader) -> VegaMcZkSNARK:
  return VegaMcZkSNARK(
    comm_W_shared=r.option(parse_commitment),
    step_instances=r.vec(parse_split_r1cs_instance),
    core_instance=parse_split_r1cs_instance(r),
    eval_arg=parse_hyrax_eval_arg(r),
    U_verifier=parse_split_multiround_instance(r),
    nifs=parse_nova_nifs(r),
    random_U=parse_relaxed_instance(r),
    relaxed_snark=parse_relaxed_spartan_proof(r),
  )


def load_proof(data: bytes) -> VegaMcZkSNARK:
  """Parse a proof from bytes and assert the whole buffer was consumed."""
  r = Reader(data)
  proof = parse_proof(r)
  r.expect_end()
  return proof
