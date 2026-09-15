import type { Capability } from "./types";

/**
 * Browser-side ML-DSA, used by CAVP step 03 (本地運算).
 *
 * The station's whole point is that the student computes locally and only the answer is
 * submitted, so this never touches the network. @noble/post-quantum is pinned to the same
 * version the answer keys were generated with (`tools/gen-cavp-vectors.mjs`); a mismatch
 * would turn correct work into "incorrect" verdicts.
 */

export interface ComputeInput {
  capability: Capability;
  parameterSet: string;
  values: Record<string, string>;
}

export type ComputeOutput =
  | { pk: string; sk: string }
  | { signature: string }
  | { testPassed: boolean };

function hexToBytes(label: string, hex: string): Uint8Array {
  const clean = hex.trim().replace(/\s+/g, "");
  if (clean === "") return new Uint8Array();
  if (!/^[0-9a-fA-F]*$/.test(clean) || clean.length % 2 !== 0) {
    throw new Error(`${label} is not a hex string`);
  }
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i += 1) {
    bytes[i] = Number.parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase();
}

function required(values: Record<string, string>, field: string): string {
  const value = values[field]?.trim();
  if (!value) {
    throw new Error(`${field} is required`);
  }
  return value;
}

async function algorithmFor(parameterSet: string) {
  const { ml_dsa44, ml_dsa65, ml_dsa87 } = await import("@noble/post-quantum/ml-dsa.js");
  switch (parameterSet.toUpperCase()) {
    case "ML-DSA-44":
      return ml_dsa44;
    case "ML-DSA-65":
      return ml_dsa65;
    case "ML-DSA-87":
      return ml_dsa87;
    default:
      throw new Error(`unsupported parameter set ${parameterSet}`);
  }
}

export async function computeCase(input: ComputeInput): Promise<ComputeOutput> {
  const algorithm = await algorithmFor(input.parameterSet);

  switch (input.capability) {
    case "key_gen": {
      const seed = hexToBytes("seed", required(input.values, "seed"));
      if (seed.length !== algorithm.lengths.seed) {
        throw new Error(`seed must be ${algorithm.lengths.seed} bytes`);
      }
      const keys = algorithm.keygen(seed);
      return { pk: bytesToHex(keys.publicKey), sk: bytesToHex(keys.secretKey) };
    }
    case "sig_gen": {
      const sk = hexToBytes("sk", required(input.values, "sk"));
      const message = hexToBytes("message", required(input.values, "message"));
      const context = hexToBytes("context", input.values.context ?? "");
      // No extra entropy: the registration declares Deterministic = true, which is what
      // makes a fixed answer key possible.
      const signature = algorithm.sign(message, sk, { context });
      return { signature: bytesToHex(signature) };
    }
    case "sig_ver": {
      const pk = hexToBytes("pk", required(input.values, "pk"));
      const message = hexToBytes("message", required(input.values, "message"));
      const signature = hexToBytes("signature", required(input.values, "signature"));
      const context = hexToBytes("context", input.values.context ?? "");
      return { testPassed: algorithm.verify(signature, message, pk, { context }) };
    }
  }
}
