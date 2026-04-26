// REQ-FE-02: shared input field with inline label and error rendering.
// Wrapper passes refs through so it composes with react-hook-form's `register`.
import { forwardRef } from "react";
import type { InputHTMLAttributes } from "react";

type FieldProps = InputHTMLAttributes<HTMLInputElement> & {
  readonly label: string;
  readonly error?: string | undefined;
  readonly helperText?: string | undefined;
};

export const Field = forwardRef<HTMLInputElement, FieldProps>(function Field(
  { label, error, helperText, id, ...inputProps },
  ref,
) {
  const inputId = id ?? inputProps.name ?? label.replace(/\s+/g, "-").toLowerCase();
  const describedByIds: string[] = [];
  const errorId = `${inputId}-error`;
  const helperId = `${inputId}-helper`;
  if (error) {
    describedByIds.push(errorId);
  } else if (helperText) {
    describedByIds.push(helperId);
  }
  const describedBy = describedByIds.length > 0 ? describedByIds.join(" ") : undefined;

  return (
    <div className="auth-field">
      <label htmlFor={inputId} className="auth-field__label">
        {label}
      </label>
      <input
        ref={ref}
        id={inputId}
        aria-invalid={error ? "true" : "false"}
        {...(describedBy !== undefined ? { "aria-describedby": describedBy } : {})}
        className="auth-field__input"
        {...inputProps}
      />
      {error ? (
        <p id={errorId} role="alert" className="auth-field__error">
          {error}
        </p>
      ) : helperText ? (
        <p id={helperId} className="auth-field__helper">
          {helperText}
        </p>
      ) : null}
    </div>
  );
});
