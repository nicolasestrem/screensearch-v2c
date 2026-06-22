// Maps a subsystem ComponentStatus onto a Chip tone and a short human label.
// Single source so the StatusRail and ReadinessBanner agree on color/wording.
import type { ComponentStatus } from "../bindings/ComponentStatus";
import type { Readiness } from "../bindings/Readiness";
import type { ChipTone } from "../components/primitives";

export function statusTone(status: ComponentStatus): ChipTone {
  switch (status) {
    case "ready":
      return "ok";
    case "initializing":
      return "warn";
    case "error":
    case "unavailable":
      return "danger";
    case "disabled":
    case "unknown":
      return "neutral";
  }
}

export function statusLabel(status: ComponentStatus): string {
  switch (status) {
    case "ready":
      return "Ready";
    case "initializing":
      return "Starting";
    case "unavailable":
      return "Unavailable";
    case "error":
      return "Error";
    case "disabled":
      return "Off";
    case "unknown":
      return "Unknown";
  }
}

/** The single subsystem most worth surfacing in an aggregate readiness chip:
 *  any error/unavailable wins, then initializing, else ready/neutral. */
export function worstStatus(readiness: Readiness): ComponentStatus {
  const order: ComponentStatus[] = [
    "error",
    "unavailable",
    "initializing",
    "ready",
    "disabled",
    "unknown",
  ];
  const statuses = [
    readiness.capture.status,
    readiness.db.status,
    readiness.embed_model.status,
    readiness.sidecar.status,
  ];
  for (const s of order) {
    if (statuses.includes(s)) return s;
  }
  return "unknown";
}
