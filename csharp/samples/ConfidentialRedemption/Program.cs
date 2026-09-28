// Disposable local proofs only; no transaction or ledger authorization.
// Real wallets persist each change opening securely before proving consumes it,
// then obtain its leaf index and root from authenticated protocol state.
using System.Security.Cryptography;
using Hyperledger.Iroha;
using Hyperledger.Iroha.Privacy;

var key = RandomNumberGenerator.GetBytes(32);
var inputRho = RandomNumberGenerator.GetBytes(32);
var changeRho = RandomNumberGenerator.GetBytes(32);
var seed = RandomNumberGenerator.GetBytes(32);
try
{
    var networkBytes = RandomNumberGenerator.GetBytes(32);
    networkBytes[31] |= 1;
    var network = NetworkId.FromBytes(networkBytes);
    var asset = new ConfidentialAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    var diversifier = ConfidentialNotes.Diversifier(seed);
    var defaultDiversifier = ConfidentialNotes.DefaultDiversifier();
    var inputCommitment = ConfidentialNotes.Commitment(asset, 7, inputRho, ConfidentialNotes.OwnerTag(key, diversifier));
    var expectedChange = ConfidentialNotes.Commitment(asset, 2, changeRho, ConfidentialNotes.OwnerTag(key, defaultDiversifier));
    using var prover = new ConfidentialProver(network, asset, key);
    CryptographicOperations.ZeroMemory(key);
    using var path = ConfidentialNotes.MerklePath([inputCommitment], 0);
    using var note = new ConfidentialInputNote(7, inputRho, diversifier, 0);
    using var change = new ConfidentialChangeNote(2, changeRho);
    var first = await prover.ProveRedemptionAsync(
        ConfidentialTreeEvidence.Paths(path.Root, [path]), [note], 5, change);
    if (first.Relation != ConfidentialProofRelation.RedemptionWithChange ||
        first.OutputCommitments.Count != 1 || !first.OutputCommitments[0].AsSpan().SequenceEqual(expectedChange))
        throw new InvalidOperationException("Change proof result differs from the opening.");

    // Keep changeRho only for this disposable example. Production uses secure storage
    // and an authenticated history/index; this locally constructed history has neither.
    using var restoredChange = new ConfidentialChangeNote(2, changeRho);
    using var restoredInput = restoredChange.ToInput(1);
    var history = new[] { inputCommitment, first.OutputCommitments[0] };
    var root = ConfidentialNotes.Root(history);
    var pending = prover.ProveRedemptionAsync(
        ConfidentialTreeEvidence.Commitments(root, history), [restoredInput], 2);
    prover.Dispose(); // Accepted native work retains its own clearing key owner.
    var second = await pending;
    if (second.Relation != ConfidentialProofRelation.FullRedemption ||
        second.OutputCommitments.Count != 0 || !second.Root.AsSpan().SequenceEqual(root))
        throw new InvalidOperationException("Final redemption proof differs from local history.");
    Console.WriteLine($"Locally verified {first.Relation}: {first.Proof.Length} proof bytes; {second.Relation}: {second.Proof.Length} proof bytes.");
}
finally
{
    CryptographicOperations.ZeroMemory(key);
    CryptographicOperations.ZeroMemory(inputRho);
    CryptographicOperations.ZeroMemory(changeRho);
    CryptographicOperations.ZeroMemory(seed);
}
