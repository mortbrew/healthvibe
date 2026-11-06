import { useEffect, useRef, useState } from "react";
import { DAY_SETTLE_MS, dayDirection } from "./day-nav";

export type DaySlide = {
  phase: "idle" | "run";
  from: string;
  to: string;
  dir: -1 | 0 | 1;
};

const idle = (date: string): DaySlide => ({ phase: "idle", from: date, to: date, dir: 0 });

/**
 * Slide from the last settled day to the newest requested day.
 * A newer request before the timer fires drops the in-between days.
 * Reduced motion snaps to the requested day and skips the slide.
 */
export function useDaySlide(date: string, reduced: boolean): DaySlide {
  const shown = useRef(date);
  const [slide, setSlide] = useState<DaySlide>(idle(date));

  useEffect(() => {
    if (reduced) {
      shown.current = date;
      setSlide(idle(date));
      return;
    }
    if (date === shown.current) return;
    const from = shown.current;
    setSlide({ phase: "run", from, to: date, dir: dayDirection(from, date) });
    const timer = window.setTimeout(() => {
      shown.current = date;
      setSlide(idle(date));
    }, DAY_SETTLE_MS);
    return () => window.clearTimeout(timer);
  }, [date, reduced]);

  return slide;
}
