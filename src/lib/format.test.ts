import { describe, expect, it } from "vitest";
import { compact, percent, variation } from "./format";

describe("compact", () => {
  it("uses English grouping and suffixes", () => {
    expect(compact(999)).toBe("999");
    expect(compact(1234)).toBe("1,234");
    expect(compact(12_345)).toBe("12.3k");
    expect(compact(1_240_000)).toBe("1.24M");
    expect(compact(1_200_000_000)).toBe("1.2B");
  });
});

describe("variation", () => {
  it("is null without a previous value", () => {
    expect(variation(10, 0)).toBeNull();
    expect(percent(variation(15, 10)!)).toBe("+50%");
  });
});
