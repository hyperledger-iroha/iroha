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
