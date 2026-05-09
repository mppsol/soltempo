// Record the soltempo demo HTML to MP4 via headed Chromium + puppeteer.
//
// Usage: node record.mjs
// Output: ./output/demo.mp4
//
// Total recording duration: 92s (matches the 90s scene timeline + 2s safety).

import puppeteer from "puppeteer";
import { PuppeteerScreenRecorder } from "puppeteer-screen-recorder";
import * as path from "path";
import * as fs from "fs";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
const OUT_FILE = path.join(OUT_DIR, "demo.mp4");
const HTML_FILE = path.join(__dirname, "index.html");

const VIDEO_CONFIG = {
  followNewTab: false,
  fps: 30,
  videoFrame: { width: 1920, height: 1080 },
  videoCrf: 18,
  videoCodec: "libx264",
  videoPreset: "medium",
  aspectRatio: "16:9",
};

const RECORD_MS = 92_000; // 92s — covers all 6 scenes (90s) plus tail

async function main() {
  if (!fs.existsSync(OUT_DIR)) fs.mkdirSync(OUT_DIR, { recursive: true });

  console.log("Launching Chromium...");
  const browser = await puppeteer.launch({
    headless: "new",
    defaultViewport: { width: 1920, height: 1080 },
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--window-size=1920,1080",
      "--hide-scrollbars",
    ],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1920, height: 1080 });

  const recorder = new PuppeteerScreenRecorder(page, VIDEO_CONFIG);
  await recorder.start(OUT_FILE);

  console.log(`Loading ${HTML_FILE}`);
  await page.goto("file://" + HTML_FILE);

  // Let the CSS keyframes do their thing.
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
