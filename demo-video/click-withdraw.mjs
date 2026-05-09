/**
 * One-off: drive the merchant-web Withdraw flow with a real browser.
 *
 * Boots headless Chromium, opens http://localhost:4001/, sets the
 * amount, clicks "Request pull-back", and waits for the activity
 * feed to show the confirmed tx signature. Extracts the signature
 * and prints the explorer URL so we can verify on devnet.
 *
 * Mirrors the demo-video puppeteer setup — same Chromium build, same
 * launch flags, just without recording.
 */

import puppeteer from "puppeteer";

const URL = "http://localhost:4001/";
const AMOUNT = process.env.AMOUNT ?? "1"; // USDC

const browser = await puppeteer.launch({
  headless: true,
  args: ["--no-sandbox", "--disable-setuid-sandbox"],
});

const page = await browser.newPage();
page.on("pageerror", (err) => console.error("page error:", err.message));
page.on("console", (msg) => {
  const t = msg.type();
  if (t === "error" || t === "warning") console.log(`[browser ${t}]`, msg.text());
});

await page.setViewport({ width: 1400, height: 900 });
await page.goto(URL, { waitUntil: "networkidle2", timeout: 30_000 });

// Give React time to hydrate and recover from any hydration mismatch.
// Errors #418/#423 are recoverable — React falls back to client render.
await new Promise((r) => setTimeout(r, 6_000));
await page.screenshot({ path: "/tmp/click-withdraw-before.png" });

const beforeState = await page.evaluate(() => {
  const btns = Array.from(document.querySelectorAll("button"));
  const withdraw = btns.find((b) => b.textContent?.trim() === "Request pull-back");
  const authorityTag = Array.from(document.querySelectorAll(".copy-tag"))
    .map((t) => t.textContent?.trim())
    .find((t) => t && t.startsWith("AmS"));
  return {
    buttonExists: !!withdraw,
    buttonDisabled: withdraw?.disabled ?? null,
    buttonText: withdraw?.textContent?.trim() ?? null,
    authorityVisible: authorityTag ?? null,
  };
});
console.log("\n--- before click ---");
console.log(JSON.stringify(beforeState, null, 2));

if (!beforeState.buttonExists) throw new Error("withdraw button not in DOM");
if (beforeState.buttonDisabled) {
  console.log("button still disabled after 6s — page may not have hydrated");
}

// The deposit-card holds the withdraw section — second deposit-card
// on the page (first is the deposit form). Find by surrounding card
// containing the Pull-back input.
const setAmountAndClick = async () => {
  return page.evaluate((amount) => {
    const sections = Array.from(document.querySelectorAll("h3.section-title"));
    const pullbackHeading = sections.find((h) =>
      h.textContent?.includes("Pull-back to Tempo"),
    );
    if (!pullbackHeading) throw new Error("Pull-back section not found");
    const card = pullbackHeading.nextElementSibling;
    if (!card) throw new Error("Pull-back card not found");
    const input = card.querySelector('input[type="number"]');
    const button = Array.from(card.querySelectorAll("button")).find(
      (b) => b.textContent?.trim() === "Request pull-back",
    );
    if (!input || !button) throw new Error("input or button missing");
    // Set the amount via React-friendly setter
    const setter = Object.getOwnPropertyDescriptor(
      window.HTMLInputElement.prototype,
      "value",
    ).set;
    setter.call(input, amount);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    button.click();
    return true;
  }, AMOUNT);
};

console.log(`▸ amount: ${AMOUNT} USDC`);
await setAmountAndClick();
console.log("▸ button clicked, waiting for tx confirmation…");

// The page surfaces the tx signature in two places once confirmed:
//   1. .copy-tag in the tx-progress card
//   2. an event row in the activity feed
// Wait for the copy-tag to populate.
await page.waitForFunction(
  () => {
    const tags = Array.from(document.querySelectorAll(".tx-progress .copy-tag"));
    return tags.length > 0 && tags[0].textContent && tags[0].textContent.length > 5;
  },
  { timeout: 60_000 },
);

const result = await page.evaluate(() => {
  const tags = Array.from(document.querySelectorAll(".tx-progress .copy-tag"));
  const steps = Array.from(document.querySelectorAll(".tx-progress .step")).map(
    (s) => s.textContent?.trim(),
  );
  const activityRows = Array.from(document.querySelectorAll(".event-row")).map(
    (r) => r.textContent?.trim(),
  );
  return {
    sigShort: tags[0]?.textContent?.trim() ?? null,
    steps,
    activityTop: activityRows.slice(0, 4),
  };
});

console.log("\n--- post-click page state ---");
console.log("tx (shortened):", result.sigShort);
console.log("\nprogress steps:");
for (const s of result.steps) console.log(" ", s);
console.log("\nactivity feed (top 4):");
for (const a of result.activityTop) console.log(" ", a);

await page.screenshot({ path: "/tmp/click-withdraw-after.png" });
console.log("\nscreenshot: /tmp/click-withdraw-after.png");

await browser.close();
