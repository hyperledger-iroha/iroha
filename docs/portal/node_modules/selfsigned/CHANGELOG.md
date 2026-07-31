# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [5.0.0] - 2025-11-26

### 🚀 Major Rewrite

Complete rewrite replacing `node-forge` with modern `@peculiar/x509` and `pkijs` libraries.

### ✨ Added

- Native WebCrypto API support for better performance and security
- TypeScript examples in documentation
- Async/await support as the primary API
- Support for `keyPair` option to use existing keys
- Updated to use Node.js native crypto for all operations
- Separate `selfsigned/pkcs7` module for tree-shakeable PKCS#7 support

### 💥 BREAKING CHANGES

1. **Async-only API**: The `generate()` function now returns a Promise. Synchronous generation has been removed.
   ```js
   // Old (v4.x)
   const pems = selfsigned.generate(attrs, options);

   // New (v5.x)
   const pems = await selfsigned.generate(attrs, options);
   ```

2. **No callback support**: Callbacks have been completely removed in favor of Promises.
   ```js
   // Old (v4.x)
   selfsigned.generate(attrs, options, function(err, pems) { ... });

   // New (v5.x)
   const pems = await selfsigned.generate(attrs, options);
   ```

3. **Minimum Node.js version**: Now requires Node.js >= 15.6.0 (was >= 10)
   - Required for native WebCrypto support

4. **Dependencies changed**:
   - ❌ Removed: `node-forge` (1.64 MB)
   - ✅ Added: `@peculiar/x509` (551 KB) - 66% smaller!
   - ✅ Added: `pkijs` (1.94 MB, only for PKCS#7 support)
   - Bundle size reduced by 66% when not using PKCS#7

5. **PKCS#7 API changed**:
   - Old: `const pems = await generate(attrs, { pkcs7: true }); pems.pkcs7`
   - New: `const { createPkcs7 } = require('selfsigned/pkcs7'); const pkcs7 = createPkcs7(pems.cert);`
   - PKCS#7 is now a separate module for better tree-shaking

### 🔧 Changed

- Default key size remains 2048 bits (was incorrectly documented as 1024)
- PEM output uses `\n` line endings (was `\r\n`)
- Private keys now use PKCS#8 format (`BEGIN PRIVATE KEY` instead of `BEGIN RSA PRIVATE KEY`)
- Certificate generation is now fully async using native WebCrypto
- **PKCS#7 is now tree-shakeable**: Moved to separate `selfsigned/pkcs7` module so bundlers can exclude it when not used

### 🐛 Fixed

- Default key size documentation corrected from 1024 to 2048 bits
- Improved error handling for certificate generation failures

### 📦 Dependencies

**Removed:**
- `node-forge@^1.3.1`
- `@types/node-forge@^1.3.0`

**Added:**
- `@peculiar/x509@^1.14.2` (required)
- `pkijs@^3.3.3` (required, but tree-shakeable via separate `selfsigned/pkcs7` module)

### 🔒 Security

- Now uses Node.js native WebCrypto API instead of JavaScript implementation
- Better integration with platform security features
- More secure random number generation

### 📚 Documentation

- Complete README rewrite with async/await examples
- Added migration guide from v4.x to v5.x
- Updated all code examples to use async/await
- Added requirements section highlighting Node.js version requirement

---

## [4.0.0] - Previous Release

See git history for changes in 4.x and earlier versions.
