// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Local persistence. Everything the user creates stays in this browser: the
// selection, the settings and any saved data exports. There is no account, no
// server and no telemetry.

const DATABASE_NAME = "harz-stamp-route-planner";
const DATABASE_VERSION = 1;
const STORE_NAME = "state";
const STATE_KEY = "app-state";

/** The persisted shape. Bump `version` when it changes incompatibly. */
export const STATE_VERSION = 1;

/** Opens the database, creating the object store on first use. */
function openDatabase() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(STORE_NAME)) {
        database.createObjectStore(STORE_NAME);
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB nicht verfügbar"));
  });
}

/** Runs one transaction against the state store. */
async function withStore(mode, run) {
  const database = await openDatabase();
  try {
    return await new Promise((resolve, reject) => {
      const transaction = database.transaction(STORE_NAME, mode);
      const store = transaction.objectStore(STORE_NAME);
      const request = run(store);
      transaction.oncomplete = () => resolve(request?.result);
      transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB-Fehler"));
      transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB-Abbruch"));
    });
  } finally {
    database.close();
  }
}

/**
 * Loads the persisted state.
 * @returns {Promise<object|null>}
 */
export async function loadState() {
  try {
    const value = await withStore("readonly", (store) => store.get(STATE_KEY));
    return value ?? null;
  } catch {
    // A blocked or unavailable IndexedDB must not break the app; the planner
    // works fine without persistence.
    return null;
  }
}

/**
 * Writes the state. Failures are reported to the caller but never thrown from
 * an event handler, so a private-mode browser cannot wedge the UI.
 * @param {object} state
 * @returns {Promise<boolean>} whether the write succeeded
 */
export async function saveState(state) {
  try {
    await withStore("readwrite", (store) => store.put(state, STATE_KEY));
    return true;
  } catch {
    return false;
  }
}

/** Removes all persisted data. */
export async function clearState() {
  try {
    await withStore("readwrite", (store) => store.delete(STATE_KEY));
    return true;
  } catch {
    return false;
  }
}

/**
 * Builds a portable export of personal data.
 * @param {object} state
 * @returns {string} JSON text
 */
export function serializeExport(state) {
  return JSON.stringify(
    {
      application: "harz-stamp-route-planner",
      state_version: STATE_VERSION,
      exported_at: new Date().toISOString(),
      state,
    },
    null,
    2,
  );
}

/**
 * Validates an import and returns the contained state.
 * @param {string} text
 * @returns {object} the validated state
 */
export function parseImport(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("Die Datei ist kein gültiges JSON.");
  }
  if (!parsed || typeof parsed !== "object") {
    throw new Error("Die Datei enthält kein Datenobjekt.");
  }
  if (parsed.application !== "harz-stamp-route-planner") {
    throw new Error("Die Datei stammt nicht von diesem Planer.");
  }
  if (parsed.state_version !== STATE_VERSION) {
    throw new Error(
      `Die Datei hat Version ${parsed.state_version}, dieser Planer erwartet ${STATE_VERSION}.`,
    );
  }
  const state = parsed.state;
  if (!state || typeof state !== "object") {
    throw new Error("Der gespeicherte Zustand fehlt.");
  }
  return state;
}
