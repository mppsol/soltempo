/**
 * Record the merchant-web dashboard end-to-end against live testnets.
 *
 * Phase 1 (≈45s): deposit 1000 pathUSD via the UI
 *   → 3 Tempo txs confirm → keeper relays → vault.total_deposits + 1000
 * Phase 2 (≈25s): withdraw 500 USDC via the UI
 *   → vault.request_pullback_to_tempo → PullbackRequested event
 *
 * Pre-reqs (must be running before this script starts):
 *   - merchant-web on :4001 with both env keys set
 *   - keeper running with TEMPO_KEEPER_PRIVATE_KEY
 *
 * Output: ./output/merchant-web-demo.mp4 (1920x1080, ~75s)
 */

import puppeteer from "puppeteer";
import { PuppeteerScreenRecorder } from "puppeteer-screen-recorder";
import * as path from "path";
import * as fs from "fs";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_FILE = path.join(__dirname, "output", "merchant-web-demo.mp4");
const URL = "http://localhost:4001/";
const DEPOSIT_AMOUNT = "1000";
const WITHDRAW_AMOUNT = "500";

// 1600x900 — same aspect ratio as opening.html / closing.html (16:9)
// so the merchant-web cut concatenates cleanly with the deck segments
// without ffmpeg scaling artifacts. Whole dashboard fits at this size
// because we apply a 0.78 page zoom below.
const VIDEO_CONFIG = {
  followNewTab: false,
  fps: 30,
  videoFrame: { width: 1600, height: 900 },
  videoCrf: 18,
  videoCodec: "libx264",
  videoPreset: "medium",
  aspectRatio: "16:9",
};

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Smooth scroll to a target Y position. CSS scroll-behavior is set to
// 'smooth' globally so scrollTo animates by default; we still wait
// the typical animation duration before continuing.
const smoothScrollTo = async (page, y, holdMs = 1500) => {
  await page.evaluate((target) => window.scrollTo({ top: target, behavior: "smooth" }), y);
  await sleep(holdMs);
};

const setAmountAndClick = async (page, sectionTitle, buttonText, amount) =>
  page.evaluate(
    ({ sectionTitle, buttonText, amount }) => {
      const heading = Array.from(document.querySelectorAll("h3.section-title")).find(
        (h) => h.textContent?.trim() === sectionTitle,
      );
      if (!heading) throw new Error(`section "${sectionTitle}" not found`);
      const card = heading.nextElementSibling;
      const input = card.querySelector('input[type="number"]');
      const button = Array.from(card.querySelectorAll("button")).find(
        (b) => b.textContent?.trim() === buttonText,
      );
      const setter = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        "value",
      ).set;
      setter.call(input, amount);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      button.click();
    },
    { sectionTitle, buttonText, amount },
  );

async function main() {
  if (!fs.existsSync(path.dirname(OUT_FILE))) {
    fs.mkdirSync(path.dirname(OUT_FILE), { recursive: true });
  }

  console.log("Launching Chromium…");
  const browser = await puppeteer.launch({
    headless: "new",
    defaultViewport: { width: 1600, height: 900 },
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--window-size=1600,900",
      "--hide-scrollbars",
    ],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1600, height: 900, deviceScaleFactor: 1 });
  page.on("pageerror", (err) => console.error("page error:", err.message));

  console.log(`Loading ${URL}…`);
  await page.goto(URL, { waitUntil: "networkidle2", timeout: 30_000 });

  // Hydrate before recording starts so the first frames are clean.
  await sleep(5_000);

  // Apply a CSS zoom so the dashboard fits comfortably in the 900px
  // viewport — but the page is still taller than the viewport once
  // the deposit + withdraw progress cards expand, so we scroll at
  // strategic moments to keep the activity feed and footer in view.
  // Enable smooth scroll behavior globally so scrollTo animates.
  await page.evaluate(() => {
    document.body.style.zoom = "0.78";
    document.documentElement.style.scrollBehavior = "smooth";
  });
  await sleep(500);

  console.log(`Starting recorder → ${OUT_FILE}`);
  const recorder = new PuppeteerScreenRecorder(page, VIDEO_CONFIG);
  await recorder.start(OUT_FILE);

  // ── 0..6s: hold top-of-page (cards + forms + empty activity feed) ─
  await sleep(6_000);

  // ── Phase 1: deposit ───────────────────────────────────────────
  console.log(`▸ deposit ${DEPOSIT_AMOUNT} pathUSD`);
  await setAmountAndClick(page, "Deposit + bridge", "Deposit + bridge", DEPOSIT_AMOUNT);

  // Wait for vault delta (or up to 75s for keeper lag).
  console.log("waiting for vault.total_deposits delta…");
  await page
    .waitForFunction(
      () => {
        const rows = Array.from(document.querySelectorAll(".event-row"));
        return rows.some((r) =>
          r.textContent?.includes("Vault total_deposits incremented"),
        );
      },
      { timeout: 75_000 },
    )
    .catch(() => console.log("(vault delta wait timed out — continuing)"));

  // Hold to let the viewer see vault delta land in cards + tx-progress.
  await sleep(4_000);

  // Scroll down to reveal the activity feed + footer with the
  // "Vault total_deposits incremented" event.
  console.log("scrolling to reveal activity feed…");
  await smoothScrollTo(page, 280, 6_000);

  // Scroll back to top so the withdraw form is fully in frame
  // when the next click happens.
  console.log("scrolling back to top…");
  await smoothScrollTo(page, 0, 2_000);

  // ── Phase 2: withdraw ──────────────────────────────────────────
  console.log(`▸ withdraw ${WITHDRAW_AMOUNT} USDC`);
  await setAmountAndClick(page, "Pull-back to Tempo", "Request pull-back", WITHDRAW_AMOUNT);

  // Wait for the withdraw to finish (signature populated).
  console.log("waiting for pull-back tx confirmation…");
  await page
    .waitForFunction(
      () => {
        const tags = Array.from(document.querySelectorAll(".tx-progress .copy-tag"));
        return tags.some((t) => t.textContent && t.textContent.length > 5);
      },
      { timeout: 60_000 },
    )
    .catch(() => console.log("(pull-back tx wait timed out — continuing)"));

  await sleep(3_000);

  // Final scroll to show the activity feed with BOTH events
  // (deposit + pull-back) populated, plus the footer.
  console.log("final scroll to show full activity feed…");
  await smoothScrollTo(page, 380, 9_000);

  console.log("Stopping recorder…");
  await recorder.stop();
  await browser.close();

  const stat = fs.statSync(OUT_FILE);
  console.log(`Done: ${OUT_FILE} (${(stat.size / 1024 / 1024).toFixed(2)} MB)`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
