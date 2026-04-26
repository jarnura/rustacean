import type { FallbackProps } from "react-error-boundary";
import { Button } from "@/components/ui/button";

export function AppErrorFallback({
  error,
  resetErrorBoundary,
}: FallbackProps): JSX.Element {
  const message = error instanceof Error ? error.message : String(error);

  return (
    <div
      role="alert"
      className="flex min-h-screen items-center justify-center bg-background p-6"
    >
      <div className="w-full max-w-md rounded-lg border border-destructive/40 bg-card p-6 shadow-sm">
        <h1 className="text-lg font-semibold text-foreground">
          Something went wrong.
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">
          The application hit an unexpected error. You can try again, and if
          the problem persists, refresh the page.
        </p>
        <pre className="mt-4 max-h-40 overflow-auto rounded-md bg-muted p-3 text-xs text-muted-foreground">
          {message}
        </pre>
        <div className="mt-4 flex gap-2">
          <Button onClick={resetErrorBoundary}>Try again</Button>
          <Button variant="outline" onClick={() => window.location.reload()}>
            Reload
          </Button>
        </div>
      </div>
    </div>
  );
}
