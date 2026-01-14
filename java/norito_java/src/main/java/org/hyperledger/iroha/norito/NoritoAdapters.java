// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.AbstractMap;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/** Factory helpers for Norito adapters. */
public final class NoritoAdapters {
  private NoritoAdapters() {}

  // Reflection avoids unchecked casts while staying compatible with Android API levels < 26.
  private static final Method TYPE_ADAPTER_ENCODE;
  private static final Method TYPE_ADAPTER_DECODE;

  static {
    try {
      TYPE_ADAPTER_ENCODE =
          TypeAdapter.class.getMethod("encode", NoritoEncoder.class, Object.class);
      TYPE_ADAPTER_DECODE = TypeAdapter.class.getMethod("decode", NoritoDecoder.class);
    } catch (NoSuchMethodException ex) {
      throw new ExceptionInInitializerError(ex);
    }
  }

  private static void encodeAdapter(TypeAdapter<?> adapter, NoritoEncoder encoder, Object value) {
    try {
      TYPE_ADAPTER_ENCODE.invoke(adapter, encoder, value);
    } catch (InvocationTargetException ex) {
      Throwable target = ex.getTargetException();
      if (target instanceof RuntimeException runtime) {
        throw runtime;
      }
      if (target instanceof Error err) {
        throw err;
      }
      throw new IllegalStateException("Unable to encode value", target);
    } catch (IllegalAccessException ex) {
      throw new IllegalStateException("Unable to encode value", ex);
    }
  }

  private static Object decodeAdapter(TypeAdapter<?> adapter, NoritoDecoder decoder) {
    try {
      return TYPE_ADAPTER_DECODE.invoke(adapter, decoder);
    } catch (InvocationTargetException ex) {
      Throwable target = ex.getTargetException();
      if (target instanceof RuntimeException runtime) {
        throw runtime;
      }
      if (target instanceof Error err) {
        throw err;
      }
      throw new IllegalStateException("Unable to decode value", target);
    } catch (IllegalAccessException ex) {
      throw new IllegalStateException("Unable to decode value", ex);
    }
  }

  public static TypeAdapter<Long> uint(int bits) {
    return new UIntAdapter(bits);
  }

  public static TypeAdapter<Long> sint(int bits) {
    return new IntAdapter(bits);
  }

  public static TypeAdapter<Boolean> boolAdapter() {
    return BoolAdapter.INSTANCE;
  }

  public static TypeAdapter<byte[]> bytesAdapter() {
    return BytesAdapter.INSTANCE;
  }

  public static TypeAdapter<byte[]> byteVecAdapter() {
    return ByteVecAdapter.INSTANCE;
  }

  public static TypeAdapter<byte[]> fixedBytes(int length) {
    return new FixedBytesAdapter(length);
  }

  public static TypeAdapter<String> stringAdapter() {
    return StringAdapter.INSTANCE;
  }

  public static <T> TypeAdapter<Optional<T>> option(TypeAdapter<T> inner) {
    return new OptionAdapter<>(inner);
  }

  public static <T, E> TypeAdapter<Result<T, E>> result(TypeAdapter<T> ok, TypeAdapter<E> err) {
    return new ResultAdapter<>(ok, err);
  }

  public static <T> TypeAdapter<List<T>> sequence(TypeAdapter<T> element) {
    return new SequenceAdapter<>(element);
  }

  public static <K, V> TypeAdapter<Map<K, V>> map(TypeAdapter<K> key, TypeAdapter<V> value) {
    return new MapAdapter<>(key, value);
  }

  public static TypeAdapter<List<Object>> tuple(List<? extends TypeAdapter<?>> elements) {
    return new TupleAdapter(elements);
  }

  public static <T> StructField<T> field(String name, TypeAdapter<T> adapter) {
    return new StructField<>(name, adapter);
  }

  public static StructAdapter struct(
      List<? extends StructField<?>> fields, StructAdapter.StructFactory factory) {
    return new StructAdapter(fields, factory);
  }

  public static StructAdapter struct(List<? extends StructField<?>> fields) {
    return new StructAdapter(fields, null);
  }

  private static final class UIntAdapter implements TypeAdapter<Long> {
    private final int bits;

    private UIntAdapter(int bits) {
      if (bits != 8 && bits != 16 && bits != 32 && bits != 64) {
        throw new IllegalArgumentException("Unsupported unsigned size");
      }
      this.bits = bits;
    }

    @Override
    public void encode(NoritoEncoder encoder, Long value) {
      encoder.writeUInt(value, bits);
    }

    @Override
    public Long decode(NoritoDecoder decoder) {
      return decoder.readUInt(bits);
    }

    @Override
    public int fixedSize() {
      return bits / 8;
    }
  }

  private static final class IntAdapter implements TypeAdapter<Long> {
    private final int bits;

    private IntAdapter(int bits) {
      if (bits != 8 && bits != 16 && bits != 32 && bits != 64) {
        throw new IllegalArgumentException("Unsupported integer size");
      }
      this.bits = bits;
    }

    @Override
    public void encode(NoritoEncoder encoder, Long value) {
      encoder.writeInt(value, bits);
    }

    @Override
    public Long decode(NoritoDecoder decoder) {
      return decoder.readInt(bits);
    }

    @Override
    public int fixedSize() {
      return bits / 8;
    }
  }

  private static final class BoolAdapter implements TypeAdapter<Boolean> {
    private static final BoolAdapter INSTANCE = new BoolAdapter();

    @Override
    public void encode(NoritoEncoder encoder, Boolean value) {
      encoder.writeByte(Boolean.TRUE.equals(value) ? 1 : 0);
    }

    @Override
    public Boolean decode(NoritoDecoder decoder) {
      return decoder.readByte() != 0;
    }

    @Override
    public int fixedSize() {
      return 1;
    }
  }

  private static final class BytesAdapter implements TypeAdapter<byte[]> {
    private static final BytesAdapter INSTANCE = new BytesAdapter();

    @Override
    public void encode(NoritoEncoder encoder, byte[] value) {
      boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
      encoder.writeLength(value.length, compact);
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(NoritoDecoder decoder) {
      long length = decoder.readLength(decoder.compactLenActive());
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Byte array too large");
      }
      return decoder.readBytes((int) length);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class ByteVecAdapter implements TypeAdapter<byte[]> {
    private static final ByteVecAdapter INSTANCE = new ByteVecAdapter();

    @Override
    public void encode(NoritoEncoder encoder, byte[] value) {
      boolean compactSeq = (encoder.flags() & NoritoHeader.COMPACT_SEQ_LEN) != 0;
      encoder.writeLength(value.length, compactSeq);
      if ((encoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        encodePacked(encoder, value);
      } else {
        boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
        for (byte b : value) {
          encoder.writeLength(1, compactLen);
          encoder.writeByte(b);
        }
      }
    }

    @Override
    public byte[] decode(NoritoDecoder decoder) {
      long length = decoder.readLength(decoder.compactSeqLenActive());
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Byte vector too large");
      }
      int count = (int) length;
      if ((decoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        return decodePacked(decoder, count);
      }
      byte[] out = new byte[count];
      boolean compactLen = decoder.compactLenActive();
      for (int i = 0; i < count; i++) {
        long elemLen = decoder.readLength(compactLen);
        if (elemLen > Integer.MAX_VALUE) {
          throw new IllegalArgumentException("Byte vector element too large");
        }
        int elemSize = (int) elemLen;
        if (elemSize == 1) {
          out[i] = (byte) decoder.readByte();
          continue;
        }
        byte[] payload = decoder.readBytes(elemSize);
        NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
        int value = child.readByte();
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Byte vector element did not consume all bytes");
        }
        out[i] = (byte) value;
      }
      return out;
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }

    private void encodePacked(NoritoEncoder encoder, byte[] value) {
      if ((encoder.flags() & NoritoHeader.VARINT_OFFSETS) != 0) {
        for (int i = 0; i < value.length; i++) {
          encoder.append(Varint.encode(1));
        }
      } else {
        long offset = 0;
        encoder.writeUInt(offset, 64);
        for (int i = 0; i < value.length; i++) {
          offset += 1;
          encoder.writeUInt(offset, 64);
        }
      }
      encoder.writeBytes(value);
    }

    private byte[] decodePacked(NoritoDecoder decoder, int count) {
      if (count == 0) {
        int tailLen = decoder.remaining();
        if (tailLen == 0) {
          return new byte[0];
        }
        if (tailLen >= Long.BYTES) {
          byte[] prefix = decoder.readBytes(Long.BYTES);
          for (byte b : prefix) {
            if (b != 0) {
              throw new IllegalArgumentException(
                  "Packed byte vector declared zero length but carried trailing data");
            }
          }
          return new byte[0];
        }
        throw new IllegalArgumentException(
            "Packed byte vector declared zero length but carried trailing data");
      }

      boolean varintOffsets = (decoder.flags() & NoritoHeader.VARINT_OFFSETS) != 0;
      List<Integer> sizes = new ArrayList<>(count);
      if (varintOffsets) {
        for (int i = 0; i < count; i++) {
          long size = decoder.readVarint();
          if (size > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Packed byte element too large");
          }
          sizes.add((int) size);
        }
      } else {
        long previous = decoder.readUInt(64);
        if (previous != 0) {
          throw new IllegalArgumentException("Packed offsets must start at 0");
        }
        for (int i = 0; i < count; i++) {
          long current = decoder.readUInt(64);
          long delta = current - previous;
          if (delta < 0 || delta > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Invalid packed offsets");
          }
          sizes.add((int) delta);
          previous = current;
        }
      }

      byte[] out = new byte[count];
      for (int i = 0; i < count; i++) {
        int size = sizes.get(i);
        byte[] payload = decoder.readBytes(size);
        NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
        int value = child.readByte();
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Packed byte element did not consume all bytes");
        }
        out[i] = (byte) value;
      }
      return out;
    }
  }

  private static final class FixedBytesAdapter implements TypeAdapter<byte[]> {
    private final int length;

    private FixedBytesAdapter(int length) {
      if (length <= 0) {
        throw new IllegalArgumentException("length must be positive");
      }
      this.length = length;
    }

    @Override
    public void encode(NoritoEncoder encoder, byte[] value) {
      if (value.length != length) {
        throw new IllegalArgumentException(
            "expected " + length + " bytes, found " + value.length);
      }
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(NoritoDecoder decoder) {
      return decoder.readBytes(length);
    }

    @Override
    public int fixedSize() {
      return length;
    }
  }

  private static final class StringAdapter implements TypeAdapter<String> {
    private static final StringAdapter INSTANCE = new StringAdapter();

    @Override
    public void encode(NoritoEncoder encoder, String value) {
      byte[] data = value.getBytes(StandardCharsets.UTF_8);
      BytesAdapter.INSTANCE.encode(encoder, data);
    }

    @Override
    public String decode(NoritoDecoder decoder) {
      byte[] data = BytesAdapter.INSTANCE.decode(decoder);
      return new String(data, StandardCharsets.UTF_8);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class OptionAdapter<T> implements TypeAdapter<Optional<T>> {
    private final TypeAdapter<T> inner;

    private OptionAdapter(TypeAdapter<T> inner) {
      this.inner = inner;
    }

    @Override
    public void encode(NoritoEncoder encoder, Optional<T> value) {
      if (value.isPresent()) {
        encoder.writeByte(1);
        NoritoEncoder child = encoder.childEncoder();
        inner.encode(child, value.get());
        byte[] payload = child.toByteArray();
        boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
        encoder.writeLength(payload.length, compact);
        encoder.writeBytes(payload);
      } else {
        encoder.writeByte(0);
      }
    }

    @Override
    public Optional<T> decode(NoritoDecoder decoder) {
      int tag = decoder.readByte();
      if (tag == 0) {
        return Optional.empty();
      }
      if (tag != 1) {
        throw new IllegalArgumentException("Invalid Option tag: " + tag);
      }
      long length = decoder.readLength(decoder.compactLenActive());
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Option payload too large");
      }
      byte[] payload = decoder.readBytes((int) length);
      NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
      T value = inner.decode(child);
      if (child.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Option payload");
      }
      return Optional.of(value);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class ResultAdapter<T, E> implements TypeAdapter<Result<T, E>> {
    private final TypeAdapter<T> ok;
    private final TypeAdapter<E> err;

    private ResultAdapter(TypeAdapter<T> ok, TypeAdapter<E> err) {
      this.ok = ok;
      this.err = err;
    }

    @Override
    public void encode(NoritoEncoder encoder, Result<T, E> value) {
      if (value instanceof Result.Ok<T, E> okValue) {
        encoder.writeByte(0);
        NoritoEncoder child = encoder.childEncoder();
        ok.encode(child, okValue.value());
        byte[] payload = child.toByteArray();
        boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
        encoder.writeLength(payload.length, compact);
        encoder.writeBytes(payload);
      } else if (value instanceof Result.Err<T, E> errValue) {
        encoder.writeByte(1);
        NoritoEncoder child = encoder.childEncoder();
        err.encode(child, errValue.error());
        byte[] payload = child.toByteArray();
        boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
        encoder.writeLength(payload.length, compact);
        encoder.writeBytes(payload);
      } else {
        throw new IllegalArgumentException("Unknown Result variant");
      }
    }

    @Override
    public Result<T, E> decode(NoritoDecoder decoder) {
      int tag = decoder.readByte();
      if (tag != 0 && tag != 1) {
        throw new IllegalArgumentException("Invalid Result tag: " + tag);
      }
      long length = decoder.readLength(decoder.compactLenActive());
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Result payload too large");
      }
      byte[] payload = decoder.readBytes((int) length);
      NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
      Object value = tag == 0 ? ok.decode(child) : err.decode(child);
      if (child.remaining() != 0) {
        throw new IllegalArgumentException("Trailing bytes after Result payload");
      }
      if (tag == 0) {
        @SuppressWarnings("unchecked")
        T okValue = (T) value;
        return new Result.Ok<>(okValue);
      }
      @SuppressWarnings("unchecked")
      E errValue = (E) value;
      return new Result.Err<>(errValue);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class SequenceAdapter<T> implements TypeAdapter<List<T>> {
    private final TypeAdapter<T> element;

    private SequenceAdapter(TypeAdapter<T> element) {
      this.element = element;
    }

    @Override
    public void encode(NoritoEncoder encoder, List<T> values) {
      boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_SEQ_LEN) != 0;
      encoder.writeLength(values.size(), compactLen);
      if ((encoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        encodePacked(encoder, values);
      } else {
        boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
        for (T value : values) {
          NoritoEncoder child = encoder.childEncoder();
          element.encode(child, value);
          byte[] payload = child.toByteArray();
          encoder.writeLength(payload.length, compact);
          encoder.writeBytes(payload);
        }
      }
    }

    private void encodePacked(NoritoEncoder encoder, List<T> values) {
      List<byte[]> encodedElements = new ArrayList<>(values.size());
      for (T value : values) {
        NoritoEncoder child = encoder.childEncoder();
        element.encode(child, value);
        encodedElements.add(child.toByteArray());
      }
      if ((encoder.flags() & NoritoHeader.VARINT_OFFSETS) != 0) {
        for (byte[] chunk : encodedElements) {
          encoder.append(Varint.encode(chunk.length));
        }
      } else {
        long offset = 0;
        encoder.writeUInt(offset, 64);
        for (byte[] chunk : encodedElements) {
          offset += chunk.length;
          encoder.writeUInt(offset, 64);
        }
      }
      for (byte[] chunk : encodedElements) {
        encoder.append(chunk);
      }
    }

    @Override
    public List<T> decode(NoritoDecoder decoder) {
      long length = decoder.readLength(decoder.compactSeqLenActive());
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Sequence too large");
      }
      int count = (int) length;
      if ((decoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        return decodePacked(decoder, count);
      }
      List<T> values = new ArrayList<>(count);
      boolean compact = decoder.compactLenActive();
      for (int i = 0; i < count; i++) {
        long elemLen = decoder.readLength(compact);
        if (elemLen > Integer.MAX_VALUE) {
          throw new IllegalArgumentException("Sequence element too large");
        }
        byte[] payload = decoder.readBytes((int) elemLen);
        NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
        T value = element.decode(child);
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Sequence element did not consume all bytes");
        }
        values.add(value);
      }
      return values;
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }

    private List<T> decodePacked(NoritoDecoder decoder, int count) {
      boolean varintOffsets = (decoder.flags() & NoritoHeader.VARINT_OFFSETS) != 0;
      if (count == 0) {
        int tailLen = decoder.remaining();
        if (tailLen == 0) {
          return Collections.emptyList();
        }
        if (tailLen >= Long.BYTES) {
          byte[] prefix = decoder.readBytes(Long.BYTES);
          for (byte b : prefix) {
            if (b != 0) {
              throw new IllegalArgumentException(
                  "Packed sequence declared zero length but carried trailing data");
            }
          }
          return Collections.emptyList();
        }
        throw new IllegalArgumentException(
            "Packed sequence declared zero length but carried trailing data");
      }

      List<Integer> sizes = new ArrayList<>(count);
      if (varintOffsets) {
        for (int i = 0; i < count; i++) {
          long size = decoder.readVarint();
          if (size > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Packed element too large");
          }
          sizes.add((int) size);
        }
      } else {
        long previous = decoder.readUInt(64);
        if (previous != 0) {
          throw new IllegalArgumentException("Packed offsets must start at 0");
        }
        for (int i = 0; i < count; i++) {
          long current = decoder.readUInt(64);
          long delta = current - previous;
          if (delta < 0 || delta > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Invalid packed offsets");
          }
          sizes.add((int) delta);
          previous = current;
        }
      }

      List<T> values = new ArrayList<>(count);
      for (int size : sizes) {
        byte[] chunk = decoder.readBytes(size);
        NoritoDecoder child = new NoritoDecoder(chunk, decoder.flags(), decoder.flagsHint());
        T value = element.decode(child);
        if (child.remaining() != 0) {
          throw new IllegalArgumentException("Packed element did not consume all bytes");
        }
        values.add(value);
      }
      return values;
    }
  }

  private static final class MapAdapter<K, V> implements TypeAdapter<Map<K, V>> {
    private final TypeAdapter<K> key;
    private final TypeAdapter<V> value;

    private MapAdapter(TypeAdapter<K> key, TypeAdapter<V> value) {
      this.key = key;
      this.value = value;
    }

    @Override
    public void encode(NoritoEncoder encoder, Map<K, V> map) {
      List<Map.Entry<K, V>> entries = sortedEntries(map);
      boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_SEQ_LEN) != 0;
      encoder.writeLength(entries.size(), compactLen);
      if ((encoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        encodePacked(encoder, entries);
      } else {
        encodeCompat(encoder, entries);
      }
    }

    @Override
    public Map<K, V> decode(NoritoDecoder decoder) {
      long length = decoder.readLength(decoder.compactSeqLenActive());
      if (length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("Map too large");
      }
      int count = (int) length;
      if ((decoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
        return decodePacked(decoder, count);
      }
      return decodeCompat(decoder, count);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }

    private Map<K, V> decodeCompat(final NoritoDecoder decoder, final int count) {
      Map<K, V> map = new LinkedHashMap<>(count);
      boolean compactLen = decoder.compactLenActive();
      for (int i = 0; i < count; i++) {
        long keyLen = decoder.readLength(compactLen);
        if (keyLen > Integer.MAX_VALUE) {
          throw new IllegalArgumentException("Map key too large");
        }
        K decodedKey = decodeSizedField(key, decoder, (int) keyLen);
        if (map.containsKey(decodedKey)) {
          throw new IllegalArgumentException("Duplicate map key");
        }
        long valueLen = decoder.readLength(compactLen);
        if (valueLen > Integer.MAX_VALUE) {
          throw new IllegalArgumentException("Map value too large");
        }
        V decodedValue = decodeSizedField(value, decoder, (int) valueLen);
        map.put(decodedKey, decodedValue);
      }
      return map;
    }

    private Map<K, V> decodePacked(final NoritoDecoder decoder, final int count) {
      boolean varintOffsets = (decoder.flags() & NoritoHeader.VARINT_OFFSETS) != 0;
      List<Integer> keySizes;
      List<Integer> valueSizes;
      if (varintOffsets) {
        keySizes = readVarintSizes(decoder, count, "Map key");
        valueSizes = readVarintSizes(decoder, count, "Map value");
      } else {
        keySizes = readFixedOffsets(decoder, count, "Map key");
        valueSizes = readFixedOffsets(decoder, count, "Map value");
      }

      List<K> keys = new ArrayList<>(count);
      for (int size : keySizes) {
        keys.add(decodeSizedField(key, decoder, size));
      }

      Map<K, V> map = new LinkedHashMap<>(count);
      for (int i = 0; i < count; i++) {
        V decodedValue = decodeSizedField(value, decoder, valueSizes.get(i));
        K decodedKey = keys.get(i);
        if (map.containsKey(decodedKey)) {
          throw new IllegalArgumentException("Duplicate map key");
        }
        map.put(decodedKey, decodedValue);
      }
      return map;
    }

    private List<Map.Entry<K, V>> sortedEntries(final Map<K, V> map) {
      List<Map.Entry<K, V>> entries = new ArrayList<>(map.size());
      entries.addAll(map.entrySet());
      for (Map.Entry<K, V> entry : entries) {
        if (entry.getKey() == null) {
          throw new IllegalArgumentException("Map keys must be non-null");
        }
      }
      if (map instanceof java.util.SortedMap<?, ?>) {
        return entries;
      }
      entries.sort((left, right) -> compareKeys(left.getKey(), right.getKey()));
      return entries;
    }

    @SuppressWarnings("unchecked")
    private int compareKeys(final K left, final K right) {
      if (left instanceof Comparable<?> comparable) {
        try {
          return ((Comparable<Object>) comparable).compareTo(right);
        } catch (final ClassCastException ex) {
          throw new IllegalArgumentException(
              "Map keys must be Comparable for deterministic encoding", ex);
        }
      }
      throw new IllegalArgumentException("Map keys must be Comparable for deterministic encoding");
    }

    private void encodeCompat(final NoritoEncoder encoder, final List<Map.Entry<K, V>> entries) {
      boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
      for (Map.Entry<K, V> entry : entries) {
        byte[] keyBytes = encodeField(encoder, key, entry.getKey());
        encoder.writeLength(keyBytes.length, compactLen);
        encoder.append(keyBytes);

        byte[] valueBytes = encodeField(encoder, value, entry.getValue());
        encoder.writeLength(valueBytes.length, compactLen);
        encoder.append(valueBytes);
      }
    }

    private void encodePacked(final NoritoEncoder encoder, final List<Map.Entry<K, V>> entries) {
      boolean varintOffsets = (encoder.flags() & NoritoHeader.VARINT_OFFSETS) != 0;
      if (entries.isEmpty()) {
        if (!varintOffsets) {
          writeFixedOffsets(encoder, List.of());
          writeFixedOffsets(encoder, List.of());
        }
        return;
      }
      List<Integer> keySizes = new ArrayList<>(entries.size());
      List<Integer> valueSizes = new ArrayList<>(entries.size());
      List<byte[]> keyPayloads = new ArrayList<>(entries.size());
      List<byte[]> valuePayloads = new ArrayList<>(entries.size());
      for (Map.Entry<K, V> entry : entries) {
        byte[] keyBytes = encodeField(encoder, key, entry.getKey());
        byte[] valueBytes = encodeField(encoder, value, entry.getValue());
        keySizes.add(keyBytes.length);
        valueSizes.add(valueBytes.length);
        keyPayloads.add(keyBytes);
        valuePayloads.add(valueBytes);
      }
      if (varintOffsets) {
        writeVarintSizes(encoder, keySizes);
        writeVarintSizes(encoder, valueSizes);
      } else {
        writeFixedOffsets(encoder, keySizes);
        writeFixedOffsets(encoder, valueSizes);
      }
      for (byte[] keyBytes : keyPayloads) {
        encoder.append(keyBytes);
      }
      for (byte[] valueBytes : valuePayloads) {
        encoder.append(valueBytes);
      }
    }

    private byte[] encodeField(
        final NoritoEncoder encoder, final TypeAdapter<?> adapter, final Object value) {
      NoritoEncoder child = encoder.childEncoder();
      encodeAdapter(adapter, child, value);
      return child.toByteArray();
    }

    private <T> T decodeSizedField(
        final TypeAdapter<T> adapter, final NoritoDecoder decoder, final int size) {
      byte[] chunk = decoder.readBytes(size);
      NoritoDecoder child = new NoritoDecoder(chunk, decoder.flags(), decoder.flagsHint());
      T value = adapter.decode(child);
      if (child.remaining() != 0) {
        throw new IllegalArgumentException("Map entry did not consume all bytes");
      }
      return value;
    }

    private List<Integer> readVarintSizes(
        final NoritoDecoder decoder, final int count, final String label) {
      List<Integer> sizes = new ArrayList<>(count);
      for (int i = 0; i < count; i++) {
        long size = decoder.readVarint();
        if (size > Integer.MAX_VALUE) {
          throw new IllegalArgumentException(label + " too large");
        }
        sizes.add((int) size);
      }
      return sizes;
    }

    private List<Integer> readFixedOffsets(
        final NoritoDecoder decoder, final int count, final String label) {
      List<Integer> sizes = new ArrayList<>(count);
      long previous = decoder.readUInt(64);
      if (previous != 0) {
        throw new IllegalArgumentException(label + " offsets must start at 0");
      }
      for (int i = 0; i < count; i++) {
        long current = decoder.readUInt(64);
        long delta = current - previous;
        if (delta < 0 || delta > Integer.MAX_VALUE) {
          throw new IllegalArgumentException("Invalid " + label + " offsets");
        }
        sizes.add((int) delta);
        previous = current;
      }
      return sizes;
    }

    private void writeVarintSizes(final NoritoEncoder encoder, final List<Integer> sizes) {
      for (int size : sizes) {
        encoder.append(Varint.encode(size));
      }
    }

    private void writeFixedOffsets(final NoritoEncoder encoder, final List<Integer> sizes) {
      long offset = 0;
      encoder.writeUInt(offset, 64);
      for (int size : sizes) {
        offset += size;
        encoder.writeUInt(offset, 64);
      }
    }
  }

  private static final class EntryAdapter<K, V> implements TypeAdapter<Map.Entry<K, V>> {
    private final TypeAdapter<K> key;
    private final TypeAdapter<V> value;

    private EntryAdapter(TypeAdapter<K> key, TypeAdapter<V> value) {
      this.key = key;
      this.value = value;
    }

    @Override
    public void encode(NoritoEncoder encoder, Map.Entry<K, V> entry) {
      key.encode(encoder, entry.getKey());
      value.encode(encoder, entry.getValue());
    }

    @Override
    public Map.Entry<K, V> decode(NoritoDecoder decoder) {
      K decodedKey = key.decode(decoder);
      V decodedValue = value.decode(decoder);
      return new AbstractMap.SimpleImmutableEntry<>(decodedKey, decodedValue);
    }

    @Override
    public int fixedSize() {
      int keySize = key.fixedSize();
      int valueSize = value.fixedSize();
      if (keySize >= 0 && valueSize >= 0) {
        return keySize + valueSize;
      }
      return -1;
    }

    @Override
    public boolean isSelfDelimiting() {
      return key.isSelfDelimiting() && value.isSelfDelimiting();
    }
  }

  private static final class TupleAdapter implements TypeAdapter<List<Object>> {
    private final List<TypeAdapter<?>> elements;

    private TupleAdapter(List<? extends TypeAdapter<?>> elements) {
      this.elements = List.copyOf(elements);
    }

    @Override
    public void encode(NoritoEncoder encoder, List<Object> value) {
      if (value.size() != elements.size()) {
        throw new IllegalArgumentException("Tuple size mismatch");
      }
      for (int i = 0; i < elements.size(); i++) {
        encodeAdapter(elements.get(i), encoder, value.get(i));
      }
    }

    @Override
    public List<Object> decode(NoritoDecoder decoder) {
      List<Object> values = new ArrayList<>(elements.size());
      for (TypeAdapter<?> element : elements) {
        values.add(decodeAdapter(element, decoder));
      }
      return values;
    }

    @Override
    public boolean isSelfDelimiting() {
      for (TypeAdapter<?> element : elements) {
        if (!element.isSelfDelimiting()) {
          return false;
        }
      }
      return true;
    }
  }

  public static final class StructField<T> {
    private final String name;
    private final TypeAdapter<T> adapter;

    public StructField(String name, TypeAdapter<T> adapter) {
      this.name = Objects.requireNonNull(name);
      this.adapter = Objects.requireNonNull(adapter);
    }

    public String name() {
      return name;
    }

    public TypeAdapter<T> adapter() {
      return adapter;
    }
  }

  public static final class StructAdapter implements TypeAdapter<Object> {
    @FunctionalInterface
    public interface StructFactory {
      Object create(Map<String, Object> fields);
    }

    private final List<StructField<?>> fields;
    private final StructFactory factory;

    private StructAdapter(List<? extends StructField<?>> fields, StructFactory factory) {
      this.fields = List.copyOf(fields);
      this.factory = factory;
    }

    @Override
    public void encode(NoritoEncoder encoder, Object value) {
      if ((encoder.flags() & NoritoHeader.PACKED_STRUCT) != 0
          && (encoder.flags() & NoritoHeader.FIELD_BITSET) != 0) {
        encodePacked(encoder, value);
        return;
      }
      for (StructField<?> field : fields) {
        Object fieldValue = extractField(value, field.name());
        encodeAdapter(field.adapter(), encoder, fieldValue);
      }
    }

    private void encodePacked(NoritoEncoder encoder, Object value) {
      List<byte[]> payloads = new ArrayList<>(fields.size());
      int bitset = 0;
      for (int i = 0; i < fields.size(); i++) {
        StructField<?> field = fields.get(i);
        Object fieldValue = extractField(value, field.name());
        NoritoEncoder child = encoder.childEncoder();
        encodeAdapter(field.adapter(), child, fieldValue);
        byte[] bytes = child.toByteArray();
        payloads.add(bytes);
        if (needsExplicitSize(field.adapter())) {
          bitset |= (1 << i);
        }
      }
      int bitsetBytes = (fields.size() + 7) / 8;
      for (int i = 0; i < bitsetBytes; i++) {
        encoder.writeByte((bitset >> (i * 8)) & 0xFF);
      }
      for (int i = 0; i < fields.size(); i++) {
        if ((bitset & (1 << i)) != 0) {
          encoder.append(Varint.encode(payloads.get(i).length));
        }
      }
      for (byte[] bytes : payloads) {
        encoder.append(bytes);
      }
    }

    @Override
    public Object decode(NoritoDecoder decoder) {
      Map<String, Object> values = new LinkedHashMap<>();
      if ((decoder.flags() & NoritoHeader.PACKED_STRUCT) != 0
          && (decoder.flags() & NoritoHeader.FIELD_BITSET) != 0) {
        decodePacked(decoder, values);
      } else {
        for (StructField<?> field : fields) {
          Object value = decodeAdapter(field.adapter(), decoder);
          values.put(field.name(), value);
        }
      }
      if (factory != null) {
        return factory.create(values);
      }
      return values;
    }

    private void decodePacked(NoritoDecoder decoder, Map<String, Object> values) {
      int bitsetBytes = (fields.size() + 7) / 8;
      byte[] bitsetData = decoder.readBytes(bitsetBytes);
      int bitset = 0;
      for (int i = 0; i < bitsetBytes; i++) {
        bitset |= (bitsetData[i] & 0xFF) << (i * 8);
      }
      List<Integer> encodedSizes = new ArrayList<>();
      for (int i = 0; i < fields.size(); i++) {
        if ((bitset & (1 << i)) != 0) {
          long size = decoder.readVarint();
          if (size > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Packed field too large");
          }
          encodedSizes.add((int) size);
        } else {
          encodedSizes.add(null);
        }
      }
      for (int i = 0; i < fields.size(); i++) {
        StructField<?> field = fields.get(i);
        TypeAdapter<?> adapter = field.adapter();
        Integer size = encodedSizes.get(i);
        Object value;
        if (size != null) {
          byte[] chunk = decoder.readBytes(size);
          NoritoDecoder child = new NoritoDecoder(chunk, decoder.flags(), decoder.flagsHint());
          value = decodeAdapter(adapter, child);
          if (child.remaining() != 0) {
            throw new IllegalArgumentException("Packed field did not consume all bytes");
          }
        } else {
          value = decodeAdapter(adapter, decoder);
        }
        values.put(field.name(), value);
      }
    }

    private static boolean needsExplicitSize(TypeAdapter<?> adapter) {
      if (adapter.fixedSize() >= 0) {
        return false;
      }
      return !adapter.isSelfDelimiting();
    }

    private static Object extractField(Object value, String name) {
      if (value instanceof Map<?, ?> map) {
        return map.get(name);
      }
      try {
        Method method = value.getClass().getMethod(name);
        return method.invoke(value);
      } catch (NoSuchMethodException missing) {
        String candidate = "get" + Character.toUpperCase(name.charAt(0)) + name.substring(1);
        try {
          Method method = value.getClass().getMethod(candidate);
          return method.invoke(value);
        } catch (Exception inner) {
          throw new IllegalArgumentException("Unable to extract field " + name, inner);
        }
      } catch (Exception ex) {
        throw new IllegalArgumentException("Unable to extract field " + name, ex);
      }
    }
  }
}
