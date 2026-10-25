// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Map view: stamp markers, the selected tour, the start point and the editable
// waypoints of the current track.
//
// Leaflet is vendored under web/vendor/leaflet so the page has no CDN
// dependency and works from a static host with a strict content policy.

import { CONFIG } from "./config.js";
import { escapeHtml, stampLabel } from "./format.js";

const SELECTED_COLOUR = "#12331f";
const SELECTED_RING = "#a9631b";
const UNSELECTED_COLOUR = "#8a968e";
const STAMPED_COLOUR = "#2f7a52";
const PLAN_COLOUR = "#a9631b";

/**
 * Creates the map and wires its interactions.
 * @param {HTMLElement} element
 * @param {{
 *   onStampToggle: (id: number) => void,
 *   onMapClick: (position: {lat: number, lon: number}) => void,
 *   onAddPoint?: (position: {lat: number, lon: number}) => void,
 *   onWaypointMove?: (index: number, position: {lat: number, lon: number}) => void,
 *   onWaypointRemove?: (index: number) => void,
 * }} handlers
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
  const handleLayer = L.layerGroup().addTo(map);

  /** @type {Map<number, L.CircleMarker>} */
  const markers = new Map();
  /** @type {L.Marker[]} */
  let handleMarkers = [];
  /** @type {{lat: number, lon: number} | null} */
  let start = null;
  /** @type {Set<number>} */
  let stamped = new Set();
  /** @type {Set<number>} */
  let selection = new Set();
  let addPointMode = false;

  map.on("click", (event) => {
    const position = { lat: event.latlng.lat, lon: event.latlng.lng };
    if (addPointMode) {
      handlers.onAddPoint?.(position);
      return;
    }
    handlers.onMapClick(position);
  });

  /** Re-applies selection and stamp-pass styling to every station marker. */
  function refreshMarkerStyles() {
    for (const [id, marker] of markers) {
      const selected = selection.has(id);
      const isStamped = stamped.has(id);
      marker.setStyle({
        fillColor: selected
          ? SELECTED_COLOUR
          : isStamped
            ? STAMPED_COLOUR
            : UNSELECTED_COLOUR,
        radius: selected ? 7 : isStamped ? 6 : 5,
        weight: selected ? 3 : 1,
        color: selected ? SELECTED_RING : "#ffffff",
      });
      marker.setRadius(selected ? 7 : isStamped ? 6 : 5);
    }
  }

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
    refreshMarkerStyles();
  }

  /**
   * Recolours markers to match the selection.
   * @param {Set<number>} selected
   */
  function setSelection(selected) {
    selection = selected;
    refreshMarkerStyles();
  }

  /**
   * Marks stations of the stamp pass. A stamped station stays visible when it
   * is not selected; the colour carries the progress at a glance.
   * @param {Set<number>} stampedIds
   */
  function setStamped(stampedIds) {
    stamped = stampedIds;
    refreshMarkerStyles();
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
   * Draws the tour as a line through the route nodes, with a marker for every
   * stop. `segments` may hold real routed geometry; when it does not, the line
   * is drawn straight between stops and the UI says so.
   * @param {Array<{points: Array<{lat: number, lon: number}>, day?: number, routed: boolean}>} segments
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
        weight: 4,
        opacity: 0.9,
        lineCap: "round",
        lineJoin: "round",
        dashArray: segment.routed ? undefined : "6 6",
      }).addTo(planLayer);
    }
  }

  /**
   * Places draggable handles for the editable waypoints of the tour.
   *
   * A handle opens a small popup with a delete button, so a node can be removed
   * directly on the map as well as from the list.
   * @param {Array<{lat: number, lon: number}>} positions
   * @param {{onMove?: (index: number, position: {lat: number, lon: number}) => void, onRemove?: (index: number) => void}} callbacks
   */
  function setTrackHandles(positions, callbacks = {}) {
    // Re-rendering the tour used to rebuild every handle. Doing that while a
    // handle was being dragged removed the marker under the pointer and
    // silently cancelled the drag, so an unchanged number of waypoints only
    // moves the existing markers.
    if (handleMarkers.length === positions.length) {
      positions.forEach((position, index) => {
        handleMarkers[index].setLatLng([position.lat, position.lon]);
      });
      return;
    }

    handleLayer.clearLayers();
    handleMarkers = [];
    positions.forEach((position, index) => {
      const icon = L.divIcon({
        className: "track-handle",
        html: `<span class="track-handle__label">${index + 1}</span>`,
        iconSize: [24, 24],
        iconAnchor: [12, 12],
        popupAnchor: [0, -14],
      });
      const marker = L.marker([position.lat, position.lon], {
        icon,
        draggable: true,
        keyboard: false,
        title: `Wegpunkt ${index + 1} verschieben`,
        alt: `Wegpunkt ${index + 1}`,
        riseOnHover: true,
      });
      marker.on("dragend", () => {
        const dropped = marker.getLatLng();
        callbacks.onMove?.(index, { lat: dropped.lat, lon: dropped.lng });
      });
      marker.bindPopup(() => {
        const wrapper = document.createElement("div");
        wrapper.className = "track-handle__popup";
        const label = document.createElement("p");
        label.textContent = `Wegpunkt ${index + 1}`;
        const button = document.createElement("button");
        button.type = "button";
        button.className = "button";
        button.textContent = "Punkt entfernen";
        button.addEventListener("click", () => {
          map.closePopup();
          callbacks.onRemove?.(index);
        });
        wrapper.append(label, button);
        return wrapper;
      });
      marker.addTo(handleLayer);
      handleMarkers.push(marker);
    });
  }

  /**
   * Switches the map between "set the start point" and "add a track point".
   * @param {boolean} active
   */
  function setAddPointMode(active) {
    addPointMode = active;
    element.classList.toggle("map--adding-point", active);
  }

  /** Removes the plan from the map. */
  function clearPlan() {
    planLayer.clearLayers();
  }

  /** Removes the draggable waypoint handles from the map. */
  function clearTrackHandles() {
    handleLayer.clearLayers();
    handleMarkers = [];
  }

  /** Fits the view to the drawn plan, if there is one. */
  function fitPlan() {
    const layers = planLayer.getLayers();
    if (layers.length === 0) {
      return;
    }
    const bounds = L.latLngBounds([]);
    for (const layer of layers) {
      if (typeof layer.getBounds === "function") {
        bounds.extend(layer.getBounds());
      }
    }
    if (bounds.isValid()) {
      map.fitBounds(bounds.pad(0.2));
    }
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
    setStamped,
    focusStamp,
    setStart,
    setPlan,
    setTrackHandles,
    setAddPointMode,
    clearPlan,
    clearTrackHandles,
    fitPlan,
    fitAllStamps,
    getStart,
    map,
    stampLabel,
  };
}
