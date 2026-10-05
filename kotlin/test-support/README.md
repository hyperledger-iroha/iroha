# Shared JVM test support

The JVM and Android unit source sets compile these test helpers directly.
`JvmApiInventory` reads compiled classfile contracts without reflective access.
This directory is included only in test compilation and adds no SDK runtime dependency.
