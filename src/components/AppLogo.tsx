export function AppLogo(props: { size: number }) {
  return (
    <svg width={props.size} height={props.size} viewBox="0 0 1024 1024" aria-hidden="true">
      <circle cx="512" cy="512" r="70" fill="currentColor" />
      <g fill="none" stroke="currentColor" strokeWidth="60" strokeLinecap="round">
        <path d="M632 392A170 170 0 0 1 632 632" />
        <path d="M392 392A170 170 0 0 0 392 632" />
        <path d="M724 300A300 300 0 0 1 724 724" />
        <path d="M300 300A300 300 0 0 0 300 724" />
      </g>
    </svg>
  );
}
