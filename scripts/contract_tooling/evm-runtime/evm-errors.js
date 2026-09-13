"use strict";

// Strict revert recognition for ethers errors from the pinned native EDR owner.
const { AbiCoder } = require("ethers");
const HEX = /^0x(?:[0-9a-fA-F]{2})*$/;

function rejectedWith(reason) {
  return (error) => {
    const candidates = [error, error?.error, error?.info?.error, error?.cause, error?.cause?.error]
      .filter((value) => value && typeof value === "object");
    for (const candidate of candidates) {
      let data;
      if (candidate.code === "CALL_EXCEPTION") {
        data = candidate.data;
      } else if (candidate.code === -32000 && candidate.data && typeof candidate.data === "object") {
        const native = candidate.data;
        if (native.reason && typeof native.reason === "object"
            && Object.keys(native.reason).length === 1 && native.reason.Revert === native.data) {
          data = native.data;
        }
      }
      if (typeof data !== "string" || !HEX.test(data)) continue;
      if (reason === undefined) return true;
      if (!data.startsWith("0x08c379a0")) continue;
      try {
        const encoded = `0x${data.slice(10)}`;
        const [decoded] = AbiCoder.defaultAbiCoder().decode(["string"], encoded);
        if (AbiCoder.defaultAbiCoder().encode(["string"], [decoded]) === encoded && decoded === reason) return true;
      } catch (_error) {
        // Malformed revert bytes never satisfy a negative contract assertion.
      }
    }
    return false;
  };
}

module.exports = { rejectedWith };
