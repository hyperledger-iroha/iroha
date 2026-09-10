/** Bind ordered Norito record decoding to the existing primitive wire codecs. */
export function createNoritoRecordDecoder(decodeStructFields, decodeOptionValue, decodeNoritoVec) {
  function decodeRecordFields(payload, context, schema) {
    const fields = decodeStructFields(payload, context, schema.map(([name]) => name));
    return Object.fromEntries(schema.map(([name, decode, kind]) => {
      const path = `${context}.${name}`;
      const bytes = fields[name];
      const value = kind === 1 ? decodeOptionValue(bytes, decode, path)
        : kind === 2 ? decodeNoritoVec(bytes, (entry, index) => decode(entry, `${path}[${index}]`), path)
          : decode(bytes, path);
      return [name, value];
    }));
  }
  return decodeRecordFields;
}

/** Bind canonical encoding to the same ordered field tables and wire primitives. */
export function createNoritoRecordEncoder(encodeStructValue, encodeOptionValue, encodeNoritoVec) {
  function encodeCanonicalRecordFields(record, context, schema, parent) {
    return encodeStructValue(schema.map(([name, , , encode, kind]) => {
      const value = parent === undefined ? record[name] : record[parent][name];
      const path = kind === 2 || kind === 3 ? null : `${context}.${name}`;
      return [kind === 1 ? encodeOptionValue(value, encode, path)
        : kind === 2 || kind === 3 ? encodeNoritoVec(kind === 3 ? value ?? [] : value, (entry, index) => encode(entry, `${context}.${name}[${index}]`))
          : encode(value, path)];
    }));
  }
  return encodeCanonicalRecordFields;
}
