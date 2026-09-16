import type { CavpPack } from "./types";

/**
 * Build the ACVP results envelope that step 04 starts from: tcId 1 carries the worked
 * example the API hands out, every other case is blank for the student to fill in.
 *
 * Keeping the shape identical to what the validator expects means a student who only edits
 * values (rather than structure) cannot produce a "malformed" verdict by accident.
 */
export function buildResultsSkeleton(pack: CavpPack): string {
  const tests = pack.prompt.testGroups[0]?.tests ?? [];
  const example = (pack.example ?? {}) as Record<string, unknown>;

  const blank = (tcId: number): Record<string, unknown> => {
    if (tcId === 1 && Object.keys(example).length > 0) {
      const { tcId: _ignored, ...answer } = example;
      return { tcId, ...answer };
    }
    switch (pack.capability) {
      case "key_gen":
        return { tcId, pk: "", sk: "" };
      case "sig_gen":
        return { tcId, signature: "" };
      case "sig_ver":
        return { tcId, testPassed: null };
    }
  };

  return JSON.stringify(
    {
      vsId: pack.vsId,
      algorithm: pack.prompt.algorithm,
      mode: pack.prompt.mode,
      revision: pack.prompt.revision,
      testGroups: [
        {
          tgId: pack.prompt.testGroups[0]?.tgId ?? 1,
          tests: tests.map((test) => blank(Number(test.tcId))),
        },
      ],
    },
    null,
    2,
  );
}
