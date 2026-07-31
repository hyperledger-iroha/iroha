"use strict";

exports.__esModule = true;
var _fs = require("fs");
Object.keys(_fs).forEach(function (key) {
  if (key === "default" || key === "__esModule") return;
  if (key in exports && exports[key] === _fs[key]) return;
  exports[key] = _fs[key];
});