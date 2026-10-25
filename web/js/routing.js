// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Routing along real hiking trails.
//
// Provider: BRouter's public instance with a hiking profile. It needs no API
// key, which is the decisive property for a static site: anything requiring a
// key would have to publish it. The endpoint is configurable, so a deployment
// with its own BRouter (or any GeoJSON service) can point elsewhere.
//
// One request covers a whole day: BRouter accepts a list of via points and
// returns a single LineString plus `track-length` and `total-time`. That keeps
// a five-day plan at five requests instead of hundreds of pairwise ones.
//
// Everything degrades: if the service is unreachable, the caller falls back to
// the straight-line estimate and says so. No result is ever invented.

import { CONFIG } from "./config.js";

/** In-memory cache, keyed by the exact request URL. */
const cache = new Map();
const MAX_CACHE_ENTRIES = 256;

/**
 * Whether a routing endpoint is configured.
 * @returns {boolean}
 */
export function isRoutingEnabled() {
  return typeof CONFIG.routing?.urlTemplate === "string" && CONFIG.routing.urlTemplate.length > 0;
}

/** Human readable provider name for the UI and for exports. */
export function routingProviderName() {
  return `${CONFIG.routing.providerLabel} (${CONFIG.routing.profile})`;
}

/** Empties the in-memory cache. */
export function clearRouteCache() {
  cache.clear();
}

/**
 * Coerces a value that may be a number or a numeric string.
 *
 * BRouter sends `track-length` and `total-time` as strings, so `Number.isFinite`
 * on the raw value is false and the distance would silently fall back to the
 * measured geometry length.
 * @param {unknown} value
 * @returns {number | null}
 */
function toNumber(value) {
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : null;
  }
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/**
 * Parses the GeoJSON subset used by BRouter, OpenRouteService and Valhalla.
 * @param {any} data
 * @returns {{points: Array<{lat: number, lon: number}>, distanceM: number, durationS: number}}
 */
export function parseRoute(data) {
  let geometry = data;
  let properties = data?.properties ?? {};

  if (data?.type === "FeatureCollection") {
    const feature = data.features?.[0];
    if (!feature?.geometry) {
      throw new Error("Die Routing-Antwort enthält keine Geometrie.");
    }
    geometry = feature.geometry;
    properties = feature.properties ?? {};
  } else if (data?.type === "Feature") {
    geometry = data.geometry;
    properties = data.properties ?? {};
  }

  if (geometry?.type !== "LineString" || !Array.isArray(geometry.coordinates)) {
    throw new Error("Die Routing-Antwort ist keine LineString-Geometrie.");
  }

  const points = [];
  for (const coordinate of geometry.coordinates) {
    if (!Array.isArray(coordinate) || coordinate.length < 2) {
      continue;
    }
    // GeoJSON is [lon, lat]; BRouter appends elevation as a third value.
    const lon = Number(coordinate[0]);
    const lat = Number(coordinate[1]);
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) {
      continue;
    }
    const point = { lat, lon };
    const elevation = Number(coordinate[2]);
    if (Number.isFinite(elevation)) {
      point.ele = elevation;
    }
    points.push(point);
  }
  if (points.length < 2) {
    throw new Error("Die Routing-Antwort enthält weniger als zwei Punkte.");
  }

  const distance =
    toNumber(properties.distance) ??
    toNumber(properties["track-length"]) ??
    toNumber(properties.summary?.distance);
  const duration =
    toNumber(properties.duration) ??
    toNumber(properties["total-time"]) ??
    toNumber(properties.time) ??
    toNumber(properties.summary?.duration);

  return {
    points,
    distanceM: distance ?? polylineLength(points),
    durationS: duration ?? Math.round(polylineLength(points) / 1.25),
  };
}

/** Great-circle length of a polyline, used when the service sends no summary. */
function polylineLength(points) {
  const earthRadius = 6371008.8;
  let total = 0;
  for (let index = 1; index < points.length; index += 1) {
    const a = points[index - 1];
    const b = points[index];
    const dLat = ((b.lat - a.lat) * Math.PI) / 180;
    const dLon = ((b.lon - a.lon) * Math.PI) / 180;
    const lat1 = (a.lat * Math.PI) / 180;
    const lat2 = (b.lat * Math.PI) / 180;
    const h =
      Math.sin(dLat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dLon / 2) ** 2;
    total += 2 * earthRadius * Math.asin(Math.min(1, Math.sqrt(h)));
  }
  return total;
}

/**
 * Builds the request URL for an ordered list of waypoints.
 * @param {Array<{lat: number, lon: number}>} waypoints
 * @returns {string}
 */
function buildUrl(waypoints) {
  const lonlats = waypoints
    .map((point) => `${point.lon.toFixed(6)},${point.lat.toFixed(6)}`)
    .join("|");
  return CONFIG.routing.urlTemplate
    .replace("{lonlats}", encodeURIComponent(lonlats))
    .replace("{profile}", encodeURIComponent(CONFIG.routing.profile));
}

/**
 * Routes one sequence of waypoints.
 *
 * Long sequences are split into overlapping chunks so a request never exceeds
 * the provider's via-point limit; distances are summed and the duplicate
 * junction point is dropped from the geometry.
 * @param {Array<{lat: number, lon: number}>} waypoints at least two points
 * @param {{signal?: AbortSignal}} [options]
 * @returns {Promise<{points: Array<{lat: number, lon: number}>, distanceM: number, durationS: number}>}
 */
export async function routeWaypoints(waypoints, options = {}) {
  if (!isRoutingEnabled()) {
    throw new Error("Kein Routing-Dienst konfiguriert.");
  }
  if (!Array.isArray(waypoints) || waypoints.length < 2) {
    throw new Error("Eine Etappe braucht mindestens zwei Punkte.");
  }

  const limit = Math.max(2, CONFIG.routing.maxWaypointsPerRequest);
  const chunks = [];
  for (let start = 0; start < waypoints.length - 1; start += limit - 1) {
    chunks.push(waypoints.slice(start, Math.min(start + limit, waypoints.length)));
  }

  const points = [];
  let distanceM = 0;
  let durationS = 0;

  for (const chunk of chunks) {
    const url = buildUrl(chunk);
    const result = await requestChunk(url, options.signal);
    distanceM += result.distanceM;
    durationS += result.durationS;
    for (const point of result.points) {
      const last = points[points.length - 1];
      if (!last || last.lat !== point.lat || last.lon !== point.lon) {
        points.push(point);
      }
    }
  }

  return { points, distanceM, durationS };
}

/**
 * Performs one HTTP request, with caching and a clear error message.
 * @param {string} url
 * @param {AbortSignal} [signal]
 */
async function requestChunk(url, signal) {
  const cached = cache.get(url);
  if (cached) {
    return cached;
  }

  const timeout = AbortSignal.timeout?.(CONFIG.routing.timeoutMs ?? 20000);
  const combined = signal && timeout ? AbortSignal.any([signal, timeout]) : (signal ?? timeout);

  let response;
  try {
    response = await fetch(url, {
      signal: combined,
      headers: { Accept: "application/geo+json, application/json" },
    });
  } catch (error) {
    if (error?.name === "AbortError") {
      throw new Error("Routing wurde abgebrochen.");
    }
    throw new Error(
      `Routing-Dienst nicht erreichbar (${CONFIG.routing.providerLabel}). ` +
        "Die Tour wird mit Luftlinien-Schätzung angezeigt.",
    );
  }

  if (!response.ok) {
    // The service describes the reason in the body (for example "no route found
    // between ..."); passing it through helps far more than a bare status code.
    const detail = (await response.text().catch(() => "")).trim().slice(0, 200);
    throw new Error(
      `Routing fehlgeschlagen (HTTP ${response.status})${detail ? `: ${detail}` : "."}`,
    );
  }

  const result = parseRoute(await response.json());

  if (cache.size >= MAX_CACHE_ENTRIES) {
    cache.clear();
  }
  cache.set(url, result);
  return result;
}
