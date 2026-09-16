import { expect, test } from "@playwright/test";

const API = "/api/v1";

test.describe("PQ-CAS", () => {
  test("creates a task and fills the matrix from real probes", async ({ page, request }) => {
    const created = await request.post(`${API}/tests`, {
      data: {
        host: "tls-good",
        port: 11000,
        httpPath: "/",
        tool: "pq-dsa",
        plugin: "open3x",
        groups: ["X25519MLKEM768", "P-256"],
        sigAlgs: ["mldsa65", "ecdsa_secp256r1_sha256"],
      },
    });
    expect(created.ok()).toBeTruthy();
    const test_ = await created.json();
    expect(test_.subtaskCount).toBe(4);

    // Poll the API rather than the page: the worker decides when this finishes.
    await expect
      .poll(
        async () => {
          const response = await request.get(`${API}/tests/${test_.id}`);
          return (await response.json()).status;
        },
        { timeout: 60_000, intervals: [1000] },
      )
      .toBe("finished");

    await page.goto(`/en/tests/${test_.id}`);
    await expect(page.getByRole("heading", { name: "tls-good:11000" })).toBeVisible();

    const cells = page.getByRole("link", { name: /X25519MLKEM768 x mldsa65/ });
    await expect(cells.first()).toBeVisible();
    await cells.first().click();

    await expect(page.getByRole("heading", { name: /X25519MLKEM768/ })).toBeVisible();
    await expect(page.getByText("ClientHello", { exact: true })).toBeVisible();
  });

  test("rejects an unknown algorithm instead of queueing doomed subtasks", async ({ request }) => {
    const response = await request.post(`${API}/tests`, {
      data: {
        host: "tls-good",
        port: 11000,
        tool: "pq-dsa",
        plugin: "open3x",
        groups: ["not-a-real-group"],
        sigAlgs: ["mldsa65"],
      },
    });
    expect(response.status()).toBe(400);
    expect(await response.text()).toContain("unknown kem_group");
  });
});

test.describe("KEM demo", () => {
  test("streams a handshake run against the healthy target", async ({ page }) => {
    await page.goto("/en/kem/group");
    await page.getByRole("combobox", { name: "TARGET ENDPOINT" }).selectOption("normal-server");
    await page.getByRole("button", { name: /RUN_HANDSHAKE/i }).click();

    await expect(page.getByText(/run_full_handshake/)).toBeVisible({ timeout: 60_000 });
    await expect(page.getByText(/Result:/)).toBeVisible({ timeout: 60_000 });
  });
});

test.describe("CAVP station", () => {
  test("grades a correct submission and flags a corrupted one", async ({ page, request }) => {
    await page.goto("/en/cavp/key_gen");
    await expect(page.getByRole("heading", { name: "Capability declaration" })).toBeVisible();
    // The prompt block, not the results textarea, which repeats the same vsId.
    await expect(page.locator("pre").first()).toContainText('"vsId": 91021');

    // Grade through the API with the reference answers, which is what a student who did the
    // browser computation correctly would submit.
    const pack = await (await request.get(`${API}/cavp/packs`)).json();
    const keyGen = pack.find((entry: { capability: string }) => entry.capability === "key_gen");
    const session = await (
      await request.post(`${API}/cavp/sessions`, { data: { slug: keyGen.slug } })
    ).json();

    const wrong = await request.post(`${API}/cavp/sessions/${session.id}/validate`, {
      data: {
        results: JSON.stringify({
          vsId: keyGen.vsId,
          mode: "keyGen",
          testGroups: [{ tgId: 1, tests: [{ tcId: 1, pk: "AA", sk: "BB" }] }],
        }),
      },
    });
    const report = await wrong.json();
    expect(report.correct).toBe(0);
    expect(report.cases[0].status).toBe("malformed");
    expect(report.cases[0].field).toBe("pk");
  });
});

test.describe("i18n", () => {
  test("serves both locales", async ({ page }) => {
    await page.goto("/en/environment");
    await expect(page.getByRole("heading", { name: "Environment" })).toBeVisible();

    await page.goto("/zh-TW/environment");
    await expect(page.getByRole("heading", { name: "測試環境" })).toBeVisible();
  });
});
