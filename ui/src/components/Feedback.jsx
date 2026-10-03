export function Spinner({ label = 'Loading…' }) {
  return <p className="muted">⏳ {label}</p>;
}

export function ErrorBanner({ message }) {
  if (!message) return null;
  return <div className="banner banner-error">⚠ {message}</div>;
}

export function SuccessBanner({ message }) {
  if (!message) return null;
  return <div className="banner banner-success">✓ {message}</div>;
}

export function EmptyState({ message }) {
  return <p className="muted empty">{message}</p>;
}
