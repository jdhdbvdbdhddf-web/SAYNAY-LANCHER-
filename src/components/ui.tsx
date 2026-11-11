import type { ButtonHTMLAttributes, ReactNode } from "react";

type BtnProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "ghost" | "danger"; busy?: boolean; small?: boolean };

export function Button({ variant = "ghost", busy, small, className, disabled, children, ...rest }: BtnProps) {
  const cls = ["btn", variant === "ghost" ? "" : variant, small ? "small" : "", className ?? ""].join(" ").trim();
  return (
    <button {...rest} className={cls} disabled={disabled || busy} aria-busy={busy ? true : undefined}>
      {children}
    </button>
  );
}

export function Panel({ title, aside, children, className }: { title?: string; aside?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`panel ${className ?? ""}`}>
      {(title || aside) && (
        <div className="panel-head">
          {title && <h2>{title}</h2>}
          {aside}
        </div>
      )}
      {children}
    </section>
  );
}

export function Notice({ kind = "info", children }: { kind?: "info" | "error" | "warn" | "good"; children: ReactNode }) {
  return (
    <div className={`notice ${kind}`} role={kind === "error" ? "alert" : undefined}>
      {children}
    </div>
  );
}

export function Meter({ value, max, label }: { value: number; max: number; label?: string }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100} aria-label={label}>
      <div className="meter">
        <i style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <span className="hint">{hint}</span>}
    </label>
  );
}

export function PageHead({ title, text, aside }: { title: string; text?: string; aside?: ReactNode }) {
  return (
    <div className="page-head">
      <div>
        <h1>{title}</h1>
        {text && <p>{text}</p>}
      </div>
      {aside}
    </div>
  );
}

export function NoInstance({ onCreate }: { onCreate: () => void }) {
  return (
    <Notice kind="warn">
      <div className="row between">
        <span>Create a Minecraft instance first. Nothing here works without one.</span>
        <Button variant="primary" small onClick={onCreate}>
          Create instance
        </Button>
      </div>
    </Notice>
  );
}
