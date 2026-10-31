// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import globals from "globals";

export default [
  { ignores: ["dist/**", "native/**", "target-cargo/**", "**/.*"] },
  {
    files: ["**/*.{js,mjs}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: globals.node,
    },
    // Preserve the existing policy for directives on rules not enabled here.
    linterOptions: { reportUnusedDisableDirectives: false },
  },
  {
    files: ["src/**/*.js"],
    languageOptions: { globals: globals.browser },
    rules: {
      "no-undef": "error",
      "no-unused-vars": [
        "error",
        {
          argsIgnorePattern: "^_",
          caughtErrors: "none",
          caughtErrorsIgnorePattern: "^_",
          destructuredArrayIgnorePattern: "^_",
          varsIgnorePattern: "^_",
        },
      ],
    },
  },
];
