// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Application wiring: catalogue -> selection -> stamp pass -> parking -> day
// tour -> editable track -> GPX.
//
// Everything is held in memory and mirrored to IndexedDB. The Rust core does the
// geometry, optimisation and parking ranking; this module moves data across the
// boundary, talks to the two public services (routing and Overpass) and renders
// the result. The track nodes live here because editing them is an interaction
// concern: the router accepts them as via points and the core never sees them.

import { CONFIG } from "./config.js";
import {
  downloadText,
  escapeHtml,
  formatDistance,
  formatDuration,
  formatKm,
  timestampForFilename,
} from "./format.js";
import { createMap } from "./mapview.js";
import {
  ATTRIBUTION as PARKING_ATTRIBUTION,
  expandBounds,
  fetchParkingCandidates,
} from "./overpass.js";
import { renderElevationProfile } from "./profile.js";
import { isRoutingEnabled, routingProviderName, routeWaypoints } from "./routing.js";
import { STATE_VERSION, loadState, parseImport, saveState, serializeExport } from "./store.js";
import { buildElevationProfile, nearestPointOnTrack, snapToTrack } from "./track.js";
import { call, load as loadCore, PlannerError } from "./wasm.js";

/** @type {ReturnType<typeof createMap> | null} */
let mapView = null;

const state = {
  dataset: null,
  /** @type {Array<object>} */
  stamps: [],
  /** @type {Map<number, object>} */
  stampsById: new Map(),
  /** @type {Set<number>} */
  selection: new Set(),
  /** Stations already stamped in the paper pass. @type {Set<number>} */
  stamped: new Set(),
  onlyUnstamped: false,
  /** @type {{lat: number, lon: number} | null} */
  start: null,
  settings: {
    maxTourKm: CONFIG.defaults.maxTourKm,
    maxWalkM: CONFIG.defaults.maxWalkM,
  },
  /** The tour computed by the Rust core, or null. */
  plan: null,
  /**
   * The stops of the current tour in walking order: stamps visited and the
   * waypoints the user added. @type {Array<{kind: "stamp", id: number} | {kind: "via", lat: number, lon: number}>}
   */
  stops: [],
  /** Routed geometry, duration and elevation profile, or null. */
  route: null,
  /** @type {Array<object>} */
  parkingSuggestions: [],
  clusters: null,
  filter: "",
  addingPoint: false,
  /** Aborts an in-flight routing run when the tour is recomputed or edited. */
  routeController: null,
};

const elements = {
  catalogBadge: document.querySelector("#catalog-badge"),
  engineBadge: document.querySelector("#engine-badge"),
  search: document.querySelector("#search"),
  stampList: document.querySelector("#stamp-list"),
  selectionCount: document.querySelector("#selection-count"),
  catalogCount: document.querySelector("#catalog-count"),
  selectAll: document.querySelector("#select-all"),
  selectNone: document.querySelector("#select-none"),
  selectInvert: document.querySelector("#select-invert"),
  stampedCount: document.querySelector("#stamped-count"),
  stampedTotal: document.querySelector("#stamped-total"),
  stampedBar: document.querySelector("#stamped-bar"),
  stampProgress: document.querySelector("#stamp-progress"),
  onlyUnstamped: document.querySelector("#only-unstamped"),
  stampSelection: document.querySelector("#stamp-selection"),
  unstampSelection: document.querySelector("#unstamp-selection"),
  findParking: document.querySelector("#find-parking"),
  parkingList: document.querySelector("#parking-list"),
  parkingStatus: document.querySelector("#parking-status"),
  parkingAttribution: document.querySelector("#parking-attribution"),
  useFirstStamp: document.querySelector("#use-first-stamp"),
  clearStart: document.querySelector("#clear-start"),
  startLabel: document.querySelector("#start-label"),
  maxTourKm: document.querySelector("#max-tour-km"),
  plan: document.querySelector("#plan"),
  cluster: document.querySelector("#cluster"),
  planStatus: document.querySelector("#plan-status"),
  resultPanel: document.querySelector("#result-panel"),
  planSummary: document.querySelector("#plan-summary"),
  planWarnings: document.querySelector("#plan-warnings"),
  tourStops: document.querySelector("#tour-stops"),
  elevationPanel: document.querySelector("#elevation-panel"),
  elevationChart: document.querySelector("#elevation-chart"),
  elevationSummary: document.querySelector("#elevation-summary"),
  trackPanel: document.querySelector("#track-panel"),
  addPoint: document.querySelector("#add-point"),
  resetTrack: document.querySelector("#reset-track"),
  trackStatus: document.querySelector("#track-status"),
  clusterPanel: document.querySelector("#cluster-panel"),
  clusterList: document.querySelector("#cluster-list"),
  exportSelection: document.querySelector("#export-selection"),
  exportPlan: document.querySelector("#export-plan"),
  importGpx: document.querySelector("#import-gpx"),
  gpxStatus: document.querySelector("#gpx-status"),
  exportData: document.querySelector("#export-data"),
  importData: document.querySelector("#import-data"),
  importDataFile: document.querySelector("#import-data-file"),
  dataStatus: document.querySelector("#data-status"),
};

/** Sets a status line, with an optional tone. */
function setStatus(element, message, tone = "") {
  element.textContent = message;
  element.classList.toggle("status--error", tone === "error");
  element.classList.toggle("status--warning", tone === "warning");
}

/** Persists the parts of the state worth keeping. */
let saveTimer = null;
function persist() {
  if (saveTimer) {
    clearTimeout(saveTimer);
  }
  saveTimer = setTimeout(async () => {
    saveTimer = null;
    const ok = await saveState({
      version: STATE_VERSION,
      selection: [...state.selection].sort((a, b) => a - b),
      stamped: [...state.stamped].sort((a, b) => a - b),
      start: state.start,
      settings: state.settings,
    });
    if (!ok) {
      setStatus(elements.dataStatus, "Hinweis: Speichern im Browser nicht möglich.", "warning");
    }
  }, 250);
}

/** Restores a previously stored state. */
function restore(saved) {
  if (!saved || typeof saved !== "object") {
    return;
  }
  if (Array.isArray(saved.selection)) {
    for (const id of saved.selection) {
      if (state.stampsById.has(id)) {
        state.selection.add(id);
      }
    }
  }
  if (Array.isArray(saved.stamped)) {
    for (const id of saved.stamped) {
      if (state.stampsById.has(id)) {
        state.stamped.add(id);
      }
    }
  }
  if (saved.start && Number.isFinite(saved.start.lat) && Number.isFinite(saved.start.lon)) {
    state.start = { lat: saved.start.lat, lon: saved.start.lon };
  }
  const settings = saved.settings ?? {};
  const maxTourKm = Number.isFinite(settings.maxTourKm)
    ? settings.maxTourKm
    : settings.maxDayKm; // state written before version 2
  if (Number.isFinite(maxTourKm) && maxTourKm > 0) {
    state.settings.maxTourKm = maxTourKm;
  }
  if (Number.isFinite(settings.maxWalkM)) {
    state.settings.maxWalkM = settings.maxWalkM;
  }
}

/** The stamps matching the current search filter. */
function filteredStamps() {
  const needle = state.filter.trim().toLowerCase();
  return state.stamps.filter((stamp) => {
    if (state.onlyUnstamped && state.stamped.has(stamp.id)) {
      return false;
    }
    if (!needle) {
      return true;
    }
    return (
      stamp.code.toLowerCase().includes(needle) ||
      stamp.name.toLowerCase().includes(needle) ||
      stamp.location.toLowerCase().includes(needle)
    );
  });
}

/** Renders the station list. */
function renderStampList() {
  const stamps = filteredStamps();
  const fragment = document.createDocumentFragment();

  for (const stamp of stamps) {
    const item = document.createElement("li");
    item.className = "stamp-row";
    if (state.stamped.has(stamp.id)) {
      item.classList.add("stamp-row--stamped");
    }

    const button = document.createElement("button");
    button.type = "button";
    button.className = "stamp-item";
    button.dataset.stampId = String(stamp.id);
    button.setAttribute("aria-pressed", state.selection.has(stamp.id) ? "true" : "false");
    button.innerHTML =
      `<span class="stamp-item__code">${escapeHtml(stamp.code)}</span>` +
      `<span class="stamp-item__name">${escapeHtml(stamp.name)}</span>` +
      `<span class="stamp-item__meta">${
        stamp.elevationM === null || stamp.elevationM === undefined
          ? ""
          : `${Math.round(stamp.elevationM)} m`
      }</span>`;
    button.title = stamp.location;

    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "stamp-toggle";
    toggle.dataset.stampedId = String(stamp.id);
    const isStamped = state.stamped.has(stamp.id);
    toggle.setAttribute("aria-pressed", isStamped ? "true" : "false");
    toggle.setAttribute(
      "aria-label",
      `${stamp.code} ${stamp.name} als ${isStamped ? "nicht gestempelt" : "gestempelt"} markieren`,
    );
    toggle.title = isStamped ? "Bereits gestempelt — Markierung entfernen" : "Als gestempelt markieren";
    toggle.textContent = isStamped ? "✓" : "○";

    item.append(button, toggle);
    fragment.append(item);
  }

  elements.stampList.replaceChildren(fragment);
  if (stamps.length === 0) {
    const empty = document.createElement("li");
    empty.className = "hint";
    empty.textContent = state.onlyUnstamped
      ? "Alle passenden Stempelstellen sind bereits gestempelt."
      : "Keine Stempelstelle passt zur Suche.";
    elements.stampList.append(empty);
  }
}

/** Number of stamped stations that still exist in the catalogue. */
function stampedCount() {
  let count = 0;
  for (const id of state.stamped) {
    if (state.stampsById.has(id)) {
      count += 1;
    }
  }
  return count;
}

/** Updates the stamp pass counter and progress bar. */
function updateStampPassUi() {
  const total = state.stamps.length;
  const count = stampedCount();
  elements.stampedCount.textContent = String(count);
  elements.stampedTotal.textContent = String(total);
  elements.stampedBar.style.width = total > 0 ? `${(count / total) * 100}%` : "0%";
  elements.stampProgress.setAttribute("aria-valuenow", String(count));
  elements.stampProgress.setAttribute("aria-valuemax", String(total));
  const hasSelection = state.selection.size > 0;
  elements.stampSelection.disabled = !hasSelection;
  elements.unstampSelection.disabled = !hasSelection;
}

/**
 * Drops a tour that no longer matches the selection or the start point.
 *
 * Keeping a stale route visible after the user changed its inputs would be the
 * kind of quiet lie this project avoids; the message says what to do instead.
 * @param {string} message
 */
function invalidateTour(message) {
  if (!state.plan) {
    return;
  }
  state.routeController?.abort();
  state.routeController = null;
  state.plan = null;
  state.route = null;
  state.stops = [];
  stopAddingPoints();
  mapView?.clearPlan();
  mapView?.clearTrackHandles();
  elements.resultPanel.hidden = true;
  elements.elevationPanel.hidden = true;
  elements.trackPanel.hidden = true;
  elements.planWarnings.replaceChildren();
  elements.tourStops.replaceChildren();
  elements.elevationChart.replaceChildren();
  setStatus(elements.trackStatus, "");
  elements.exportPlan.disabled = true;
  if (message) {
    setStatus(elements.planStatus, message);
  }
}

/** Updates counters and button availability. */
function updateSelectionUi() {
  const count = state.selection.size;
  elements.selectionCount.textContent = String(count);
  elements.catalogCount.textContent = String(state.stamps.length);
  elements.plan.disabled = count === 0;
  elements.cluster.disabled = count === 0;
  elements.findParking.disabled = count === 0;
  elements.exportSelection.disabled = count === 0;
  elements.exportPlan.disabled = state.plan === null;
  mapView?.setSelection(state.selection);
  mapView?.setStamped(state.stamped);
  updateStampPassUi();
  updateTrackUi();
}

/** Toggles one station in the tour selection and re-renders the affected row. */
function toggleStamp(id) {
  if (state.selection.has(id)) {
    state.selection.delete(id);
  } else {
    state.selection.add(id);
  }
  const pressed = state.selection.has(id);
  const button = elements.stampList.querySelector(`[data-stamp-id="${id}"]`);
  if (button) {
    button.setAttribute("aria-pressed", pressed ? "true" : "false");
  }
  invalidateTour("Auswahl geändert — bitte die Tagestour neu berechnen.");
  updateSelectionUi();
  persist();
}

/** Marks one station as already stamped in the paper pass. */
function toggleStamped(id) {
  const stamped = !state.stamped.has(id);
  if (stamped) {
    state.stamped.add(id);
  } else {
    state.stamped.delete(id);
  }
  const button = elements.stampList.querySelector(`[data-stamped-id="${id}"]`);
  if (button) {
    button.setAttribute("aria-pressed", stamped ? "true" : "false");
    button.textContent = stamped ? "✓" : "○";
    const stamp = state.stampsById.get(id);
    if (stamp) {
      button.setAttribute(
        "aria-label",
        `${stamp.code} ${stamp.name} als ${stamped ? "nicht gestempelt" : "gestempelt"} markieren`,
      );
      button.title = stamped
        ? "Bereits gestempelt — Markierung entfernen"
        : "Als gestempelt markieren";
    }
    button.closest(".stamp-row")?.classList.toggle("stamp-row--stamped", stamped);
  }
  if (state.onlyUnstamped) {
    renderStampList();
  }
  updateSelectionUi();
  persist();
}

/** Sets the start point shown on the map and in the summary. */
function setStart(position, label) {
  const previous = state.plan?.start;
  state.start = position;
  mapView?.setStart(position);
  elements.startLabel.textContent = position
    ? (label ?? `${position.lat.toFixed(5)}, ${position.lon.toFixed(5)}`)
    : "nicht gesetzt";
  const moved =
    previous &&
    position &&
    (Math.abs(previous.lat - position.lat) > 1e-7 ||
      Math.abs(previous.lon - position.lon) > 1e-7);
  if (moved || (previous && !position)) {
    invalidateTour("Startpunkt geändert — bitte die Tagestour neu berechnen.");
  }
  persist();
}

/** Selected stations in catalogue order. */
function selectedStamps() {
  return state.stamps.filter((stamp) => state.selection.has(stamp.id));
}

// ---------------------------------------------------------------------------
// Parking candidates from OpenStreetMap
// ---------------------------------------------------------------------------

/** Bounding box of the selection, grown by the walking radius. */
function selectionBounds() {
  const stamps = selectedStamps();
  if (stamps.length === 0) {
    return null;
  }
  let south = 90;
  let west = 180;
  let north = -90;
  let east = -180;
  for (const stamp of stamps) {
    south = Math.min(south, stamp.lat);
    north = Math.max(north, stamp.lat);
    west = Math.min(west, stamp.lon);
    east = Math.max(east, stamp.lon);
  }
  return expandBounds({ south, west, north, east }, state.settings.maxWalkM);
}

/** Renders the ranked parking suggestions. */
function renderParking(suggestions) {
  state.parkingSuggestions = suggestions;
  elements.parkingList.replaceChildren();
  elements.parkingList.hidden = suggestions.length === 0;

  const kindLabels = {
    trailhead: "Wanderparkplatz",
    public: "Parkplatz",
    roadside: "Haltebucht",
  };

  for (const suggestion of suggestions) {
    const candidate = suggestion.candidate;
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "parking-item";
    button.dataset.parkingId = candidate.id;
    button.setAttribute(
      "aria-label",
      `${candidate.name} als Startpunkt verwenden`,
    );

    const capacity =
      candidate.capacity === null || candidate.capacity === undefined
        ? ""
        : `<span class="parking-item__meta">${candidate.capacity} Plätze</span>`;

    button.innerHTML =
      `<span class="parking-item__name">${escapeHtml(candidate.name)}</span>` +
      `<span class="parking-item__meta">${escapeHtml(kindLabels[candidate.kind] ?? "Parkplatz")}</span>` +
      `<span class="parking-item__meta">${suggestion.servedStampIds.length} Stempel · ` +
      `${formatMeters(suggestion.nearestWalkM)}</span>` +
      capacity;

    button.addEventListener("click", () => {
      setStart(
        { lat: candidate.lat, lon: candidate.lon },
        `${candidate.name} (${candidate.lat.toFixed(5)}, ${candidate.lon.toFixed(5)})`,
      );
      for (const row of elements.parkingList.querySelectorAll(".parking-item")) {
        row.setAttribute("aria-pressed", row === button ? "true" : "false");
      }
      setStatus(
        elements.parkingStatus,
        `Startpunkt: ${candidate.name}. Jetzt „Tagestour berechnen“ drücken.`,
      );
    });

    item.append(button);
    elements.parkingList.append(item);
  }
}

/** Formats a short distance in metres. */
function formatMeters(metres) {
  if (!Number.isFinite(metres)) {
    return "–";
  }
  return metres < 1000
    ? `${Math.round(metres)} m`
    : `${(metres / 1000).toLocaleString("de-DE", { maximumFractionDigits: 1 })} km`;
}

/** Queries OpenStreetMap and ranks the candidates in the Rust core. */
async function searchParking() {
  const stamps = selectedStamps();
  if (stamps.length === 0) {
    setStatus(elements.parkingStatus, "Bitte zuerst Stempelstellen auswählen.", "error");
    return;
  }
  const bounds = selectionBounds();
  if (!bounds) {
    return;
  }

  elements.findParking.disabled = true;
  // The whole-Harz query takes about 35 seconds on the public instance. Say so
  // instead of leaving the user staring at "Frage OpenStreetMap ab …".
  const span = Math.max(bounds.north - bounds.south, bounds.east - bounds.west);
  setStatus(
    elements.parkingStatus,
    span > 0.15
      ? "Große Auswahl: Die Abfrage kann bis zu einer Minute dauern. Weniger Stempel beschleunigen sie."
      : "Frage OpenStreetMap ab … der öffentliche Dienst braucht manchmal bis zu einer Minute.",
  );
  try {
    const { candidates } = await fetchParkingCandidates(bounds);
    if (candidates.length === 0) {
      elements.parkingList.hidden = true;
      setStatus(
        elements.parkingStatus,
        "Keine Parkplätze in diesem Gebiet gefunden. Startpunkt bitte per Kartenklick setzen.",
        "warning",
      );
      return;
    }

    const ranked = call("parking", {
      stampIds: stamps.map((stamp) => stamp.id),
      candidates,
      maxWalkM: state.settings.maxWalkM,
      maxCandidates: CONFIG.defaults.maxParkingCandidates,
      minStamps: 1,
    });
    renderParking(ranked.suggestions);
    elements.parkingAttribution.hidden = false;
    setStatus(
      elements.parkingStatus,
      `${ranked.suggestions.length} Vorschläge aus ${candidates.length} Parkplätzen ` +
        `im Umkreis von ${formatMeters(state.settings.maxWalkM)}.`,
    );
  } catch (error) {
    elements.parkingList.hidden = true;
    handleError(elements.parkingStatus, error, "Parkplatzsuche");
  } finally {
    elements.findParking.disabled = false;
  }
}

// ---------------------------------------------------------------------------
// Planning, routing and track editing
// ---------------------------------------------------------------------------

/** The position of a tour stop. */
function stopPosition(stop) {
  if (!stop) {
    return null;
  }
  if (stop.kind === "via") {
    return { lat: stop.lat, lon: stop.lon };
  }
  const stamp = state.stampsById.get(stop.id);
  return stamp ? { lat: stamp.lat, lon: stamp.lon } : null;
}

/** The editable waypoints in walk order. */
function viaPoints() {
  return state.stops.filter((stop) => stop.kind === "via");
}

/** All waypoints of the tour for the router: start, stops, start. */
function tourWaypoints() {
  const start = state.plan?.start ?? state.start;
  const points = [{ lat: start.lat, lon: start.lon }];
  for (const stop of state.stops) {
    const position = stopPosition(stop);
    if (position) {
      points.push(position);
    }
  }
  points.push({ lat: start.lat, lon: start.lon });
  return points;
}

/** Warnings for the current distance. Replaces the estimated distance after routing. */
function currentWarnings(distanceM) {
  const limitM = state.settings.maxTourKm * 1000;
  if (Number.isFinite(distanceM) && distanceM > limitM + 1) {
    return [{ kind: "oversized_tour", distanceM, limitM }];
  }
  return [];
}

/** Renders the computed day tour. */
function renderPlan() {
  if (!state.plan) {
    return;
  }
  const routed = Boolean(state.route && state.route.geometry.length >= 2);
  const distanceM = state.route?.distanceM ?? state.plan.distanceM;
  const durationS = state.route?.durationS ?? 0;
  const profile = state.route?.profile ?? null;

  elements.resultPanel.hidden = false;
  const summaryItems = [
    ["Stempel", String(state.plan.stampsVisited.length)],
    ["Strecke", formatKm(distanceM)],
  ];
  if (durationS > 0) {
    summaryItems.push(["Gehzeit", formatDuration(durationS)]);
  }
  if (profile) {
    summaryItems.push(["Aufstieg", formatDistance(profile.ascentM)]);
    summaryItems.push(["Abstieg", formatDistance(profile.descentM)]);
  }
  summaryItems.push(["Distanzen", routed ? "geroutet" : "geschätzt"]);
  elements.planSummary.innerHTML = summaryItems
    .map(
      ([label, value]) =>
        `<div class="summary__item"><span class="summary__label">${label}</span>` +
        `<span class="summary__value">${escapeHtml(value)}</span></div>`,
    )
    .join("");

  elements.planWarnings.replaceChildren();
  for (const warning of currentWarnings(distanceM)) {
    const box = document.createElement("p");
    box.className = "warning";
    box.textContent = describeWarning(warning);
    elements.planWarnings.append(box);
  }

  renderTourStops();

  if (profile) {
    elements.elevationPanel.hidden = false;
    renderElevationProfile(elements.elevationChart, profile);
    elements.elevationSummary.textContent =
      `Aufstieg ${formatDistance(profile.ascentM)} · Abstieg ${formatDistance(profile.descentM)} · ` +
      `${Math.round(profile.minEleM)}–${Math.round(profile.maxEleM)} m · ` +
      `Berechnet aus der Weggeometrie (${routingProviderName()}).`;
  } else {
    elements.elevationPanel.hidden = true;
    elements.elevationChart.replaceChildren();
    elements.elevationSummary.textContent = "";
  }

  updateTrackUi();
}

/** Renders the walk order, including the editable waypoints. */
function renderTourStops() {
  const fragment = document.createDocumentFragment();
  const startItem = document.createElement("li");
  startItem.className = "tour-stops__fixed";
  startItem.innerHTML = `<span class="tour-stops__index">Start</span><span>Parkplatz / Startpunkt</span>`;
  fragment.append(startItem);

  let viaNumber = 0;
  state.stops.forEach((stop) => {
    const item = document.createElement("li");
    item.className = "tour-stops__item";
    if (stop.kind === "via") {
      viaNumber += 1;
      const position = stopPosition(stop);
      item.classList.add("tour-stops__item--via");
      const label = document.createElement("span");
      label.className = "tour-stops__label";
      label.textContent = `Wegpunkt ${viaNumber} (${position.lat.toFixed(4)}, ${position.lon.toFixed(4)})`;
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "link-button";
      remove.dataset.waypointIndex = String(viaNumber - 1);
      remove.textContent = "entfernen";
      remove.setAttribute("aria-label", `Wegpunkt ${viaNumber} entfernen`);
      item.append(label, remove);
    } else {
      const stamp = state.stampsById.get(stop.id);
      const code = document.createElement("span");
      code.className = "tour-stops__code";
      code.textContent = stamp?.code ?? `HWN${String(stop.id).padStart(3, "0")}`;
      const label = document.createElement("span");
      label.className = "tour-stops__label";
      label.textContent = stamp?.name ?? "";
      item.append(code, label);
    }
    fragment.append(item);
  });

  const endItem = document.createElement("li");
  endItem.className = "tour-stops__fixed";
  endItem.innerHTML = `<span class="tour-stops__index">Ziel</span><span>zurück zum Start</span>`;
  fragment.append(endItem);

  elements.tourStops.replaceChildren(fragment);
}

/** Draws the tour and the draggable waypoints on the map. */
function renderMap({ fit = false } = {}) {
  if (!mapView || !state.plan) {
    return;
  }
  mapView.clearPlan();
  const geometry = state.route?.geometry;
  const routed = Boolean(geometry && geometry.length >= 2);
  mapView.setPlan([
    {
      points: routed ? geometry : tourWaypoints(),
      routed,
    },
  ]);
  mapView.setTrackHandles(
    viaPoints().map((stop) => ({ lat: stop.lat, lon: stop.lon })),
    {
      onMove: (index, position) => moveWaypoint(index, position),
      onRemove: (index) => removeWaypoint(index),
    },
  );
  if (fit) {
    mapView.fitPlan();
  }
}

/** Enables or disables the track controls for the current state. */
function updateTrackUi() {
  const hasPlan = state.plan !== null;
  elements.trackPanel.hidden = !hasPlan;
  const canEdit = hasPlan && isRoutingEnabled();
  elements.addPoint.disabled = !canEdit;
  elements.resetTrack.disabled = !hasPlan || viaPoints().length === 0;
  if (hasPlan && !isRoutingEnabled() && !elements.trackStatus.textContent) {
    setStatus(
      elements.trackStatus,
      "Ohne Routing-Dienst lassen sich keine Wegpunkte setzen oder verschieben.",
      "warning",
    );
  }
}

/** Leaves "add point" mode and resets the button state. */
function stopAddingPoints() {
  state.addingPoint = false;
  elements.addPoint.setAttribute("aria-pressed", "false");
  mapView?.setAddPointMode(false);
}

/**
 * Index in `state.stops` after which a new waypoint belongs.
 *
 * The clicked position is projected onto the current route; every existing
 * stop is projected as well, so the new waypoint lands between the stops that
 * surround it on the walked line.
 */
function insertionIndex(position) {
  const geometry = state.route?.geometry;
  if (!geometry || geometry.length < 2) {
    return state.stops.length;
  }
  const hit = nearestPointOnTrack(geometry, position);
  if (!hit) {
    return state.stops.length;
  }
  let index = 0;
  state.stops.forEach((stop, stopIndex) => {
    const stopAt = stopPosition(stop);
    const nearest = stopAt ? nearestPointOnTrack(geometry, stopAt) : null;
    if (nearest && nearest.alongM <= hit.alongM + 1) {
      index = stopIndex + 1;
    }
  });
  return index;
}

/** Inserts a waypoint at a map position and reroutes. */
function addWaypointAt(position) {
  if (!state.plan) {
    setStatus(elements.trackStatus, "Bitte zuerst eine Tagestour berechnen.", "error");
    return;
  }
  if (!isRoutingEnabled()) {
    setStatus(
      elements.trackStatus,
      "Track bearbeiten braucht den Routing-Dienst.",
      "warning",
    );
    return;
  }

  let point = position;
  const geometry = state.route?.geometry;
  if (geometry && geometry.length >= 2) {
    // Snap unconditionally onto the existing route: that is the trail the user
    // sees, and it keeps the new node on a mapped path.
    const nearest = nearestPointOnTrack(geometry, position);
    if (nearest) {
      point = { lat: nearest.lat, lon: nearest.lon };
    }
  }
  const index = insertionIndex(point);
  state.stops.splice(index, 0, { kind: "via", lat: point.lat, lon: point.lon });
  renderPlan();
  renderMap();
  setStatus(
    elements.trackStatus,
    `Wegpunkt ${viaPoints().length} eingefügt. Route wird angepasst …`,
  );
  void routeTour({ fit: false, quiet: true });
}

/** Moves a waypoint that was dragged on the map. */
function moveWaypoint(index, position) {
  const via = viaPoints()[index];
  if (!via) {
    return;
  }
  const geometry = state.route?.geometry;
  const snapped =
    geometry && geometry.length >= 2
      ? snapToTrack(geometry, position, CONFIG.track.snapRadiusM)
      : { ...position, snapped: false };
  via.lat = snapped.lat;
  via.lon = snapped.lon;
  // The dragged marker already sits at the dropped position. Rebuilding the
  // handles here would remove the marker while Leaflet is still finishing the
  // drag, so the map is redrawn after the route request instead.
  renderPlan();
  setStatus(
    elements.trackStatus,
    snapped.snapped
      ? "Wegpunkt auf die Weggeometrie eingerastet. Route wird angepasst …"
      : "Wegpunkt verschoben. Route wird angepasst …",
  );
  void routeTour({ fit: false, quiet: true });
}

/** Removes one editable waypoint. */
function removeWaypoint(index) {
  const via = viaPoints()[index];
  if (!via) {
    return;
  }
  const stopIndex = state.stops.indexOf(via);
  if (stopIndex >= 0) {
    state.stops.splice(stopIndex, 1);
  }
  renderPlan();
  renderMap();
  setStatus(elements.trackStatus, "Wegpunkt entfernt.");
  if (isRoutingEnabled()) {
    void routeTour({ fit: false, quiet: true });
  }
}

/** Removes every added waypoint. */
function resetTrack() {
  state.stops = state.stops.filter((stop) => stop.kind !== "via");
  stopAddingPoints();
  renderPlan();
  renderMap();
  setStatus(elements.trackStatus, "Alle Wegpunkte entfernt.");
  if (isRoutingEnabled()) {
    void routeTour({ fit: false, quiet: true });
  }
}

/** Toggles "click on the map to add a waypoint". */
function toggleAddPoint() {
  if (!state.plan) {
    return;
  }
  if (!isRoutingEnabled()) {
    setStatus(elements.trackStatus, "Track bearbeiten braucht den Routing-Dienst.", "warning");
    return;
  }
  state.addingPoint = !state.addingPoint;
  elements.addPoint.setAttribute("aria-pressed", state.addingPoint ? "true" : "false");
  mapView?.setAddPointMode(state.addingPoint);
  setStatus(
    elements.trackStatus,
    state.addingPoint
      ? "In die Karte klicken, um einen Wegpunkt einzufügen. Mit Escape beenden."
      : "",
  );
}

/** Turns a warning from the core into a sentence. */
function describeWarning(warning) {
  if (!warning || typeof warning !== "object") {
    return "Unbekannter Hinweis.";
  }
  if (warning.kind === "oversized_tour") {
    return (
      `Die Tour ist ${formatKm(warning.distanceM)} lang, erlaubt sind höchstens ` +
      `${formatKm(warning.limitM)}. Entferne Stempel aus der Auswahl oder erhöhe die maximale Strecke.`
    );
  }
  return JSON.stringify(warning);
}

/** Computes a day tour for the current selection and routes it. */
async function computePlan() {
  const stamps = selectedStamps();
  if (stamps.length === 0) {
    setStatus(elements.planStatus, "Bitte zuerst Stempelstellen auswählen.", "error");
    return;
  }

  const start = state.start ?? { lat: stamps[0].lat, lon: stamps[0].lon };
  if (!state.start) {
    setStart(start);
  }

  state.routeController?.abort();
  state.routeController = null;
  stopAddingPoints();
  setStatus(elements.planStatus, "Berechne Reihenfolge …");

  let plan;
  try {
    plan = call("plan", {
      stampIds: stamps.map((stamp) => stamp.id),
      start,
      maxTourDistanceM: state.settings.maxTourKm * 1000,
      providerName: "straight-line-estimate",
    });
  } catch (error) {
    handleError(elements.planStatus, error, "Planung fehlgeschlagen");
    return;
  }

  state.plan = plan;
  state.stops = plan.stampsVisited.map((id) => ({ kind: "stamp", id }));
  state.route = null;

  // Show the estimate immediately: the ordering is already useful, and the
  // routed distances replace it a few seconds later.
  renderPlan();
  renderMap({ fit: true });
  updateSelectionUi();
  elements.exportPlan.disabled = false;

  if (!isRoutingEnabled()) {
    setStatus(
      elements.planStatus,
      "Kein Routing-Dienst konfiguriert; Distanzen sind geschätzt.",
      "warning",
    );
    return;
  }

  await routeTour({ fit: true, quiet: false });
}

/**
 * Routes the current stops along real hiking trails.
 *
 * One request covers the whole day tour: BRouter accepts the list of via
 * points, which keeps the load on the public instance low and gives a single
 * contiguous track for the map, the elevation profile and the GPX export.
 *
 * @param {{fit?: boolean, quiet?: boolean}} [options]
 */
async function routeTour(options = {}) {
  if (!state.plan) {
    return;
  }
  const points = tourWaypoints();
  if (points.length < 2) {
    return;
  }

  state.routeController?.abort();
  const controller = new AbortController();
  state.routeController = controller;
  if (!options.quiet) {
    setStatus(elements.planStatus, "Berechne Wegführung entlang der Wanderwege …");
  }

  try {
    const result = await routeWaypoints(points, { signal: controller.signal });
    if (controller.signal.aborted) {
      return;
    }
    state.route = {
      distanceM: result.distanceM,
      durationS: result.durationS,
      geometry: result.points,
      profile: buildElevationProfile(result.points),
    };
    settleWaypoints();
    renderPlan();
    renderMap({ fit: options.fit ?? false });
    setStatus(
      elements.planStatus,
      `Distanzen entlang der Wege von ${routingProviderName()}.`,
    );
  } catch (error) {
    if (controller.signal.aborted || /abgebrochen/i.test(error?.message ?? "")) {
      return;
    }
    // The old geometry no longer matches the edited stops, so fall back to the
    // clearly marked straight-line estimate instead of showing a stale track.
    state.route = null;
    renderPlan();
    renderMap({ fit: false });
    setStatus(
      elements.planStatus,
      `${error.message} Die Tour wird als Schätzung angezeigt.`,
      "warning",
    );
  } finally {
    if (state.routeController === controller) {
      state.routeController = null;
    }
  }
}

/**
 * Settles newly routed waypoints onto the fresh geometry when they are close.
 *
 * A waypoint that was dragged onto a path was sent to the router as a point
 * that may lie a few metres beside the resulting line. Snapping it back keeps
 * the handle on the visible trail without changing the route.
 */
function settleWaypoints() {
  const geometry = state.route?.geometry;
  if (!geometry || geometry.length < 2) {
    return;
  }
  for (const stop of viaPoints()) {
    const nearest = nearestPointOnTrack(geometry, stop);
    if (nearest && nearest.distanceM <= CONFIG.track.settleRadiusM) {
      stop.lat = nearest.lat;
      stop.lon = nearest.lon;
    }
  }
}

/** Groups the selection spatially. */
function computeClusters() {
  const stamps = selectedStamps();
  if (stamps.length === 0) {
    setStatus(elements.planStatus, "Bitte zuerst Stempelstellen auswählen.", "error");
    return;
  }
  try {
    const result = call("clusters", {
      stampIds: stamps.map((stamp) => stamp.id),
      maxStamps: CONFIG.defaults.clusterMaxStamps,
      maxDiameterM: CONFIG.defaults.clusterMaxDiameterM,
    });
    state.clusters = result.clusters;
    elements.clusterPanel.hidden = false;
    elements.clusterList.replaceChildren();

    for (const cluster of result.clusters) {
      const item = document.createElement("li");
      item.className = "cluster-card";
      const names = cluster.stampIds
        .map((id) => state.stampsById.get(id))
        .filter(Boolean)
        .map((stamp) => escapeHtml(stamp.code))
        .join(", ");
      item.innerHTML =
        `<div class="cluster-card__head"><span>Gruppe ${cluster.id + 1}</span>` +
        `<span>${formatKm(cluster.diameterM)} ⌀</span></div>` +
        `<p class="cluster-card__stamps">${cluster.stampIds.length} Stempel: ${names}</p>`;
      elements.clusterList.append(item);
    }
    setStatus(elements.planStatus, `${result.clusters.length} Gruppen gebildet.`);
  } catch (error) {
    handleError(elements.planStatus, error, "Gruppierung fehlgeschlagen");
  }
}

/** Exports the current selection as GPX waypoints. */
async function exportSelection() {
  const stamps = selectedStamps();
  if (stamps.length === 0) {
    return;
  }
  try {
    const result = call("exportGpx", {
      stampIds: stamps.map((stamp) => stamp.id),
      name: `Harzer Wandernadel – Auswahl (${stamps.length} Stempel)`,
      description: `Auswahl von ${stamps.length} Stempelstellen der Harzer Wandernadel.`,
      includeTrack: false,
    });
    downloadText(`harzer-wandernadel-auswahl-${timestampForFilename()}.gpx`, result.gpx);
    setStatus(elements.gpxStatus, `${result.waypointCount} Wegpunkte exportiert.`);
  } catch (error) {
    handleError(elements.gpxStatus, error, "Export fehlgeschlagen");
  }
}

/** Exports the computed day tour as GPX, with elevation when routed. */
async function exportPlan() {
  if (!state.plan) {
    return;
  }
  const geometry = state.route?.geometry ?? [];
  const routed = geometry.length >= 2;
  const trackPoints = routed
    ? geometry.map((point) => ({
        lat: point.lat,
        lon: point.lon,
        ...(Number.isFinite(point.ele) ? { ele: point.ele } : {}),
      }))
    : undefined;
  const stampCount = state.plan.stampsVisited.length;

  const description = routed
    ? `Tagestour mit ${stampCount} Stempelstellen entlang der Wege (${routingProviderName()}).`
    : `Tagestour mit ${stampCount} Stempelstellen. Achtung: Die Distanzen sind ` +
      `Luftlinien-Schätzungen, die Datei enthält daher keine Weggeometrie, sondern ` +
      `Wegpunkte in empfohlener Reihenfolge.`;

  try {
    const result = call("exportGpx", {
      stampIds: state.plan.stampsVisited,
      name: `Harzer Wandernadel – Tagestour (${stampCount} Stempel)`,
      description,
      trackPoints,
      includeTrack: routed,
    });
    downloadText(`harzer-wandernadel-tagestour-${timestampForFilename()}.gpx`, result.gpx);
    setStatus(
      elements.gpxStatus,
      routed
        ? `Tagestour mit ${result.waypointCount} Wegpunkten und ${result.trackPointCount} Trackpunkten exportiert.`
        : `Tagestour mit ${result.waypointCount} Wegpunkten exportiert (ohne Weggeometrie, siehe Hinweis).`,
      routed ? "" : "warning",
    );
  } catch (error) {
    handleError(elements.gpxStatus, error, "Export fehlgeschlagen");
  }
}

/** Imports a GPX file and preselects the stations it contains. */
async function importGpxFile(file) {
  setStatus(elements.gpxStatus, "Lese GPX …");
  try {
    const text = await file.text();
    const result = call("importGpx", { gpx: text });
    const matched = result.matchedStampIds.filter((id) => state.stampsById.has(id));
    if (matched.length === 0) {
      setStatus(
        elements.gpxStatus,
        `Keine bekannte Stempelstelle gefunden (${result.waypointCount} Wegpunkte gelesen).`,
        "warning",
      );
      return;
    }
    for (const id of matched) {
      state.selection.add(id);
    }
    invalidateTour("Auswahl geändert — bitte die Tagestour neu berechnen.");
    renderStampList();
    updateSelectionUi();
    persist();
    setStatus(
      elements.gpxStatus,
      `${matched.length} Stempelstellen übernommen, ${result.unmatchedNames.length} Wegpunkte ohne Zuordnung.`,
    );
  } catch (error) {
    handleError(elements.gpxStatus, error, "Import fehlgeschlagen");
  }
}

/** Sanitises an error into a status line. */
function handleError(element, error, prefix) {
  if (error instanceof PlannerError) {
    setStatus(element, `${prefix}: ${error.message}`, "error");
  } else {
    setStatus(element, `${prefix}: ${error.message ?? error}`, "error");
  }
}

/** Wires every event listener. */
function bindEvents() {
  elements.search.addEventListener("input", () => {
    state.filter = elements.search.value;
    renderStampList();
  });

  elements.stampList.addEventListener("click", (event) => {
    const stampedButton = event.target.closest("[data-stamped-id]");
    if (stampedButton) {
      toggleStamped(Number(stampedButton.dataset.stampedId));
      return;
    }
    const button = event.target.closest("[data-stamp-id]");
    if (!button) {
      return;
    }
    const id = Number(button.dataset.stampId);
    toggleStamp(id);
    mapView?.focusStamp(id);
  });

  elements.selectAll.addEventListener("click", () => {
    for (const stamp of filteredStamps()) {
      state.selection.add(stamp.id);
    }
    invalidateTour("Auswahl geändert — bitte die Tagestour neu berechnen.");
    renderStampList();
    updateSelectionUi();
    persist();
  });

  elements.selectNone.addEventListener("click", () => {
    state.selection.clear();
    invalidateTour("Auswahl geändert — bitte die Tagestour neu berechnen.");
    renderStampList();
    updateSelectionUi();
    persist();
  });

  elements.selectInvert.addEventListener("click", () => {
    for (const stamp of filteredStamps()) {
      if (state.selection.has(stamp.id)) {
        state.selection.delete(stamp.id);
      } else {
        state.selection.add(stamp.id);
      }
    }
    invalidateTour("Auswahl geändert — bitte die Tagestour neu berechnen.");
    renderStampList();
    updateSelectionUi();
    persist();
  });

  elements.onlyUnstamped.addEventListener("change", () => {
    state.onlyUnstamped = elements.onlyUnstamped.checked;
    renderStampList();
  });

  elements.stampSelection.addEventListener("click", () => {
    for (const id of state.selection) {
      state.stamped.add(id);
    }
    renderStampList();
    updateSelectionUi();
    persist();
  });

  elements.unstampSelection.addEventListener("click", () => {
    for (const id of state.selection) {
      state.stamped.delete(id);
    }
    renderStampList();
    updateSelectionUi();
    persist();
  });

  elements.findParking.addEventListener("click", () => {
    void searchParking();
  });

  elements.useFirstStamp.addEventListener("click", () => {
    const stamps = selectedStamps();
    if (stamps.length === 0) {
      setStatus(elements.parkingStatus, "Bitte zuerst Stempelstellen auswählen.", "error");
      return;
    }
    setStart({ lat: stamps[0].lat, lon: stamps[0].lon });
    setStatus(elements.parkingStatus, "Startpunkt ist der erste ausgewählte Stempel.");
  });

  elements.clearStart.addEventListener("click", () => {
    setStart(null);
    for (const row of elements.parkingList.querySelectorAll(".parking-item")) {
      row.setAttribute("aria-pressed", "false");
    }
  });

  elements.maxTourKm.addEventListener("change", () => {
    const value = Number(elements.maxTourKm.value);
    if (Number.isFinite(value) && value > 0) {
      state.settings.maxTourKm = value;
      persist();
      if (state.plan) {
        // The limit only affects the warning, not the order.
        renderPlan();
      }
    }
  });

  elements.plan.addEventListener("click", () => {
    void computePlan();
  });

  elements.cluster.addEventListener("click", () => computeClusters());

  elements.addPoint.addEventListener("click", () => toggleAddPoint());

  elements.resetTrack.addEventListener("click", () => resetTrack());

  elements.tourStops.addEventListener("click", (event) => {
    const button = event.target.closest("[data-waypoint-index]");
    if (!button) {
      return;
    }
    removeWaypoint(Number(button.dataset.waypointIndex));
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && state.addingPoint) {
      stopAddingPoints();
      setStatus(elements.trackStatus, "");
    }
  });

  elements.exportSelection.addEventListener("click", () => {
    void exportSelection();
  });

  elements.exportPlan.addEventListener("click", () => {
    void exportPlan();
  });

  elements.importGpx.addEventListener("change", () => {
    const file = elements.importGpx.files?.[0];
    if (file) {
      void importGpxFile(file);
    }
    elements.importGpx.value = "";
  });

  elements.exportData.addEventListener("click", () => {
    const text = serializeExport({
      selection: [...state.selection].sort((a, b) => a - b),
      stamped: [...state.stamped].sort((a, b) => a - b),
      start: state.start,
      settings: state.settings,
    });
    downloadText(
      `harzer-wandernadel-daten-${timestampForFilename()}.json`,
      text,
      "application/json",
    );
    setStatus(elements.dataStatus, "Daten exportiert.");
  });

  elements.importData.addEventListener("click", () => elements.importDataFile.click());

  elements.importDataFile.addEventListener("change", async () => {
    const file = elements.importDataFile.files?.[0];
    elements.importDataFile.value = "";
    if (!file) {
      return;
    }
    try {
      const restored = parseImport(await file.text());
      invalidateTour("");
      state.selection.clear();
      state.stamped.clear();
      state.start = null;
      restore(restored);
      elements.maxTourKm.value = String(state.settings.maxTourKm);
      mapView?.setStart(state.start);
      elements.startLabel.textContent = state.start
        ? `${state.start.lat.toFixed(5)}, ${state.start.lon.toFixed(5)}`
        : "nicht gesetzt";
      renderStampList();
      updateSelectionUi();
      persist();
      setStatus(elements.dataStatus, "Daten importiert.");
    } catch (error) {
      handleError(elements.dataStatus, error, "Import fehlgeschlagen");
    }
  });
}

/** Registers the service worker for offline use of the app shell. */
function registerServiceWorker() {
  if (!("serviceWorker" in navigator)) {
    return;
  }
  if (
    location.protocol !== "https:" &&
    location.hostname !== "localhost" &&
    location.hostname !== "127.0.0.1"
  ) {
    return;
  }
  navigator.serviceWorker.register("sw.js").catch(() => {
    // Offline support is an enhancement; failing to register must stay silent.
  });
}

/** Boots the application. */
async function main() {
  bindEvents();

  try {
    await loadCore();
    const catalog = call("catalog");
    state.dataset = catalog.dataset;
    state.stamps = catalog.stamps;
    state.stampsById = new Map(catalog.stamps.map((stamp) => [stamp.id, stamp]));

    elements.catalogBadge.textContent = `${catalog.stamps.length} Stempel · Stand ${catalog.dataset.released}`;
    elements.engineBadge.textContent = isRoutingEnabled()
      ? `Rechenkern: Rust/WASM · Wege: ${routingProviderName()}`
      : "Rechenkern: Rust/WASM · Wege: Schätzung";
    elements.parkingAttribution.innerHTML = PARKING_ATTRIBUTION;

    mapView = createMap(document.querySelector("#map"), {
      onStampToggle: (id) => toggleStamp(id),
      onMapClick: (position) => {
        setStart(position);
        setStatus(elements.parkingStatus, "Startpunkt per Kartenklick gesetzt.");
      },
      onAddPoint: (position) => addWaypointAt(position),
      onWaypointMove: (index, position) => moveWaypoint(index, position),
      onWaypointRemove: (index) => removeWaypoint(index),
    });
    mapView.setStamps(catalog.stamps);
    // Open on the Harz rather than on a fixed guess: the catalogue knows its
    // own extent, so the first view always contains every station.
    mapView.fitAllStamps();

    restore(await loadState());
    elements.maxTourKm.value = String(state.settings.maxTourKm);
    mapView.setStart(state.start);
    mapView.setStamped(state.stamped);
    elements.startLabel.textContent = state.start
      ? `${state.start.lat.toFixed(5)}, ${state.start.lon.toFixed(5)}`
      : "nicht gesetzt";

    renderStampList();
    updateSelectionUi();
    registerServiceWorker();
  } catch (error) {
    setStatus(
      elements.planStatus,
      `Der Planer konnte nicht gestartet werden: ${error.message ?? error}`,
      "error",
    );
    elements.engineBadge.textContent = "Rechenkern: Fehler";
  }
}

void main();
