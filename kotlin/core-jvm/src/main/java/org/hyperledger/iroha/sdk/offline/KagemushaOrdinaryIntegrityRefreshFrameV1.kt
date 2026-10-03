// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
/** Exact data-only PI JNI grammar. Completed-key read selects no generation or financial grant. */
object KagemushaOrdinaryIntegrityRefreshFrameV1 {
 fun requireRequest(phase:Int,original:ByteArray) {
  when(phase){1,3,7,8,9,10->require(original.isEmpty());2->require(original.size==514)
   4->require(original.size in 8..72);5->require(original.size in 1..65536 && original.all{it.toInt() in 0x21..0x7e})
   6->require(original.size in 1..4096);else->error("Unknown ordinary Integrity phase")}
 }
 fun responseFields(phase:Int,handle:Long,response:List<ByteArray>):List<ByteArray> {
  require(handle!=0L && response.size>=3 && response[0].contentEquals(byteArrayOf(1,0)) &&
   response[1].contentEquals(byteArrayOf(phase.toByte())) && response[2].contentEquals(ByteArray(8){(handle ushr (8*it)).toByte()}))
  val fields=response.drop(3);requireResponse(phase,fields);return fields.map(ByteArray::copyOf)
 }
 fun requireResponse(phase:Int,f:List<ByteArray>) {
  require(f.sumOf{it.size.toLong()}<=128*1024)
  when(phase){1->{require(f.size==2);f.forEach(::digest)}
   2,3->{require(f.size==4 && f[0].size==514 && f[1].size in 1..8192);digest(f[2]);digest(f[3])
    val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(f[0])
    require(c.possessionSigningBytes().contentEquals(f[1]) && c.playIntegrityRequestHash().contentEquals(f[2]) && c.attestedKeyId().contentEquals(f[3]))}
   4,7->require(f.isEmpty());5->{require(f.size==4);digest(f[0]);require(f[1].size==514 && f[2].size in 8..72 && f[3].size in 1..65536)
    require(KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(f[1]).operationId().contentEquals(f[0]) && f[3].all{it.toInt() in 0x21..0x7e})}
   6->{require(f.size==1);digest(f[0])}
   8->{require(f.size==8 && f[0].size in 1..32768 && f[1].size in 1..32768 && f[2].size in listOf(0,32))
    require(f[3].size==1 && f[3][0].toInt() in listOf(0,2,3))
    require(f[4].size in listOf(0,514) && (f[5].isEmpty()||f[5].size in 8..72) && f[6].size<=65536 && f[7].size<=4096)
    if(f[2].isEmpty()) require(f[3][0]==0.toByte() && f.drop(4).all{it.isEmpty()})
    if(f[2].isNotEmpty()) digest(f[2])
    if(f[4].isNotEmpty()) {
     val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(f[4]);require(f[2].size==32 && c.nonce().contentEquals(f[2]))
    }
    if(f[3][0]==0.toByte()) require(f[5].isEmpty() && f[6].isEmpty() && f[7].isEmpty())
    if(f[3][0]!=0.toByte()) require(f[4].size==514 && f[5].size in 8..72)
    if(f[6].isNotEmpty()) require(f[3][0]==3.toByte() && f[6].all{it.toInt() in 0x21..0x7e})}
   9->require(f.size==1 && f[0].size==1 && f[0][0].toInt() in 0..1)
   10->{require(f.size==11);digest(f[0]);require(f[1].size in 1..512 && f[1].all{it.toInt() in 0x21..0x7e})
    digest(f[2]);require(f[3].size==65 && f[3][0]==4.toByte());digest(f[4]);require(f[5].size==1 && f[5][0].toInt() in 1..3)
    require(f[6].size in 1..32768 && f[7].size in 1..32768 && f[8].size in 1..16384 && f[9].size==8);digest(f[10])}
   else->error("Unknown ordinary Integrity reply")}
 }
 private fun digest(raw:ByteArray){require(raw.size==32 && raw.any{it!=0.toByte()})}
}
