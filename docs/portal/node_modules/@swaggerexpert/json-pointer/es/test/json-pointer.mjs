import parse from "../parse/index.mjs";
const testJSONPointer = jsonPointer => {
  try {
    const parseResult = parse(jsonPointer, {
      translator: null
    });
    return parseResult.result.success;
  } catch {
    return false;
  }
};
export default testJSONPointer;