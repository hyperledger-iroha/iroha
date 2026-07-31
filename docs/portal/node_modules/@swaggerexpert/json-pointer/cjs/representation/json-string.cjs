"use strict";

exports.__esModule = true;
exports.to = exports.from = void 0;
const to = jsonPointer => {
  return JSON.stringify(jsonPointer);
};
exports.to = to;
const from = jsonString => {
  try {
    return String(JSON.parse(jsonString));
  } catch {
    return jsonString;
  }
};
exports.from = from;