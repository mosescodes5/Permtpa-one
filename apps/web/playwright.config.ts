import { defineConfig, devices } from "@playwright/test";

// Full-stack: real Next.js -> real Rust API -> real PostgreSQL + Redis (see scripts/e2e-setup.sh).
// Optional: use a pre-installed Chromium where `playwright install` is blocked.
const executablePath = process.env.PW_CHROMIUM_PATH;
const launchOptions = executablePath ? { executablePath, args: ["--no-sandbox", "--disable-gpu"] } : undefined;

const WEB = "http://localhost:3100";
const API = "http://127.0.0.1:8081";
const apiEnv = {
  APP_ENV: "development",
  API_BIND: "127.0.0.1:8081",
  DATABASE_URL: process.env.E2E_DATABASE_URL ?? "postgres://permtpa_app:e2e_app_pw@127.0.0.1:5432/permtpa_one_e2e",
  REDIS_URL: process.env.E2E_REDIS_URL ?? "redis://127.0.0.1:6379",
  CSRF_SECRET: "e2e-only-csrf-secret-0123456789abcdef",
  MFA_ENCRYPTION_KEY: "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=",
  PUBLIC_URL: WEB,
  // Next appends the TCP peer to X-Forwarded-For. With 2 trusted hops the API reads the client-supplied entry,
  // letting each test present a unique IP so per-IP rate limits do not couple unrelated tests. E2E ONLY.
  TRUSTED_PROXY_HOPS: "2",
  RUST_LOG: "warn",
};

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  timeout: 45_000,
  use: { baseURL: WEB, trace: "retain-on-failure" },
  webServer: [
    { command: "../../target/debug/permtpa-api", url: `${API}/healthz`, env: apiEnv, reuseExistingServer: false, timeout: 60_000 },
    { command: "npm run build && npx next start -p 3100", url: WEB, env: { API_INTERNAL_URL: API }, reuseExistingServer: false, timeout: 240_000 },
  ],
  projects: [
    { name: "desktop", use: { ...devices["Desktop Chrome"], launchOptions } },
    { name: "mobile", use: { ...devices["Pixel 7"], launchOptions } },
  ],
});
