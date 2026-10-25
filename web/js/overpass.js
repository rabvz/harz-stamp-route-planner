// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Trailhead parking from OpenStreetMap.
//
// The project deliberately ships no parking dataset: a hand-maintained list
// would have unclear licensing and would rot. Instead the browser asks the
// Overpass API for `amenity=parking` and `highway=layby` inside the area of the
// user's current selection, and the Rust core ranks what comes back.
//
// Consequences that the UI must communicate:
// * the query goes to a third party (overpass-api.de) — only after an explicit
//   click, never automatically;
// * the data is © OpenStreetMap contributors, ODbL;
// * "parking" in OSM means "somebody mapped a parking area", not "official
//   trailhead of the Harzer Wandernadel".

import { CONFIG } from "./config.js";

/** Half a degree of padding is plenty and keeps the query bounded. */
const MAX_BBOX_SPAN_DEGREES = 0.5;

/** Cached responses per bounding box, so re-opening the list is free. */
const cache = new Map();
const CACHE_TTL_MS = 10 * 60 * 1000;

/**
 * Builds the Overpass query for a bounding box.
 * @param {{south: number, west: number, north: number, east: number}} bounds
 * @param {number} timeoutSeconds
 * @returns {string}
 */
export function buildQuery(bounds, timeoutSeconds) {
  const bbox = `${bounds.south.toFixed(5)},${bounds.west.toFixed(5)},${bounds.north.toFixed(5)},${bounds.east.toFixed(5)}`;
  return [
    `[out:json][timeout:${timeoutSeconds}];`,
    "(",
    `  node["amenity"="parking"](${bbox});`,
    `  way["amenity"="parking"](${bbox});`,
    `  node["highway"="layby"](${bbox});`,
    `  way["highway"="layby"](${bbox});`,
    ");",
    "out center tags;",
  ].join("\n");
}

/**
 * Expands a bounding box by a radius in metres.
 * @param {{south: number, west: number, north: number, east: number}} bounds
 * @param {number} radiusM
 */
export function expandBounds(bounds, radiusM) {
  const dLat = radiusM / 111_320;
  const meanLat = (bounds.south + bounds.north) / 2;
  const cosLat = Math.max(0.01, Math.abs(Math.cos((meanLat * Math.PI) / 180)));
  const dLon = dLat / cosLat;
  const south = Math.max(-89.9, bounds.south - dLat);
  const north = Math.min(89.9, bounds.north + dLat);
  const west = Math.max(-179.9, bounds.west - dLon);
  const east = Math.min(179.9, bounds.east + dLon);

  // Keep a selection spanning half the country from producing a query that the
  // public instance will reject or that takes minutes.
  const clampSpan = (low, high) =>
    high - low > MAX_BBOX_SPAN_DEGREES ? low + MAX_BBOX_SPAN_DEGREES : high;

  return {
    south,
    west,
    north: clampSpan(south, north),
    east: clampSpan(west, east),
  };
}

/**
 * Turns an Overpass element into a parking candidate, or `null` if unusable.
 * @param {any} element
 * @returns {{id: string, name: string, lat: number, lon: number, kind: string, capacity: number|null, source: string} | null}
 */
export function toCandidate(element) {
  const tags = element?.tags ?? {};

  // Closed or explicitly private areas are not places to start a hike.
  const access = String(tags.access ?? "").toLowerCase();
  if (["private", "no", "customers"].includes(access)) {
    return null;
  }

  const lat = Number(element.lat ?? element.center?.lat);
  const lon = Number(element.lon ?? element.center?.lon);
  if (!Number.isFinite(lat) || !Number.isFinite(lon)) {
    return null;
  }

  const isLayby = tags.highway === "layby";
  const name =
    (tags.name && String(tags.name).trim()) ||
    (tags.ref && `Parkplatz ${String(tags.ref).trim()}`) ||
    (isLayby ? "Haltebucht (ohne Namen)" : "Parkplatz (ohne Namen)");

  // A trailhead is anything the mapper marked as aimed at hikers, or a named
  // "Wanderparkplatz". Everything else stays a generic public car park; the
  // ranking in the Rust core already prefers the former.
  const looksLikeTrailhead =
    String(tags.hiking ?? "").toLowerCase() === "yes" ||
    /wander\s*-?\s*parkplatz/i.test(String(tags.name ?? ""));

  const kind = isLayby ? "roadside" : looksLikeTrailhead ? "trailhead" : "public";

  const capacityRaw = Number.parseInt(String(tags.capacity ?? ""), 10);
  const capacity = Number.isFinite(capacityRaw) && capacityRaw > 0 ? capacityRaw : null;

  return {
    id: `${element.type}/${element.id}`,
    name,
    lat,
    lon,
    kind,
    capacity,
    source: "openstreetmap",
  };
}

/**
 * Performs one request attempt.
 *
 * Separated from the caller so a transient timeout can be retried without
 * duplicating the request construction or the error mapping.
 * @param {string} query
 * @param {AbortSignal | undefined} outerSignal
 */
async function requestOnce(query, outerSignal) {
  const timeout = AbortSignal.timeout?.(CONFIG.overpass.clientTimeoutMs ?? 60000);
  const combined =
    outerSignal && timeout ? AbortSignal.any([outerSignal, timeout]) : (outerSignal ?? timeout);

  try {
    return await fetch(CONFIG.overpass.endpoint, {
      method: "POST",
      // A plain form body keeps this a CORS "simple request": no preflight,
      // which the public instance handles more reliably than JSON.
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({ data: query }).toString(),
      signal: combined,
    });
  } catch (error) {
    if (outerSignal?.aborted) {
      throw new Error("Die Parkplatzsuche wurde abgebrochen.");
    }
    if (error?.name === "AbortError" || error?.name === "TimeoutError") {
      throw new Error(
        "OpenStreetMap (Overpass) hat nicht rechtzeitig geantwortet. " +
          "Der öffentliche Dienst ist zeitweise ausgelastet; bitte erneut versuchen " +
          "oder den Startpunkt per Kartenklick setzen.",
      );
    }
    throw new Error(
      "OpenStreetMap (Overpass) ist nicht erreichbar. Startpunkt bitte per Kartenklick setzen.",
    );
  }
}

/**
 * Statuses that mean "the service is busy", not "your request is wrong".
 *
 * The public Overpass instance answers an overloaded backend with 502 or 504
 * within a few hundred milliseconds. Reporting that as "Parkplatzsuche
 * fehlgeschlagen (HTTP 502)" — as an earlier revision did — tells the user
 * nothing and invites them to think the app is broken.
 */
function isBusyStatus(status) {
  return status === 429 || status === 500 || status === 502 || status === 503 || status === 504;
}

/** Explains a busy response in a way the user can act on. */
function busyMessage(status) {
  if (status === 429) {
    return (
      "OpenStreetMap (Overpass) drosselt gerade Anfragen. Bitte in einer Minute erneut " +
      "versuchen — oder den Startpunkt per Kartenklick setzen."
    );
  }
  return (
    `OpenStreetMap (Overpass) hatte einen vorübergehenden Fehler (HTTP ${status}). ` +
    "Das kommt bei dem öffentlichen Dienst gelegentlich vor. Bitte noch einmal auf " +
    "„Parkplätze suchen“ drücken — oder den Startpunkt per Kartenklick setzen."
  );
}

/**
 * Fetches parking candidates inside the given bounds.
 * @param {{south: number, west: number, north: number, east: number}} bounds
 * @param {{signal?: AbortSignal, force?: boolean}} [options]
 * @returns {Promise<{candidates: Array<object>, attribution: string}>}
 */
export async function fetchParkingCandidates(bounds, options = {}) {
  const query = buildQuery(bounds, CONFIG.overpass.timeoutSeconds ?? 30);
  const cached = cache.get(query);
  if (!options.force && cached && Date.now() - cached.at < CACHE_TTL_MS) {
    return { candidates: cached.candidates, attribution: ATTRIBUTION };
  }

  const attempts = Math.max(1, CONFIG.overpass.attempts ?? 3);
  const pause = () =>
    new Promise((resolve) => setTimeout(resolve, CONFIG.overpass.retryDelayMs ?? 2000));

  let response = null;
  let lastError = null;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      response = await requestOnce(query, options.signal);
    } catch (error) {
      // A timeout is worth another try; an abort or a DNS failure is not.
      lastError = error;
      if (!/nicht rechtzeitig/.test(error.message) || attempt + 1 >= attempts) {
        throw error;
      }
      await pause();
      continue;
    }

    if (response.ok) {
      break;
    }

    if (isBusyStatus(response.status)) {
      lastError = new Error(busyMessage(response.status));
      if (attempt + 1 < attempts) {
        await pause();
        continue;
      }
      throw lastError;
    }

    throw new Error(`Parkplatzsuche fehlgeschlagen (HTTP ${response.status}).`);
  }

  if (!response || !response.ok) {
    throw lastError ?? new Error("Parkplatzsuche fehlgeschlagen.");
  }

  const payload = await response.json();
  const candidates = [];
  const seen = new Set();
  for (const element of payload.elements ?? []) {
    const candidate = toCandidate(element);
    if (candidate && !seen.has(candidate.id)) {
      seen.add(candidate.id);
      candidates.push(candidate);
    }
  }

  if (cache.size >= 32) {
    cache.clear();
  }
  cache.set(query, { at: Date.now(), candidates });
  return { candidates, attribution: ATTRIBUTION };
}

/** Attribution text required by the ODbL. */
export const ATTRIBUTION =
  'Parkplätze: © <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap</a>-Mitwirkende (ODbL)';
