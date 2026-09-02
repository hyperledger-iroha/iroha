# Iroha Kagemusha Wallet SDK — consumer ProGuard/R8 rules
#
# This wrapper currently exposes the client Android SDK without adding classes
# that require module-specific retention rules. The client/core artifacts carry
# their own JNI, cryptography, and serialization rules transitively.
