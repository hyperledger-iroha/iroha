export const to = jsonPointer => {
  return JSON.stringify(jsonPointer);
};
export const from = jsonString => {
  try {
    return String(JSON.parse(jsonString));
  } catch {
    return jsonString;
  }
};