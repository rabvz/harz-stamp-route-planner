// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Small formatting helpers shared by the UI modules.

/**
 * Formats metres as kilometres with one decimal.
 * @param {number} metres
 * @returns {string} for example "12,4 km"
 */
export function formatKm(metres) {
  if (!Number.isFinite(metres)) {
    return "–";
  }
  const kilometres = metres / 1000;
  return `${kilometres.toLocaleString("de-DE", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  })} km`;
}

/**
 * Formats metres for short distances.
 * @param {number} metres
 * @returns {string} for example "850 m"
 */
export function formatDistance(metres) {
  if (!Number.isFinite(metres)) {
    return "–";
  }
  if (metres < 1000) {
    return `${Math.round(metres).toLocaleString("de-DE")} m`;
  }
  return formatKm(metres);
}

/**
 * Formats a duration in seconds as hours and minutes.
 * @param {number} seconds
 * @returns {string}
 */
export function formatDuration(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return "–";
  }
  const totalMinutes = Math.round(seconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours === 0) {
    return `${minutes} min`;
  }
  return `${hours} h ${String(minutes).padStart(2, "0")} min`;
}

/**
 * Escapes text for interpolation into HTML.
 *
 * Content from the station catalogue and from imported files is untrusted
 * input; every interpolation goes through this or through textContent.
 * @param {unknown} value
 * @returns {string}
 */
export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

/**
 * Formats a station for display, for example "HWN001 Eckertalsperre".
 * @param {{code: string, name: string}} stamp
 * @returns {string}
 */
export function stampLabel(stamp) {
  return `${stamp.code} ${stamp.name}`;
}

/**
 * Triggers a browser download for generated text.
 * @param {string} filename
 * @param {string} text
 * @param {string} mimeType
 */
export function downloadText(filename, text, mimeType = "application/gpx+xml") {
  const blob = new Blob([text], { type: `${mimeType};charset=utf-8` });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  // Revoking immediately can cancel the download in some browsers.
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}

/**
 * Builds a filesystem-safe timestamp for file names.
 * @returns {string} for example "2026-10-25-0930"
 */
export function timestampForFilename() {
  const now = new Date();
  const pad = (value) => String(value).padStart(2, "0");
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}-${pad(
    now.getHours(),
  )}${pad(now.getMinutes())}`;
}
