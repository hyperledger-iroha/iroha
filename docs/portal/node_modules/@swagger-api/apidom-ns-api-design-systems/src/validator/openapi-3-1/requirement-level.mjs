export const may = (value, values) => {
  if (value === null) return true;
  return values.includes(value);
};
export const must = (value, values) => {
  return values.includes(value);
};