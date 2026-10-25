// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Application wiring: catalogue -> selection -> parking -> plan -> GPX.
//
// Everything is held in memory and mirrored to IndexedDB. The Rust core does
// the geometry, optimisation and parking ranking; this module moves data across
// the boundary, talks to the two public services (routing and Overpass) and
// renders the result.

import { CONFIG } from "./config.js";
import {
  downloadText,
  escapeHtml,
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
import { isRoutingEnabled, routingProviderName, routeWaypoints } from "./routing.js";
import { STATE_VERSION, loadState, parseImport, saveState, serializeExport } from "./store.js";
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
  /** @type {{lat: number, lon: number} | null} */
  start: null,
  settings: {
    maxDayKm: CONFIG.defaults.maxDayKm,
    maxDays: CONFIG.defaults.maxDays,
    maxWalkM: CONFIG.defaults.maxWalkM,
  },
  plan: null,
  /** @type {Map<number, Array<{lat: number, lon: number}>> | null} */
  planGeometry: null,
  /** @type {Array<object>} */
  parkingSuggestions: [],
  clusters: null,
  filter: "",
  /** Aborts an in-flight routing run when a new plan is requested. */
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
  findParking: document.querySelector("#find-parking"),
  parkingList: document.querySelector("#parking-list"),
  parkingStatus: document.querySelector("#parking-status"),
  parkingAttribution: document.querySelector("#parking-attribution"),
  useFirstStamp: document.querySelector("#use-first-stamp"),
  clearStart: document.querySelector("#clear-start"),
  startLabel: document.querySelector("#start-label"),
  maxDayKm: document.querySelector("#max-day-km"),
  maxDays: document.querySelector("#max-days"),
  plan: document.querySelector("#plan"),
  cluster: document.querySelector("#cluster"),
  planStatus: document.querySelector("#plan-status"),
  resultPanel: document.querySelector("#result-panel"),
  planSummary: document.querySelector("#plan-summary"),
  planWarnings: document.querySelector("#plan-warnings"),
  dayList: document.querySelector("#day-list"),
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
  if (saved.start && Number.isFinite(saved.start.lat) && Number.isFinite(saved.start.lon)) {
    state.start = { lat: saved.start.lat, lon: saved.start.lon };
  }
  if (saved.settings) {
    if (Number.isFinite(saved.settings.maxDayKm)) {
      state.settings.maxDayKm = saved.settings.maxDayKm;
    }
    if (Number.isFinite(saved.settings.maxDays)) {
      state.settings.maxDays = saved.settings.maxDays;
    }
    if (Number.isFinite(saved.settings.maxWalkM)) {
      state.settings.maxWalkM = saved.settings.maxWalkM;
    }
  }
}

/** The stamps matching the current search filter. */
function filteredStamps() {
  const needle = state.filter.trim().toLowerCase();
  if (!needle) {
    return state.stamps;
  }
  return state.stamps.filter((stamp) => {
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
    item.append(button);
    fragment.append(item);
  }

  elements.stampList.replaceChildren(fragment);
  if (stamps.length === 0) {
    const empty = document.createElement("li");
    empty.className = "hint";
    empty.textContent = "Keine Stempelstelle passt zur Suche.";
    elements.stampList.append(empty);
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
}

/** Toggles one station and re-renders the affected rows. */
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
  updateSelectionUi();
  persist();
}

/** Sets the start point shown on the map and in the summary. */
function setStart(position, label) {
  state.start = position;
  mapView?.setStart(position);
  elements.startLabel.textContent = position
    ? (label ?? `${position.lat.toFixed(5)}, ${position.lon.toFixed(5)}`)
    : "nicht gesetzt";
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
        `Startpunkt: ${candidate.name}. Jetzt „Tour berechnen“ drücken.`,
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
  setStatus(
    elements.parkingStatus,
    "Frage OpenStreetMap ab … der öffentliche Dienst braucht manchmal bis zu einer Minute.",
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
// Planning and routing
// ---------------------------------------------------------------------------

/** Renders the computed plan. */
function renderPlan(itinerary) {
  const days = itinerary.days ?? [];
  const estimated = itinerary.estimated !== false;

  elements.resultPanel.hidden = false;
  const totalDuration = days.reduce((sum, day) => sum + (day.durationS ?? 0), 0);
  elements.planSummary.innerHTML =
    `<div class="summary__item"><span class="summary__label">Tage</span>` +
    `<span class="summary__value">${days.length}</span></div>` +
    `<div class="summary__item"><span class="summary__label">Stempel</span>` +
    `<span class="summary__value">${itinerary.stampsVisited.length}</span></div>` +
    `<div class="summary__item"><span class="summary__label">Gesamt</span>` +
    `<span class="summary__value">${formatKm(itinerary.totalDistanceM)}</span></div>` +
    (totalDuration > 0
      ? `<div class="summary__item"><span class="summary__label">Gehzeit</span>` +
        `<span class="summary__value">${formatDuration(totalDuration)}</span></div>`
      : "") +
    `<div class="summary__item"><span class="summary__label">Distanzen</span>` +
    `<span class="summary__value">${estimated ? "geschätzt" : "geroutet"}</span></div>`;

  elements.planWarnings.replaceChildren();
  for (const warning of itinerary.warnings ?? []) {
    const box = document.createElement("p");
    box.className = "warning";
    box.textContent = describeWarning(warning);
    elements.planWarnings.append(box);
  }

  elements.dayList.replaceChildren();
  for (const day of days) {
    const item = document.createElement("li");
    item.className = `day-card${day.oversized ? " day-card--oversized" : ""}`;

    const names = day.stamps
      .map((id) => state.stampsById.get(id))
      .filter(Boolean)
      .map((stamp) => escapeHtml(stamp.code))
      .join(", ");
    const duration = day.durationS ? ` · ${formatDuration(day.durationS)}` : "";

    item.innerHTML =
      `<div class="day-card__head"><span>Tag ${day.day}</span>` +
      `<span>${formatKm(day.distanceM)}${duration}</span></div>` +
      `<p class="day-card__stamps">${names || "—"}</p>`;
    elements.dayList.append(item);
  }

  setStatus(
    elements.planStatus,
    estimated
      ? "Distanzen sind Luftlinien-Schätzungen. Der Routing-Dienst war nicht erreichbar."
      : `Distanzen entlang der Wege von ${itinerary.providerName}.`,
    estimated ? "warning" : "",
  );
}

/** Turns a warning from the core into a sentence. */
function describeWarning(warning) {
  if (!warning || typeof warning !== "object") {
    return "Unbekannter Hinweis.";
  }
  if (warning.kind === "day_limit_exceeded") {
    return `Die Tour braucht ${warning.required} Tage, erlaubt waren ${warning.allowed}.`;
  }
  if (warning.kind === "oversized_day") {
    return (
      `Tag ${warning.day} braucht ${formatKm(warning.distanceM)} statt höchstens ` +
      `${formatKm(warning.limitM)} (Stempel ${warning.stamp}).`
    );
  }
  return JSON.stringify(warning);
}

/** Draws the plan on the map, using routed geometry when available. */
function renderPlanGeometry(itinerary) {
  if (!mapView) {
    return;
  }
  mapView.clearPlan();
  const start = itinerary.start;
  const segments = [];

  for (const day of itinerary.days ?? []) {
    const routed = state.planGeometry?.get(day.day);
    if (routed && routed.length >= 2) {
      segments.push({ points: routed, day: day.day, routed: true });
      continue;
    }
    const points = [{ lat: start.lat, lon: start.lon }];
    for (const id of day.stamps) {
      const stamp = state.stampsById.get(id);
      if (stamp) {
        points.push({ lat: stamp.lat, lon: stamp.lon });
      }
    }
    points.push({ lat: start.lat, lon: start.lon });
    segments.push({ points, day: day.day, routed: false });
  }

  mapView.setPlan(segments);
  // After a plan is computed, show it: the previous view was the whole Harz.
  mapView.fitPlan();
}

/** Computes a plan for the current selection and routes it. */
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
  setStatus(elements.planStatus, "Berechne Reihenfolge …");

  let itinerary;
  try {
    itinerary = call("plan", {
      stampIds: stamps.map((stamp) => stamp.id),
      start,
      maxDayDistanceM: state.settings.maxDayKm * 1000,
      maxDays: state.settings.maxDays,
      providerName: "straight-line-estimate",
    });
  } catch (error) {
    handleError(elements.planStatus, error, "Planung fehlgeschlagen");
    return;
  }

  // Show the estimate immediately: the ordering is already useful, and the
  // routed distances replace it a few seconds later.
  state.plan = itinerary;
  state.planGeometry = null;
  renderPlan(itinerary);
  renderPlanGeometry(itinerary);
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

  await routePlan(itinerary, start);
}

/**
 * Replaces estimated distances with real ones along hiking trails.
 *
 * One request per day: BRouter accepts the whole day as a list of via points,
 * which keeps the load on the public instance low and gives a single contiguous
 * track for the map and the GPX export.
 *
 * If a routed day exceeds the daily limit, the last station of that day is moved
 * to the next day and both days are routed again (at most two passes). This is
 * the one place where the plan is adjusted after the core computed it — the
 * alternative would be to show a plan that violates its own limit.
 */
async function routePlan(itinerary, start) {
  const limit = state.settings.maxDayKm * 1000;
  const controller = new AbortController();
  state.routeController = controller;

  /** @type {Array<{day: number, stamps: number[], distanceM: number, durationS: number, geometry: any[], oversized: boolean}>} */
  const days = (itinerary.days ?? []).map((day) => ({
    day: day.day,
    stamps: [...day.stamps],
    distanceM: day.distanceM,
    durationS: 0,
    geometry: [],
    oversized: false,
  }));

  const routeDay = async (day) => {
    const points = [{ lat: start.lat, lon: start.lon }];
    for (const id of day.stamps) {
      const stamp = state.stampsById.get(id);
      if (stamp) {
        points.push({ lat: stamp.lat, lon: stamp.lon });
      }
    }
    points.push({ lat: start.lat, lon: start.lon });

    const result = await routeWaypoints(points, { signal: controller.signal });
    day.distanceM = result.distanceM;
    day.durationS = result.durationS;
    day.geometry = result.points;
    day.oversized = result.distanceM > limit + 1;
  };

  try {
    for (let index = 0; index < days.length; index += 1) {
      setStatus(elements.planStatus, `Route Tag ${index + 1} von ${days.length} …`);
      await routeDay(days[index]);
    }

    // Real trail distances are often twice the straight-line estimate, so the
    // day split computed from estimates is frequently too optimistic. Move the
    // last station of an oversized day into the next one and route again,
    // repeating until every multi-station day fits or no day can give a station
    // away. Each pass costs two routing requests and they are cached.
    for (let pass = 0; pass < 8; pass += 1) {
      const index = days.findIndex((day) => day.oversized && day.stamps.length > 1);
      if (index < 0) {
        break;
      }
      const from = days[index];
      const moved = from.stamps.pop();
      let target = days[index + 1];
      if (!target && days.length < state.settings.maxDays) {
        target = {
          day: days.length + 1,
          stamps: [],
          distanceM: 0,
          durationS: 0,
          geometry: [],
          oversized: false,
        };
        days.push(target);
      }
      if (!target) {
        from.stamps.push(moved);
        break;
      }
      target.stamps.unshift(moved);
      setStatus(elements.planStatus, "Passe Tagesgrenzen an die echten Wege an …");
      await routeDay(from);
      await routeDay(target);
    }

    const warnings = [];
    for (const day of days) {
      if (day.stamps.length > 0 && day.distanceM > limit + 1) {
        warnings.push({
          kind: "oversized_day",
          day: day.day,
          distanceM: day.distanceM,
          limitM: limit,
          stamp: day.stamps[0],
        });
      }
    }
    if (days.length > state.settings.maxDays) {
      warnings.push({
        kind: "day_limit_exceeded",
        required: days.length,
        allowed: state.settings.maxDays,
      });
    }

    const routed = {
      start,
      estimated: false,
      providerName: routingProviderName(),
      days: days.map((day, index) => ({
        day: index + 1,
        stamps: day.stamps,
        distanceM: day.distanceM,
        durationS: day.durationS,
        oversized: day.oversized,
      })),
      totalDistanceM: days.reduce((sum, day) => sum + day.distanceM, 0),
      stampsVisited: days.flatMap((day) => day.stamps),
      warnings,
    };

    state.planGeometry = new Map(days.map((day, index) => [index + 1, day.geometry]));
    state.plan = routed;
    renderPlan(routed);
    renderPlanGeometry(routed);
  } catch (error) {
    if (error?.message?.includes("abgebrochen")) {
      return;
    }
    setStatus(
      elements.planStatus,
      `${error.message} Die Tour bleibt als Schätzung stehen.`,
      "warning",
    );
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

/** Exports the computed plan as GPX. */
async function exportPlan() {
  if (!state.plan) {
    return;
  }
  const routed = state.planGeometry !== null;
  const trackPoints = routed
    ? [...state.planGeometry.entries()]
        .sort((a, b) => a[0] - b[0])
        .flatMap(([, points]) => points)
    : undefined;

  const description = routed
    ? `Tour mit ${state.plan.days.length} Tagen entlang der Wege (${state.plan.providerName}).`
    : `Tour mit ${state.plan.days.length} Tagen. Achtung: Die Distanzen sind Luftlinien-Schätzungen, ` +
      `die Datei enthält daher keine Weggeometrie, sondern Wegpunkte in empfohlener Reihenfolge.`;

  try {
    const result = call("exportGpx", {
      stampIds: state.plan.stampsVisited,
      name: `Harzer Wandernadel – Tour (${state.plan.days.length} Tage)`,
      description,
      trackPoints,
      includeTrack: routed,
    });
    downloadText(`harzer-wandernadel-tour-${timestampForFilename()}.gpx`, result.gpx);
    setStatus(
      elements.gpxStatus,
      routed
        ? `Tour mit ${result.waypointCount} Wegpunkten und ${result.trackPointCount} Trackpunkten exportiert.`
        : `Tour mit ${result.waypointCount} Wegpunkten exportiert (ohne Weggeometrie, siehe Hinweis).`,
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
    renderStampList();
    updateSelectionUi();
    persist();
  });

  elements.selectNone.addEventListener("click", () => {
    state.selection.clear();
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

  elements.maxDayKm.addEventListener("change", () => {
    const value = Number(elements.maxDayKm.value);
    if (Number.isFinite(value) && value > 0) {
      state.settings.maxDayKm = value;
      persist();
    }
  });

  elements.maxDays.addEventListener("change", () => {
    const value = Number(elements.maxDays.value);
    if (Number.isFinite(value) && value > 0) {
      state.settings.maxDays = value;
      persist();
    }
  });

  elements.plan.addEventListener("click", () => {
    void computePlan();
  });

  elements.cluster.addEventListener("click", () => computeClusters());

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
      start: state.start,
      settings: state.settings,
    });
    downloadText(`harzer-wandernadel-daten-${timestampForFilename()}.json`, text, "application/json");
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
      state.selection.clear();
      state.start = null;
      restore(restored);
      elements.maxDayKm.value = String(state.settings.maxDayKm);
      elements.maxDays.value = String(state.settings.maxDays);
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
    });
    mapView.setStamps(catalog.stamps);
    // Open on the Harz rather than on a fixed guess: the catalogue knows its
    // own extent, so the first view always contains every station.
    mapView.fitAllStamps();

    restore(await loadState());
    elements.maxDayKm.value = String(state.settings.maxDayKm);
    elements.maxDays.value = String(state.settings.maxDays);
    mapView.setStart(state.start);
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
