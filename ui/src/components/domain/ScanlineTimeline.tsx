// ScanlineTimeline — the signature instrument (UI_REFERENCE §1/§7). A canvas
// density ribbon (drawn by the shared drawDensityRibbon) under a sweeping
// signal-orange scan-head, with a faint scanline texture and hover thumbnails. It
// is a real slider: role="slider" with valuemin/max/now/text, full keyboard scrub
// (Arrows step, Shift = coarse, Home/End jump, Enter opens), and pointer drag. The
// scan-head and texture are DOM overlays (crisp accent + token glow); only the
// density is canvas. devicePixelRatio-crisp; ambient drift is gated by
// prefers-reduced-motion in globals.css.
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";

import type { TimelineBucket } from "../../bindings/TimelineBucket";
import type { TimeRange } from "../../bindings/TimeRange";
import type { FrameMeta } from "../../bindings/FrameMeta";
import { FrameImage } from "./FrameImage";
import { drawDensityRibbon, readRibbonColors } from "./timelineDraw";
import { absoluteTime, clockTime } from "../../lib/time";

export interface ScanlineTimelineProps {
  buckets: TimelineBucket[];
  range: TimeRange;
  /** The focused moment (unix ms); the scan-head sits here. Controlled. */
  position: number;
  /** User moved the head (drag / click / keyboard step). */
  onScrub: (position: number) => void;
  /** User committed (Enter / double-click) — open the moment under the head. */
  onOpen: (position: number) => void;
  /** Frames for the hover thumbnail preview (nearest to the cursor). */
  thumbnails?: FrameMeta[];
  /** Ribbon height in CSS px. */
  height?: number;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

export function ScanlineTimeline({
  buckets,
  range,
  position,
  onScrub,
  onOpen,
  thumbnails = [],
  height = 96,
}: ScanlineTimelineProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dragging = useRef(false);
  const [size, setSize] = useState({ w: 0, h: height });
  const [hover, setHover] = useState<{ x: number; time: number } | null>(null);

  const span = Math.max(0, range.end - range.start);

  // Track the container's pixel box so the canvas can be redrawn crisply on resize.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Draw the density ribbon whenever the data or the box changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container || size.w === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(size.w * dpr);
    canvas.height = Math.round(size.h * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    drawDensityRibbon(ctx, {
      width: size.w,
      height: size.h,
      dpr,
      buckets,
      range,
      colors: readRibbonColors(container),
    });
  }, [size, buckets, range]);

  const xToTime = (clientX: number): number => {
    const el = containerRef.current;
    if (!el || span === 0) return range.start;
    const rect = el.getBoundingClientRect();
    const frac = clamp((clientX - rect.left) / rect.width, 0, 1);
    return Math.round(range.start + frac * span);
  };

  const positionPct = span === 0 ? 0 : (clamp(position, range.start, range.end) - range.start) / span;

  // Nearest frame to a given time, for the hover preview.
  const nearestThumb = (time: number): FrameMeta | null => {
    let best: FrameMeta | null = null;
    let bestD = Infinity;
    for (const f of thumbnails) {
      const d = Math.abs(f.captured_at - time);
      if (d < bestD) {
        bestD = d;
        best = f;
      }
    }
    return best;
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    dragging.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
    containerRef.current?.focus();
    onScrub(xToTime(e.clientX));
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const el = containerRef.current;
    if (el) {
      const rect = el.getBoundingClientRect();
      setHover({ x: e.clientX - rect.left, time: xToTime(e.clientX) });
    }
    if (dragging.current) onScrub(xToTime(e.clientX));
  };
  const endDrag = (e: ReactPointerEvent<HTMLDivElement>) => {
    dragging.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (span === 0) return;
    const step = Math.max(1, Math.round(span / 240));
    const bigStep = Math.max(step, Math.round(span / 24));
    let next: number | null = null;
    switch (e.key) {
      case "ArrowRight":
        next = clamp(position + (e.shiftKey ? bigStep : step), range.start, range.end);
        break;
      case "ArrowLeft":
        next = clamp(position - (e.shiftKey ? bigStep : step), range.start, range.end);
        break;
      case "Home":
        next = range.start;
        break;
      case "End":
        next = range.end - 1;
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        onOpen(position);
        return;
      default:
        return;
    }
    e.preventDefault();
    if (next !== null) onScrub(next);
  };

  const hoverFrame = hover ? nearestThumb(hover.time) : null;

  return (
    <div
      ref={containerRef}
      role="slider"
      tabIndex={0}
      aria-label="Capture timeline — arrow keys scrub, Enter opens the moment"
      aria-valuemin={range.start}
      aria-valuemax={range.end}
      aria-valuenow={clamp(position, range.start, range.end)}
      aria-valuetext={absoluteTime(position)}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onPointerLeave={() => {
        setHover(null);
      }}
      onDoubleClick={(e) => onOpen(xToTime(e.clientX))}
      onKeyDown={onKeyDown}
      className="relative w-full cursor-crosshair select-none overflow-hidden rounded-none bg-base outline-none"
      style={{ height }}
    >
      <canvas ref={canvasRef} className="block h-full w-full" />

      {/* Faint scanline texture — the subject's native material; drift is disabled
          under prefers-reduced-motion (globals.css). */}
      <div className="scanlines scanlines-drift pointer-events-none absolute inset-0" aria-hidden="true" />

      {/* The sweeping signal-orange scan-head + its time read-out. */}
      <div
        className="pointer-events-none absolute bottom-0 top-0 w-0.5 bg-accent shadow-scan"
        style={{ left: `${positionPct * 100}%` }}
        aria-hidden="true"
      >
        <span className="absolute -top-px left-1 whitespace-nowrap rounded-chip bg-overlay px-1 font-mono text-data text-accent">
          {clockTime(position)}
        </span>
      </div>

      {/* Hover thumbnail preview (nearest frame to the cursor). */}
      {hover && hoverFrame && (
        <div
          className="pointer-events-none absolute bottom-full z-rail mb-2 w-40 -translate-x-1/2"
          style={{ left: clamp(hover.x, 80, Math.max(80, size.w - 80)) }}
          aria-hidden="true"
        >
          <FrameImage
            imagePath={hoverFrame.image_path}
            alt=""
            className="aspect-video w-full rounded-chip border border-line object-cover bg-overlay"
          />
          <span className="mt-1 block text-center font-mono text-data text-ink-muted">
            {clockTime(hoverFrame.captured_at)}
          </span>
        </div>
      )}
    </div>
  );
}
