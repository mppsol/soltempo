/**
 * Record pitch.html → output/pitch.mp4 via headed Chromium + puppeteer.
 *
 * Same toolchain as record.mjs (the original 90s deck recorder), but
 * targets pitch.html and runs for 122s to cover the 6 × 20s scenes
 * plus a 2s tail safety. 1600×900 25fps so it concats with the
 * opening/closing/kamino-proof segments without re-encoding.
 */

import puppeteer from "puppeteer";
import { PuppeteerScreenRecorder } from "puppeteer-screen-recorder";
import * as path from "path";
import * as fs from "fs";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
const OUT_FILE = path.join(OUT_DIR, "pitch.mp4");
const HTML_FILE = path.join(__dirname, "pitch.html");

const VIDEO_CONFIG = {
  followNewTab: false,
  fps: 25,
  videoFrame: { width: 1600, height: 900 },
  videoCrf: 18,
  videoCodec: "libx264",
  videoPreset: "medium",
  aspectRatio: "16:9",
};

const RECORD_MS = 122_000; // 6 scenes × 20s + 2s tail

async function main() {
  if (!fs.existsSync(OUT_DIR)) fs.mkdirSync(OUT_DIR, { recursive: true });

  console.log("Launching Chromium...");
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
  await page.setViewport({ width: 1600, height: 900 });

  const recorder = new PuppeteerScreenRecorder(page, VIDEO_CONFIG);
  await recorder.start(OUT_FILE);

  console.log(`Loading ${HTML_FILE}`);
  await page.goto("file://" + HTML_FILE);

  console.log(`Recording ${RECORD_MS / 1000}s...`);
  await new Promise((r) => setTimeout(r, RECORD_MS));

  await recorder.stop();
  await browser.close();

  const stat = fs.statSync(OUT_FILE);
  console.log(`Done: ${OUT_FILE} (${(stat.size / 1024 / 1024).toFixed(2)} MB)`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
