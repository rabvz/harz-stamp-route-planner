// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Browser smoke test.
//
// `cargo test` proves the Rust core; it cannot prove that the page boots, that
// the WebAssembly boundary works in a real engine, or that a click produces a
// plan. This script drives a headless Chromium over the DevTools protocol and
// asserts on what the user would see.
//
// Usage (the site must already be served):
//   cargo xtask web
//   cargo xtask serve &
//   node scripts/smoke-test.mjs [http://127.0.0.1:8080]
//
// Environment:
//   CHROME_PATH  path to a Chromium-based browser (auto-detected otherwise)

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const BASE_URL = process.argv[2] ?? "http://127.0.0.1:8080";
const DEBUG_PORT = 9333;

const CANDIDATES = [
  process.env.CHROME_PATH,
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean);

const failures = [];
function check(name, condition, detail = "") {
  if (condition) {
    console.log(`  ok   ${name}`);
  } else {
    console.log(`  FAIL ${name} ${detail}`);
    failures.push(`${name} ${detail}`);
  }
}

/**
 * Whether the run may rely on the public services.
 *
 * BRouter and Overpass are shared, public instances; a CI run can legitimately
 * be rate limited. With `EXPECT_NETWORK=1` a missing route or an empty parking
 * list is a failure; without it the test only requires that the UI reacts — and
 * says why it could not deliver — instead of failing silently.
 */
const EXPECT_NETWORK = process.env.EXPECT_NETWORK === "1";

/** Polls an expression in the page until the predicate holds or time is up. */
async function poll(devtools, expression, predicate, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    last = await devtools.evaluate(expression);
    if (predicate(last)) {
      return last;
    }
    await new Promise((resolve) => setTimeout(resolve, 400));
  }
  return last;
}

function findBrowser() {
  for (const candidate of CANDIDATES) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  throw new Error("no Chromium-based browser found; set CHROME_PATH");
}

/** Minimal CDP client over the WebSocket built into Node 22. */
class DevTools {
  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        if (message.error) {
          reject(new Error(message.error.message));
        } else {
          resolve(message.result);
        }
      }
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  /** Evaluates an expression and returns its JSON value. */
  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.text ?? "evaluation failed");
    }
    return result.result.value;
  }
}

async function waitForTarget() {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${DEBUG_PORT}/json/list`);
      const targets = await response.json();
      const page = targets.find((target) => target.type === "page");
      if (page?.webSocketDebuggerUrl) {
        return page.webSocketDebuggerUrl;
      }
    } catch {
      // The browser is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error("browser did not expose a debugging target");
}

const browserPath = findBrowser();
console.log(`browser: ${browserPath}`);
console.log(`target:  ${BASE_URL}`);

// A fresh profile per run. Reusing one would restore the selection, service
// worker and IndexedDB state from the previous run, so a test that clicks five
// rows would toggle five already-selected stations off and see zero.
const profile = process.env.SMOKE_PROFILE ?? mkdtempSync(join(tmpdir(), "hsr-smoke-"));

const browser = spawn(
  browserPath,
  [
    "--headless=new",
    "--disable-gpu",
    "--no-sandbox",
    "--no-first-run",
    "--no-default-browser-check",
    "--remote-allow-origins=*",
    `--remote-debugging-port=${DEBUG_PORT}`,
    // A dedicated profile is required: Chromium refuses remote debugging on the
    // default profile.
    "--user-data-dir=" + profile,
    "about:blank",
  ],
  { stdio: ["ignore", "ignore", "ignore"] },
);

let exitCode = 0;
try {
  const webSocketUrl = await waitForTarget();
  const socket = new WebSocket(webSocketUrl);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  const devtools = new DevTools(socket);

  await devtools.send("Page.enable");
  await devtools.send("Runtime.enable");
  await devtools.send("Network.enable");

  const consoleErrors = [];
  const failedRequests = [];
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.method === "Runtime.exceptionThrown") {
      consoleErrors.push(message.params.exceptionDetails?.text ?? "exception");
    }
    if (message.method === "Network.responseReceived") {
      const { response } = message.params;
      // Only same-origin requests are our responsibility. Map tiles come from a
      // third party and may legitimately be rate limited.
      if (response.status >= 400 && response.url.startsWith(BASE_URL)) {
        failedRequests.push(`${response.status} ${response.url}`);
      }
    }
  });

  await devtools.send("Page.navigate", { url: BASE_URL });
  await new Promise((resolve) => setTimeout(resolve, 4000));

  console.log("\nboot");
  const badge = await devtools.evaluate(
    "document.querySelector('#catalog-badge')?.textContent ?? ''",
  );
  check("catalogue badge reports 222 stations", badge.includes("222"), `got "${badge}"`);

  const engine = await devtools.evaluate(
    "document.querySelector('#engine-badge')?.textContent ?? ''",
  );
  check("engine badge reports Rust/WASM", engine.includes("Rust/WASM"), `got "${engine}"`);

  const rows = await devtools.evaluate("document.querySelectorAll('.stamp-item').length");
  check("all stations are listed", rows === 222, `got ${rows}`);

  const markers = await devtools.evaluate("document.querySelectorAll('.leaflet-interactive').length");
  check("all stations are on the map", markers >= 222, `got ${markers}`);

  console.log("\nselection");
  await devtools.evaluate(`
    (() => {
      const rows = [...document.querySelectorAll('.stamp-item')].slice(0, 5);
      for (const row of rows) row.click();
      return rows.length;
    })()
  `);
  const selected = await devtools.evaluate(
    "document.querySelector('#selection-count')?.textContent ?? ''",
  );
  check("clicking rows selects stations", selected === "5", `got "${selected}"`);

  console.log("\nplanning");
  await devtools.evaluate("document.querySelector('#use-first-stamp').click()");
  const startLabel = await devtools.evaluate(
    "document.querySelector('#start-label')?.textContent ?? ''",
  );
  check("start point is set", startLabel.includes(","), `got "${startLabel}"`);

  await devtools.evaluate("document.querySelector('#plan').click()");
  await new Promise((resolve) => setTimeout(resolve, 1500));

  const summary = await devtools.evaluate(
    "document.querySelector('#plan-summary')?.textContent ?? ''",
  );
  const days = await devtools.evaluate("document.querySelectorAll('.day-card').length");
  check("a plan is produced", days >= 1, `got ${days} day cards`);
  check("summary shows stamps and distance", /Stempel/.test(summary) && /km/.test(summary), `got "${summary}"`);

  const polyline = await devtools.evaluate(
    "document.querySelectorAll('path.leaflet-interactive').length",
  );
  check("plan is drawn on the map", polyline >= 222, `got ${polyline} paths`);

  // The plan line has its own stroke colour; counting every interactive path
  // would also count the 222 station circles, so check the colour explicitly.
  const planLines = await devtools.evaluate(
    "document.querySelectorAll('path[stroke=\"#a9631b\"]').length",
  );
  check("plan polyline uses the plan colour", planLines >= 1, `got ${planLines}`);

  console.log("\nrouting along trails");
  const routing = await poll(
    devtools,
    `(() => ({
      summary: document.querySelector('#plan-summary')?.textContent ?? '',
      status: document.querySelector('#plan-status')?.textContent ?? '',
      durations: document.querySelectorAll('.day-card__head span:nth-child(2)').length,
    }))()`,
    (value) => /geroutet/.test(value.summary) || /geschätzt|nicht erreichbar|fehlgeschlagen|abgebrochen/i.test(value.status),
    EXPECT_NETWORK ? 45000 : 30000,
  );
  if (EXPECT_NETWORK) {
    check("plan uses routed distances", /geroutet/.test(routing.summary), `summary "${routing.summary}", status "${routing.status}"`);
    // Real routing also yields a walking time per day; an estimate has none.
    check("day cards show a walking time", routing.durations > 0, `got ${routing.durations}`);
  } else {
    check(
      "routing reports its outcome instead of staying silent",
      /geroutet/.test(routing.summary) || routing.status.trim().length > 0,
      `summary "${routing.summary}", status "${routing.status}"`,
    );
  }

  console.log("\nparking from OpenStreetMap");
  await devtools.evaluate("document.querySelector('#find-parking').click()");
  const parking = await poll(
    devtools,
    `(() => ({
      items: document.querySelectorAll('.parking-item').length,
      status: document.querySelector('#parking-status')?.textContent ?? '',
    }))()`,
    (value) => value.items > 0 || /fehlgeschlagen|nicht erreichbar|ausgelastet|Keine Parkplätze/i.test(value.status),
    EXPECT_NETWORK ? 45000 : 30000,
  );
  if (EXPECT_NETWORK) {
    check("parking search returns candidates", parking.items > 0, `status "${parking.status}"`);
  } else {
    check(
      "parking search reports its outcome instead of staying silent",
      parking.items > 0 || parking.status.trim().length > 0,
      `items ${parking.items}, status "${parking.status}"`,
    );
  }

  // Picking a candidate must move the start point; only meaningful when the
  // list is populated.
  if (parking.items > 0) {
    const moved = await devtools.evaluate(`
      (() => {
        const before = document.querySelector('#start-label')?.textContent ?? '';
        document.querySelector('.parking-item').click();
        const after = document.querySelector('#start-label')?.textContent ?? '';
        return { before, after };
      })()
    `);
    check("choosing a parking sets the start point", moved.after.includes(","), JSON.stringify(moved));
  }

  console.log("\nclustering");
  await devtools.evaluate("document.querySelector('#cluster').click()");
  await new Promise((resolve) => setTimeout(resolve, 600));
  const clusters = await devtools.evaluate("document.querySelectorAll('.cluster-card').length");
  check("clustering produces groups", clusters >= 1, `got ${clusters}`);

  console.log("\ngpx export");
  const gpx = await devtools.evaluate(`
    (async () => {
      const module = await import('/js/wasm.js');
      return module.call('exportGpx', { stampIds: [1, 2, 3], name: 'Smoke test' });
    })()
  `);
  check("export returns a GPX document", typeof gpx?.gpx === "string" && gpx.gpx.includes("<gpx"));
  check("export contains all waypoints", gpx?.waypointCount === 3, `got ${gpx?.waypointCount}`);

  console.log("\noffline shell");
  const manifest = await devtools.evaluate(`
    (async () => {
      const response = await fetch('manifest.webmanifest');
      return response.ok ? await response.json() : null;
    })()
  `);
  check("web app manifest is served", manifest?.name?.includes("Harzer"), JSON.stringify(manifest?.name));

  // A missing vendored asset (Leaflet's marker images, for example) does not
  // throw: it degrades into broken-image alt text on the map. Only the network
  // responses reveal that class of defect.
  check(
    "no same-origin request failed",
    failedRequests.length === 0,
    failedRequests.join("; "),
  );

  check("no uncaught page exceptions", consoleErrors.length === 0, consoleErrors.join("; "));

  socket.close();
} catch (error) {
  console.error(`\nsmoke test failed: ${error.message}`);
  failures.push(error.message);
} finally {
  browser.kill();
  // Leave no temporary profile behind; the default profile path is a fresh
  // directory, so removing it is safe.
  if (!process.env.SMOKE_PROFILE) {
    try {
      rmSync(profile, { recursive: true, force: true });
    } catch {
      // A lock held by a process that has not exited yet is not a failure.
    }
  }
}

if (failures.length > 0) {
  exitCode = 1;
  console.error(`\n${failures.length} check(s) failed`);
} else {
  console.log("\nall smoke checks passed");
}
process.exit(exitCode);
