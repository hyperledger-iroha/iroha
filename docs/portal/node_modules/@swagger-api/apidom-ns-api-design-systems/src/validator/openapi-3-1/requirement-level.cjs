"use strict";

exports.__esModule = true;
exports.must = exports.may = void 0;
const may = (value, values) => {
  if (value === null) return true;
  return values.includes(value);
};
exports.may = may;
const must = (value, values) => {
  return values.includes(value);
};
exports.must = must;