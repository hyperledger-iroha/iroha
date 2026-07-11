"use strict";

const crypto = require("crypto");
const fs = require("fs");
const Module = require("module");
const path = require("path");

function loadAuthenticatedCompiler() {
  const compilerPath = process.env.SCCP_SOLJSON_PATH;
  const expectedDigest = process.env.SCCP_SOLJSON_SHA256;
  if (!compilerPath || !path.isAbsolute(compilerPath)) {
    throw new Error("SCCP_SOLJSON_PATH must name an absolute verified compiler file");
  }
  if (!/^[0-9a-f]{64}$/.test(expectedDigest || "")) {
    throw new Error("SCCP_SOLJSON_SHA256 must be one lowercase SHA-256 digest");
  }
  const stat = fs.lstatSync(compilerPath);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size === 0) {
    throw new Error("authenticated soljson input must be one nonempty regular file");
  }
  const source = fs.readFileSync(compilerPath);
  const actualDigest = crypto.createHash("sha256").update(source).digest("hex");
  if (actualDigest !== expectedDigest) {
    throw new Error("authenticated soljson digest mismatch before execution");
  }

  // Compile the exact buffer that was hashed instead of reopening the path.
  const compilerModule = new Module(compilerPath, module);
  compilerModule.filename = compilerPath;
  compilerModule.paths = Module._nodeModulePaths(path.dirname(compilerPath));
  compilerModule._compile(source.toString("utf8"), compilerPath);
  const soljson = compilerModule.exports;
  if (!soljson || typeof soljson.cwrap !== "function") {
    throw new Error("authenticated soljson does not expose the expected compiler ABI");
  }
  const compile = soljson.cwrap("solidity_compile", "string", ["string", "number", "number"]);
  const version = soljson.cwrap("solidity_version", "string", []);
  return {
    compile(input) {
      if (typeof input !== "string") {
        throw new TypeError("standard-json compiler input must be a string");
      }
      return compile(input, 0, 0);
    },
    version() {
      return version();
    },
  };
}

module.exports = loadAuthenticatedCompiler();
