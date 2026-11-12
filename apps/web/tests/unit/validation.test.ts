import { describe, expect, it } from "vitest";
import { loginSchema, passwordStrength, registerSchema } from "@/lib/validation";

const valid = { fullName: "Ada Okafor", businessName: "Okafor Trading", email: "ada@example.com", password: "correct horse battery", acceptTerms: true };

describe("registerSchema", () => {
  it("accepts valid input", () => { expect(registerSchema.safeParse(valid).success).toBe(true); });
  it("rejects short passwords", () => { expect(registerSchema.safeParse({ ...valid, password: "short" }).success).toBe(false); });
  it("rejects unaccepted terms", () => { expect(registerSchema.safeParse({ ...valid, acceptTerms: false }).success).toBe(false); });
  it("rejects bad emails", () => { expect(registerSchema.safeParse({ ...valid, email: "nope" }).success).toBe(false); });
  it("rejects unknown keys (e.g. a client-supplied role or org id)", () => {
    expect(registerSchema.safeParse({ ...valid, role: "owner", organizationId: "x" }).success).toBe(false);
  });
});
describe("loginSchema", () => { it("requires both fields", () => { expect(loginSchema.safeParse({ email: "", password: "" }).success).toBe(false); }); });
describe("passwordStrength", () => {
  it("is monotonic-ish", () => {
    expect(passwordStrength("")).toBe(0);
    expect(passwordStrength("aaaaaaaaaaaa")).toBe(1);
    expect(passwordStrength("Correct-Horse-Battery-9")).toBe(4);
  });
});
