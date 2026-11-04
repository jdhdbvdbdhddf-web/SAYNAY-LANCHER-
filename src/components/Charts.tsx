import { fmtNum } from "../format";

export interface CmpRow {
  label: string;
  before: number | null | undefined;
  after: number | null | undefined;
  unit: string;
  higherBetter: boolean;
  digits?: number;
}

/** Before/after bars with the real percentage change. Rows with a missing side show "n/a", never a guess. */
export function CompareRows({ rows }: { rows: CmpRow[] }) {
  return (
    <div>
      {rows.map((r) => {
        const max = Math.max(r.before ?? 0, r.after ?? 0, 1e-9);
        const have = r.before != null && r.after != null && r.before !== 0;
        const pct = have ? (((r.after as number) - (r.before as number)) / (r.before as number)) * 100 : null;
        const good = pct == null ? false : r.higherBetter ? pct >= 0 : pct <= 0;
        const d = r.digits ?? 1;
        return (
          <div className="cmp-row" key={r.label}>
            <span>{r.label}</span>
            <div className="cmp-bars">
              <div>
                <i style={{ width: `${((r.before ?? 0) / max) * 100}%`, background: "#4a5a9c" }} />
                {fmtNum(r.before, d)} {r.before != null ? r.unit : ""}
              </div>
              <div>
                <i style={{ width: `${((r.after ?? 0) / max) * 100}%`, background: "linear-gradient(90deg,#8b5cf6,#38bdf8)" }} />
                {fmtNum(r.after, d)} {r.after != null ? r.unit : ""}
              </div>
            </div>
            <span className={`delta ${pct == null ? "" : good ? "good" : "bad"}`}>{pct == null ? "n/a" : `${pct >= 0 ? "+" : ""}${pct.toFixed(1)}%`}</span>
          </div>
        );
      })}
    </div>
  );
}

export interface Series {
  label: string;
  color: string;
  values: number[];
}

/** Frame time over the run (lower and flatter is better). Each series is stretched to the same width. */
export function FrameLine({ series }: { series: Series[] }) {
  const all = series.flatMap((s) => s.values);
  if (all.length === 0) return <p className="muted">No frame data was recorded for these runs.</p>;
  const W = 720, H = 220, L = 46, R = 12, T = 12, B = 22;
  const sorted = [...all].sort((a, b) => a - b);
  const cap = Math.max(8, sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.995))] * 1.15);
  const pts = (v: number[]) =>
    v
      .map((y, i) => {
        const x = L + (i / Math.max(1, v.length - 1)) * (W - L - R);
        const yy = T + (1 - Math.min(y, cap) / cap) * (H - T - B);
        return `${x.toFixed(1)},${yy.toFixed(1)}`;
      })
      .join(" ");
  return (
    <div>
      <svg viewBox={`0 0 ${W} ${H}`} width="100%" role="img" aria-label="Frame time chart in milliseconds">
        {[0, 0.5, 1].map((f) => {
          const y = T + (1 - f) * (H - T - B);
          return (
            <g key={f}>
              <line x1={L} x2={W - R} y1={y} y2={y} stroke="rgba(139,125,255,.18)" />
              <text x={L - 8} y={y + 4} textAnchor="end" fontSize="11" fill="#8b92bd">
                {(cap * f).toFixed(0)} ms
              </text>
            </g>
          );
        })}
        {series.map((s) => (
          <polyline key={s.label} points={pts(s.values)} fill="none" stroke={s.color} strokeWidth="1.6" strokeLinejoin="round" />
        ))}
      </svg>
      <div className="legend">
        {series.map((s) => (
          <span key={s.label}>
            <i style={{ background: s.color }} />
            {s.label}
          </span>
        ))}
      </div>
    </div>
  );
}
