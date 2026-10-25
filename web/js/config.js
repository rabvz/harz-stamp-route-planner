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
   * Optional routing service returning a GeoJSON LineString.
   *
   * `{lonlats}` is replaced with `lon,lat|lon,lat`. Leave empty to use
   * straight-line estimates, which are clearly labelled in the UI.
   *
   * Example for a self-hosted BRouter:
   *   https://brouter.example.org/brouter?lonlats={lonlats}&profile=hiking-mountain&format=geojson
   *
   * See docs/routing-design.md for the exact contract and the reasoning.
   */
  routingUrlTemplate: "",

  /** Overpass endpoint used only when the user explicitly asks for car parks. */
  overpassEndpoint: "https://overpass-api.de/api/interpreter",

  defaults: {
    maxDayKm: 15,
    maxDays: 7,
    clusterMaxStamps: 8,
    clusterMaxDiameterM: 8000,
    maxWalkM: 1500,
  },
};
