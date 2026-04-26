import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  ThemeContext,
  THEME_STORAGE_KEY,
  type Theme,
  type ThemeContextValue,
} from "@/components/theme/theme-context";

function readStoredTheme(defaultTheme: Theme): Theme {
  if (typeof window === "undefined") return defaultTheme;
  const stored = window.localStorage.getItem(THEME_STORAGE_KEY) as
    | Theme
    | null;
  return stored === "light" || stored === "dark" || stored === "system"
    ? stored
    : defaultTheme;
}

function systemPrefersDark(): boolean {
  if (typeof window === "undefined") return false;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function applyThemeClass(resolved: "light" | "dark"): void {
  const root = document.documentElement;
  root.classList.toggle("dark", resolved === "dark");
  root.style.colorScheme = resolved;
}

interface ThemeProviderProps {
  children: ReactNode;
  defaultTheme?: Theme;
}

export function ThemeProvider({
  children,
  defaultTheme = "system",
}: ThemeProviderProps): JSX.Element {
  const [theme, setThemeState] = useState<Theme>(() =>
    readStoredTheme(defaultTheme),
  );
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">(() =>
    readStoredTheme(defaultTheme) === "dark" ||
    (readStoredTheme(defaultTheme) === "system" && systemPrefersDark())
      ? "dark"
      : "light",
  );

  useEffect(() => {
    const resolved: "light" | "dark" =
      theme === "system" ? (systemPrefersDark() ? "dark" : "light") : theme;
    setResolvedTheme(resolved);
    applyThemeClass(resolved);
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent): void => {
      const resolved: "light" | "dark" = event.matches ? "dark" : "light";
      setResolvedTheme(resolved);
      applyThemeClass(resolved);
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  const setTheme = useCallback((next: Theme): void => {
    window.localStorage.setItem(THEME_STORAGE_KEY, next);
    setThemeState(next);
  }, []);

  const value = useMemo<ThemeContextValue>(
    () => ({ theme, resolvedTheme, setTheme }),
    [theme, resolvedTheme, setTheme],
  );

  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
}
