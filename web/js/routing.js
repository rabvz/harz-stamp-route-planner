// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Optional routing integration.
//
// The planner itself runs offline on straight-line distances. When the operator
// of a deployment configures `routingUrlTemplate`, the UI can additionally
// request real trail geometry for the computed itinerary, which replaces the
// estimated distances and produces a usable GPX track.
//
// The contract is intentionally narrow: the endpoint takes two coordinates and
// returns a GeoJSON LineString (optionally with `properties.summary.distance`
// and `.duration`). That is the shape returned by BRouter, OpenRouteService and
// Valhalla in GeoJSON mode, so no vendor-specific code is needed.

import { CONFIG } from "./config.js";

/**
 * Whether a routing endpoint is configured.
 * @returns {boolean}
 */
export function isRoutingEnabled() {
  return typeof CONFIG.routingUrlTemplate === "string" && CONFIG.routingUrlTemplate.length > 0;
}

/**
 * Requests one leg.
 * @param {{lat: number, lon: number}} from
 * @param {{lat: number, lon: number}} to
 * @param {AbortSignal} [signal]
 * @returns {Promise<{points: Array<{lat: number, lon: number}>, distanceM: number, durationS: number}>}
 */
async function requestLeg(from, to, signal) {
  const coordinates = `${from.lon.toFixed(6)},${from.lat.toFixed(6)}|${to.lon.toFixed(6)},${to.lat.toFixed(6)}`;
  const url = CONFIG.routingUrlTemplate.replace("{lonlats}", encodeURIComponent(coordinates));

  const response = await fetch(url, { signal, headers: { Accept: "application/geo+json, application/json" } });
  if (!response.ok) {
    throw new Error(`Routing-Dienst antwortete mit HTTP ${response.status}.`);
  }
  return parseRoute(await response.json());
}

/**
 * Parses the supported GeoJSON subset.
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
  }

  if (geometry?.type !== "LineString" || !Array.isArray(geometry.coordinates)) {
    throw new Error("Die Routing-Antwort ist keine LineString-Geometrie.");
  }

  const points = [];
  for (const pair of geometry.coordinates) {
    if (!Array.isArray(pair) || pair.length < 2) {
      continue;
    }
    const [lon, lat] = pair;
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) {
      continue;
    }
    points.push({ lat, lon });
  }
  if (points.length < 2) {
    throw new Error("Die Routing-Antwort enthält weniger als zwei Punkte.");
  }

  const summary = properties?.summary ?? properties;
  const distanceM = Number.isFinite(summary?.distance)
    ? Number(summary.distance)
    : polylineLength(points);
  const durationS = Number.isFinite(summary?.duration)
    ? Number(summary.duration)
    : Number.isFinite(summary?.time)
      ? Number(summary.time)
      : (distanceM / 1.25) | 0;

  return { points, distanceM, durationS };
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
 * Routes a sequence of legs, sequentially to stay polite to the service.
 * @param {Array<{from: {lat: number, lon: number}, to: {lat: number, lon: number}}>} legs
 * @param {(done: number, total: number) => void} [onProgress]
 * @returns {Promise<Array<{points: Array<{lat: number, lon: number}>, distanceM: number, durationS: number}>>}
 */
export async function routeLegs(legs, onProgress) {
  const results = [];
  for (let index = 0; index < legs.length; index += 1) {
    const leg = legs[index];
    results.push(await requestLeg(leg.from, leg.to));
    onProgress?.(index + 1, legs.length);
  }
  return results;
}
