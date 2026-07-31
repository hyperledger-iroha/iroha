"use strict";

exports.__esModule = true;
var _index = require("../index.cjs");
Object.keys(_index).forEach(function (key) {
  if (key === "default" || key === "__esModule") return;
  if (key in exports && exports[key] === _index[key]) return;
  exports[key] = _index[key];
});