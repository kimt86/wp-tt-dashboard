import { useEffect, useRef } from "react";
import {
  Chart,
  LineController,
  LineElement,
  PointElement,
  LinearScale,
  CategoryScale,
  Filler,
  Tooltip,
} from "chart.js";

Chart.register(LineController, LineElement, PointElement, LinearScale, CategoryScale, Filler, Tooltip);
Chart.defaults.font.family = "Inter";
Chart.defaults.font.size = 10;
Chart.defaults.color = "#94a3b8";
Chart.defaults.borderColor = "#1f2940";

interface LineProps {
  values: number[];
  labels?: string[];
  color?: string;
  baseline?: number | null;
  target?: number | null;
  axes?: boolean;
  fill?: boolean;
}

/** Minimal Chart.js line wrapper handling the canvas lifecycle. */
export function LineChart({ values, labels, color = "#60a5fa", baseline, target, axes = false, fill = true }: LineProps) {
  const ref = useRef<HTMLCanvasElement>(null);
  const chart = useRef<Chart | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    const datasets: any[] = [
      {
        data: values,
        borderColor: color,
        backgroundColor: fill ? color + "22" : "transparent",
        fill,
        tension: 0.3,
        pointRadius: 0,
        borderWidth: 2,
      },
    ];
    const refLine = (val: number, c: string) => ({
      data: values.map(() => val),
      borderColor: c,
      borderDash: [4, 3],
      borderWidth: 1,
      pointRadius: 0,
      fill: false,
    });
    if (baseline != null) datasets.push(refLine(baseline, "#64748b"));
    if (target != null) datasets.push(refLine(target, "#22c55e"));

    chart.current = new Chart(ref.current, {
      type: "line",
      data: { labels: labels ?? values.map(() => ""), datasets },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { display: false }, tooltip: { enabled: axes } },
        scales: {
          x: { display: axes, grid: { display: false } },
          y: { display: axes },
        },
        animation: false,
      },
    });
    return () => chart.current?.destroy();
  }, [JSON.stringify(values), JSON.stringify(labels), color, baseline, target, axes, fill]);

  return <canvas ref={ref} />;
}
