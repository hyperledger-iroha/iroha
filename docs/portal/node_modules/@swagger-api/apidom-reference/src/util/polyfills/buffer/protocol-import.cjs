"use strict";

exports.__esModule = true;
var _nodeBuffer = require("node:buffer");
Object.keys(_nodeBuffer).forEach(function (key) {
  if (key === "default" || key === "__esModule") return;
  if (key in exports && exports[key] === _nodeBuffer[key]) return;
  exports[key] = _nodeBuffer[key];
});