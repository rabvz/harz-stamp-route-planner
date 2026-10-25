// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Service worker for the app shell.
//
// Scope, deliberately: the interface, the WebAssembly module and the vendored
// map library are cached so the planner opens without a network. Map *tiles*
// are NOT cached — the OpenStreetMap tile usage policy forbids building an
// offline tile archive from the public tile server, so the map needs a
// connection. Everything the planner computes works offline; only the
// background map does not. The UI says so instead of pretending otherwise.

// Bumped whenever the shell changes shape; the activate handler drops older
// caches, so a new version also evicts stale files.
const CACHE_NAME = "hsr-shell-v2";

const SHELL = [
  "./",
  "index.html",
  "styles.css",
  "manifest.webmanifest",
  "attribution.html",
  "icons/icon.svg",
  "js/app.js",
  "js/config.js",
  "js/format.js",
  "js/mapview.js",
  "js/overpass.js",
  "js/routing.js",
  "js/store.js",
  "js/wasm.js",
  "pkg/route_planner.wasm",
  "vendor/leaflet/leaflet.js",
  "vendor/leaflet/leaflet.css",
];

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE_NAME);
      // `addAll` fails the whole install if one file is missing; add them
      // individually so a single 404 cannot disable offline support entirely.
      await Promise.all(
        SHELL.map(async (path) => {
          try {
            await cache.add(new Request(path, { cache: "reload" }));
          } catch {
            // A missing optional asset is not fatal.
          }
        }),
      );
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const names = await caches.keys();
      await Promise.all(names.filter((name) => name !== CACHE_NAME).map((name) => caches.delete(name)));
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") {
    return;
  }

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) {
    // Third-party requests (map tiles, optional routing service) always go to
    // the network and are never cached.
    return;
  }

  // Navigations: prefer the network so a deployment is picked up, fall back to
  // the cached shell when offline.
  if (request.mode === "navigate") {
    event.respondWith(
      (async () => {
        try {
          const response = await fetch(request);
          const cache = await caches.open(CACHE_NAME);
          cache.put("index.html", response.clone());
          return response;
        } catch {
          const cached = (await caches.match("index.html")) ?? (await caches.match("./"));
          return cached ?? Response.error();
        }
      })(),
    );
    return;
  }

  // Static assets: network first, cache as the offline fallback.
  //
  // This used to be cache-first, which meant a deployment never reached
  // returning visitors: the worker kept serving the previous app.js and
  // routing.js until someone cleared site data. For a small app the extra
  // request is cheap, and correctness beats a few saved milliseconds. When the
  // network is unavailable the cached copy is used, so offline still works.
  event.respondWith(
    (async () => {
      try {
        const response = await fetch(request);
        if (response.ok) {
          const cache = await caches.open(CACHE_NAME);
          cache.put(request, response.clone());
        }
        return response;
      } catch {
        const cached = await caches.match(request);
        return cached ?? Response.error();
      }
    })(),
  );
});
