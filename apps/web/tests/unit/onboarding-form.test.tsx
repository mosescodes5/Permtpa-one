import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OnboardingForm } from "@/components/auth/forms";
import { setCsrfToken } from "@/lib/api";

const TYPES = [
  ["electronics", "Electronics", "Phones, gadgets, accessories"],
  ["restaurant", "Restaurant", "Meals, drinks, quick service"],
  ["pharmacy", "Pharmacy", "OTC, prescriptions, sundries"],
  ["building_materials", "Building materials", "Cement, sand, tiles, cables — sold by bag, weight or roll"],
  ["general_trade", "General trade", "Mixed retail or services"],
] as const;
const caps = { tracksExpiry: false, soldByWeight: false, hasTables: false, hasBookings: false };
const typeList = { items: TYPES.map(([code, label, description]) => ({ code, label, description, capabilities: caps })) };

const json = (status: number, body: unknown) => Promise.resolve(new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } }));
let fetchMock: ReturnType<typeof vi.fn>;
/** Route by "METHOD /path"; anything unexpected fails the test loudly. */
function route(extra: Record<string, () => Promise<Response>> = {}) {
  const table: Record<string, () => Promise<Response>> = {
    "GET /api/v1/business-types": () => json(200, typeList),
    "POST /api/v1/organizations": () => json(201, { id: "org-1", slug: "x", name: "x", role: "owner" }),
    "PATCH /api/v1/orgs/org-1": () => json(200, {}),
    "POST /api/v1/orgs/org-1/sample-catalogue": () => json(200, { loaded: 5, alreadyPresent: 0, skippedSkuTaken: 0 }),
    ...extra,
  };
  fetchMock.mockImplementation((url: string, init?: RequestInit) => {
    const key = `${init?.method ?? "GET"} ${url}`;
    const h = table[key];
    if (!h) throw new Error(`unexpected request: ${key}`);
    return h();
  });
}
const calls = () => fetchMock.mock.calls.map((c) => `${(c[1] as RequestInit | undefined)?.method ?? "GET"} ${c[0] as string}`);
const bodyOf = (key: string) => {
  const c = fetchMock.mock.calls.find((x) => `${(x[1] as RequestInit | undefined)?.method ?? "GET"} ${x[0] as string}` === key);
  return JSON.parse((c![1] as RequestInit).body as string);
};

beforeEach(() => { fetchMock = vi.fn(); vi.stubGlobal("fetch", fetchMock); setCsrfToken("test-csrf"); });
afterEach(() => vi.unstubAllGlobals());

describe("OnboardingForm", () => {
  it("shows the business types from the API with their descriptions", async () => {
    route();
    render(<OnboardingForm onCreated={vi.fn()} />);
    expect(await screen.findAllByRole("radio")).toHaveLength(TYPES.length);
    expect(screen.getByRole("radio", { name: /^Electronics/ })).toBeInTheDocument();
    expect(screen.getByText("Phones, gadgets, accessories")).toBeInTheDocument();
    expect(screen.getByText(/sold by bag, weight or roll/)).toBeInTheDocument();
    expect(screen.getByLabelText(/load a sample catalogue/i)).not.toBeChecked();
    expect(screen.getByText(/test a sale immediately/i)).toBeInTheDocument();
  });

  it("validates before calling the API", async () => {
    route();
    const user = userEvent.setup();
    render(<OnboardingForm onCreated={vi.fn()} />);
    await screen.findAllByRole("radio");
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    expect(await screen.findByText("Enter your business name")).toBeInTheDocument();
    expect(await screen.findByText("Choose a business type")).toBeInTheDocument();
    expect(calls()).toEqual(["GET /api/v1/business-types"]);
  });

  it("rejects a malformed phone number before calling the API", async () => {
    route();
    const user = userEvent.setup();
    render(<OnboardingForm onCreated={vi.fn()} />);
    await user.type(screen.getByLabelText("Business name"), "Ada Stores");
    await user.type(screen.getByLabelText(/phone number/i), "call me maybe");
    await user.click(await screen.findByRole("radio", { name: /^Pharmacy/ }));
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    expect(await screen.findByText("Enter a valid phone number")).toBeInTheDocument();
    expect(calls()).toEqual(["GET /api/v1/business-types"]);
  });

  it("creates the workspace with only name and type, then saves the phone and loads samples", async () => {
    route();
    const onCreated = vi.fn();
    const user = userEvent.setup();
    render(<OnboardingForm onCreated={onCreated} />);
    await user.type(screen.getByLabelText("Business name"), "  Ada Stores ");
    await user.type(screen.getByLabelText(/phone number/i), "+234 803 123 4567");
    await user.click(await screen.findByRole("radio", { name: /^Pharmacy/ }));
    await user.click(screen.getByLabelText(/load a sample catalogue/i));
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(calls()).toEqual([
      "GET /api/v1/business-types",
      "POST /api/v1/organizations",
      "PATCH /api/v1/orgs/org-1",
      "POST /api/v1/orgs/org-1/sample-catalogue",
    ]);
    expect(bodyOf("POST /api/v1/organizations")).toEqual({ name: "Ada Stores", businessType: "pharmacy" });
    expect(bodyOf("PATCH /api/v1/orgs/org-1")).toEqual({ phone: "+234 803 123 4567" });
  });

  it("makes no phone or sample call when those are left empty or unchecked", async () => {
    route();
    const onCreated = vi.fn();
    const user = userEvent.setup();
    render(<OnboardingForm onCreated={onCreated} />);
    await user.type(screen.getByLabelText("Business name"), "Ada Stores");
    await user.click(await screen.findByRole("radio", { name: /^General trade/ }));
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
    expect(calls()).toEqual(["GET /api/v1/business-types", "POST /api/v1/organizations"]);
  });

  it("does not continue silently when loading samples fails after the workspace was created", async () => {
    route({ "POST /api/v1/orgs/org-1/sample-catalogue": () => json(403, { code: "forbidden", message: "no" }) });
    const onCreated = vi.fn();
    const user = userEvent.setup();
    render(<OnboardingForm onCreated={onCreated} />);
    await user.type(screen.getByLabelText("Business name"), "Ada Stores");
    await user.click(await screen.findByRole("radio", { name: /^Electronics/ }));
    await user.click(screen.getByLabelText(/load a sample catalogue/i));
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    expect(await screen.findByText(/workspace was created, but we could not load the sample catalogue/i)).toBeInTheDocument();
    expect(onCreated).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: /go to dashboard/i }));
    expect(onCreated).toHaveBeenCalledTimes(1);
    expect(calls().filter((c) => c === "POST /api/v1/organizations")).toHaveLength(1);
  });

  it("shows the server's message and stays on the form when the workspace cannot be created", async () => {
    route({ "POST /api/v1/organizations": () => json(400, { code: "invalid_business_type", message: "Unknown business type." }) });
    const onCreated = vi.fn();
    const user = userEvent.setup();
    render(<OnboardingForm onCreated={onCreated} />);
    await user.type(screen.getByLabelText("Business name"), "Ada Stores");
    await user.click(await screen.findByRole("radio", { name: /^Electronics/ }));
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(onCreated).not.toHaveBeenCalled();
    expect(calls()).not.toContain("POST /api/v1/orgs/org-1/sample-catalogue");
  });
});
