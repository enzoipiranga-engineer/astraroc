import { useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

export interface Series {
  label: string;
  color: string;
  t: number[];
  y: number[];
}

/** Junta séries com tempos diferentes numa grade comum; cada série é contínua no seu intervalo. */
function align(series: Series[]): uPlot.AlignedData {
  const ts = [...new Set(series.flatMap((s) => s.t))].sort((a, b) => a - b);
  const index = new Map(ts.map((t, i) => [t, i]));
  const ys = series.map((s) => {
    const col: (number | null)[] = new Array(ts.length).fill(null);
    s.t.forEach((t, k) => (col[index.get(t)!] = Number.isFinite(s.y[k]!) ? s.y[k]! : null));
    return col;
  });
  return [ts, ...ys] as uPlot.AlignedData;
}

export function Chart({ title, unit, series, cursorSync, onPick }: { title: string; unit: string; series: Series[]; cursorSync: string; onPick?: ((t: number) => void) | undefined }) {
  const el = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const host = el.current;
    if (!host || series.length === 0) return;
    const opts: uPlot.Options = {
      title,
      width: host.clientWidth || 600,
      height: 220,
      cursor: { sync: { key: cursorSync } },
      scales: { x: { time: false } },
      axes: [
        { stroke: "#8296ad", grid: { stroke: "rgba(130,150,173,0.12)" }, label: "t (s)" },
        { stroke: "#8296ad", grid: { stroke: "rgba(130,150,173,0.12)" }, label: unit, size: 60 },
      ],
      series: [{ label: "t (s)" }, ...series.map((s) => ({ label: s.label, stroke: s.color, width: 1.6, spanGaps: true }))],
    };
    const plot = new uPlot(opts, align(series), host);
    const click = () => {
      const left = plot.cursor.left;
      if (onPick && left !== undefined && left >= 0) onPick(plot.posToVal(left, "x"));
    };
    plot.over.addEventListener("click", click);
    const ro = new ResizeObserver(() => plot.setSize({ width: host.clientWidth, height: 220 }));
    ro.observe(host);
    return () => {
      ro.disconnect();
      plot.over.removeEventListener("click", click);
      plot.destroy();
    };
  }, [title, unit, series, cursorSync, onPick]);
  return <div className="chart" ref={el} />;
}
