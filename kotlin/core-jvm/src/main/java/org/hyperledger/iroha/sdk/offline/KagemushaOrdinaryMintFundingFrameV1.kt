// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
/** Exact dedicated Native87 grammar. Bounds are actual Model originals; framing is data only. */
object KagemushaOrdinaryMintFundingFrameV1 {
 const val SELECTION_MAX=32*1024;const val REQUEST_MAX=64*1024;const val C_MAX=16*1024
 const val PI_MAX=4096;const val CLOCK_MAX=16*1024*1024+4096;const val CONTROL_MAX=32*1024
 const val DECISION_MAX=16*1024;const val DATA_MAX=128*1024;const val NODE_MAX=60*1024*1024
 const val PLATFORM_MAX=4096;const val FINALIZED_MAX=KagemushaOrdinaryIncomingFrameV1.FINALIZED_MAXIMUM_BYTES
 fun requireRequest(phase:Int,f:List<ByteArray>) {
  when(phase){1->{require(f.size==1 && f[0].size==16 && f[0].any{it!=0.toByte()})}
   3->{require(f.size==1);original(f[0],PLATFORM_MAX)}
   8->{require(f.size==5);listOf(DECISION_MAX,CLOCK_MAX,CONTROL_MAX,DATA_MAX,NODE_MAX).indices.forEach{original(f[it],listOf(DECISION_MAX,CLOCK_MAX,CONTROL_MAX,DATA_MAX,NODE_MAX)[it])}}
   2,4,5,6,7,9,10,11,12,13,14,15,16,17->require(f.isEmpty())
   else->error("Unknown dedicated Mint funding phase")}
 }
 fun requireResponse(phase:Int,f:List<ByteArray>){when(phase){
  1->{require(f.size==3);digest(f[0]);original(f[1],PLATFORM_MAX);original(f[2],C_MAX)}
  4,9->{require(f.size==1);digest(f[0])};5->require(f.size==1 && f[0].size==64)
  6,7,14->{require(f.size==8);val bounds=listOf(SELECTION_MAX,REQUEST_MAX,64,C_MAX,PI_MAX,CLOCK_MAX,CONTROL_MAX,CONTROL_MAX)
   f.indices.forEach{if(it==4)require(f[it].size<=bounds[it])else original(f[it],bounds[it])};require(f[2].size==64)}
  11->{require(f.size==2 && f[0].size==1);when(f[0][0].toInt()){0->require(f[1].isEmpty());1->original(f[1],FINALIZED_MAX);else->error("Unknown finalized status")}}
  12->{require(f.size==5 && f[0].size==1 && f[0][0].toInt() in 0..3);digest(f[1]);original(f[2],PLATFORM_MAX);original(f[3],C_MAX)
   if(f[0][0].toInt()<2)require(f[4].isEmpty())else original(f[4],PLATFORM_MAX)}
  17->{require(f.size==2);digest(f[0]);require(f[1].size==1 && f[1][0].toInt() in 0..12)}
  15->require(f.size==2 && (f[0].contentEquals(byteArrayOf(5)) && f[1].isEmpty() || f[0].contentEquals(byteArrayOf(4)) && f[1].size==4))
  2,3,8,10,13,16->require(f.isEmpty());else->error("Unknown dedicated Mint funding response")}}
 fun responseFields(phase:Int,handle:Long,response:List<ByteArray>):List<ByteArray>{
  require(handle!=0L && response.size>=3 && response[0].contentEquals(byteArrayOf(1,0)) &&
   response[1].contentEquals(byteArrayOf(phase.toByte())) && response[2].contentEquals(ByteArray(8){(handle ushr(it*8)).toByte()}))
  return response.drop(3).also{requireResponse(phase,it)}.map(ByteArray::copyOf)
 }
 private fun original(b:ByteArray,max:Int)=require(b.size in 1..max)
 private fun digest(b:ByteArray)=require(b.size==32 && b.any{it!=0.toByte()})
}
