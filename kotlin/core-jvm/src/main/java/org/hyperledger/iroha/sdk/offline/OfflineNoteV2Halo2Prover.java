package org.hyperledger.iroha.sdk.offline;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.sdk.norito.NoritoCodec;
import org.hyperledger.iroha.sdk.norito.NoritoEncoder;
import org.hyperledger.iroha.sdk.norito.NoritoHeader;
import org.hyperledger.iroha.sdk.norito.TypeAdapter;

/** Pure JVM Halo2/IPA prover for Offline Note V2. */
public final class OfflineNoteV2Halo2Prover {
  public static final String CIRCUIT_ID = OfflineNoteV2.RECURSIVE_VERIFIER_NAME;
  public static final int BACKEND_TAG = 0;
  public static final int IPA_K = 7;
  public static final int MAX_ENVELOPE_BYTES = 20 * 1024;

  private static final int PROOF_DEGREE = 6;
  private static final int BLINDING_FACTORS = 5;
  private static final int PUBLIC_VALUE_COUNT = 16;
  private static final int ADVICE_COUNT = 22;
  private static final byte[] CANONICAL_VK_HASH =
      hexBytes("4736be739f171bad842a749930347abd1124c7e6f63ba6976eeb0d491aff3e1d");
  private static final byte[] CANONICAL_TRANSCRIPT_REPR =
      hexBytes("7db2235914292d4e825d6d51e1a880da77f107eb2c7853e3ec9c9d0dccc59813");
  private static final SecureRandom RNG = new SecureRandom();

  private OfflineNoteV2Halo2Prover() {}

  public static void prewarm() {
    CONTEXT.requireReady();
  }

  public static OfflineNoteV2.RecursiveProofV2 proveRedeem(
      final OfflineNoteV2.RedeemV2 redemption) {
    return prove(OfflineNoteV2.InstanceBuilder.redeemInstanceValues(redemption));
  }

  public static OfflineNoteV2.RecursiveProofV2 proveAudit(
      final OfflineNoteV2.AuditBundleV2 audit) {
    return prove(OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit));
  }

  public static OfflineNoteV2.RecursiveProofV2 prove(final OfflineNoteV2.InstanceValues values) {
    final byte[] payload = proveZk1Payload(values);
    final byte[] envelope = openVerifyEnvelope(payload);
    if (envelope.length > MAX_ENVELOPE_BYTES) {
      throw new IllegalArgumentException("Offline V2 proof envelope exceeds QR budget: " + envelope.length);
    }
    return new OfflineNoteV2.RecursiveProofV2(
        publicInputsHash(values.publicValues()),
        new OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, envelope));
  }

  public static byte[] proveZk1Payload(final OfflineNoteV2.InstanceValues values) {
    final Context context = CONTEXT.requireReady();
    final Params params = context.params;
    final Domain domain = context.domain;
    final F[] publicScalars = fp(values.publicValues());
    final F[] inputScalars = fp(values.inputAmounts());
    final F[] outputScalars = fp(values.outputAmounts());
    if (publicScalars.length != PUBLIC_VALUE_COUNT || inputScalars.length != 4 || outputScalars.length != 2) {
      throw new IllegalArgumentException("invalid Offline V2 instance values");
    }

    final WriteTranscript transcript = new WriteTranscript();
    transcript.commonScalar(context.vkRepr);

    final List<F[]> instanceLagrange = new ArrayList<>();
    final List<F[]> instancePolys = new ArrayList<>();
    for (final F scalar : publicScalars) {
      final F[] column = zeroes(FP, domain.n);
      column[0] = scalar;
      instanceLagrange.add(column);
      instancePolys.add(domain.lagrangeToCoeff(column));
      transcript.commonPoint(params.commitLagrangeSparse(Collections.singletonList(new SparseEntry(0, scalar)), F.one(FP)).toAffine());
    }

    final List<F[]> adviceLagrange = new ArrayList<>();
    for (final F scalar : concat(publicScalars, inputScalars, outputScalars)) {
      final F[] column = zeroes(FP, domain.n);
      column[0] = scalar;
      adviceLagrange.add(column);
    }
    final int unusableRowsStart = domain.n - (BLINDING_FACTORS + 1);
    for (final F[] column : adviceLagrange) {
      for (int row = unusableRowsStart; row < domain.n; row++) {
        column[row] = randomScalar();
      }
    }

    final List<F> adviceBlinds = new ArrayList<>();
    for (final F[] column : adviceLagrange) {
      final F blind = randomScalar();
      adviceBlinds.add(blind);
      transcript.writePoint(params.commitLagrangeSparse(sparseEntries(column), blind).toAffine());
    }

    final List<F[]> advicePolys = new ArrayList<>();
    for (final F[] column : adviceLagrange) {
      advicePolys.add(domain.lagrangeToCoeff(column));
    }

    transcript.squeezeChallenge();
    transcript.squeezeChallenge();
    transcript.squeezeChallenge();

    final F[] randomPoly = zeroes(FP, domain.n);
    for (int i = 0; i < randomPoly.length; i++) {
      randomPoly[i] = randomScalar();
    }
    final F randomBlind = randomScalar();
    transcript.writePoint(params.commit(randomPoly, randomBlind).toAffine());
    final F y = transcript.squeezeChallenge().scalar;

    final F[] hCoefficients = evaluateQuotientCoefficients(
        domain, y, context.fixedPolys, advicePolys, instancePolys);
    final List<F[]> hPieces = new ArrayList<>();
    final List<F> hBlinds = new ArrayList<>();
    for (int offset = 0; offset < hCoefficients.length; offset += domain.n) {
      hPieces.add(Arrays.copyOfRange(hCoefficients, offset, offset + domain.n));
      hBlinds.add(randomScalar());
    }
    for (int i = 0; i < hPieces.size(); i++) {
      transcript.writePoint(params.commit(hPieces.get(i), hBlinds.get(i)).toAffine());
    }

    final F x = transcript.squeezeChallenge().scalar;
    final F xN = x.pow(BigInteger.valueOf(domain.n));
    final List<F> instanceEvals = new ArrayList<>();
    final List<F> adviceEvals = new ArrayList<>();
    final List<F> fixedEvals = new ArrayList<>();
    for (final F[] poly : instancePolys) {
      instanceEvals.add(evaluatePolynomial(poly, x));
    }
    for (final F[] poly : advicePolys) {
      adviceEvals.add(evaluatePolynomial(poly, x));
    }
    for (final F[] poly : context.fixedPolys) {
      fixedEvals.add(evaluatePolynomial(poly, x));
    }
    final F randomEval = evaluatePolynomial(randomPoly, x);

    for (final F eval : instanceEvals) {
      transcript.writeScalar(eval);
    }
    for (final F eval : adviceEvals) {
      transcript.writeScalar(eval);
    }
    for (final F eval : fixedEvals) {
      transcript.writeScalar(eval);
    }
    transcript.writeScalar(randomEval);

    F[] hPoly = zeroes(FP, domain.n);
    for (int i = hPieces.size() - 1; i >= 0; i--) {
      hPoly = add(scale(hPoly, xN), hPieces.get(i));
    }
    F hBlind = F.zero(FP);
    for (int i = hBlinds.size() - 1; i >= 0; i--) {
      hBlind = hBlind.mul(xN).add(hBlinds.get(i));
    }
    final F hEval = evaluatePolynomial(hPoly, x);
    final F vanishingInv = xN.sub(F.one(FP)).invert();
    final F expected = evaluateGate(
        fixedEvals.get(0), singletonRows(adviceEvals), singletonRows(instanceEvals), 0, y).mul(vanishingInv);
    if (!hEval.equals(expected)) {
      throw new IllegalArgumentException("invalid Offline V2 quotient evaluation");
    }

    final List<ProverQuery> queries = new ArrayList<>();
    for (final F[] poly : instancePolys) {
      queries.add(new ProverQuery(x, poly, F.one(FP)));
    }
    for (int i = 0; i < advicePolys.size(); i++) {
      queries.add(new ProverQuery(x, advicePolys.get(i), adviceBlinds.get(i)));
    }
    for (final F[] poly : context.fixedPolys) {
      queries.add(new ProverQuery(x, poly, F.one(FP)));
    }
    queries.add(new ProverQuery(x, hPoly, hBlind));
    queries.add(new ProverQuery(x, randomPoly, randomBlind));
    IPA.appendSamePointMultiOpeningProof(params, transcript, queries);
    return zk1ProofPayload(transcript.proofBytes(), values.publicValues());
  }

  public static boolean verifyZk1Payload(final byte[] payload, final long[] publicValues) {
    if (publicValues.length != PUBLIC_VALUE_COUNT) {
      throw new IllegalArgumentException("invalid Offline V2 public value count");
    }
    final DecodedPayload decoded = decodeZk1ProofPayload(payload);
    if (!Arrays.equals(decoded.publicValues, publicValues)) {
      return false;
    }

    final Context context = CONTEXT.requireReady();
    final Params params = context.params;
    final Domain domain = context.domain;
    final F[] publicScalars = fp(publicValues);
    final ReadTranscript transcript = new ReadTranscript(decoded.proofTranscript);
    transcript.commonScalar(context.vkRepr);

    final List<Affine> instanceCommitments = new ArrayList<>();
    for (final F scalar : publicScalars) {
      instanceCommitments.add(
          params.commitLagrangeSparse(Collections.singletonList(new SparseEntry(0, scalar)), F.one(FP)).toAffine());
    }
    for (final Affine commitment : instanceCommitments) {
      transcript.commonPoint(commitment);
    }

    final List<Affine> adviceCommitments = new ArrayList<>();
    for (int i = 0; i < ADVICE_COUNT; i++) {
      adviceCommitments.add(transcript.readPoint());
    }
    transcript.squeezeChallenge();
    transcript.squeezeChallenge();
    transcript.squeezeChallenge();

    final Affine randomCommitment = transcript.readPoint();
    final F y = transcript.squeezeChallenge().scalar;
    final Affine fixedCommitment =
        params.commitLagrangeSparse(Collections.singletonList(new SparseEntry(0, F.one(FP))), F.one(FP)).toAffine();
    final List<Affine> hCommitments = new ArrayList<>();
    for (int i = 0; i < domain.quotientPolynomialDegree; i++) {
      hCommitments.add(transcript.readPoint());
    }

    final F x = transcript.squeezeChallenge().scalar;
    final F xN = x.pow(BigInteger.valueOf(domain.n));
    final List<F> instanceEvals = transcript.readScalars(PUBLIC_VALUE_COUNT);
    final List<F> adviceEvals = transcript.readScalars(ADVICE_COUNT);
    final List<F> fixedEvals = transcript.readScalars(1);
    final F randomEval = transcript.readScalar();
    final F expectedHEval = evaluateGate(
        fixedEvals.get(0), singletonRows(adviceEvals), singletonRows(instanceEvals), 0, y);
    final F vanishingInv = xN.sub(F.one(FP)).invert();

    final List<VerifierQuery> queries = new ArrayList<>();
    for (int i = 0; i < instanceCommitments.size(); i++) {
      queries.add(new VerifierQuery(instanceCommitments.get(i).projective(), instanceEvals.get(i)));
    }
    for (int i = 0; i < adviceCommitments.size(); i++) {
      queries.add(new VerifierQuery(adviceCommitments.get(i).projective(), adviceEvals.get(i)));
    }
    queries.add(new VerifierQuery(fixedCommitment.projective(), fixedEvals.get(0)));

    Projective hCommitment = Projective.identity();
    for (int i = hCommitments.size() - 1; i >= 0; i--) {
      hCommitment = hCommitment.multiply(xN).add(hCommitments.get(i).projective());
    }
    queries.add(new VerifierQuery(hCommitment, expectedHEval.mul(vanishingInv)));
    queries.add(new VerifierQuery(randomCommitment.projective(), randomEval));

    final F x1 = transcript.squeezeChallenge().scalar;
    transcript.squeezeChallenge();
    Projective qCommitment = Projective.identity();
    F qEval = F.zero(FP);
    F power = F.one(FP);
    for (int i = queries.size() - 1; i >= 0; i--) {
      final VerifierQuery query = queries.get(i);
      qCommitment = qCommitment.add(query.commitment.multiply(power));
      qEval = qEval.add(query.eval.mul(power));
      power = power.mul(x1);
    }

    final Affine qPrimeCommitment = transcript.readPoint();
    final F x3 = transcript.squeezeChallenge().scalar;
    final F qAtX3 = transcript.readScalar();
    final F x4 = transcript.squeezeChallenge().scalar;
    final F quotientEval = qAtX3.sub(qEval).mul(x3.sub(x).invert());
    final Projective pCommitment = qPrimeCommitment.projective().multiply(x4).add(qCommitment);
    final F pValue = quotientEval.mul(x4).add(qAtX3);
    return IPA.verifyInTranscript(params, pCommitment, x3, pValue, transcript)
        && transcript.remainingBytes() == 0;
  }

  public static byte[] openVerifyEnvelope(final byte[] proofPayload) {
    final Object marker = new Object();
    return NoritoCodec.encode(
        marker,
        "iroha_data_model::zk::OpenVerifyEnvelope",
        new TypeAdapter<Object>() {
          @Override
          public void encode(final NoritoEncoder encoder, final Object value) {
            writeField(encoder, child -> child.writeUInt(BACKEND_TAG, 32));
            writeField(encoder, child -> writeString(child, CIRCUIT_ID));
            writeField(encoder, child -> child.writeBytes(CANONICAL_VK_HASH));
            writeField(encoder, child -> writeBytesVec(child, OfflineNoteV2.RECURSIVE_PUBLIC_INPUTS_SCHEMA_V1.getBytes(StandardCharsets.UTF_8)));
            writeField(encoder, child -> writeBytesVec(child, proofPayload));
            writeField(encoder, child -> writeBytesVec(child, new byte[0]));
          }

          @Override
          public Object decode(final org.hyperledger.iroha.sdk.norito.NoritoDecoder decoder) {
            throw new UnsupportedOperationException("OpenVerifyEnvelope decoding is not supported");
          }
        },
        NoritoHeader.COMPACT_LEN);
  }

  private static Context buildContext() {
    try {
      final Params params = Params.generated(IPA_K);
      final Domain domain = new Domain(PROOF_DEGREE, IPA_K);
      final F vkRepr = F.fromCanonicalBytes(FP, CANONICAL_TRANSCRIPT_REPR);
      final F[] selector = zeroes(FP, domain.n);
      selector[0] = F.one(FP);
      return new Context(params, domain, vkRepr, Collections.singletonList(domain.lagrangeToCoeff(selector)), null);
    } catch (final RuntimeException ex) {
      return new Context(null, null, null, null, ex);
    }
  }

  private static F[] evaluateQuotientCoefficients(
      final Domain domain,
      final F y,
      final List<F[]> fixedPolys,
      final List<F[]> advicePolys,
      final List<F[]> instancePolys) {
    final F[] selector = fixedPolys.get(0);
    final List<F[]> publicValues = advicePolys.subList(0, 16);
    final List<F[]> inputs = advicePolys.subList(16, 20);
    final List<F[]> outputs = advicePolys.subList(20, 22);
    final F[] mode = publicValues.get(4);
    final F[] inputCount = publicValues.get(5);
    final F[] outputCount = publicValues.get(6);
    final F[] inputSumPublic = publicValues.get(7);
    final F[] outputSumPublic = publicValues.get(8);
    final F[] one = new F[] {F.one(FP)};
    final F[] two = new F[] {F.of(FP, 2)};
    final F[] three = new F[] {F.of(FP, 3)};
    final F[] four = new F[] {F.of(FP, 4)};

    final List<F[]> constraints = new ArrayList<>();
    for (int i = 0; i < 16; i++) {
      constraints.add(polyMul(selector, polySub(publicValues.get(i), instancePolys.get(i))));
    }
    constraints.add(polyMul(selector, polyMul(polySub(mode, one), polySub(mode, two))));
    constraints.add(polyMul(
        selector,
        polyMul(
            polyMul(polySub(inputCount, one), polySub(inputCount, two)),
            polyMul(polySub(inputCount, three), polySub(inputCount, four)))));
    constraints.add(polyMul(selector, polyMul(polySub(outputCount, one), polySub(outputCount, two))));
    constraints.add(polyMul(selector, polyMul(polySub(mode, two), polySub(outputCount, one))));
    constraints.add(polyMul(selector, polySub(polySum(inputs), inputSumPublic)));
    constraints.add(polyMul(selector, polySub(polySum(outputs), outputSumPublic)));
    constraints.add(polyMul(selector, polySub(inputSumPublic, outputSumPublic)));
    constraints.add(polyMul(
        selector,
        polyMul(inputs.get(1), polyMul(polySub(inputCount, two), polyMul(polySub(inputCount, three), polySub(inputCount, four))))));
    constraints.add(polyMul(
        selector,
        polyMul(inputs.get(2), polyMul(polySub(inputCount, three), polySub(inputCount, four)))));
    constraints.add(polyMul(selector, polyMul(inputs.get(3), polySub(inputCount, four))));
    constraints.add(polyMul(selector, polyMul(outputs.get(1), polySub(outputCount, two))));

    for (int i = 0; i < constraints.size(); i++) {
      if (!evaluatePolynomial(constraints.get(i), F.one(FP)).isZero()) {
        throw new IllegalArgumentException("Offline V2 gate constraint does not vanish: " + i);
      }
    }
    F[] numerator = new F[0];
    for (final F[] constraint : constraints) {
      numerator = polyAdd(polyScale(numerator, y), constraint);
    }
    F root = F.one(FP);
    for (int row = 0; row < domain.n; row++) {
      if (!evaluatePolynomial(numerator, root).isZero()) {
        throw new IllegalArgumentException("Offline V2 quotient numerator does not vanish at row " + row);
      }
      root = root.mul(domain.omega);
    }
    return divideByVanishingPolynomial(numerator, domain.n, domain.n * domain.quotientPolynomialDegree);
  }

  private static F evaluateGate(
      final F selector,
      final List<List<F>> advice,
      final List<List<F>> instance,
      final int row,
      final F y) {
    final List<F> publicValues = new ArrayList<>();
    final List<F> inputs = new ArrayList<>();
    final List<F> outputs = new ArrayList<>();
    for (int i = 0; i < 16; i++) {
      publicValues.add(advice.get(i).get(row));
    }
    for (int i = 0; i < 4; i++) {
      inputs.add(advice.get(16 + i).get(row));
    }
    for (int i = 0; i < 2; i++) {
      outputs.add(advice.get(20 + i).get(row));
    }
    final F mode = publicValues.get(4);
    final F inputCount = publicValues.get(5);
    final F outputCount = publicValues.get(6);
    final F inputSumPublic = publicValues.get(7);
    final F outputSumPublic = publicValues.get(8);
    final F one = F.one(FP);
    final F two = F.of(FP, 2);
    final F three = F.of(FP, 3);
    final F four = F.of(FP, 4);
    final List<F> constraints = new ArrayList<>();
    for (int i = 0; i < 16; i++) {
      constraints.add(selector.mul(publicValues.get(i).sub(instance.get(i).get(row))));
    }
    constraints.add(selector.mul(mode.sub(one)).mul(mode.sub(two)));
    constraints.add(selector.mul(inputCount.sub(one)).mul(inputCount.sub(two)).mul(inputCount.sub(three)).mul(inputCount.sub(four)));
    constraints.add(selector.mul(outputCount.sub(one)).mul(outputCount.sub(two)));
    constraints.add(selector.mul(mode.sub(two)).mul(outputCount.sub(one)));
    constraints.add(selector.mul(sum(inputs).sub(inputSumPublic)));
    constraints.add(selector.mul(sum(outputs).sub(outputSumPublic)));
    constraints.add(selector.mul(inputSumPublic.sub(outputSumPublic)));
    constraints.add(selector.mul(inputs.get(1)).mul(inputCount.sub(two)).mul(inputCount.sub(three)).mul(inputCount.sub(four)));
    constraints.add(selector.mul(inputs.get(2)).mul(inputCount.sub(three)).mul(inputCount.sub(four)));
    constraints.add(selector.mul(inputs.get(3)).mul(inputCount.sub(four)));
    constraints.add(selector.mul(outputs.get(1)).mul(outputCount.sub(two)));

    F acc = F.zero(FP);
    for (final F constraint : constraints) {
      acc = acc.mul(y).add(constraint);
    }
    return acc;
  }

  private static byte[] zk1ProofPayload(final byte[] proofTranscript, final long[] publicValues) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, new byte[] {0x5A, 0x4B, 0x31, 0x00});
    appendTlv(out, "PROF", proofTranscript);
    final ByteArrayOutputStream instances = new ByteArrayOutputStream();
    writeUInt32LE(instances, 16);
    writeUInt32LE(instances, 1);
    for (final long value : publicValues) {
      write(instances, OfflineNoteV2.instanceScalarBytes(value));
    }
    appendTlv(out, "I10P", instances.toByteArray());
    return out.toByteArray();
  }

  private static DecodedPayload decodeZk1ProofPayload(final byte[] payload) {
    if (payload.length < 4 || payload[0] != 0x5A || payload[1] != 0x4B || payload[2] != 0x31 || payload[3] != 0) {
      throw new IllegalArgumentException("invalid ZK1 proof payload");
    }
    int cursor = 4;
    byte[] proof = null;
    long[] publicValues = null;
    while (cursor < payload.length) {
      if (cursor + 8 > payload.length) {
        throw new IllegalArgumentException("invalid ZK1 proof payload");
      }
      final String tag = new String(payload, cursor, 4, StandardCharsets.UTF_8);
      cursor += 4;
      final int length = (int) readUInt32LE(payload, cursor);
      cursor += 4;
      if (cursor + length > payload.length) {
        throw new IllegalArgumentException("invalid ZK1 proof payload");
      }
      final byte[] value = Arrays.copyOfRange(payload, cursor, cursor + length);
      cursor += length;
      if ("PROF".equals(tag)) {
        proof = value;
      } else if ("I10P".equals(tag)) {
        publicValues = decodeI10PPublicValues(value);
      }
    }
    if (proof == null || proof.length == 0 || publicValues == null) {
      throw new IllegalArgumentException("invalid ZK1 proof payload");
    }
    return new DecodedPayload(proof, publicValues);
  }

  private static long[] decodeI10PPublicValues(final byte[] payload) {
    if (payload.length != 8 + 16 * 32 || readUInt32LE(payload, 0) != 16 || readUInt32LE(payload, 4) != 1) {
      throw new IllegalArgumentException("invalid I10P payload");
    }
    final long[] values = new long[16];
    int cursor = 8;
    for (int i = 0; i < 16; i++) {
      final byte[] scalar = Arrays.copyOfRange(payload, cursor, cursor + 32);
      F.fromCanonicalBytes(FP, scalar);
      long value = 0;
      for (int j = 0; j < 8; j++) {
        value |= (payload[cursor + j] & 0xFFL) << (j * 8);
      }
      for (int j = 8; j < 32; j++) {
        if (payload[cursor + j] != 0) {
          throw new IllegalArgumentException("invalid I10P scalar");
        }
      }
      values[i] = value;
      cursor += 32;
    }
    return values;
  }

  private static byte[] publicInputsHash(final long[] publicValues) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    for (int i = 0; i < 4; i++) {
      long word = publicValues[i];
      for (int j = 0; j < 8; j++) {
        out.write((int) (word & 0xFFL));
        word >>>= 8;
      }
    }
    return out.toByteArray();
  }

  private static F[] fp(final long[] values) {
    final F[] out = new F[values.length];
    for (int i = 0; i < values.length; i++) {
      out[i] = F.ofUnsignedLong(FP, values[i]);
    }
    return out;
  }

  private static F[] concat(final F[] a, final F[] b, final F[] c) {
    final F[] out = new F[a.length + b.length + c.length];
    System.arraycopy(a, 0, out, 0, a.length);
    System.arraycopy(b, 0, out, a.length, b.length);
    System.arraycopy(c, 0, out, a.length + b.length, c.length);
    return out;
  }

  private static List<List<F>> singletonRows(final List<F> values) {
    final List<List<F>> rows = new ArrayList<>();
    for (final F value : values) {
      rows.add(Collections.singletonList(value));
    }
    return rows;
  }

  private static List<SparseEntry> sparseEntries(final F[] values) {
    final List<SparseEntry> entries = new ArrayList<>();
    for (int i = 0; i < values.length; i++) {
      if (!values[i].isZero()) {
        entries.add(new SparseEntry(i, values[i]));
      }
    }
    return entries;
  }

  private static F randomScalar() {
    final byte[] bytes = new byte[64];
    RNG.nextBytes(bytes);
    return F.fromUniformBytes64(FP, bytes);
  }

  private static F evaluatePolynomial(final F[] polynomial, final F point) {
    F acc = F.zero(FP);
    for (int i = polynomial.length - 1; i >= 0; i--) {
      acc = acc.mul(point).add(polynomial[i]);
    }
    return acc;
  }

  private static F[] add(final F[] lhs, final F[] rhs) {
    final F[] out = zeroes(FP, lhs.length);
    for (int i = 0; i < lhs.length; i++) {
      out[i] = lhs[i].add(rhs[i]);
    }
    return out;
  }

  private static F[] scale(final F[] values, final F scalar) {
    final F[] out = new F[values.length];
    for (int i = 0; i < values.length; i++) {
      out[i] = values[i].mul(scalar);
    }
    return out;
  }

  private static F sum(final List<F> values) {
    F out = F.zero(FP);
    for (final F value : values) {
      out = out.add(value);
    }
    return out;
  }

  private static F[] polySum(final List<F[]> values) {
    F[] out = new F[0];
    for (final F[] value : values) {
      out = polyAdd(out, value);
    }
    return out;
  }

  private static F[] polyAdd(final F[] lhs, final F[] rhs) {
    final int count = Math.max(lhs.length, rhs.length);
    final F[] out = zeroes(FP, count);
    for (int i = 0; i < lhs.length; i++) {
      out[i] = out[i].add(lhs[i]);
    }
    for (int i = 0; i < rhs.length; i++) {
      out[i] = out[i].add(rhs[i]);
    }
    return trim(out);
  }

  private static F[] polySub(final F[] lhs, final F[] rhs) {
    final int count = Math.max(lhs.length, rhs.length);
    final F[] out = zeroes(FP, count);
    for (int i = 0; i < lhs.length; i++) {
      out[i] = out[i].add(lhs[i]);
    }
    for (int i = 0; i < rhs.length; i++) {
      out[i] = out[i].sub(rhs[i]);
    }
    return trim(out);
  }

  private static F[] polyMul(final F[] lhs, final F[] rhs) {
    if (lhs.length == 0 || rhs.length == 0) {
      return new F[0];
    }
    final F[] out = zeroes(FP, lhs.length + rhs.length - 1);
    for (int i = 0; i < lhs.length; i++) {
      if (lhs[i].isZero()) {
        continue;
      }
      for (int j = 0; j < rhs.length; j++) {
        if (!rhs[j].isZero()) {
          out[i + j] = out[i + j].add(lhs[i].mul(rhs[j]));
        }
      }
    }
    return trim(out);
  }

  private static F[] polyScale(final F[] values, final F scalar) {
    if (scalar.isZero()) {
      return new F[0];
    }
    final F[] out = new F[values.length];
    for (int i = 0; i < values.length; i++) {
      out[i] = values[i].mul(scalar);
    }
    return trim(out);
  }

  private static F[] divideByVanishingPolynomial(
      final F[] numerator, final int domainSize, final int quotientLength) {
    F[] remainder = trim(numerator);
    F[] quotient = zeroes(FP, Math.max(0, remainder.length - domainSize));
    while (remainder.length - 1 >= domainSize) {
      final int degree = remainder.length - 1;
      final F coefficient = remainder[degree];
      final int quotientIndex = degree - domainSize;
      if (quotientIndex >= quotient.length) {
        quotient = Arrays.copyOf(quotient, quotientIndex + 1);
        for (int i = 0; i < quotient.length; i++) {
          if (quotient[i] == null) {
            quotient[i] = F.zero(FP);
          }
        }
      }
      quotient[quotientIndex] = quotient[quotientIndex].add(coefficient);
      remainder[degree] = remainder[degree].sub(coefficient);
      remainder[quotientIndex] = remainder[quotientIndex].add(coefficient);
      remainder = trim(remainder);
    }
    for (final F value : remainder) {
      if (!value.isZero()) {
        throw new IllegalArgumentException("invalid Offline V2 quotient evaluation");
      }
    }
    for (int i = quotientLength; i < quotient.length; i++) {
      if (!quotient[i].isZero()) {
        throw new IllegalArgumentException("invalid Offline V2 quotient evaluation");
      }
    }
    if (quotient.length < quotientLength) {
      final F[] padded = zeroes(FP, quotientLength);
      System.arraycopy(quotient, 0, padded, 0, quotient.length);
      quotient = padded;
    }
    return Arrays.copyOf(quotient, quotientLength);
  }

  private static F[] trim(final F[] values) {
    int len = values.length;
    while (len > 0 && values[len - 1].isZero()) {
      len--;
    }
    return Arrays.copyOf(values, len);
  }

  private static F[] zeroes(final ParamsField params, final int count) {
    final F[] out = new F[count];
    Arrays.fill(out, F.zero(params));
    return out;
  }

  private static void appendTlv(final ByteArrayOutputStream out, final String tag, final byte[] value) {
    write(out, tag.getBytes(StandardCharsets.UTF_8));
    writeUInt32LE(out, value.length);
    write(out, value);
  }

  private static long readUInt32LE(final byte[] bytes, final int offset) {
    return (bytes[offset] & 0xFFL)
        | ((bytes[offset + 1] & 0xFFL) << 8)
        | ((bytes[offset + 2] & 0xFFL) << 16)
        | ((bytes[offset + 3] & 0xFFL) << 24);
  }

  private static void writeUInt32LE(final ByteArrayOutputStream out, final int value) {
    out.write(value & 0xFF);
    out.write((value >>> 8) & 0xFF);
    out.write((value >>> 16) & 0xFF);
    out.write((value >>> 24) & 0xFF);
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  private static void writeField(final NoritoEncoder parent, final FieldWriter writer) {
    final NoritoEncoder child = parent.childEncoder();
    writer.write(child);
    final byte[] payload = child.toByteArray();
    parent.writeLength(payload.length, true);
    parent.writeBytes(payload);
  }

  private static void writeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, true);
    encoder.writeBytes(bytes);
  }

  private static void writeBytesVec(final NoritoEncoder encoder, final byte[] bytes) {
    encoder.writeUInt(bytes.length, 64);
    encoder.writeBytes(bytes);
  }

  private static byte[] hexBytes(final String value) {
    final byte[] out = new byte[value.length() / 2];
    for (int i = 0; i < value.length(); i += 2) {
      out[i / 2] = (byte) Integer.parseInt(value.substring(i, i + 2), 16);
    }
    return out;
  }

  private static BigInteger limbs(final String... littleEndianHex) {
    BigInteger value = BigInteger.ZERO;
    for (int i = 0; i < littleEndianHex.length; i++) {
      value = value.add(new BigInteger(littleEndianHex[i], 16).shiftLeft(i * 64));
    }
    return value;
  }

  private static BigInteger unsignedLong(final long value) {
    final byte[] bytes = new byte[9];
    for (int i = 0; i < 8; i++) {
      bytes[8 - i] = (byte) (value >>> (i * 8));
    }
    return new BigInteger(bytes);
  }

  private static final ParamsField FP =
      new ParamsField(
          limbs("992d30ed00000001", "224698fc094cf91b", "0000000000000000", "4000000000000000"),
          limbs("bdad6fabd87ea32f", "ea322bf2b7bb7584", "362120830561f81a", "2bce74deac30ebda"),
          limbs("f0b87c7db2ce91f6", "84a0a1d8859f066f", "b4ed8e647196dad1", "2cd5282c53116b5c"),
          limbs("1dad5ebdfdfe4ab9", "1d1f8bd237ad3149", "2caad5dc57aab1b0", "12ccca834acdba71"),
          limbs("094cf91b992d30ed", "00000000224698fc", "0000000000000000", "0000000040000000"),
          limbs("04a67c8dcc969877", "0000000011234c7e", "0000000000000000", "0000000020000000"),
          32);

  private static final ParamsField FQ =
      new ParamsField(
          limbs("8c46eb2100000001", "224698fc0994a8dd", "0000000000000000", "4000000000000000"),
          limbs("a70e2c1102b6d05f", "9bb97ea3c106f049", "9e5c4dfd492ae26e", "2de6a9b8746d3f58"),
          limbs("57eecda0a84b6836", "4ad38b9084b8a80c", "f4c8f353124086c1", "2235e1a7415bf936"),
          limbs("2aa9d2e050aa0e4f", "0fed467d47c033af", "511db4d81cf70f5a", "06819a58283e528e"),
          limbs("0994a8dd8c46eb21", "00000000224698fc", "0000000000000000", "0000000040000000"),
          limbs("04ca546ec6237591", "0000000011234c7e", "0000000000000000", "0000000020000000"),
          32);

  private static final class Context {
    private final Params params;
    private final Domain domain;
    private final F vkRepr;
    private final List<F[]> fixedPolys;
    private final RuntimeException error;

    private Context(
        final Params params,
        final Domain domain,
        final F vkRepr,
        final List<F[]> fixedPolys,
        final RuntimeException error) {
      this.params = params;
      this.domain = domain;
      this.vkRepr = vkRepr;
      this.fixedPolys = fixedPolys;
      this.error = error;
    }

    private Context requireReady() {
      if (error != null) {
        throw error;
      }
      return this;
    }
  }

  private static final class DecodedPayload {
    private final byte[] proofTranscript;
    private final long[] publicValues;

    private DecodedPayload(final byte[] proofTranscript, final long[] publicValues) {
      this.proofTranscript = proofTranscript;
      this.publicValues = publicValues;
    }
  }

  private static final class SparseEntry {
    private final int index;
    private final F scalar;

    private SparseEntry(final int index, final F scalar) {
      this.index = index;
      this.scalar = scalar;
    }
  }

  private static final class ParamsField {
    private final BigInteger modulus;
    private final BigInteger root;
    private final BigInteger rootInv;
    private final BigInteger zeta;
    private final BigInteger t;
    private final BigInteger tPlusOneOverTwo;
    private final int twoAdicity;

    private ParamsField(
        final BigInteger modulus,
        final BigInteger root,
        final BigInteger rootInv,
        final BigInteger zeta,
        final BigInteger t,
        final BigInteger tPlusOneOverTwo,
        final int twoAdicity) {
      this.modulus = modulus;
      this.root = root;
      this.rootInv = rootInv;
      this.zeta = zeta;
      this.t = t;
      this.tPlusOneOverTwo = tPlusOneOverTwo;
      this.twoAdicity = twoAdicity;
    }
  }

  private static final class F {
    private final ParamsField params;
    private final BigInteger value;

    private F(final ParamsField params, final BigInteger value) {
      this.params = params;
      this.value = value.mod(params.modulus);
    }

    private F(final ParamsField params, final BigInteger value, final boolean canonical) {
      this.params = params;
      this.value = canonical ? value : value.mod(params.modulus);
    }

    private static F zero(final ParamsField params) {
      return new F(params, BigInteger.ZERO, true);
    }

    private static F one(final ParamsField params) {
      return new F(params, BigInteger.ONE, true);
    }

    private static F of(final ParamsField params, final long value) {
      return new F(params, BigInteger.valueOf(value));
    }

    private static F ofUnsignedLong(final ParamsField params, final long value) {
      return new F(params, unsignedLong(value));
    }

    private static F rawLimbs(final ParamsField params, final String... littleEndianHex) {
      return new F(params, limbs(littleEndianHex));
    }

    private static F rootOfUnity(final ParamsField params) {
      return new F(params, params.root);
    }

    private static F rootOfUnityInv(final ParamsField params) {
      return new F(params, params.rootInv);
    }

    private static F zeta(final ParamsField params) {
      return new F(params, params.zeta);
    }

    private static F fromCanonicalBytes(final ParamsField params, final byte[] bytes) {
      if (bytes.length != 32) {
        throw new IllegalArgumentException("field element must be 32 bytes");
      }
      final BigInteger value = fromLittleEndian(bytes);
      if (value.compareTo(params.modulus) >= 0) {
        throw new IllegalArgumentException("field element is not canonical");
      }
      return new F(params, value, true);
    }

    private static F fromUniformBytes64(final ParamsField params, final byte[] littleEndian) {
      if (littleEndian.length != 64) {
        throw new IllegalArgumentException("uniform field input must be 64 bytes");
      }
      return new F(params, fromLittleEndian(littleEndian));
    }

    private F add(final F rhs) {
      same(rhs);
      BigInteger sum = value.add(rhs.value);
      if (sum.compareTo(params.modulus) >= 0) {
        sum = sum.subtract(params.modulus);
      }
      return new F(params, sum, true);
    }

    private F sub(final F rhs) {
      same(rhs);
      BigInteger difference = value.subtract(rhs.value);
      if (difference.signum() < 0) {
        difference = difference.add(params.modulus);
      }
      return new F(params, difference, true);
    }

    private F neg() {
      return isZero() ? this : new F(params, params.modulus.subtract(value), true);
    }

    private F doubleValue() {
      BigInteger doubled = value.shiftLeft(1);
      if (doubled.compareTo(params.modulus) >= 0) {
        doubled = doubled.subtract(params.modulus);
      }
      return new F(params, doubled, true);
    }

    private F eightValue() {
      BigInteger scaled = value.shiftLeft(3);
      while (scaled.compareTo(params.modulus) >= 0) {
        scaled = scaled.subtract(params.modulus);
      }
      return new F(params, scaled, true);
    }

    private F mul(final F rhs) {
      same(rhs);
      return new F(params, value.multiply(rhs.value));
    }

    private F square() {
      return new F(params, value.multiply(value));
    }

    private F invert() {
      if (isZero()) {
        throw new IllegalArgumentException("zero is not invertible");
      }
      return new F(params, value.modInverse(params.modulus));
    }

    private F pow(final BigInteger exponent) {
      return new F(params, value.modPow(exponent, params.modulus));
    }

    private F sqrt() {
      if (isZero()) {
        return this;
      }
      F z = rootOfUnity(params);
      F w = pow(params.t);
      F x = pow(params.tPlusOneOverTwo);
      int v = params.twoAdicity;
      while (!w.equals(one(params))) {
        int k = 1;
        F probe = w.square();
        while (!probe.equals(one(params))) {
          probe = probe.square();
          k++;
          if (k >= v) {
            return null;
          }
        }
        F b = z;
        final int squarings = v - k - 1;
        for (int i = 0; i < squarings; i++) {
          b = b.square();
        }
        x = x.mul(b);
        z = b.square();
        w = w.mul(z);
        v = k;
      }
      return x.square().equals(this) ? x : null;
    }

    private static SqrtRatio sqrtRatio(final F numerator, final F denominator) {
      if (denominator.isZero()) {
        throw new IllegalArgumentException("zero denominator");
      }
      final F ratio = numerator.mul(denominator.invert());
      final F root = ratio.sqrt();
      if (root != null) {
        return new SqrtRatio(true, root);
      }
      final F rootOfZeta = ratio.mul(rootOfUnity(numerator.params)).sqrt();
      if (rootOfZeta == null) {
        throw new IllegalArgumentException("ratio has no square root");
      }
      return new SqrtRatio(false, rootOfZeta);
    }

    private boolean isZero() {
      return value.signum() == 0;
    }

    private boolean isOdd() {
      return value.testBit(0);
    }

    private byte[] canonicalBytes() {
      return toLittleEndian(value, 32);
    }

    private void same(final F rhs) {
      if (params != rhs.params) {
        throw new IllegalArgumentException("mixed Pasta fields");
      }
    }

    @Override
    public boolean equals(final Object obj) {
      if (!(obj instanceof F)) {
        return false;
      }
      final F rhs = (F) obj;
      return params == rhs.params && value.equals(rhs.value);
    }

    @Override
    public int hashCode() {
      return Objects.hash(System.identityHashCode(params), value);
    }
  }

  private static final class SqrtRatio {
    private final boolean isSquare;
    private final F root;

    private SqrtRatio(final boolean isSquare, final F root) {
      this.isSquare = isSquare;
      this.root = root;
    }
  }

  private static BigInteger fromLittleEndian(final byte[] bytes) {
    final byte[] be = new byte[bytes.length + 1];
    for (int i = 0; i < bytes.length; i++) {
      be[be.length - 1 - i] = bytes[i];
    }
    return new BigInteger(be);
  }

  private static byte[] toLittleEndian(final BigInteger value, final int length) {
    final byte[] out = new byte[length];
    final byte[] be = value.toByteArray();
    for (int i = 0; i < be.length; i++) {
      final int outIndex = i;
      final int beIndex = be.length - 1 - i;
      if (outIndex < length) {
        out[outIndex] = be[beIndex];
      }
    }
    return out;
  }

  private static final class Affine {
    private final F x;
    private final F y;
    private final boolean identity;

    private static final Affine IDENTITY = new Affine(F.zero(FQ), F.zero(FQ), true);
    private static final Affine GENERATOR = new Affine(F.one(FQ).neg(), F.of(FQ, 2), false);

    private Affine(final F x, final F y, final boolean identity) {
      this.x = x;
      this.y = y;
      this.identity = identity;
    }

    private static Affine of(final F x, final F y) {
      if (!y.square().equals(x.square().mul(x).add(F.of(FQ, 5)))) {
        throw new IllegalArgumentException("point is not on Vesta");
      }
      return new Affine(x, y, false);
    }

    private static Affine fromCompressedBytes(final byte[] bytes) {
      if (bytes.length != 32) {
        throw new IllegalArgumentException("compressed point must be 32 bytes");
      }
      final byte[] xBytes = Arrays.copyOf(bytes, bytes.length);
      final boolean ySign = (xBytes[31] & 0x80) != 0;
      xBytes[31] &= 0x7F;
      final F x = F.fromCanonicalBytes(FQ, xBytes);
      if (x.isZero() && !ySign) {
        return IDENTITY;
      }
      final F rhs = x.square().mul(x).add(F.of(FQ, 5));
      F y = rhs.sqrt();
      if (y == null) {
        throw new IllegalArgumentException("invalid compressed point");
      }
      if (y.isOdd() != ySign) {
        y = y.neg();
      }
      return of(x, y);
    }

    private byte[] compressedBytes() {
      if (identity) {
        return new byte[32];
      }
      final byte[] out = x.canonicalBytes();
      if (y.isOdd()) {
        out[31] |= (byte) 0x80;
      }
      return out;
    }

    private Projective projective() {
      return new Projective(this);
    }

    private Affine add(final Affine rhs) {
      return projective().mixedAdd(rhs).toAffine();
    }

    @Override
    public boolean equals(final Object obj) {
      if (!(obj instanceof Affine)) {
        return false;
      }
      final Affine rhs = (Affine) obj;
      return identity == rhs.identity && x.equals(rhs.x) && y.equals(rhs.y);
    }

    @Override
    public int hashCode() {
      return Objects.hash(x, y, identity);
    }
  }

  private static final class Projective {
    private final F x;
    private final F y;
    private final F z;

    private Projective(final F x, final F y, final F z) {
      this.x = x;
      this.y = y;
      this.z = z;
    }

    private Projective(final Affine affine) {
      if (affine.identity) {
        this.x = F.zero(FQ);
        this.y = F.one(FQ);
        this.z = F.zero(FQ);
      } else {
        this.x = affine.x;
        this.y = affine.y;
        this.z = F.one(FQ);
      }
    }

    private static Projective identity() {
      return new Projective(F.zero(FQ), F.one(FQ), F.zero(FQ));
    }

    private boolean isIdentity() {
      return z.isZero();
    }

    private Affine toAffine() {
      if (isIdentity()) {
        return Affine.IDENTITY;
      }
      final F zInv = z.invert();
      final F z2 = zInv.square();
      final F z3 = z2.mul(zInv);
      return Affine.of(x.mul(z2), y.mul(z3));
    }

    private Projective neg() {
      return isIdentity() ? this : new Projective(x, y.neg(), z);
    }

    private Projective doublePoint() {
      if (isIdentity() || y.isZero()) {
        return identity();
      }
      final F a = x.square();
      final F b = y.square();
      final F c = b.square();
      final F xPlusB = x.add(b);
      final F xPlusBSquared = xPlusB.square();
      final F d0 = xPlusBSquared.sub(a).sub(c);
      final F d = d0.add(d0);
      final F e = a.add(a).add(a);
      final F f = e.square();
      final F x3 = f.sub(d).sub(d);
      final F y3 = e.mul(d.sub(x3)).sub(c.eightValue());
      final F z3 = y.add(y).mul(z);
      return new Projective(x3, y3, z3);
    }

    private Projective mixedAdd(final Affine rhs) {
      if (rhs.identity) {
        return this;
      }
      if (isIdentity()) {
        return new Projective(rhs);
      }
      final F z2 = z.square();
      final F u2 = rhs.x.mul(z2);
      final F s2 = rhs.y.mul(z).mul(z2);
      final F h = u2.sub(x);
      final F r0 = s2.sub(y);
      if (h.isZero()) {
        return r0.isZero() ? doublePoint() : identity();
      }
      final F hh = h.square();
      final F i = hh.add(hh).add(hh).add(hh);
      final F j = h.mul(i);
      final F r = r0.add(r0);
      final F v = x.mul(i);
      final F x3 = r.square().sub(j).sub(v).sub(v);
      final F y3 = r.mul(v.sub(x3)).sub(y.doubleValue().mul(j));
      final F z3 = z.add(h).square().sub(z2).sub(hh);
      return new Projective(x3, y3, z3);
    }

    private Projective add(final Projective rhs) {
      if (rhs.isIdentity()) {
        return this;
      }
      if (isIdentity()) {
        return rhs;
      }
      final F z1z1 = z.square();
      final F z2z2 = rhs.z.square();
      final F u1 = x.mul(z2z2);
      final F u2 = rhs.x.mul(z1z1);
      final F s1 = y.mul(rhs.z).mul(z2z2);
      final F s2 = rhs.y.mul(z).mul(z1z1);
      final F h = u2.sub(u1);
      final F r0 = s2.sub(s1);
      if (h.isZero()) {
        return r0.isZero() ? doublePoint() : identity();
      }
      final F i = h.add(h).square();
      final F j = h.mul(i);
      final F r = r0.add(r0);
      final F v = u1.mul(i);
      final F x3 = r.square().sub(j).sub(v).sub(v);
      final F y3 = r.mul(v.sub(x3)).sub(s1.doubleValue().mul(j));
      final F z3 = z.add(rhs.z).square().sub(z1z1).sub(z2z2).mul(h);
      return new Projective(x3, y3, z3);
    }

    private Projective multiply(final F scalar) {
      if (scalar.isZero() || isIdentity()) {
        return identity();
      }
      final byte[] bytes = scalar.canonicalBytes();
      final Projective[] table = new Projective[16];
      table[0] = identity();
      table[1] = this;
      for (int i = 2; i < table.length; i++) {
        table[i] = table[i - 1].add(this);
      }
      Projective result = identity();
      for (int window = 63; window >= 0; window--) {
        if (!result.isIdentity()) {
          result = result.doublePoint().doublePoint().doublePoint().doublePoint();
        }
        final int byteIndex = window / 2;
        final int nibble = (window & 1) == 0 ? bytes[byteIndex] & 0x0F : (bytes[byteIndex] >>> 4) & 0x0F;
        if (nibble != 0) {
          result = result.add(table[nibble]);
        }
      }
      return result;
    }

    private static Projective msm(final List<F> scalars, final List<Affine> bases) {
      if (scalars.size() != bases.size()) {
        throw new IllegalArgumentException("MSM scalar/base length mismatch");
      }
      int active = 0;
      for (int i = 0; i < scalars.size(); i++) {
        if (!scalars.get(i).isZero() && !bases.get(i).identity) {
          active++;
        }
      }
      if (active < 8) {
        return msmNaive(scalars, bases);
      }

      final byte[][] scalarBytes = new byte[scalars.size()][];
      for (int i = 0; i < scalars.size(); i++) {
        if (!scalars.get(i).isZero() && !bases.get(i).identity) {
          scalarBytes[i] = scalars.get(i).canonicalBytes();
        }
      }

      Projective result = identity();
      for (int window = 63; window >= 0; window--) {
        if (!result.isIdentity()) {
          result = result.doublePoint().doublePoint().doublePoint().doublePoint();
        }
        final Projective[] buckets = new Projective[16];
        for (int i = 0; i < scalarBytes.length; i++) {
          if (scalarBytes[i] == null) {
            continue;
          }
          final int nibble = windowNibble(scalarBytes[i], window);
          if (nibble != 0) {
            buckets[nibble] =
                buckets[nibble] == null
                    ? bases.get(i).projective()
                    : buckets[nibble].mixedAdd(bases.get(i));
          }
        }
        Projective running = identity();
        for (int bucket = buckets.length - 1; bucket > 0; bucket--) {
          if (buckets[bucket] != null) {
            running = running.add(buckets[bucket]);
          }
          if (!running.isIdentity()) {
            result = result.add(running);
          }
        }
      }
      return result;
    }

    private static Projective msmNaive(final List<F> scalars, final List<Affine> bases) {
      Projective acc = identity();
      for (int i = 0; i < scalars.size(); i++) {
        if (!scalars.get(i).isZero() && !bases.get(i).identity) {
          acc = acc.add(bases.get(i).projective().multiply(scalars.get(i)));
        }
      }
      return acc;
    }

    private static int windowNibble(final byte[] scalarBytes, final int window) {
      final int value = scalarBytes[window / 2] & 0xFF;
      return (window & 1) == 0 ? value & 0x0F : (value >>> 4) & 0x0F;
    }
  }

  private static final class IsoProjective {
    private final F x;
    private final F y;
    private final F z;

    private IsoProjective(final F x, final F y, final F z) {
      this.x = x;
      this.y = y;
      this.z = z;
    }

    private IsoAffine toAffine() {
      if (z.isZero()) {
        return IsoAffine.IDENTITY;
      }
      final F zInv = z.invert();
      final F z2 = zInv.square();
      final F z3 = z2.mul(zInv);
      return new IsoAffine(x.mul(z2), y.mul(z3), false);
    }
  }

  private static final class IsoAffine {
    private static final IsoAffine IDENTITY = new IsoAffine(F.zero(FQ), F.zero(FQ), true);
    private final F x;
    private final F y;
    private final boolean identity;

    private IsoAffine(final F x, final F y, final boolean identity) {
      this.x = x;
      this.y = y;
      this.identity = identity;
    }

    private IsoAffine add(final IsoAffine rhs) {
      if (identity) {
        return rhs;
      }
      if (rhs.identity) {
        return this;
      }
      if (x.equals(rhs.x)) {
        if (y.add(rhs.y).isZero()) {
          return IDENTITY;
        }
        final F slope = F.of(FQ, 3).mul(x.square()).add(ISO_A).mul(F.of(FQ, 2).mul(y).invert());
        final F x3 = slope.square().sub(x).sub(rhs.x);
        final F y3 = slope.mul(x.sub(x3)).sub(y);
        return new IsoAffine(x3, y3, false);
      }
      final F slope = rhs.y.sub(y).mul(rhs.x.sub(x).invert());
      final F x3 = slope.square().sub(x).sub(rhs.x);
      final F y3 = slope.mul(x.sub(x3)).sub(y);
      return new IsoAffine(x3, y3, false);
    }
  }

  private static final F ISO_A =
      F.rawLimbs(FQ, "c515ad7242eaa6b1", "9673928c7d01b212", "81639c4d96f78773", "267f9b2ee592271a");
  private static final F ISO_B = F.of(FQ, 1265);
  private static final F SSWU_Z =
      F.rawLimbs(FQ, "8c46eb20fffffff4", "224698fc0994a8dd", "0000000000000000", "4000000000000000");
  private static final F THETA =
      F.rawLimbs(FQ, "632cae9872df1b5d", "38578ccadf03ac27", "53c3808d9e2f2357", "2b3483a1ee9a382f");
  private static final F[] ISO = new F[] {
      F.rawLimbs(FQ, "43cd42c800000001", "0205dd51cfa0961a", "8e38e38e38e38e39", "38e38e38e38e38e3"),
      F.rawLimbs(FQ, "8b95c6aaf703bcc5", "216b8861ec72bd5d", "acecf10f5f7c09a2", "1d935247b4473d17"),
      F.rawLimbs(FQ, "aeac67bbeb586a3d", "d59d03d23b39cb11", "ed7ee4a9cdf78f8f", "18760c7f7a9ad20d"),
      F.rawLimbs(FQ, "fb539a6f0000002b", "e1c521a795ac8356", "1c71c71c71c71c71", "31c71c71c71c71c7"),
      F.rawLimbs(FQ, "b7284f7eaf21a2e9", "a3ad678129b604d3", "1454798a5b5c56b2", "0a2de485568125d5"),
      F.rawLimbs(FQ, "f169c187d2533465", "30cd6d53df49d235", "0c621de8b91c242a", "14735171ee542778"),
      F.rawLimbs(FQ, "6bef1642aaaaaaab", "5601f4709a8adcb3", "0da12f684bda12f68", "12f684bda12f684b"),
      F.rawLimbs(FQ, "8bee58e5fb81de63", "21d910aefb03b31d", "d6767887afbe04d1", "2ec9a923da239e8b"),
      F.rawLimbs(FQ, "4986913ab4443034", "97a3ca5c24e9ea63", "66d1466e9de10e64", "19b0d87e16e25788"),
      F.rawLimbs(FQ, "8f64842c55555533", "8bc32d36fb21a6a3", "425ed097b425ed09", "1ed097b425ed097b"),
      F.rawLimbs(FQ, "58dfecce86b2745e", "06a767bfc35b5bac", "9e7eb64f890a820c", "2f44d6c801c1b8bf"),
      F.rawLimbs(FQ, "d43d449776f99d2f", "926847fb9ddd76a1", "252659ba2b546c7e", "3d59f455cafc7668"),
      F.rawLimbs(FQ, "8c46eb20fffffde5", "224698fc0994a8dd", "0000000000000000", "4000000000000000")
  };

  private static final Context CONTEXT = buildContext();

  private static final class HashToCurve {
    private static Projective hash(final String domainPrefix, final byte[] message) {
      final F[] fields = hashToField(domainPrefix, message);
      final IsoAffine q0 = mapToIsoCurve(fields[0]).toAffine();
      final IsoAffine q1 = mapToIsoCurve(fields[1]).toAffine();
      return isoMap(q0.add(q1));
    }

    private static F[] hashToField(final String domainPrefix, final byte[] message) {
      final byte[] dst =
          (domainPrefix + "-vesta_XMD:BLAKE2b_SSWU_RO_").getBytes(StandardCharsets.UTF_8);
      final ByteArrayOutputStream b0Input = new ByteArrayOutputStream();
      write(b0Input, new byte[128]);
      write(b0Input, message);
      write(b0Input, new byte[] {0, (byte) 128, 0});
      write(b0Input, dst);
      b0Input.write(dst.length);
      final byte[] b0 = Blake2b.digest(b0Input.toByteArray(), 64, null);

      final ByteArrayOutputStream b1Input = new ByteArrayOutputStream();
      write(b1Input, b0);
      b1Input.write(1);
      write(b1Input, dst);
      b1Input.write(dst.length);
      final byte[] b1 = Blake2b.digest(b1Input.toByteArray(), 64, null);

      final ByteArrayOutputStream b2Input = new ByteArrayOutputStream();
      for (int i = 0; i < b0.length; i++) {
        b2Input.write(b0[i] ^ b1[i]);
      }
      b2Input.write(2);
      write(b2Input, dst);
      b2Input.write(dst.length);
      final byte[] b2 = Blake2b.digest(b2Input.toByteArray(), 64, null);
      return new F[] {F.fromUniformBytes64(FQ, reverse(b1)), F.fromUniformBytes64(FQ, reverse(b2))};
    }

    private static IsoProjective mapToIsoCurve(final F u) {
      final F zU2 = SSWU_Z.mul(u.square());
      final F ta = zU2.square().add(zU2);
      final F numX1 = ISO_B.mul(ta.add(F.one(FQ)));
      final F div = ISO_A.mul(ta.isZero() ? SSWU_Z : ta.neg());
      final F numX1Squared = numX1.square();
      final F divSquared = div.square();
      final F divCubed = divSquared.mul(div);
      final F numGX1 = numX1Squared.add(ISO_A.mul(divSquared)).mul(numX1).add(ISO_B.mul(divCubed));
      final F numX2 = zU2.mul(numX1);
      final SqrtRatio sqrt = F.sqrtRatio(numGX1, divCubed);
      final F y1 = sqrt.root;
      final F y2 = THETA.mul(zU2).mul(u).mul(y1);
      final F numX = sqrt.isSquare ? numX1 : numX2;
      F y = sqrt.isSquare ? y1 : y2;
      if (u.isOdd() != y.isOdd()) {
        y = y.neg();
      }
      return new IsoProjective(numX.mul(div), y.mul(divCubed), div);
    }

    private static Projective isoMap(final IsoAffine point) {
      if (point.identity) {
        return Projective.identity();
      }
      final F x = point.x;
      final F y = point.y;
      final F numX = ISO[0].mul(x).add(ISO[1]).mul(x).add(ISO[2]).mul(x).add(ISO[3]);
      final F divX = x.add(ISO[4]).mul(x).add(ISO[5]);
      final F numY = ISO[6].mul(x).add(ISO[7]).mul(x).add(ISO[8]).mul(x).add(ISO[9]).mul(y);
      final F divY = x.add(ISO[10]).mul(x).add(ISO[11]).mul(x).add(ISO[12]);
      final F zOut = divX.mul(divY);
      final F xOut = numX.mul(divY).mul(zOut);
      final F yOut = numY.mul(divX).mul(zOut.square());
      return new Projective(xOut, yOut, zOut);
    }
  }

  private static final class Domain {
    private final int n;
    private final int quotientPolynomialDegree;
    private final F omega;
    private final F omegaInv;
    private final F nInv;

    private Domain(final int degree, final int k) {
      this.n = 1 << k;
      this.quotientPolynomialDegree = degree - 1;
      F extendedOmega = F.rootOfUnity(FP);
      int extendedK = k;
      while ((1 << extendedK) < n * quotientPolynomialDegree) {
        extendedK++;
      }
      for (int i = extendedK; i < FP.twoAdicity; i++) {
        extendedOmega = extendedOmega.square();
      }
      F omegaLocal = extendedOmega;
      for (int i = k; i < extendedK; i++) {
        omegaLocal = omegaLocal.square();
      }
      this.omega = omegaLocal;
      this.omegaInv = omegaLocal.invert();
      this.nInv = F.of(FP, n).invert();
    }

    private F[] lagrangeToCoeff(final F[] evaluations) {
      return fft(evaluations, omegaInv, nInv);
    }
  }

  private static F[] fft(final F[] input, final F root, final F scale) {
    final F[] values = Arrays.copyOf(input, input.length);
    bitReverse(values);
    int m = 1;
    while (m < values.length) {
      final F step = root.pow(BigInteger.valueOf(values.length / (2L * m)));
      for (int start = 0; start < values.length; start += 2 * m) {
        F w = F.one(FP);
        for (int offset = 0; offset < m; offset++) {
          final F even = values[start + offset];
          final F odd = values[start + offset + m].mul(w);
          values[start + offset] = even.add(odd);
          values[start + offset + m] = even.sub(odd);
          w = w.mul(step);
        }
      }
      m *= 2;
    }
    if (scale != null) {
      for (int i = 0; i < values.length; i++) {
        values[i] = values[i].mul(scale);
      }
    }
    return values;
  }

  private static void bitReverse(final Object[] values) {
    int j = 0;
    for (int i = 1; i < values.length; i++) {
      int bit = values.length >> 1;
      while ((j & bit) != 0) {
        j ^= bit;
        bit >>= 1;
      }
      j ^= bit;
      if (i < j) {
        final Object tmp = values[i];
        values[i] = values[j];
        values[j] = tmp;
      }
    }
  }

  private static final class Params {
    private final int k;
    private final int n;
    private final List<Affine> g;
    private final List<Affine> gLagrange;
    private final Affine w;
    private final Affine u;

    private Params(final int k, final List<Affine> g, final List<Affine> gLagrange, final Affine w, final Affine u) {
      this.k = k;
      this.n = 1 << k;
      this.g = g;
      this.gLagrange = gLagrange;
      this.w = w;
      this.u = u;
    }

    private static Params generated(final int k) {
      final int n = 1 << k;
      final List<Projective> gProjective = new ArrayList<>();
      for (int i = 0; i < n; i++) {
        final byte[] message = new byte[5];
        message[1] = (byte) (i & 0xFF);
        message[2] = (byte) ((i >>> 8) & 0xFF);
        message[3] = (byte) ((i >>> 16) & 0xFF);
        message[4] = (byte) ((i >>> 24) & 0xFF);
        gProjective.add(HashToCurve.hash("Halo2-Parameters", message));
      }
      final List<Affine> g = new ArrayList<>();
      for (final Projective point : gProjective) {
        g.add(point.toAffine());
      }
      final List<Affine> gLagrange = lagrangeBasisGenerators(gProjective, k);
      final Affine w = HashToCurve.hash("Halo2-Parameters", new byte[] {1}).toAffine();
      final Affine u = HashToCurve.hash("Halo2-Parameters", new byte[] {2}).toAffine();
      return new Params(k, g, gLagrange, w, u);
    }

    private Projective commit(final F[] coefficients, final F blind) {
      if (coefficients.length != n) {
        throw new IllegalArgumentException("invalid polynomial length");
      }
      final List<F> scalars = new ArrayList<>();
      final List<Affine> bases = new ArrayList<>();
      for (int i = 0; i < coefficients.length; i++) {
        scalars.add(coefficients[i]);
        bases.add(g.get(i));
      }
      scalars.add(blind);
      bases.add(w);
      return Projective.msm(scalars, bases);
    }

    private Projective commitLagrangeSparse(final List<SparseEntry> entries, final F blind) {
      Projective acc = Projective.identity();
      for (final SparseEntry entry : entries) {
        if (!entry.scalar.isZero()) {
          acc = acc.add(gLagrange.get(entry.index).projective().multiply(entry.scalar));
        }
      }
      if (!blind.isZero()) {
        acc = acc.add(w.projective().multiply(blind));
      }
      return acc;
    }

    private static List<Affine> lagrangeBasisGenerators(final List<Projective> coefficientGenerators, final int k) {
      F omegaInv = F.rootOfUnityInv(FP);
      for (int i = k; i < FP.twoAdicity; i++) {
        omegaInv = omegaInv.square();
      }
      final Projective[] values = coefficientGenerators.toArray(new Projective[0]);
      bitReverse(values);
      int m = 1;
      while (m < values.length) {
        final F step = omegaInv.pow(BigInteger.valueOf(values.length / (2L * m)));
        for (int start = 0; start < values.length; start += 2 * m) {
          F scalar = F.one(FP);
          for (int offset = 0; offset < m; offset++) {
            final Projective even = values[start + offset];
            final Projective odd = values[start + offset + m].multiply(scalar);
            values[start + offset] = even.add(odd);
            values[start + offset + m] = even.add(odd.neg());
            scalar = scalar.mul(step);
          }
        }
        m *= 2;
      }
      final F nInv = F.of(FP, values.length).invert();
      final List<Affine> out = new ArrayList<>();
      for (final Projective value : values) {
        out.add(value.multiply(nInv).toAffine());
      }
      return out;
    }
  }

  private static final class ProverQuery {
    private final F point;
    private final F[] polynomial;
    private final F blind;

    private ProverQuery(final F point, final F[] polynomial, final F blind) {
      this.point = point;
      this.polynomial = polynomial;
      this.blind = blind;
    }
  }

  private static final class VerifierQuery {
    private final Projective commitment;
    private final F eval;

    private VerifierQuery(final Projective commitment, final F eval) {
      this.commitment = commitment;
      this.eval = eval;
    }
  }

  private static final class IPA {
    private static void appendSamePointMultiOpeningProof(
        final Params params, final WriteTranscript transcript, final List<ProverQuery> queries) {
      final ProverQuery first = queries.get(0);
      final F x1 = transcript.squeezeChallenge().scalar;
      transcript.squeezeChallenge();
      F[] qPolynomial = Arrays.copyOf(first.polynomial, first.polynomial.length);
      F qBlind = first.blind;
      for (int i = 1; i < queries.size(); i++) {
        qPolynomial = add(scale(qPolynomial, x1), queries.get(i).polynomial);
        qBlind = qBlind.mul(x1).add(queries.get(i).blind);
      }
      F[] qPrimePolynomial = kateDivision(qPolynomial, first.point);
      qPrimePolynomial = Arrays.copyOf(qPrimePolynomial, params.n);
      for (int i = 0; i < qPrimePolynomial.length; i++) {
        if (qPrimePolynomial[i] == null) {
          qPrimePolynomial[i] = F.zero(FP);
        }
      }
      final F qPrimeBlind = randomScalar();
      transcript.writePoint(params.commit(qPrimePolynomial, qPrimeBlind).toAffine());
      final F x3 = transcript.squeezeChallenge().scalar;
      transcript.writeScalar(evaluatePolynomial(qPolynomial, x3));
      final F x4 = transcript.squeezeChallenge().scalar;
      final F[] pPolynomial = add(scale(qPrimePolynomial, x4), qPolynomial);
      final F pBlind = qPrimeBlind.mul(x4).add(qBlind);
      appendProof(params, transcript, pPolynomial, pBlind, x3);
    }

    private static void appendProof(
        final Params params, final WriteTranscript transcript, final F[] polynomial, final F blind, final F point) {
      final F[] sPolynomial = zeroes(FP, polynomial.length);
      for (int i = 0; i < sPolynomial.length; i++) {
        sPolynomial[i] = randomScalar();
      }
      final F sAtPoint = evaluatePolynomial(sPolynomial, point);
      sPolynomial[0] = sPolynomial[0].sub(sAtPoint);
      final F sBlind = randomScalar();
      transcript.writePoint(params.commit(sPolynomial, sBlind).toAffine());
      final F xi = transcript.squeezeChallenge().scalar;
      final F z = transcript.squeezeChallenge().scalar;
      F[] pPrime = add(scale(sPolynomial, xi), polynomial);
      final F v = evaluatePolynomial(pPrime, point);
      pPrime[0] = pPrime[0].sub(v);
      F f = sBlind.mul(xi).add(blind);
      F[] b = powers(point, params.n);
      List<Affine> gPrime = new ArrayList<>(params.g);
      for (int round = 0; round < params.k; round++) {
        final int half = 1 << (params.k - round - 1);
        final F[] pLo = Arrays.copyOfRange(pPrime, 0, half);
        final F[] pHi = Arrays.copyOfRange(pPrime, half, half * 2);
        final F[] bLo = Arrays.copyOfRange(b, 0, half);
        final F[] bHi = Arrays.copyOfRange(b, half, half * 2);
        final List<Affine> gLo = new ArrayList<>(gPrime.subList(0, half));
        final List<Affine> gHi = new ArrayList<>(gPrime.subList(half, half * 2));
        final F valueL = innerProduct(pHi, bLo);
        final F valueR = innerProduct(pLo, bHi);
        final F lRandomness = randomScalar();
        final F rRandomness = randomScalar();
        final List<F> lScalars = new ArrayList<>(Arrays.asList(pHi));
        final List<Affine> lBases = new ArrayList<>(gLo);
        lScalars.add(valueL.mul(z));
        lBases.add(params.u);
        lScalars.add(lRandomness);
        lBases.add(params.w);
        final List<F> rScalars = new ArrayList<>(Arrays.asList(pLo));
        final List<Affine> rBases = new ArrayList<>(gHi);
        rScalars.add(valueR.mul(z));
        rBases.add(params.u);
        rScalars.add(rRandomness);
        rBases.add(params.w);
        transcript.writePoint(Projective.msm(lScalars, lBases).toAffine());
        transcript.writePoint(Projective.msm(rScalars, rBases).toAffine());
        final F challenge = transcript.squeezeChallenge().scalar;
        final F challengeInv = challenge.invert();
        for (int i = 0; i < half; i++) {
          pPrime[i] = pPrime[i].add(pPrime[i + half].mul(challengeInv));
          b[i] = b[i].add(b[i + half].mul(challenge));
          gPrime.set(i, gPrime.get(i).projective().add(gPrime.get(i + half).projective().multiply(challenge)).toAffine());
        }
        pPrime = Arrays.copyOf(pPrime, half);
        b = Arrays.copyOf(b, half);
        gPrime = new ArrayList<>(gPrime.subList(0, half));
        f = f.add(lRandomness.mul(challengeInv)).add(rRandomness.mul(challenge));
      }
      transcript.writeScalar(pPrime[0]);
      transcript.writeScalar(f);
    }

    private static boolean verifyInTranscript(
        final Params params, final Projective commitment, final F point, final F value, final ReadTranscript transcript) {
      Projective accumulator = commitment.add(params.g.get(0).projective().multiply(value.neg()));
      final Affine sCommitment = transcript.readPoint();
      final F xi = transcript.squeezeChallenge().scalar;
      accumulator = accumulator.add(sCommitment.projective().multiply(xi));
      final F z = transcript.squeezeChallenge().scalar;
      final List<F> roundChallenges = new ArrayList<>();
      for (int i = 0; i < params.k; i++) {
        final Affine l = transcript.readPoint();
        final Affine r = transcript.readPoint();
        final F challenge = transcript.squeezeChallenge().scalar;
        accumulator = accumulator.add(l.projective().multiply(challenge.invert()));
        accumulator = accumulator.add(r.projective().multiply(challenge));
        roundChallenges.add(challenge);
      }
      final F c = transcript.readScalar();
      final F f = transcript.readScalar();
      final F b = computeB(point, roundChallenges);
      accumulator = accumulator.add(params.u.projective().multiply(c.neg().mul(b).mul(z)));
      accumulator = accumulator.add(params.w.projective().multiply(f.neg()));
      final F[] gScalars = computeS(roundChallenges, c.neg());
      for (int i = 0; i < gScalars.length; i++) {
        if (!gScalars[i].isZero()) {
          accumulator = accumulator.add(params.g.get(i).projective().multiply(gScalars[i]));
        }
      }
      return accumulator.toAffine().equals(Affine.IDENTITY);
    }

    private static F[] kateDivision(final F[] polynomial, final F point) {
      if (polynomial.length <= 1) {
        return new F[0];
      }
      final F negPoint = point.neg();
      final F[] quotient = zeroes(FP, polynomial.length - 1);
      F tmp = F.zero(FP);
      for (int i = quotient.length - 1; i >= 0; i--) {
        final F lead = polynomial[i + 1].sub(tmp);
        quotient[i] = lead;
        tmp = lead.mul(negPoint);
      }
      return quotient;
    }

    private static F innerProduct(final F[] lhs, final F[] rhs) {
      F out = F.zero(FP);
      for (int i = 0; i < lhs.length; i++) {
        out = out.add(lhs[i].mul(rhs[i]));
      }
      return out;
    }

    private static F[] powers(final F point, final int count) {
      final F[] powers = new F[count];
      F current = F.one(FP);
      for (int i = 0; i < count; i++) {
        powers[i] = current;
        current = current.mul(point);
      }
      return powers;
    }

    private static F computeB(final F point, final List<F> challenges) {
      F result = F.one(FP);
      F current = point;
      for (int i = challenges.size() - 1; i >= 0; i--) {
        result = result.mul(F.one(FP).add(challenges.get(i).mul(current)));
        current = current.mul(current);
      }
      return result;
    }

    private static F[] computeS(final List<F> challenges, final F initial) {
      final F[] values = zeroes(FP, 1 << challenges.size());
      values[0] = initial;
      for (int idx = 0; idx < challenges.size(); idx++) {
        final F challenge = challenges.get(challenges.size() - 1 - idx);
        final int len = 1 << idx;
        for (int slot = 0; slot < len; slot++) {
          values[len + slot] = values[slot].mul(challenge);
        }
      }
      return values;
    }
  }

  private static final class Challenge {
    private final F scalar;

    private Challenge(final F scalar) {
      this.scalar = scalar;
    }
  }

  private static final class WriteTranscript {
    private final ByteArrayOutputStream state = new ByteArrayOutputStream();
    private final ByteArrayOutputStream proof = new ByteArrayOutputStream();

    private void commonPoint(final Affine point) {
      if (point.identity) {
        throw new IllegalArgumentException("transcript point at infinity");
      }
      state.write(1);
      write(state, point.x.canonicalBytes());
      write(state, point.y.canonicalBytes());
    }

    private void commonScalar(final F scalar) {
      state.write(2);
      write(state, scalar.canonicalBytes());
    }

    private void writePoint(final Affine point) {
      commonPoint(point);
      write(proof, point.compressedBytes());
    }

    private void writeScalar(final F scalar) {
      commonScalar(scalar);
      write(proof, scalar.canonicalBytes());
    }

    private Challenge squeezeChallenge() {
      state.write(0);
      final byte[] digest = Blake2b.digest(state.toByteArray(), 64, "Halo2-Transcript".getBytes(StandardCharsets.UTF_8));
      return new Challenge(F.fromUniformBytes64(FP, digest));
    }

    private byte[] proofBytes() {
      return proof.toByteArray();
    }
  }

  private static final class ReadTranscript {
    private final byte[] proof;
    private int offset;
    private final ByteArrayOutputStream state = new ByteArrayOutputStream();

    private ReadTranscript(final byte[] proof) {
      this.proof = proof;
    }

    private void commonPoint(final Affine point) {
      if (point.identity) {
        throw new IllegalArgumentException("transcript point at infinity");
      }
      state.write(1);
      write(state, point.x.canonicalBytes());
      write(state, point.y.canonicalBytes());
    }

    private void commonScalar(final F scalar) {
      state.write(2);
      write(state, scalar.canonicalBytes());
    }

    private Affine readPoint() {
      final Affine point = Affine.fromCompressedBytes(read(32));
      if (point.identity) {
        throw new IllegalArgumentException("invalid transcript point");
      }
      commonPoint(point);
      return point;
    }

    private F readScalar() {
      final F scalar = F.fromCanonicalBytes(FP, read(32));
      commonScalar(scalar);
      return scalar;
    }

    private List<F> readScalars(final int count) {
      final List<F> out = new ArrayList<>();
      for (int i = 0; i < count; i++) {
        out.add(readScalar());
      }
      return out;
    }

    private Challenge squeezeChallenge() {
      state.write(0);
      final byte[] digest = Blake2b.digest(state.toByteArray(), 64, "Halo2-Transcript".getBytes(StandardCharsets.UTF_8));
      return new Challenge(F.fromUniformBytes64(FP, digest));
    }

    private int remainingBytes() {
      return proof.length - offset;
    }

    private byte[] read(final int count) {
      if (offset + count > proof.length) {
        throw new IllegalArgumentException("truncated proof");
      }
      final byte[] out = Arrays.copyOfRange(proof, offset, offset + count);
      offset += count;
      return out;
    }
  }

  private static byte[] reverse(final byte[] bytes) {
    final byte[] out = Arrays.copyOf(bytes, bytes.length);
    for (int i = 0; i < out.length / 2; i++) {
      final byte tmp = out[i];
      out[i] = out[out.length - 1 - i];
      out[out.length - 1 - i] = tmp;
    }
    return out;
  }

  private static final class Blake2b {
    private static final int BLOCK_BYTES = 128;
    private static final long[] IV = {
        0x6a09e667f3bcc908L, 0xbb67ae8584caa73bL, 0x3c6ef372fe94f82bL, 0xa54ff53a5f1d36f1L,
        0x510e527fade682d1L, 0x9b05688c2b3e6c1fL, 0x1f83d9abfb41bd6bL, 0x5be0cd19137e2179L
    };
    private static final byte[][] SIGMA = {
        {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15},
        {14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3},
        {11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4},
        {7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8},
        {9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13},
        {2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9},
        {12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11},
        {13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10},
        {6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5},
        {10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0},
        {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15},
        {14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3}
    };

    private static byte[] digest(final byte[] message, final int outLen, final byte[] personal) {
      final long[] h = IV.clone();
      h[0] ^= 0x01010000L ^ outLen;
      if (personal != null && personal.length > 0) {
        final byte[] buffer = new byte[16];
        System.arraycopy(personal, 0, buffer, 0, Math.min(personal.length, buffer.length));
        h[6] ^= readLongLE(buffer, 0);
        h[7] ^= readLongLE(buffer, 8);
      }
      long t0 = 0;
      long t1 = 0;
      if (message.length > 0) {
        int offset = 0;
        final int fullBlocks = message.length / BLOCK_BYTES;
        final int remainder = message.length % BLOCK_BYTES;
        for (int i = 0; i < fullBlocks; i++) {
          final boolean last = i == fullBlocks - 1 && remainder == 0;
          t0 += BLOCK_BYTES;
          if (t0 < BLOCK_BYTES) {
            t1++;
          }
          compress(h, message, offset, t0, t1, last);
          offset += BLOCK_BYTES;
        }
        if (remainder > 0) {
          final byte[] block = new byte[BLOCK_BYTES];
          System.arraycopy(message, message.length - remainder, block, 0, remainder);
          t0 += remainder;
          if (t0 < remainder) {
            t1++;
          }
          compress(h, block, 0, t0, t1, true);
        }
      } else {
        compress(h, new byte[BLOCK_BYTES], 0, 0, 0, true);
      }
      final byte[] out = new byte[outLen];
      int idx = 0;
      for (final long word : h) {
        long value = word;
        for (int i = 0; i < 8 && idx < outLen; i++) {
          out[idx++] = (byte) (value & 0xFFL);
          value >>>= 8;
        }
      }
      return out;
    }

    private static void compress(
        final long[] h, final byte[] block, final int offset, final long t0, final long t1, final boolean last) {
      final long[] m = new long[16];
      for (int i = 0; i < 16; i++) {
        m[i] = readLongLE(block, offset + i * 8);
      }
      final long[] v = new long[16];
      System.arraycopy(h, 0, v, 0, 8);
      System.arraycopy(IV, 0, v, 8, 8);
      v[12] ^= t0;
      v[13] ^= t1;
      if (last) {
        v[14] ^= -1L;
      }
      for (int r = 0; r < 12; r++) {
        final byte[] s = SIGMA[r];
        g(v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        g(v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        g(v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        g(v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        g(v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        g(v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        g(v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        g(v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
      }
      for (int i = 0; i < 8; i++) {
        h[i] ^= v[i] ^ v[i + 8];
      }
    }

    private static void g(
        final long[] v, final int a, final int b, final int c, final int d, final long x, final long y) {
      v[a] = v[a] + v[b] + x;
      v[d] = Long.rotateRight(v[d] ^ v[a], 32);
      v[c] = v[c] + v[d];
      v[b] = Long.rotateRight(v[b] ^ v[c], 24);
      v[a] = v[a] + v[b] + y;
      v[d] = Long.rotateRight(v[d] ^ v[a], 16);
      v[c] = v[c] + v[d];
      v[b] = Long.rotateRight(v[b] ^ v[c], 63);
    }

    private static long readLongLE(final byte[] bytes, final int offset) {
      long value = 0;
      for (int i = 0; i < 8; i++) {
        value |= (bytes[offset + i] & 0xFFL) << (i * 8);
      }
      return value;
    }
  }
}
