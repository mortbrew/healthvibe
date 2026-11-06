/** Stage band thickness as a fraction of one lane. Detail and the compact card share this. */
export function stageBandHeight(rowH: number, frac: number): number {
  const f = Math.min(0.78, Math.max(0.45, frac));
  return Math.max(10, rowH * f);
}

/** Width follows the sample. A very short block stays a rounded stub, not a hairline. */
export function stageBarWidth(spanPx: number, bandH: number): number {
  return Math.max(spanPx, bandH * 0.42);
}

/** Stadium end cap. */
export function capsuleRadius(width: number, height: number): number {
  return Math.min(Math.max(width, 0), Math.max(height, 0)) / 2;
}

/** Translucent connector thickness, wide enough to read on a phone. */
export function connectorWidth(bandH: number): number {
  return Math.max(4, bandH * 0.28);
}

export const CONNECTOR_OPACITY = 0.5;

/** Week-column segment height. Keep a visible sliver and a small gap between stages. */
export function weekSegmentHeight(spanPx: number): number {
  return Math.max(3, spanPx - 1.6);
}

export type HypnogramVariant = "detail" | "compact";

export function hypnogramFrame(variant: HypnogramVariant) {
  if (variant === "compact") {
    return {
      plotW: 360,
      H: 128,
      pad: { l: 2, r: 2, t: 6, b: 6 },
      bandFrac: 0.72,
      showLabels: false,
      showAxis: false,
    };
  }
  return {
    plotW: 360,
    H: 300,
    pad: { l: 56, r: 8, t: 10, b: 28 },
    bandFrac: 0.62,
    showLabels: true,
    showAxis: true,
  };
}
