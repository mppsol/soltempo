/**
 * Render logo.html → ../logo.png via headless Chromium + puppeteer.
 * 1200×1200 @ 2x DPR = effective 2400×2400 PNG.
 */

import puppeteer from "puppeteer";
import * as path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const HTML_FILE = path.join(__dirname, "logo.html");
const OUT_FILE = path.join(__dirname, "..", "logo.png");

const browser = await puppeteer.launch({
  headless: "new",
  defaultViewport: { width: 1200, height: 1200, deviceScaleFactor: 2 },
});
const page = await browser.newPage();
await page.goto(`file://${HTML_FILE}`, { waitUntil: "networkidle0" });
await page.screenshot({ path: OUT_FILE, type: "png", omitBackground: false });
await browser.close();
console.log("Logo saved to", OUT_FILE);
