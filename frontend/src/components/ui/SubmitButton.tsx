// REQ-FE-02: primary submit button with loading state.
import type { ButtonHTMLAttributes } from "react";

type SubmitButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly isLoading?: boolean;
  readonly loadingLabel?: string;
};

export function SubmitButton({
  isLoading = false,
  loadingLabel,
  disabled,
  children,
  ...rest
}: SubmitButtonProps): JSX.Element {
  const label = isLoading ? loadingLabel ?? "Working…" : children;
  return (
    <button
      type="submit"
      className="auth-button"
      disabled={isLoading || disabled === true}
      aria-busy={isLoading ? "true" : "false"}
      {...rest}
    >
      {label}
    </button>
  );
}
