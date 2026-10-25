// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Map view: stamp markers, the selected plan and the start point.
//
// Leaflet is vendored under web/vendor/leaflet so the page has no CDN
// dependency and works from a static host with a strict content policy.

import { CONFIG } from "./config.js";
import { escapeHtml, stampLabel } from "./format.js";

const SELECTED_COLOUR = "#1f4d3a";
const UNSELECTED_COLOUR = "#8a968e";
const PLAN_COLOUR = "#a9631b";

/**
 * Creates the map and wires its interactions.
 * @param {HTMLElement} element
 * @param {{onStampToggle: (id: number) => void, onMapClick: (latlng: {lat: number, lon: number}) => void}} handlers
 */
export function createMap(element, handlers) {
  const map = L.map(element, {
    center: CONFIG.mapCenter,
    zoom: CONFIG.mapZoom,
    maxBounds: CONFIG.mapMaxBounds,
    maxBoundsViscosity: 0.8,
    minZoom: 7,
    maxZoom: CONFIG.tileMaxZoom,
  });

  L.tileLayer(CONFIG.tileUrl, {
    maxZoom: CONFIG.tileMaxZoom,
    attribution: CONFIG.tileAttribution,
    detectRetina: false,
  }).addTo(map);

  const stampLayer = L.layerGroup().addTo(map);
  const planLayer = L.layerGroup().addTo(map);
  const startLayer = L.layerGroup().addTo(map);

  /** @type {Map<number, L.CircleMarker>} */
  const markers = new Map();
  /** @type {{lat: number, lon: number} | null} */
  let start = null;

  map.on("click", (event) => {
    handlers.onMapClick({ lat: event.latlng.lat, lon: event.latlng.lng });
  });

  /**
   * Adds one marker per station. Called once after the catalogue loads.
   * @param {Array<{id: number, code: string, name: string, location: string, lat: number, lon: number}>} stamps
   */
  function setStamps(stamps) {
    stampLayer.clearLayers();
    markers.clear();

    for (const stamp of stamps) {
      const marker = L.circleMarker([stamp.lat, stamp.lon], {
        radius: 5,
        weight: 1,
        color: "#ffffff",
        fillColor: UNSELECTED_COLOUR,
        fillOpacity: 0.9,
        className: "stamp-marker",
      });
      marker.bindTooltip(
        `<strong>${escapeHtml(stamp.code)}</strong> ${escapeHtml(stamp.name)}<br />${escapeHtml(
          stamp.location,
        )}`,
        { direction: "top", offset: [0, -4] },
      );
      marker.on("click", (event) => {
        // Keep the map's own click handler from also moving the start point.
        L.DomEvent.stopPropagation(event);
        handlers.onStampToggle(stamp.id);
      });
      marker.addTo(stampLayer);
      markers.set(stamp.id, marker);
    }
  }

  /**
   * Recolours markers to match the selection.
   * @param {Set<number>} selection
   */
  function setSelection(selection) {
    for (const [id, marker] of markers) {
      const selected = selection.has(id);
      marker.setStyle({
        fillColor: selected ? SELECTED_COLOUR : UNSELECTED_COLOUR,
        radius: selected ? 7 : 5,
        weight: selected ? 2 : 1,
        color: selected ? "#ffffff" : "#ffffff",
      });
    }
  }

  /**
   * Shows a station on the map without changing the selection.
   * @param {number} id
   */
  function focusStamp(id) {
    const marker = markers.get(id);
    if (!marker) {
      return;
    }
    map.panTo(marker.getLatLng());
    marker.openTooltip();
  }

  /**
   * Sets or clears the start marker.
   * @param {{lat: number, lon: number} | null} position
   */
  function setStart(position) {
    start = position;
    startLayer.clearLayers();
    if (!position) {
      return;
    }
    // A div icon instead of Leaflet's default marker: the default icon is an
    // image file that would have to be vendored next to the CSS, and a missing
    // file degrades into broken-image alt text on the map. An inline SVG has no
    // such failure mode.
    const icon = L.divIcon({
      className: "start-marker",
      html:
        '<svg viewBox="0 0 24 32" aria-hidden="true" focusable="false">' +
        '<path d="M12 1C5.9 1 1 5.9 1 12c0 8.2 9.6 18.1 10 18.5a1.4 1.4 0 0 0 2 0C13.4 30.1 23 20.2 23 12 23 5.9 18.1 1 12 1z" ' +
        'fill="#a9631b" stroke="#ffffff" stroke-width="2"/>' +
        '<circle cx="12" cy="12" r="4" fill="#ffffff"/></svg>',
      iconSize: [24, 32],
      iconAnchor: [12, 31],
      tooltipAnchor: [0, -28],
    });
    L.marker([position.lat, position.lon], {
      icon,
      title: "Startpunkt der Tour",
      alt: "Startpunkt der Tour",
      keyboard: false,
    })
      .bindTooltip("Startpunkt", { permanent: false })
      .addTo(startLayer);
  }

  /**
   * Draws the plan as a line through the itinerary nodes, with a marker for
   * every stop. `segments` may hold real routed geometry; when it does not,
   * the line is drawn straight between stops and the UI says so.
   * @param {Array<{points: Array<{lat: number, lon: number}>, day: number}>} segments
   */
  function setPlan(segments) {
    planLayer.clearLayers();
    for (const segment of segments) {
      if (!segment.points || segment.points.length < 2) {
        continue;
      }
      const latlngs = segment.points.map((point) => [point.lat, point.lon]);
      L.polyline(latlngs, {
        color: PLAN_COLOUR,
        weight: 3,
        opacity: 0.85,
        dashArray: segment.routed ? undefined : "6 6",
      }).addTo(planLayer);
    }
  }

  /** Removes the plan from the map. */
  function clearPlan() {
    planLayer.clearLayers();
  }

  /** Fits the view to all stations. */
  function fitAllStamps() {
    const points = [...markers.values()].map((marker) => marker.getLatLng());
    if (points.length > 0) {
      map.fitBounds(L.latLngBounds(points).pad(0.08));
    }
  }

  /** The current start position, if any. */
  function getStart() {
    return start;
  }

  // Leaflet needs a size recalculation when its container becomes visible or
  // changes size (for example when the mobile layout switches).
  const observer = new ResizeObserver(() => map.invalidateSize());
  observer.observe(element);

  return {
    setStamps,
    setSelection,
    focusStamp,
    setStart,
    setPlan,
    clearPlan,
    fitAllStamps,
    getStart,
    map,
    stampLabel,
  };
}
