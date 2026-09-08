Library provides single value and key-value map structures.

For the execution flow specific for the blockchains:
1. transactions are grouped in blocks and executed one by one (every transaction is atomic)
2. blocks are committed sequentially (every block is atomic as well so either effect of every successful transaction is visible or no effect)

Features:
- single writer/multiple readers
- transactional properties of transactions and blocks (rollback changes on drop or explicitly commit)
- ability to revert changes created in the latest block

The storage layer uses concread's B-tree maps and epoch cells. Its dependency
enables `maps`, `ebr`, and the existing `foldhash` backend explicitly; unused
async and adaptive-cache defaults are disabled.
