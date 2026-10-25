// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Runtime configuration. Everything here is visible in the shipped bundle,
// which is why it contains no keys: a static site cannot keep a secret.

export const CONFIG = {
  /** Raster tiles. Replace with your own tile server if you have one. */
  tileUrl: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
  tileAttribution:
    '&copy; <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap</a>-Mitwirkende',
  tileMaxZoom: 19,
  mapCenter: [51.74, 10.75],
  mapZoom: 9,
  mapMaxBounds: [
    [51.2, 9.4],
    [52.2, 12.1],
  ],

  /**
   * Routing along real hiking trails.
   *
   * BRouter's public instance is the default because it needs no API key — the
   * decisive property for a static site. It accepts a list of via points and
   * returns one GeoJSON LineString per request, so a day tour costs one
   * request.
   *
   * `{lonlats}` is replaced with `lon,lat|lon,lat...`, `{profile}` with the
   * profile name. Point this at your own instance if you run one:
   *   https://brouter.example.org/brouter?lonlats={lonlats}&profile={profile}&format=geojson
   *
   * Leave `urlTemplate` empty to work purely on straight-line estimates.
   */
  routing: {
    providerLabel: "BRouter",
    urlTemplate:
      "https://brouter.de/brouter?lonlats={lonlats}&profile={profile}&alternativeidx=0&format=geojson",
    profile: "hiking-beta",
    timeoutMs: 25000,
    /** Via-point limit per request; longer tours are split into chunks. */
    maxWaypointsPerRequest: 16,
  },

  /**
   * Editing the track.
   *
   * Snapping uses the route that was just computed, which lies on mapped hiking
   * trails. It does not search every nearby path — that would need an Overpass
   * query on each drag (see docs/routing-design.md).
   */
  track: {
    /** A dragged waypoint inside this radius snaps onto the computed route. */
    snapRadiusM: 120,
    /** After routing, a waypoint this close settles onto the new geometry. */
    settleRadiusM: 60,
  },

  /**
   * Parking candidates from OpenStreetMap.
   *
   * Queried only when the user clicks the button, never on page load. The
   * public instance is a shared resource; responses are cached for ten minutes.
   */
  overpass: {
    endpoint: "https://overpass-api.de/api/interpreter",
    /** Server-side query budget, in seconds. */
    timeoutSeconds: 30,
    /** Client-side budget; generous because the public instance queues queries. */
    clientTimeoutMs: 60000,
    /** Three attempts ride out the occasional 502 or dropped request. */
    attempts: 3,
    retryDelayMs: 2000,
  },

  defaults: {
    maxTourKm: 15,
    clusterMaxStamps: 8,
    clusterMaxDiameterM: 8000,
    maxWalkM: 1500,
    maxParkingCandidates: 12,
  },
};
