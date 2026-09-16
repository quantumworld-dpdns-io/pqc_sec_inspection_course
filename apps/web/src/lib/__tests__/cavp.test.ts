import { describe, expect, it } from "vitest";

import { buildResultsSkeleton } from "@/lib/cavp";
import type { CavpPack } from "@/lib/types";

function pack(overrides: Partial<CavpPack> = {}): CavpPack {
  return {
    slug: "mldsa65-02-keygen",
    capability: "key_gen",
    parameterSet: "ML-DSA-65",
    vsId: 91021,
    registration: {},
    example: { tcId: 1, pk: "AA", sk: "BB" },
    prompt: {
      vsId: 91021,
      algorithm: "ML-DSA",
      mode: "keyGen",
      revision: "FIPS204",
      isSample: true,
      testGroups: [
        {
          tgId: 1,
          testType: "AFT",
          parameterSet: "ML-DSA-65",
          tests: [{ tcId: 1, seed: "00" }, { tcId: 2, seed: "11" }, { tcId: 3, seed: "22" }],
        },
      ],
    },
    ...overrides,
  };
}

describe("buildResultsSkeleton", () => {
  it("keeps the ACVP envelope the validator expects", () => {
    const parsed = JSON.parse(buildResultsSkeleton(pack()));
    expect(parsed.vsId).toBe(91021);
    expect(parsed.mode).toBe("keyGen");
    expect(parsed.testGroups).toHaveLength(1);
    expect(parsed.testGroups[0].tests).toHaveLength(3);
  });

  it("pre-fills tcId 1 from the worked example and blanks the rest", () => {
    const tests = JSON.parse(buildResultsSkeleton(pack())).testGroups[0].tests;
    expect(tests[0]).toEqual({ tcId: 1, pk: "AA", sk: "BB" });
    expect(tests[1]).toEqual({ tcId: 2, pk: "", sk: "" });
  });

  it("uses the right answer fields per capability", () => {
    const sigGen = JSON.parse(
      buildResultsSkeleton(pack({ capability: "sig_gen", example: null })),
    ).testGroups[0].tests;
    expect(sigGen[0]).toEqual({ tcId: 1, signature: "" });

    const sigVer = JSON.parse(
      buildResultsSkeleton(pack({ capability: "sig_ver", example: null })),
    ).testGroups[0].tests;
    expect(sigVer[0]).toEqual({ tcId: 1, testPassed: null });
  });
});
