// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.nio.file.Files;
import java.nio.file.Path;

/** Simple CLI to print Norito header information. */
public final class NoritoDump {
  private NoritoDump() {}

  public static void main(String[] args) throws Exception {
    if (args.length != 1) {
      System.err.println("Usage: NoritoDump <path>");
      System.exit(1);
    }
    byte[] data = Files.readAllBytes(Path.of(args[0]));
    NoritoHeader.DecodeResult result = NoritoHeader.decode(data, null);
    NoritoHeader header = result.header();
    System.out.println("Norito header:");
    System.out.println("  version: " + NoritoHeader.MAJOR_VERSION + "." + NoritoHeader.MINOR_VERSION);
    System.out.println("  schema hash: " + bytesToHex(header.schemaHash()));
    System.out.println("  compression: " + header.compression());
    System.out.println("  payload length: " + header.payloadLength());
    System.out.printf("  checksum: 0x%016x%n", header.checksum());
    System.out.printf("  flags: 0x%02x%n", header.flags());
  }

  private static String bytesToHex(byte[] bytes) {
    StringBuilder sb = new StringBuilder();
    for (byte b : bytes) {
      sb.append(String.format("%02x", b));
    }
    return sb.toString();
  }
}
