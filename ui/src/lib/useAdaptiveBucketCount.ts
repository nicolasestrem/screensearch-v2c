import { useEffect, useRef, useState } from "react";

export function useAdaptiveBucketCount(
  fallback: number,
  min: number,
  max: number,
  pxPerBucket: number,
) {
  const ref = useRef<HTMLDivElement>(null);
  const [count, setCount] = useState(fallback);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const next = Math.round(el.clientWidth / pxPerBucket);
      setCount(Math.min(max, Math.max(min, next || fallback)));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [fallback, max, min, pxPerBucket]);

  return [ref, count] as const;
}
