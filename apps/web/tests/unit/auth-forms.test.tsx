import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const replace = vi.fn();
let search = new URLSearchParams();
vi.mock("next/navigation", () => ({ useRouter: () => ({ replace, push: vi.fn() }), useSearchParams: () => search, usePathname: () => "/login" }));

import { LoginForm, RegisterForm, safeNext } from "@/components/auth/forms";
import { setCsrfToken } from "@/lib/api";

const json = (status: number, body: unknown) => Promise.resolve(new Response(status === 204 ? null : JSON.stringify(body), { status, headers: { "content-type": "application/json" } }));
let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => { fetchMock = vi.fn(); vi.stubGlobal("fetch", fetchMock); replace.mockReset(); search = new URLSearchParams(); setCsrfToken(null); });
afterEach(() => vi.unstubAllGlobals());

const lastBody = () => JSON.parse(fetchMock.mock.calls.at(-1)![1].body as string);

describe("safeNext (open-redirect guard)", () => {
  it("allows only same-site relative paths", () => {
    expect(safeNext("/dashboard")).toBe("/dashboard");
    expect(safeNext("/settings?tab=security")).toBe("/settings?tab=security");
    for (const bad of [null, "", "//evil.com", "https://evil.com", "/\\evil.com", "javascript:alert(1)", "dashboard"]) expect(safeNext(bad)).toBeNull();
  });
});

describe("LoginForm", () => {
  it("validates before calling the API", async () => {
    render(<LoginForm />);
    await userEvent.click(screen.getByRole("button", { name: /log in/i }));
    expect(await screen.findByText(/enter your email address/i)).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("logs in, stores the CSRF token and follows a safe ?next", async () => {
    search = new URLSearchParams("next=/settings");
    fetchMock.mockReturnValueOnce(json(200, { csrfToken: "tok", user: {} }));
    render(<LoginForm />);
    await userEvent.type(screen.getByLabelText("Email"), "ada@example.com");
    await userEvent.type(screen.getByLabelText("Password"), "correct horse battery");
    await userEvent.click(screen.getByRole("button", { name: /log in/i }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/settings"));
    expect(fetchMock.mock.calls[0]![0]).toBe("/api/v1/auth/login");
    expect(lastBody()).toEqual({ email: "ada@example.com", password: "correct horse battery" });
  });

  it("ignores a malicious ?next", async () => {
    search = new URLSearchParams("next=//evil.com");
    fetchMock.mockReturnValueOnce(json(200, { csrfToken: "t", user: {} }));
    render(<LoginForm />);
    await userEvent.type(screen.getByLabelText("Email"), "a@b.co");
    await userEvent.type(screen.getByLabelText("Password"), "whatever-password");
    await userEvent.click(screen.getByRole("button", { name: /log in/i }));
    await waitFor(() => expect(replace).toHaveBeenCalledWith("/dashboard"));
  });

  it("shows a generic message for bad credentials", async () => {
    fetchMock.mockReturnValueOnce(json(401, { code: "invalid_credentials", detail: "Authentication failed." }));
    render(<LoginForm />);
    await userEvent.type(screen.getByLabelText("Email"), "a@b.co");
    await userEvent.type(screen.getByLabelText("Password"), "wrong-password-1");
    await userEvent.click(screen.getByRole("button", { name: /log in/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Incorrect email or password.");
    expect(replace).not.toHaveBeenCalled();
  });

  it("reveals the code field when MFA is required, then sends the code", async () => {
    fetchMock.mockReturnValueOnce(json(401, { code: "mfa_required" })).mockReturnValueOnce(json(200, { csrfToken: "t", user: {} }));
    render(<LoginForm />);
    await userEvent.type(screen.getByLabelText("Email"), "a@b.co");
    await userEvent.type(screen.getByLabelText("Password"), "correct horse battery");
    await userEvent.click(screen.getByRole("button", { name: /log in/i }));
    const code = await screen.findByLabelText("Verification code");
    await userEvent.type(code, "123456");
    await userEvent.click(screen.getByRole("button", { name: /verify and log in/i }));
    await waitFor(() => expect(replace).toHaveBeenCalled());
    expect(lastBody()).toMatchObject({ mfaCode: "123456" });
  });

  it("offers to resend verification for unverified accounts", async () => {
    fetchMock.mockReturnValueOnce(json(403, { code: "email_not_verified" })).mockReturnValueOnce(json(202, {}));
    render(<LoginForm />);
    await userEvent.type(screen.getByLabelText("Email"), "a@b.co");
    await userEvent.type(screen.getByLabelText("Password"), "correct horse battery");
    await userEvent.click(screen.getByRole("button", { name: /log in/i }));
    await userEvent.click(await screen.findByRole("button", { name: /resend verification/i }));
    expect(await screen.findByText(/new link is on its way/i)).toBeInTheDocument();
  });

  it("toggles password visibility", async () => {
    render(<LoginForm />);
    const input = screen.getByLabelText("Password");
    expect(input).toHaveAttribute("type", "password");
    await userEvent.click(screen.getByRole("button", { name: /show password/i }));
    expect(input).toHaveAttribute("type", "text");
  });
});

describe("RegisterForm", () => {
  it("requires terms and never sends unexpected fields", async () => {
    render(<RegisterForm />);
    await userEvent.click(screen.getByRole("button", { name: /create account/i }));
    expect(await screen.findByText(/must accept the terms/i)).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(screen.queryByLabelText(/business name/i)).not.toBeInTheDocument();

    fetchMock.mockReturnValueOnce(json(202, { message: "ok" }));
    await userEvent.type(screen.getByLabelText("Full name"), "Ada Okafor");
    await userEvent.type(screen.getByLabelText("Work email"), "ada@example.com");
    await userEvent.type(screen.getByLabelText("Password"), "correct horse battery");
    await userEvent.click(screen.getByRole("checkbox"));
    await userEvent.click(screen.getByRole("button", { name: /create account/i }));
    expect(await screen.findByText(/check your email/i)).toBeInTheDocument();
    expect(Object.keys(lastBody()).sort()).toEqual(["email", "fullName", "password"]);
  });
});
