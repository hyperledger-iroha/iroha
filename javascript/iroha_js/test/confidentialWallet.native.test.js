import test from "node:test";
import * as sdk from "../src/index.js";
import { confidentialWalletNativeCases } from "./helpers/confidentialWalletNativeCases.js";

// The real public constructor fails if its native artifact is missing or invalid.
for (const [name, run] of confidentialWalletNativeCases(sdk)) test(name, run);
