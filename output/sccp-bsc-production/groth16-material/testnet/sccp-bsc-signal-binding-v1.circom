pragma circom 2.1.6;

template SccpBscSignalBindingV1() {
  signal input publicSignals[9];
  signal input witnessSignals[9];
  signal diff[9];

  for (var i = 0; i < 9; i++) {
    diff[i] <== witnessSignals[i] - publicSignals[i];
    diff[i] * diff[i] === 0;
  }
}

component main { public [publicSignals] } = SccpBscSignalBindingV1();
