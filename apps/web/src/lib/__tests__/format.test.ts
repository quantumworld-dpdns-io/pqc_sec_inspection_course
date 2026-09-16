import { describe, expect, it } from "vitest";

import { countLinesAndChars, resultChipClass, resultGlyph } from "@/lib/format";

describe("countLinesAndChars", () => {
  it("counts an empty document as zero lines", () => {
    expect(countLinesAndChars("")).toEqual({ lines: 0, chars: 0 });
  });

  it("counts lines the way an editor does", () => {
    expect(countLinesAndChars("a\nb\nc")).toEqual({ lines: 3, chars: 5 });
  });
});

describe("result presentation", () => {
  it("has a class and a glyph for every verdict the API can return", () => {
    for (const status of ["passed", "failed", "unsupported", "disabled", "pending"] as const) {
      expect(resultChipClass[status]).toBeTruthy();
      expect(resultGlyph[status]).toBeTruthy();
    }
  });
});
