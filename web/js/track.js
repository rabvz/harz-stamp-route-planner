// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Track geometry helpers.
//
// Everything here works on plain `{lat, lon, ele?}` arrays as returned by the
// routing service. There is no Leaflet dependency, so the functions are
// testable without a browser and can be reused by the smoke test.
//
// Snapping works on the route that was just computed. That route lies on
// OpenStreetMap hiking paths, so snapping to it keeps an inserted waypoint on a
// real trail. Searching *all* nearby paths would need an Overpass query on
// every drag; the documentation names that limitation instead of pretending
// otherwise.

const EARTH_RADIUS_M = 6_371_008.8;

/**
 * Great-circle distance between two positions, in metres.
 * @param {{lat: number, lon: number}} a
 * @param {{lat: number, lon: number}} b
 * @returns {number}
 */
export function distanceM(a, b) {
  const dLat = ((b.lat - a.lat) * Math.PI) / 180;
  const dLon = ((b.lon - a.lon) * Math.PI) / 180;
  const lat1 = (a.lat * Math.PI) / 180;
  const lat2 = (b.lat * Math.PI) / 180;
  const h =
    Math.sin(dLat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(h)));
}

/**
 * Distance from the start for every point of a polyline.
 * @param {Array<{lat: number, lon: number}>} points
 * @returns {Float64Array}
 */
export function cumulativeDistances(points) {
  const result = new Float64Array(points.length);
  for (let index = 1; index < points.length; index += 1) {
    result[index] = result[index - 1] + distanceM(points[index - 1], points[index]);
  }
  return result;
}

/**
 * Local flat projection around a reference latitude.
 *
 * The Harz is small enough that treating a few kilometres as a plane costs
 * centimetres, and it makes segment intersection easy to express.
 */
function projector(referenceLat) {
  const metresPerDegreeLat = (Math.PI / 180) * EARTH_RADIUS_M;
  const metresPerDegreeLon = metresPerDegreeLat * Math.cos((referenceLat * Math.PI) / 180);
  return {
    x: (lon) => lon * metresPerDegreeLon,
    y: (lat) => lat * metresPerDegreeLat,
  };
}

/**
 * Nearest point on a polyline.
 *
 * Returns the position on the line, the distance to it, and how far along the
 * line that point lies. `index` is the segment index (the point before the
 * projected position).
 * @param {Array<{lat: number, lon: number, ele?: number}>} points
 * @param {{lat: number, lon: number}} position
 * @returns {{lat: number, lon: number, ele?: number, distanceM: number, alongM: number, index: number} | null}
 */
export function nearestPointOnTrack(points, position) {
  if (!Array.isArray(points) || points.length === 0) {
    return null;
  }
  const first = points[0];
  if (points.length === 1) {
    return {
      lat: first.lat,
      lon: first.lon,
      ...(Number.isFinite(first.ele) ? { ele: first.ele } : {}),
      distanceM: distanceM(position, first),
      alongM: 0,
      index: 0,
    };
  }

  const project = projector(position.lat);
  const px = project.x(position.lon);
  const py = project.y(position.lat);

  let bestDistance2 = Infinity;
  let bestIndex = 0;
  let bestT = 0;

  const cumulative = cumulativeDistances(points);
  for (let index = 0; index < points.length - 1; index += 1) {
    const a = points[index];
    const b = points[index + 1];
    const ax = project.x(a.lon);
    const ay = project.y(a.lat);
    const bx = project.x(b.lon);
    const by = project.y(b.lat);
    const dx = bx - ax;
    const dy = by - ay;
    const length2 = dx * dx + dy * dy;

    let t = 0;
    if (length2 > 0) {
      t = ((px - ax) * dx + (py - ay) * dy) / length2;
      t = Math.max(0, Math.min(1, t));
    }
    const qx = ax + dx * t;
    const qy = ay + dy * t;
    const distance2 = (px - qx) ** 2 + (py - qy) ** 2;
    if (distance2 < bestDistance2) {
      bestDistance2 = distance2;
      bestIndex = index;
      bestT = t;
    }
  }

  const a = points[bestIndex];
  const b = points[bestIndex + 1];
  const segmentLength = cumulative[bestIndex + 1] - cumulative[bestIndex];
  const result = {
    lat: a.lat + (b.lat - a.lat) * bestT,
    lon: a.lon + (b.lon - a.lon) * bestT,
    distanceM: Math.sqrt(bestDistance2),
    alongM: cumulative[bestIndex] + segmentLength * bestT,
    index: bestIndex,
  };
  if (Number.isFinite(a.ele) && Number.isFinite(b.ele)) {
    result.ele = a.ele + (b.ele - a.ele) * bestT;
  }
  return result;
}

/**
 * Snaps a position onto a polyline when it is close enough.
 *
 * Beyond the tolerance the original position is returned: dragging a waypoint
 * onto another trail must stay possible, and the router then finds the nearest
 * path to that point itself.
 * @param {Array<{lat: number, lon: number}>} points
 * @param {{lat: number, lon: number}} position
 * @param {number} toleranceM
 * @returns {{lat: number, lon: number, snapped: boolean}}
 */
export function snapToTrack(points, position, toleranceM) {
  const nearest = nearestPointOnTrack(points, position);
  if (nearest && nearest.distanceM <= toleranceM) {
    return { lat: nearest.lat, lon: nearest.lon, snapped: true };
  }
  return { lat: position.lat, lon: position.lon, snapped: false };
}

/**
 * Smooths a series with a small moving average.
 * @param {number[]} values
 * @param {number} window odd window size
 * @returns {number[]}
 */
function smooth(values, window) {
  const half = Math.floor(window / 2);
  return values.map((_, index) => {
    let sum = 0;
    let count = 0;
    for (let offset = -half; offset <= half; offset += 1) {
      const value = values[index + offset];
      if (Number.isFinite(value)) {
        sum += value;
        count += 1;
      }
    }
    return count > 0 ? sum / count : values[index];
  });
}

/**
 * Builds an elevation profile from routed geometry.
 *
 * BRouter returns elevation as the third coordinate. Points without elevation
 * are tolerated, but a route with elevation on fewer than 80 % of its points is
 * treated as having none — a profile drawn from holes would invent elevation.
 * @param {Array<{lat: number, lon: number, ele?: number}>} points
 * @param {{maxSamples?: number}} [options]
 * @returns {{samples: Array<{distanceM: number, ele: number}>, totalDistanceM: number, ascentM: number, descentM: number, minEleM: number, maxEleM: number} | null}
 */
export function buildElevationProfile(points, options = {}) {
  if (!Array.isArray(points) || points.length < 2) {
    return null;
  }
  const withElevation = points.filter((point) => Number.isFinite(point.ele));
  if (withElevation.length < Math.ceil(points.length * 0.8) || withElevation.length < 2) {
    return null;
  }

  const cumulative = cumulativeDistances(points);
  const elevations = smooth(
    points.map((point) => (Number.isFinite(point.ele) ? point.ele : null)),
    5,
  );

  // Ascent and descent from the smoothed series. A single threshold avoids
  // counting every centimetre of terrain model noise as a climb.
  let ascentM = 0;
  let descentM = 0;
  for (let index = 1; index < elevations.length; index += 1) {
    const delta = elevations[index] - elevations[index - 1];
    if (delta > 1) {
      ascentM += delta;
    } else if (delta < -1) {
      descentM -= delta;
    }
  }

  const maxSamples = Math.max(2, options.maxSamples ?? 240);
  const step = Math.max(1, Math.ceil(points.length / maxSamples));
  const samples = [];
  for (let index = 0; index < points.length; index += step) {
    if (Number.isFinite(elevations[index])) {
      samples.push({ distanceM: cumulative[index], ele: elevations[index] });
    }
  }
  const last = points.length - 1;
  if (
    Number.isFinite(elevations[last]) &&
    (samples.length === 0 || samples[samples.length - 1].distanceM < cumulative[last])
  ) {
    samples.push({ distanceM: cumulative[last], ele: elevations[last] });
  }
  if (samples.length < 2) {
    return null;
  }

  const elevationValues = samples.map((sample) => sample.ele);
  return {
    samples,
    totalDistanceM: cumulative[last],
    ascentM,
    descentM,
    minEleM: Math.min(...elevationValues),
    maxEleM: Math.max(...elevationValues),
  };
}
