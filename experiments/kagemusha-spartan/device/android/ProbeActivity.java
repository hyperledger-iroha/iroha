package org.hyperledger.iroha.spartan.hardwarefixture;

import android.app.Activity;
import android.os.Bundle;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.security.keystore.*;
import android.widget.TextView;
import java.security.*;
import java.security.cert.Certificate;
import java.security.interfaces.ECPublicKey;
import java.security.spec.ECGenParameterSpec;
import java.math.BigInteger;
import java.io.*;
import java.util.*;
import org.json.*;

/** Hardware-only fixture. No account, ledger, money, native acceptance or wallet custody. */
public final class ProbeActivity extends Activity {
  private static final String ALIAS = "spartan-isolated-hardware-fixture-v1";
  private static String hex(byte[] b) { StringBuilder s=new StringBuilder(); for(byte x:b)s.append(String.format("%02x",x&255)); return s.toString(); }
  private static byte[] scalar(BigInteger v) { byte[] b=v.toByteArray(), out=new byte[32]; int n=Math.min(b.length,32); System.arraycopy(b,b.length-n,out,32-n,n); return out; }
  private static byte[] unhex(String s) { if(!s.matches("[0-9a-f]{64}"))throw new IllegalArgumentException("exact message"); byte[] o=new byte[32]; for(int i=0;i<32;i++)o[i]=(byte)Integer.parseInt(s.substring(i*2,i*2+2),16); return o; }
  private static byte[] raw(byte[] d) { int i=0; if(d[i++]!=48||(d[i++]&255)!=d.length-2||d[i++]!=2)throw new IllegalArgumentException("DER"); int n=d[i++]&255; BigInteger r=new BigInteger(1,Arrays.copyOfRange(d,i,i+n));i+=n;if(d[i++]!=2)throw new IllegalArgumentException("DER S");n=d[i++]&255;BigInteger s=new BigInteger(1,Arrays.copyOfRange(d,i,i+n));i+=n;if(i!=d.length)throw new IllegalArgumentException("DER trailing");BigInteger order=new BigInteger("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551",16);if(s.compareTo(order.shiftRight(1))>0)s=order.subtract(s);byte[] o=new byte[64];System.arraycopy(scalar(r),0,o,0,32);System.arraycopy(scalar(s),0,o,32,32);return o; }
  protected void onCreate(Bundle b){super.onCreate(b);run(getIntent());}
  protected void onNewIntent(Intent i){super.onNewIntent(i);setIntent(i);run(i);}
  private void generate(byte[] challenge, boolean strong) throws Exception {
    KeyPairGenerator g=KeyPairGenerator.getInstance("EC","AndroidKeyStore");
    KeyGenParameterSpec.Builder b=new KeyGenParameterSpec.Builder(ALIAS,KeyProperties.PURPOSE_SIGN|KeyProperties.PURPOSE_VERIFY).setAlgorithmParameterSpec(new ECGenParameterSpec("secp256r1")).setDigests(KeyProperties.DIGEST_SHA256).setAttestationChallenge(challenge).setUserAuthenticationRequired(false);
    if(android.os.Build.VERSION.SDK_INT>=28)b.setIsStrongBoxBacked(strong);
    g.initialize(b.build());g.generateKeyPair();
  }
  private void run(Intent intent){TextView view=new TextView(this);setContentView(view);try{
    KeyStore ks=KeyStore.getInstance("AndroidKeyStore");ks.load(null);
    File challengeFile=new File(getFilesDir(),"challenge.bin");
    if(!ks.containsAlias(ALIAS)){
      byte[] challenge=new byte[32];new SecureRandom().nextBytes(challenge);
      boolean strong=android.os.Build.VERSION.SDK_INT>=28&&getPackageManager().hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE);
      try{generate(challenge,strong);}catch(StrongBoxUnavailableException e){ks.deleteEntry(ALIAS);generate(challenge,false);}
      try(FileOutputStream f=new FileOutputStream(challengeFile)){f.write(challenge);f.getFD().sync();}
    }
    PrivateKey key=(PrivateKey)ks.getKey(ALIAS,null);KeyInfo facts=KeyFactory.getInstance("EC","AndroidKeyStore").getKeySpec(key,KeyInfo.class);
    if(key.getEncoded()!=null||!facts.isInsideSecureHardware())throw new IllegalStateException("hardware custody required");
    int level=android.os.Build.VERSION.SDK_INT>=31?facts.getSecurityLevel():-1;
    if(level!=1&&level!=2)throw new IllegalStateException("hardware security level unproven");
    ECPublicKey pub=(ECPublicKey)ks.getCertificate(ALIAS).getPublicKey();byte[] q=new byte[65];q[0]=4;System.arraycopy(scalar(pub.getW().getAffineX()),0,q,1,32);System.arraycopy(scalar(pub.getW().getAffineY()),0,q,33,32);
    byte[] message=new byte[32];String encoded=intent.getStringExtra("spartanMessage");if(encoded==null)new SecureRandom().nextBytes(message);else message=unhex(encoded);
    Signature sign=Signature.getInstance("SHA256withECDSA");sign.initSign(key);sign.update(message);byte[] der=sign.sign();sign.initVerify(pub);sign.update(message);if(!sign.verify(der))throw new IllegalStateException("verify");
    JSONArray chain=new JSONArray();for(Certificate c:ks.getCertificateChain(ALIAS))chain.put(hex(c.getEncoded()));
    JSONObject out=new JSONObject().put("schema","kagemusha-spartan-hardware-probe-v1").put("package",getPackageName()).put("public_key_sec1",hex(q)).put("message",hex(message)).put("signature_der",hex(der)).put("signature_raw_low_s",hex(raw(der))).put("security_level",level).put("nonexportable",true).put("certificate_chain_der",chain);
    try(FileInputStream f=new FileInputStream(challengeFile)){byte[] challenge=new byte[32];if(f.read(challenge)!=32)throw new IOException("challenge");out.put("attestation_challenge",hex(challenge));}
    try(FileOutputStream f=new FileOutputStream(new File(getFilesDir(),"spartan-hardware-probe.json"))){f.write(out.toString(2).getBytes("UTF-8"));f.getFD().sync();}
    view.setText(out.toString(2));
  }catch(Exception e){view.setText("FAILED: "+e);try(FileOutputStream f=new FileOutputStream(new File(getFilesDir(),"spartan-hardware-error.txt"))){f.write(e.toString().getBytes("UTF-8"));}catch(Exception ignored){}}}
}
