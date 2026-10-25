// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Elevation profile rendering.
//
// A hand-written SVG instead of a chart library: the whole page stays
// dependency-free, works offline and keeps the bundle small. The profile is
// drawn from the routed geometry (BRouter returns elevation per point), never
// from an invented curve.

import { formatKm } from "./format.js";

const WIDTH = 640;
const HEIGHT = 200;
const PADDING = { top: 16, right: 16, bottom: 28, left: 52 };

/**
 * Renders a profile into a container element.
 * @param {HTMLElement} container
 * @param {{samples: Array<{distanceM: number, ele: number}>, totalDistanceM: number, ascentM: number, descentM: number, minEleM: number, maxEleM: number}} profile
 * @returns {boolean} whether a profile was drawn
 */
export function renderElevationProfile(container, profile) {
  container.replaceChildren();
  if (!profile || !Array.isArray(profile.samples) || profile.samples.length < 2) {
    return false;
  }

  const plotWidth = WIDTH - PADDING.left - PADDING.right;
  const plotHeight = HEIGHT - PADDING.top - PADDING.bottom;
  const range = Math.max(1, profile.maxEleM - profile.minEleM);
  const x = (distanceM) =>
    PADDING.left + (distanceM / Math.max(1, profile.totalDistanceM)) * plotWidth;
  const y = (elevation) =>
    PADDING.top + ((profile.maxEleM - elevation) / range) * plotHeight;

  const points = profile.samples.map((sample) => [
    x(sample.distanceM),
    y(sample.ele),
  ]);
  const line = points.map(([px, py], index) => `${index === 0 ? "M" : "L"}${px.toFixed(1)},${py.toFixed(1)}`).join(" ");
  const baseline = PADDING.top + plotHeight;
  const area = `${line} L${x(profile.totalDistanceM).toFixed(1)},${baseline} L${PADDING.left},${baseline} Z`;

  const gridLines = [0.25, 0.5, 0.75]
    .map((fraction) => {
      const py = PADDING.top + plotHeight * fraction;
      return `<line class="elevation-chart__grid" x1="${PADDING.left}" y1="${py}" x2="${
        WIDTH - PADDING.right
      }" y2="${py}" />`;
    })
    .join("");

  const middle = profile.totalDistanceM / 2;
  const xLabels = [
    { at: 0, text: "0 km" },
    { at: middle, text: formatKm(middle) },
    { at: profile.totalDistanceM, text: formatKm(profile.totalDistanceM) },
  ]
    .map(
      (label) =>
        `<text class="elevation-chart__label" x="${x(label.at).toFixed(1)}" y="${
          HEIGHT - 8
        }" text-anchor="${label.at === 0 ? "start" : label.at === profile.totalDistanceM ? "end" : "middle"}">${label.text}</text>`,
    )
    .join("");

  // The y axis shows the highest and lowest point; exact intermediate ticks
  // would add noise without helping the question "how much climbing is this?".
  const yLabels = [
    `<text class="elevation-chart__label" x="${PADDING.left - 8}" y="${PADDING.top + 4}" text-anchor="end">${Math.round(profile.maxEleM)} m</text>`,
    `<text class="elevation-chart__label" x="${PADDING.left - 8}" y="${baseline + 4}" text-anchor="end">${Math.round(profile.minEleM)} m</text>`,
  ].join("");

  container.innerHTML =
    `<svg class="elevation-chart__svg" viewBox="0 0 ${WIDTH} ${HEIGHT}" role="img" aria-hidden="true" focusable="false">` +
    `<path class="elevation-chart__area" d="${area}" />` +
    `<path class="elevation-chart__line" d="${line}" />` +
    gridLines +
    `<line class="elevation-chart__axis" x1="${PADDING.left}" y1="${baseline}" x2="${
      WIDTH - PADDING.right
    }" y2="${baseline}" />` +
    xLabels +
    yLabels +
    `</svg>`;
  return true;
}
