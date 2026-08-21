export type AppSection =
  | "home"
  | "prompt-history"
  | "prompt-library"
  | "general"
  | "appearance"
  | "history"
  | "models"
  | "transcribe"
  | "advanced"
  | "postprocessing"
  | "debug"
  | "about";

export interface SettingsSectionProps {
  onNavigate?: (section: AppSection) => void;
}
