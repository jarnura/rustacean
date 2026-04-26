import { Moon, Sun, SunMoon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTheme, type Theme } from "@/components/theme/theme-context";

const ORDER: Theme[] = ["light", "dark", "system"];

const LABEL: Record<Theme, string> = {
  light: "Light",
  dark: "Dark",
  system: "System",
};

export function ThemeToggle(): JSX.Element {
  const { theme, setTheme } = useTheme();

  const next = (): void => {
    const idx = ORDER.indexOf(theme);
    const nextTheme = ORDER[(idx + 1) % ORDER.length] ?? "system";
    setTheme(nextTheme);
  };

  const icon =
    theme === "light" ? (
      <Sun aria-hidden="true" />
    ) : theme === "dark" ? (
      <Moon aria-hidden="true" />
    ) : (
      <SunMoon aria-hidden="true" />
    );

  return (
    <Button
      variant="outline"
      size="sm"
      onClick={next}
      aria-label={`Theme: ${LABEL[theme]}. Click to change.`}
      title={`Theme: ${LABEL[theme]}`}
    >
      {icon}
      <span className="hidden sm:inline">{LABEL[theme]}</span>
    </Button>
  );
}
