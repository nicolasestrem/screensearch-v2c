// TimelineMinimap — the thin, read-only reuse of the Scanline ribbon (UI_REFERENCE
// §1/§5: "Reused as a thin minimap strip"). Shares drawDensityRibbon with the full
// ScanlineTimeline, so density reads identically. Click (or Enter) seeks: it reports
// the time under the cursor so the host can jump into the full timeline there.
import { useEffect, useRef, useState } from "react";

import type { TimelineBucket } from "../../bindings/TimelineBucket";
import type { TimeRange } from "../../bindings/TimeRange";
import { drawDensityRibbon, readRibbonColors } from "./timelineDraw";

export interface TimelineMinimapProps {
  buckets: TimelineBucket[];
  range: TimeRange;
  /** Click / Enter reports the time under the cursor (defaults to the window end). */
  onSeek?: (time: number) => void;
  height?: number;
}

export function TimelineMinimap({ buckets, range, onSeek, height = 28 }: TimelineMinimapProps) {
  const wrapRef = useRef<HTMLButtonElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);
  const span = Math.max(0, range.end - range.start);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    const wrap = wrapRef.current;
    if (!canvas || !wrap || width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    drawDensityRibbon(ctx, { width, height, dpr, buckets, range, colors: readRibbonColors(wrap) });
  }, [width, height, buckets, range]);

  const seekFromX = (clientX: number) => {
    const el = wrapRef.current;
    if (!el || span === 0) return onSeek?.(range.end - 1);
    const rect = el.getBoundingClientRect();
    const frac = Math.min(Math.max((clientX - rect.left) / rect.width, 0), 1);
    onSeek?.(Math.round(range.start + frac * span));
  };

  return (
    <button
      ref={wrapRef}
      type="button"
      aria-label="Open the timeline"
      onClick={(e) => seekFromX(e.clientX)}
      className="relative block w-full overflow-hidden rounded-none bg-base"
      style={{ height }}
    >
      <canvas ref={canvasRef} className="block h-full w-full" />
      <span className="scanlines pointer-events-none absolute inset-0" aria-hidden="true" />
    </button>
  );
}
