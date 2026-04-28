interface StatusBadgeProps {
  readonly status: string;
}

export function StatusBadge({ status }: StatusBadgeProps): JSX.Element {
  const colors =
    status === "connected"
      ? "border-green-500/30 bg-green-50 text-green-700 dark:bg-green-950 dark:text-green-300"
      : status === "ingesting"
        ? "border-blue-500/30 bg-blue-50 text-blue-700 dark:bg-blue-950 dark:text-blue-300"
        : "border-border bg-muted text-muted-foreground";
  return (
    <span
      className={`inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium ${colors}`}
    >
      {status}
    </span>
  );
}
