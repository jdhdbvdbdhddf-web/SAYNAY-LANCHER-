export default function Logo({ size = 38 }: { size?: number }) {
  return (
    <svg viewBox="0 0 256 256" width={size} height={size} role="img" aria-label="SAYNAY">
      <defs>
        <linearGradient id="sg" x1="30" y1="20" x2="226" y2="236" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#a78bfa" />
          <stop offset="0.55" stopColor="#6366f1" />
          <stop offset="1" stopColor="#38bdf8" />
        </linearGradient>
      </defs>
      <rect width="256" height="256" rx="58" fill="#070916" />
      <g fill="none" stroke="url(#sg)" strokeLinejoin="round" strokeLinecap="round">
        <polygon points="128,22 220,75 220,181 128,234 36,181 36,75" strokeWidth="8" />
        <path d="M176 80H108L84 104L108 128H148L172 152L148 176H80" strokeWidth="16" />
      </g>
    </svg>
  );
}
