// Vitest setup, shared by every spec (wired in vite.config.ts `test.setupFiles`).
// - jest-dom matchers (`toBeInTheDocument`, …) augment Vitest's `expect`.
// - jsdom has no ResizeObserver, which `useAdaptiveBucketCount` constructs on
//   mount; a no-op stub lets the measure fall back to its default bucket count.
// - RTL teardown after each test keeps the jsdom document clean between specs.
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

class ResizeObserverStub {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

globalThis.ResizeObserver = globalThis.ResizeObserver ?? (ResizeObserverStub as unknown as typeof ResizeObserver);

afterEach(() => {
  cleanup();
});
