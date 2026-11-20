import { describe, expect, it } from "vitest";
import { loginSchema, onboardingSchema, passwordStrength, registerSchema } from "@/lib/validation";

const valid = { fullName: "Ada Okafor", email: "ada@example.com", password: "correct horse battery", acceptTerms: true };

describe("registerSchema", () => {
  it("accepts valid input", () => { expect(registerSchema.safeParse(valid).success).toBe(true); });
  it("rejects short passwords", () => { expect(registerSchema.safeParse({ ...valid, password: "short" }).success).toBe(false); });
  it("rejects unaccepted terms", () => { expect(registerSchema.safeParse({ ...valid, acceptTerms: false }).success).toBe(false); });
  it("rejects bad emails", () => { expect(registerSchema.safeParse({ ...valid, email: "nope" }).success).toBe(false); });
  it("no longer accepts businessName at registration (organizations are created after sign-in)", () => { expect(registerSchema.safeParse({ ...valid, businessName: "X Co" }).success).toBe(false); });
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

describe("onboardingSchema", () => {
  const ok = { name: "Ada Stores", phone: "", businessType: "pharmacy", loadSamples: false };
  it("accepts a name, a type and no phone", () => { expect(onboardingSchema.safeParse(ok).success).toBe(true); });
  it("requires a business name and a business type", () => {
    expect(onboardingSchema.safeParse({ ...ok, name: " " }).success).toBe(false);
    expect(onboardingSchema.safeParse({ ...ok, businessType: "" }).success).toBe(false);
  });
  it("accepts well-formed phone numbers and rejects malformed ones", () => {
    for (const p of ["+234 803 123 4567", "(0803) 123-4567", "08031234567"]) expect(onboardingSchema.safeParse({ ...ok, phone: p }).success).toBe(true);
    for (const p of ["abc", "123456", "call me maybe", "0803+1234567"]) expect(onboardingSchema.safeParse({ ...ok, phone: p }).success).toBe(false);
  });
});
