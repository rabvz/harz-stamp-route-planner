// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Captures the screenshot used in the README.
//
// The image is taken from the running application with a real selection and a
// real computed plan, so the documentation cannot drift away from what the page
// actually looks like.
//
// Usage:
//   cargo xtask web
//   cargo xtask serve &
//   node scripts/screenshot.mjs [http://127.0.0.1:8080] [docs/images/screenshot.png]

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const BASE_URL = process.argv[2] ?? "http://127.0.0.1:8080";
const OUTPUT = process.argv[3] ?? "docs/images/screenshot.png";
const DEBUG_PORT = 9334;
const WIDTH = 1440;
const HEIGHT = 900;

const CANDIDATES = [
  process.env.CHROME_PATH,
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
].filter(Boolean);

const browserPath = CANDIDATES.find((candidate) => existsSync(candidate));
if (!browserPath) {
  console.error("no Chromium-based browser found; set CHROME_PATH");
  process.exit(1);
}

/**
 * Polls a page expression until it is truthy.
 *
 * The screenshot waits for real work: the Overpass query and the routing
 * requests. Fixed sleeps would either be too short (empty list in the image) or
 * waste time.
 * @param {(expression: string) => Promise<any>} evaluate
 * @param {string} expression
 * @param {number} timeoutMs
 */
async function waitUntil(evaluate, expression, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await evaluate(expression)) {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`timed out waiting for: ${expression}`);
}

// A fresh profile keeps the screenshot reproducible: no restored selection and
// no cached service worker from an earlier run.
const profile = mkdtempSync(join(tmpdir(), "hsr-screenshot-"));

const browser = spawn(
  browserPath,
  [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--no-first-run",
    "--remote-allow-origins=*",
    `--remote-debugging-port=${DEBUG_PORT}`,
    "--window-size=" + WIDTH + "," + HEIGHT,
    "--user-data-dir=" + profile,
    "about:blank",
  ],
  { stdio: ["ignore", "ignore", "ignore"] },
);

async function waitForTarget() {  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${DEBUG_PORT}/json/list`)).json();
      const page = targets.find((target) => target.type === "page");
      if (page?.webSocketDebuggerUrl) {
        return page.webSocketDebuggerUrl;
      }
    } catch {
      // still starting
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error("browser did not expose a debugging target");
}

let exitCode = 0;
try {
  const socket = new WebSocket(await waitForTarget());
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });

  let nextId = 1;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const { resolve } = pending.get(message.id);
      pending.delete(message.id);
      resolve(message.result);
    }
  });
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      const id = nextId++;
      pending.set(id, { resolve });
      socket.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const result = await send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    return result.result?.value;
  };

  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", {
    width: WIDTH,
    height: HEIGHT,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await send("Page.navigate", { url: BASE_URL });
  await new Promise((resolve) => setTimeout(resolve, 5000));

  // Build a plausible selection around the Brocken and plan it, so the image
  // shows the application doing its job rather than an empty map.
  await evaluate(`
    (() => {
      const wanted = ['HWN001','HWN002','HWN003','HWN005','HWN006'];
      let count = 0;
      for (const row of document.querySelectorAll('.stamp-item')) {
        const code = row.querySelector('.stamp-item__code')?.textContent?.trim();
        if (wanted.includes(code)) { row.click(); count += 1; }
      }
      return count;
    })()
  `);

  // A generous distance budget keeps the example tour realistic rather than
  // showing a page full of limit warnings.
  await evaluate(`
    (() => {
      const field = document.querySelector('#max-tour-km');
      field.value = '30';
      field.dispatchEvent(new Event('change', { bubbles: true }));
      return field.value;
    })()
  `);

  // Show the stamp pass in the image as well: two stations already collected.
  await evaluate(`
    (() => {
      const toggles = [...document.querySelectorAll('.stamp-toggle')].slice(0, 2);
      for (const toggle of toggles) toggle.click();
      return toggles.length;
    })()
  `);

  // Real trailhead from OpenStreetMap, then a real routed tour: the image should
  // show what the application actually does, not a staged state. The public
  // Overpass instance is occasionally rate limited; the first stamp is an
  // honest fallback start rather than an empty screenshot.
  await evaluate("document.querySelector('#find-parking').click()");
  let parked = false;
  try {
    await waitUntil(
      evaluate,
      "document.querySelectorAll('.parking-item').length > 0",
      30000,
    );
    parked = true;
  } catch {
    // Fall through to the first stamp.
  }
  await evaluate(
    parked
      ? "document.querySelector('.parking-item')?.click()"
      : "document.querySelector('#use-first-stamp').click()",
  );
  await evaluate("document.querySelector('#plan').click()");
  await waitUntil(
    evaluate,
    "(document.querySelector('#plan-summary')?.textContent ?? '').includes('geroutet')",
    60000,
  );
  await evaluate("document.querySelector('#cluster').click()");

  // Tooltips opened by clicking rows would sit on top of the tour.
  await evaluate("document.querySelectorAll('.leaflet-tooltip').forEach((el) => el.remove())");
  // Scroll the sidebar to the result so the image shows the computed tour and
  // its elevation profile rather than only the station list.
  await evaluate(`
    (() => {
      const panel = document.querySelector('#result-panel');
      if (panel) panel.scrollIntoView({ block: 'start' });
      return true;
    })()
  `);
  await new Promise((resolve) => setTimeout(resolve, 1500));

  const screenshot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  mkdirSync(dirname(OUTPUT), { recursive: true });
  writeFileSync(OUTPUT, Buffer.from(screenshot.data, "base64"));
  console.log(`screenshot written to ${OUTPUT}`);

  socket.close();
} catch (error) {
  console.error(`screenshot failed: ${error.message}`);
  exitCode = 1;
} finally {
  browser.kill();
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {
    // The browser may still hold a lock briefly; the temp directory is cleaned
    // up by the OS either way.
  }
}

process.exit(exitCode);
