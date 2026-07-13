package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.List;
import java.util.Objects;

/** Exact positive Kagemusha amount in authoritative asset-scale atomic units. */
public final class KagemushaScaledAmount {
  public static final int MAXIMUM_SCALE = 28;
  public static final String MAXIMUM_ATOMIC_UNITS =
      "340282366920938463463374607431768211455";
  private static final BigInteger MAXIMUM = new BigInteger(MAXIMUM_ATOMIC_UNITS);

  private final String atomicUnits;
  private final int scale;

  private KagemushaScaledAmount(final String atomicUnits, final int scale) {
    this.atomicUnits = atomicUnits;
    this.scale = scale;
  }

  public static KagemushaScaledAmount fromAtomicUnits(
      final String atomicUnits, final int scale) {
    requireScale(scale);
    if (!isCanonicalPositiveInteger(atomicUnits)) {
      throw new IllegalArgumentException("atomicUnits must be a canonical positive integer");
    }
    if (new BigInteger(atomicUnits).compareTo(MAXIMUM) > 0) {
      throw new IllegalArgumentException("atomicUnits must fit in u128");
    }
    return new KagemushaScaledAmount(atomicUnits, scale);
  }

  /** Converts exactly and rejects excess precision; this method never rounds. */
  public static KagemushaScaledAmount fromDecimal(final String decimal, final int scale) {
    requireScale(scale);
    Objects.requireNonNull(decimal, "decimal");
    final int separator = decimal.indexOf('.');
    if (decimal.isEmpty()
        || (separator >= 0 && separator != decimal.lastIndexOf('.'))) {
      throw new IllegalArgumentException("decimal must be canonical and positive");
    }
    final String whole = separator < 0 ? decimal : decimal.substring(0, separator);
    final String fractional = separator < 0 ? "" : decimal.substring(separator + 1);
    if (whole.isEmpty()
        || !isAsciiDigits(whole)
        || (!whole.equals("0") && whole.charAt(0) == '0')
        || (separator >= 0 && fractional.isEmpty())
        || !isAsciiDigits(fractional)) {
      throw new IllegalArgumentException("decimal must be canonical and positive");
    }
    if (fractional.length() > scale) {
      throw new IllegalArgumentException(
          "decimal has more fractional digits than the asset scale");
    }
    final StringBuilder combined = new StringBuilder(whole).append(fractional);
    for (int index = fractional.length(); index < scale; index++) {
      combined.append('0');
    }
    int firstNonZero = 0;
    while (firstNonZero < combined.length() && combined.charAt(firstNonZero) == '0') {
      firstNonZero++;
    }
    final String atomic = firstNonZero == combined.length()
        ? "0"
        : combined.substring(firstNonZero);
    return fromAtomicUnits(atomic, scale);
  }

  public static KagemushaScaledAmount sum(final List<KagemushaScaledAmount> amounts) {
    Objects.requireNonNull(amounts, "amounts");
    if (amounts.isEmpty()) {
      throw new IllegalArgumentException("amounts must not be empty");
    }
    final int scale = Objects.requireNonNull(amounts.get(0), "amount").scale;
    BigInteger total = BigInteger.ZERO;
    for (final KagemushaScaledAmount amount : amounts) {
      Objects.requireNonNull(amount, "amount");
      if (amount.scale != scale) {
        throw new IllegalArgumentException("amount scales must match");
      }
      total = total.add(new BigInteger(amount.atomicUnits));
      if (total.compareTo(MAXIMUM) > 0) {
        throw new IllegalArgumentException("amount sum must fit in u128");
      }
    }
    return fromAtomicUnits(total.toString(), scale);
  }

  public String atomicUnits() {
    return atomicUnits;
  }

  public int scale() {
    return scale;
  }

  /** Canonical Iroha Numeric spelling at the authoritative asset scale. */
  public String scaledNumericDecimal() {
    if (scale == 0) {
      return atomicUnits;
    }
    final StringBuilder padded = new StringBuilder(atomicUnits);
    while (padded.length() < scale + 1) {
      padded.insert(0, '0');
    }
    final int split = padded.length() - scale;
    return padded.substring(0, split) + "." + padded.substring(split);
  }

  /** Minimal user-facing decimal spelling without insignificant zeroes. */
  public String displayDecimal() {
    final String numeric = scaledNumericDecimal();
    if (scale == 0) {
      return numeric;
    }
    int end = numeric.length();
    while (end > 0 && numeric.charAt(end - 1) == '0') {
      end--;
    }
    if (end > 0 && numeric.charAt(end - 1) == '.') {
      end--;
    }
    return numeric.substring(0, end);
  }

  public KagemushaScaledAmount adding(final KagemushaScaledAmount other) {
    return sum(List.of(this, other));
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof KagemushaScaledAmount
        && atomicUnits.equals(((KagemushaScaledAmount) other).atomicUnits)
        && scale == ((KagemushaScaledAmount) other).scale;
  }

  @Override
  public int hashCode() {
    return 31 * atomicUnits.hashCode() + scale;
  }

  private static void requireScale(final int scale) {
    if (scale < 0 || scale > MAXIMUM_SCALE) {
      throw new IllegalArgumentException(
          "scale must be between 0 and " + MAXIMUM_SCALE);
    }
  }

  private static boolean isCanonicalPositiveInteger(final String value) {
    return value != null
        && !value.isEmpty()
        && isAsciiDigits(value)
        && !value.equals("0")
        && (value.length() == 1 || value.charAt(0) != '0');
  }

  private static boolean isAsciiDigits(final String value) {
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '0' || character > '9') {
        return false;
      }
    }
    return true;
  }
}
