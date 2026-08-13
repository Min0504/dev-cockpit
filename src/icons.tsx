// Minimal inline icon set (stroke icons, currentColor).
interface IconProps {
  size?: number;
  open?: boolean;
}

const base = (size: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.5,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
});

export const IconPlay = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M5 3.5v9l7-4.5-7-4.5Z" fill="currentColor" stroke="none" />
  </svg>
);

export const IconStop = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <rect x="4" y="4" width="8" height="8" rx="1" fill="currentColor" stroke="none" />
  </svg>
);

export const IconRestart = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M13 8a5 5 0 1 1-1.5-3.6" />
    <path d="M13 2.5V5h-2.5" />
  </svg>
);

export const IconTerminal = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M3 5l3 3-3 3" />
    <path d="M8 11h5" />
  </svg>
);

export const IconGlobe = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <circle cx="8" cy="8" r="5.5" />
    <path d="M2.5 8h11M8 2.5c-1.8 1.5-2.6 3.4-2.6 5.5S6.2 12 8 13.5c1.8-1.5 2.6-3.4 2.6-5.5S9.8 4 8 2.5Z" />
  </svg>
);

export const IconLogs = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M3 4h10M3 8h10M3 12h6" />
  </svg>
);

export const IconFolder = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.5 1.7h4.9A1.5 1.5 0 0 1 14 6.2v5.3a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5v-7Z" />
  </svg>
);

export const IconCode = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M5.5 5 3 8l2.5 3M10.5 5 13 8l-2.5 3" />
  </svg>
);

export const IconGear = ({ size = 13 }: IconProps = {}) => (
  <svg {...base(size)}>
    <circle cx="8" cy="8" r="2" />
    <path d="M8 2.2v1.6M8 12.2v1.6M2.2 8h1.6M12.2 8h1.6M3.9 3.9l1.1 1.1M11 11l1.1 1.1M12.1 3.9 11 5M5 11l-1.1 1.1" />
  </svg>
);

export const IconPin = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M9.5 2.5 13.5 6.5 10 8l-1 4-5.5-5.5 4-1 2-3Z" />
    <path d="M6 10l-3.5 3.5" />
  </svg>
);

export const IconSearch = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <circle cx="7" cy="7" r="4.5" />
    <path d="m10.5 10.5 3 3" />
  </svg>
);

export const IconX = ({ size = 11 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M4 4l8 8M12 4l-8 8" />
  </svg>
);

export const IconChevron = ({ size = 11, open = false }: IconProps = {}) => (
  <svg
    {...base(size)}
    style={{ transform: open ? "rotate(90deg)" : "none", transition: "transform 120ms" }}
  >
    <path d="m6 4 4 4-4 4" />
  </svg>
);

export const IconBranch = ({ size = 11 }: IconProps = {}) => (
  <svg {...base(size)}>
    <circle cx="4.5" cy="4" r="1.6" />
    <circle cx="4.5" cy="12" r="1.6" />
    <circle cx="11.5" cy="6" r="1.6" />
    <path d="M4.5 5.6v4.8M11.5 7.6c0 2.5-4 2-6 3" />
  </svg>
);

export const IconWarn = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M8 2.8 14 13H2L8 2.8Z" />
    <path d="M8 7v2.6M8 11.6v.1" />
  </svg>
);

export const IconRefresh = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M13 8a5 5 0 1 1-1.5-3.6" />
    <path d="M13 2.5V5h-2.5" />
  </svg>
);

export const IconMore = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <circle cx="3.5" cy="8" r="0.9" fill="currentColor" stroke="none" />
    <circle cx="8" cy="8" r="0.9" fill="currentColor" stroke="none" />
    <circle cx="12.5" cy="8" r="0.9" fill="currentColor" stroke="none" />
  </svg>
);

export const IconPause = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M6 4v8M10 4v8" strokeWidth={2} />
  </svg>
);

export const IconArrowDown = ({ size = 12 }: IconProps = {}) => (
  <svg {...base(size)}>
    <path d="M8 3v10M4 9l4 4 4-4" />
  </svg>
);
