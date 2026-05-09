// Record a single HTML scene-set to MP4 via headless Chromium + puppeteer.
//
// Usage: node record-segment.mjs <input.html> <output.mp4> <duration-seconds> [width=1600] [height=900] [fps=25]
//
// Designed so opening.mp4 + terminal-demo.mp4 + closing.mp4 can be
// concat'd via `ffmpeg -c copy` with no re-encoding — the defaults
// (1600x900, 25fps, h264) match terminal-demo.mp4 exactly.

import puppeteer from "puppeteer";
import { PuppeteerScreenRecorder } from "puppeteer-screen-recorder";
import * as path from "path";
import * as fs from "fs";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const [, , inputArg, outputArg, durArg, widthArg, heightArg, fpsArg] =
  process.argv;

if (!inputArg || !outputArg || !durArg) {
  console.error(
    "Usage: node record-segment.mjs <input.html> <output.mp4> <duration-s> [width] [height] [fps]",
  );
  process.exit(1);
}

const INPUT = path.resolve(__dirname, inputArg);
const OUTPUT = path.resolve(__dirname, outputArg);
const RECORD_MS = parseFloat(durArg) * 1000;
const WIDTH = parseInt(widthArg ?? "1600", 10);
const HEIGHT = parseInt(heightArg ?? "900", 10);
const FPS = parseInt(fpsArg ?? "25", 10);

if (!fs.existsSync(INPUT)) {
  console.error(`Input not found: ${INPUT}`);
  process.exit(1);
}
const outDir = path.dirname(OUTPUT);
if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true });

async function main() {
  console.log(`Launching Chromium at ${WIDTH}x${HEIGHT}, ${FPS}fps`);
  const browser = await puppeteer.launch({
    headless: "new",
    defaultViewport: { width: WIDTH, height: HEIGHT },
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      `--window-size=${WIDTH},${HEIGHT}`,
      "--hide-scrollbars",
    ],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: WIDTH, height: HEIGHT });

  const recorder = new PuppeteerScreenRecorder(page, {
    followNewTab: false,
    fps: FPS,
    videoFrame: { width: WIDTH, height: HEIGHT },
    videoCrf: 18,
    videoCodec: "libx264",
    videoPreset: "medium",
    aspectRatio: "16:9",
  });

  await recorder.start(OUTPUT);
  console.log(`Loading ${INPUT}`);
  await page.goto("file://" + INPUT);

  console.log(`Recording ${RECORD_MS / 1000}s...`);
  await new Promise((r) => setTimeout(r, RECORD_MS));

  await recorder.stop();
  await browser.close();

  const stat = fs.statSync(OUTPUT);
  console.log(`Done: ${OUTPUT} (${(stat.size / 1024 / 1024).toFixed(2)} MB)`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
