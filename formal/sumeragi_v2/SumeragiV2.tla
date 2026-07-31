---- MODULE SumeragiV2 ----
EXTENDS SumeragiV2Inductive

(***************************************************************************
Compatibility entry point for bounded TLC configurations.

The protocol relation now lives in SumeragiV2Core.  Keeping this thin module
lets existing model-check commands continue to name SumeragiV2 while proof
modules import the smaller concern-specific modules directly.
***************************************************************************)

=============================================================================
