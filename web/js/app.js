// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Application wiring: catalogue -> selection -> plan -> GPX.
//
// Everything is held in memory and mirrored to IndexedDB. The Rust core does
// the geometry and optimisation; this module only moves data across the
// boundary and renders it.

import { CONFIG } from "./config.js";
import {
  downloadText,
  escapeHtml,
  formatKm,
  stampLabel,
  timestampForFilename,
} from "./format.js";
import { createMap } from "./mapview.js";
import { isRoutingEnabled, routeLegs } from "./routing.js";
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
  },
  plan: null,
  /** @type {Map<number, Array<{lat: number, lon: number}>> | null} */
  planGeometry: null,
  clusters: null,
  filter: "",
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
function setStart(position) {
  state.start = position;
  mapView?.setStart(position);
  elements.startLabel.textContent = position
    ? `${position.lat.toFixed(5)}, ${position.lon.toFixed(5)}`
    : "nicht gesetzt";
  persist();
}

/** Selected stations in catalogue order. */
function selectedStamps() {
  return state.stamps.filter((stamp) => state.selection.has(stamp.id));
}

/** Renders the computed plan. */
function renderPlan(itinerary) {
  const days = itinerary.days ?? [];
  const estimated = itinerary.estimated !== false;

  elements.resultPanel.hidden = false;
  elements.planSummary.innerHTML =
    `<div class="summary__item"><span class="summary__label">Tage</span>` +
    `<span class="summary__value">${days.length}</span></div>` +
    `<div class="summary__item"><span class="summary__label">Stempel</span>` +
    `<span class="summary__value">${itinerary.stampsVisited.length}</span></div>` +
    `<div class="summary__item"><span class="summary__label">Gesamt</span>` +
    `<span class="summary__value">${formatKm(itinerary.totalDistanceM)}</span></div>` +
    `<div class="summary__item"><span class="summary__label">Distanzen</span>` +
    `<span class="summary__value">${estimated ? "geschätzt" : "geroutet"}</span></div>`;

  const warnings = itinerary.warnings ?? [];
  elements.planWarnings.replaceChildren();
  for (const warning of warnings) {
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

    item.innerHTML =
      `<div class="day-card__head"><span>Tag ${day.day}</span>` +
      `<span>${formatKm(day.distanceM)}</span></div>` +
      `<p class="day-card__stamps">${names || "—"}</p>`;
    elements.dayList.append(item);
  }

  setStatus(
    elements.planStatus,
    estimated
      ? "Distanzen sind Luftlinien-Schätzungen (Faktor 1,25). Konfigurieren Sie einen Routing-Dienst für echte Wege."
      : "Distanzen stammen aus einem Routing-Dienst.",
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
    return `Tag ${warning.day} überschreitet mit ${formatKm(warning.distanceM)} das Tageslimit von ${formatKm(
      warning.limitM,
    )}; Stempel ${warning.stamp} ist nicht kürzer erreichbar.`;
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
    const points = [{ lat: start.lat, lon: start.lon }];
    for (const leg of day.legs) {
      if (leg.to && leg.to.kind === "stamp") {
        const stamp = state.stampsById.get(leg.to.stamp);
        if (stamp) {
          points.push({ lat: stamp.lat, lon: stamp.lon });
        }
      }
    }
    points.push({ lat: start.lat, lon: start.lon });

    const routed = state.planGeometry?.get(day.day);
    segments.push(
      routed ? { points: routed, day: day.day, routed: true } : { points, day: day.day, routed: false },
    );
  }

  mapView.setPlan(segments);
}

/** Computes a plan for the current selection. */
async function computePlan() {
  const stamps = selectedStamps();
  if (stamps.length === 0) {
    setStatus(elements.planStatus, "Bitte zuerst Stempelstellen auswählen.", "error");
    return;
  }

  const start =
    state.start ?? { lat: stamps[0].lat, lon: stamps[0].lon };
  if (!state.start) {
    setStart(start);
  }

  setStatus(elements.planStatus, "Berechne …");
  try {
    const itinerary = call("plan", {
      stampIds: stamps.map((stamp) => stamp.id),
      start,
      maxDayDistanceM: state.settings.maxDayKm * 1000,
      maxDays: state.settings.maxDays,
      providerName: "straight-line-estimate",
    });
    state.plan = itinerary;
    state.planGeometry = null;
    renderPlan(itinerary);
    renderPlanGeometry(itinerary);
    updateSelectionUi();
    elements.exportPlan.disabled = false;

    if (isRoutingEnabled()) {
      await applyRouting(itinerary);
    }
  } catch (error) {
    handleError(elements.planStatus, error, "Planung fehlgeschlagen");
  }
}

/**
 * Replaces straight-line distances with routed ones when a routing service is
 * configured. Failures are reported but do not discard the estimate.
 */
async function applyRouting(itinerary) {
  const start = itinerary.start;
  const legs = [];
  for (const day of itinerary.days ?? []) {
    let from = start;
    for (const leg of day.legs) {
      const to =
        leg.to && leg.to.kind === "stamp"
          ? state.stampsById.get(leg.to.stamp)
          : start;
      if (!to) {
        continue;
      }
      legs.push({ day: day.day, from, to });
      from = to;
    }
  }
  if (legs.length === 0) {
    return;
  }

  setStatus(elements.planStatus, `Route ${legs.length} Etappen …`);
  try {
    const routed = await routeLegs(legs, (done, total) => {
      setStatus(elements.planStatus, `Route Etappe ${done}/${total} …`);
    });

    const geometryByDay = new Map();
    const distanceByDay = new Map();
    let total = 0;
    for (let index = 0; index < legs.length; index += 1) {
      const leg = legs[index];
      const result = routed[index];
      const day = leg.day;
      const points = geometryByDay.get(day) ?? [];
      for (const point of result.points) {
        const last = points[points.length - 1];
        if (!last || last.lat !== point.lat || last.lon !== point.lon) {
          points.push(point);
        }
      }
      geometryByDay.set(day, points);
      distanceByDay.set(day, (distanceByDay.get(day) ?? 0) + result.distanceM);
      total += result.distanceM;
    }

    state.planGeometry = geometryByDay;
    const updated = {
      ...itinerary,
      estimated: false,
      providerName: "configured-routing-service",
      totalDistanceM: total,
      days: (itinerary.days ?? []).map((day) => ({
        ...day,
        distanceM: distanceByDay.get(day.day) ?? day.distanceM,
      })),
    };
    state.plan = updated;
    renderPlan(updated);
    renderPlanGeometry(updated);
  } catch (error) {
    setStatus(
      elements.planStatus,
      `Routing fehlgeschlagen (${error.message}); es bleiben Schätzwerte.`,
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
    ? `Tour mit ${state.plan.days.length} Tagen, geroutete Distanzen.`
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

  elements.useFirstStamp.addEventListener("click", () => {
    const stamps = selectedStamps();
    if (stamps.length === 0) {
      setStatus(elements.planStatus, "Bitte zuerst Stempelstellen auswählen.", "error");
      return;
    }
    setStart({ lat: stamps[0].lat, lon: stamps[0].lon });
  });

  elements.clearStart.addEventListener("click", () => setStart(null));

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
      const state_ = parseImport(await file.text());
      state.selection.clear();
      state.start = null;
      restore(state_);
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
  if (location.protocol !== "https:" && location.hostname !== "localhost" && location.hostname !== "127.0.0.1") {
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
    elements.engineBadge.textContent = "Rechenkern: Rust/WASM";

    mapView = createMap(document.querySelector("#map"), {
      onStampToggle: (id) => toggleStamp(id),
      onMapClick: (position) => setStart(position),
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
