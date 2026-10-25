// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Loader and call bridge for the Rust/WebAssembly core.
//
// The module exports a deliberately tiny C ABI (see crates/wasm-bindings):
// hsr_alloc, hsr_free, hsr_invoke and hsr_abi_version. Requests and responses
// are UTF-8 JSON. Everything below is bookkeeping around that contract.

const ABI_VERSION = 1;
const MODULE_URL = "pkg/route_planner.wasm";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8");

/** Error surfaced from the Rust core, with its machine readable code. */
export class PlannerError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "PlannerError";
    this.code = code;
  }
}

let exports = null;

/**
 * Instantiates the module and verifies the ABI version.
 * @param {string} [url] location of the .wasm file
 * @returns {Promise<{abiVersion: number}>}
 */
export async function load(url = MODULE_URL) {
  if (exports) {
    return { abiVersion: ABI_VERSION };
  }

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(
      `WebAssembly-Modul konnte nicht geladen werden (HTTP ${response.status}). ` +
        `Wurde "cargo xtask web" ausgeführt?`,
    );
  }

  let instance;
  if (typeof WebAssembly.instantiateStreaming === "function") {
    try {
      ({ instance } = await WebAssembly.instantiateStreaming(response.clone(), {}));
    } catch {
      // Some static hosts serve the wrong MIME type; fall back to ArrayBuffer.
      ({ instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {}));
    }
  } else {
    ({ instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {}));
  }

  const api = instance.exports;
  for (const name of ["hsr_alloc", "hsr_free", "hsr_invoke", "hsr_abi_version", "memory"]) {
    if (!(name in api)) {
      throw new Error(`WebAssembly-Modul exportiert ${name} nicht.`);
    }
  }

  const abiVersion = api.hsr_abi_version();
  if (abiVersion !== ABI_VERSION) {
    throw new Error(
      `ABI-Version ${abiVersion} passt nicht zum Frontend (${ABI_VERSION}). ` +
        `Bitte die Seite neu bauen.`,
    );
  }

  exports = api;
  return { abiVersion };
}

/**
 * Calls one operation on the Rust core.
 * @param {string} op operation name, for example "catalog"
 * @param {object} [payload] operation arguments
 * @returns {any} the operation's `data` member
 */
export function call(op, payload = {}) {
  if (!exports) {
    throw new Error("Das WebAssembly-Modul ist noch nicht geladen.");
  }

  const request = JSON.stringify({ op, payload });
  const bytes = encoder.encode(request);

  const requestPointer = exports.hsr_alloc(bytes.length);
  if (requestPointer === 0) {
    throw new Error("Speicher für die Anfrage konnte nicht reserviert werden.");
  }
  // The view must be created after the allocation, and re-created after any
  // call that can grow the module's memory.
  new Uint8Array(exports.memory.buffer, requestPointer, bytes.length).set(bytes);

  let packed;
  try {
    packed = exports.hsr_invoke(requestPointer, bytes.length);
  } finally {
    exports.hsr_free(requestPointer, bytes.length);
  }

  if (packed === 0n || packed === 0) {
    throw new Error("Der Rechenkern hat keine Antwort geliefert.");
  }

  const value = typeof packed === "bigint" ? packed : BigInt(packed);
  const responsePointer = Number(value >> 32n);
  const responseLength = Number(value & 0xffffffffn);

  let text;
  try {
    const view = new Uint8Array(exports.memory.buffer, responsePointer, responseLength);
    text = decoder.decode(view);
  } finally {
    exports.hsr_free(responsePointer, responseLength);
  }

  let response;
  try {
    response = JSON.parse(text);
  } catch {
    throw new Error("Die Antwort des Rechenkerns war kein gültiges JSON.");
  }

  if (!response.ok) {
    const error = response.error ?? {};
    throw new PlannerError(error.code ?? "unknown", error.message ?? "Unbekannter Fehler");
  }
  return response.data;
}

/** Whether the module has been loaded successfully. */
export function isReady() {
  return exports !== null;
}
