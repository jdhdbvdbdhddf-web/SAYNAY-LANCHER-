export const fmtMb = (mb?: number | null): string =>
  mb == null ? "Unavailable" : mb >= 1024 ? `${(mb / 1024).toFixed(mb >= 10240 ? 0 : 1)} GB` : `${Math.round(mb)} MB`;

export const fmtNum = (n?: number | null, digits = 1): string => (n == null ? "n/a" : n.toFixed(digits));

export const profileName = (id: string): string =>
  ({ potato: "Potato PC", low: "Low-End PC", balanced: "Balanced PC", high: "High-End PC", max: "Maximum Performance" } as Record<string, string>)[id] ?? id;

export const when = (unix: number): string => new Date(unix * 1000).toLocaleString();
