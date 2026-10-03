// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.*
import org.junit.jupiter.api.Test
/** Public synthetic shape controls over actual Rust model fixtures; no issued authority. */
class KagemushaOrdinaryIncomingProjectionV1Test {
 private fun specimen(tag: String, terminal: Boolean): Pair<ByteArray,ByteArray> {
  val rows=fixtures()
  val w=rows.getValue("w_${tag}_9").copyOf();val s=rows.getValue("s_${tag}_9").copyOf()
  w.copyInto(s,155,213,245);w[52]=if(terminal) 1 else 2
  if(terminal) {s.fill(31,364,396);s.fill(32,396,428)} else s.fill(0,364,428)
  // Real ordinary W2 window is finite 10 seconds, fixed data only.
  w.fill(0,309,325);w[309]=1;w[317]=2;sha(s).copyInto(w,245)
  return w to s
 }
 // Gradle tests execute from the module directory. Read the same maintained Rust fixture
 // from its actual repository ancestor; no copied or synthetic fallback is accepted.
 private fun fixtures(): Map<String,ByteArray> {
  var directory=Paths.get("").toAbsolutePath().normalize()
  while(directory!=null) {
   val path=directory.resolve("fixtures/offline/kagemusha_app_platform_messages_v1.tsv")
   if(Files.isRegularFile(path)) return Files.readAllLines(path,Charsets.UTF_8)
    .filter { !it.startsWith("#") && it.isNotEmpty() }.associate { line ->
     val columns=line.split('\t');require(columns.size==2)
     columns[0] to columns[1].chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
   directory=directory.parent
  }
  error("missing Rust app platform message fixture")
 }
 private fun sha(b:ByteArray)=MessageDigest.getInstance("SHA-256").digest(b)
 private fun binding(w:ByteArray,s:ByteArray)=KagemushaOrdinaryCashApprovalOriginalBindingV1(
  w.copyOfRange(53,85),w.copyOfRange(117,149),w.copyOfRange(149,181),w.copyOfRange(181,213),
  w.copyOfRange(213,245),w.copyOfRange(277,309),s)
 @Test fun incomingPurposesAreDistinctAndFullSubjectHashIsRaw460() {
  for(tag in listOf("mint_fold","receive_fold")) for(terminal in listOf(false,true)) {
   val(w,s)=specimen(tag,terminal);val b=binding(w,s)
   val p=if(terminal) KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingTerminal(w,s,b)
     else KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingPreparation(w,s,b)
   assertContentEquals(sha(s),p.subjectSigningDigest());assertContentEquals(s,p.selectionBytes())
   if(terminal) assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(w,s,b) }
   assertFailsWith<IllegalArgumentException> { if(terminal) KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingPreparation(w,s,b)
     else KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingTerminal(w,s,b) }
  }
 }
 @Test fun incomingTerminalNeedsBothCommitmentsAndExactFinancialEdge() {
  for(tag in listOf("mint_fold","receive_fold")) for(offset in listOf(364,396,428,444,155)) {
   val(w,s)=specimen(tag,true);val changed=s.copyOf().also { it.fill(0,offset,offset+if(offset>=428)16 else 32) }
   sha(changed).copyInto(w,245)
   assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingTerminal(w,changed,binding(w,changed)) }
  }
 }
 @Test fun incomingCannotRelabelOutgoingOrUseLongPreparationWindow() {
  val(w,s)=specimen("send_split",false)
  assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingPreparation(w,s,binding(w,s)) }
  val(a,b)=specimen("mint_fold",false);a[319]=1
  assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingPreparation(a,b,binding(a,b)) }
 }
}
