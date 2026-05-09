/**
 * One-off: drive the merchant-web Deposit + bridge flow with a real browser.
 *
 * Boots headless Chromium, opens http://localhost:4001/, sets the
 * deposit amount, clicks "Deposit + bridge", waits for all three
 * Tempo txs to confirm and then for the keeper to relay to Solana
 * (vault.total_deposits incremented detected by the polling loop).
 *
 * Pairs with click-withdraw.mjs — same Chromium build, same launch
 * flags, just a different button + a longer wait.
 */

import puppeteer from "puppeteer";

const URL = "http://localhost:4001/";
const AMOUNT = process.env.AMOUNT ?? "1"; // pathUSD

const browser = await puppeteer.launch({
  headless: true,
  args: ["--no-sandbox", "--disable-setuid-sandbox"],
});

const page = await browser.newPage();
page.on("pageerror", (err) => console.error("page error:", err.message));
page.on("console", (msg) => {
  if (msg.type() === "error") console.log(`[browser error]`, msg.text());
});

await page.setViewport({ width: 1400, height: 900 });
await page.goto(URL, { waitUntil: "networkidle2", timeout: 30_000 });

// React hydration time.
await new Promise((r) => setTimeout(r, 6_000));
await page.screenshot({ path: "/tmp/click-deposit-before.png" });

const before = await page.evaluate(() => {
  const heading = Array.from(document.querySelectorAll("h3.section-title")).find(
    (h) => h.textContent?.trim() === "Deposit + bridge",
  );
  const card = heading?.nextElementSibling;
  const button = card
    ? Array.from(card.querySelectorAll("button")).find(
        (b) => b.textContent?.trim() === "Deposit + bridge",
      )
    : null;
  const merchantTag = Array.from(document.querySelectorAll(".card-foot")).find(
    (n) => n.textContent && n.textContent.startsWith("0x"),
  );
  return {
    buttonExists: !!button,
    buttonDisabled: button?.disabled ?? null,
    buttonText: button?.textContent?.trim() ?? null,
    merchantVisible: merchantTag?.textContent?.trim() ?? null,
  };
});
console.log("\n--- before click ---");
console.log(JSON.stringify(before, null, 2));
if (!before.buttonExists) throw new Error("Deposit button not in DOM");
if (before.buttonDisabled) throw new Error("Deposit button disabled — env not loaded");

// Set amount + click.
await page.evaluate((amount) => {
  const heading = Array.from(document.querySelectorAll("h3.section-title")).find(
    (h) => h.textContent?.trim() === "Deposit + bridge",
  );
  const card = heading.nextElementSibling;
  const input = card.querySelector('input[type="number"]');
  const button = Array.from(card.querySelectorAll("button")).find(
    (b) => b.textContent?.trim() === "Deposit + bridge",
  );
  const setter = Object.getOwnPropertyDescriptor(
    window.HTMLInputElement.prototype,
    "value",
  ).set;
  setter.call(input, amount);
  input.dispatchEvent(new Event("input", { bubbles: true }));
  button.click();
}, AMOUNT);

console.log(`▸ amount: ${AMOUNT} pathUSD`);
console.log("▸ button clicked, waiting for 3 Tempo txs to confirm…");

// Wait for all 3 tx hashes (approve + deposit + intent) to populate.
// approve may be skipped if allowance was already sufficient.
await page.waitForFunction(
  () => {
    const tags = Array.from(document.querySelectorAll(".tx-progress .copy-tag"));
    // intent hash is the 3rd (last); deposit is the 2nd.
    return tags.length >= 2;
  },
  { timeout: 90_000 },
);

const tempoState = await page.evaluate(() => {
  const tags = Array.from(document.querySelectorAll(".tx-progress .copy-tag")).map(
    (t) => t.textContent?.trim(),
  );
  const steps = Array.from(document.querySelectorAll(".tx-progress .step")).map(
    (s) => s.textContent?.trim(),
  );
  return { tags, steps };
});
console.log("\n--- tempo confirmations ---");
for (const s of tempoState.steps) console.log(" ", s);

// Now wait for the polling loop to detect the vault delta.
// Polling is every 4s; keeper relay typically <25s. Give it 60s.
console.log("\n▸ waiting for keeper relay → vault.total_deposits delta…");
await page
  .waitForFunction(
    () => {
      const rows = Array.from(document.querySelectorAll(".event-row"));
      return rows.some((r) =>
        r.textContent?.includes("Vault total_deposits incremented"),
      );
    },
    { timeout: 90_000 },
  )
  .catch((e) => {
    console.log("▸ vault delta not detected within 90s — may be keeper lag");
    throw e;
  });

const final = await page.evaluate(() => {
  const rows = Array.from(document.querySelectorAll(".event-row")).map((r) =>
    r.textContent?.trim(),
  );
  const totalDeposits = Array.from(document.querySelectorAll(".card")).find(
    (c) => c.querySelector(".card-eyebrow")?.textContent?.includes("vault.total_deposits"),
  );
  return {
    activityRows: rows.slice(0, 6),
    totalDeposits: totalDeposits?.querySelector(".card-value")?.textContent?.trim(),
  };
});

console.log("\n--- post-relay activity feed ---");
for (const r of final.activityRows) console.log(" ", r);
console.log("\nvault.total_deposits card:", final.totalDeposits);

await page.screenshot({ path: "/tmp/click-deposit-after.png" });
console.log("\nscreenshot: /tmp/click-deposit-after.png");

await browser.close();
