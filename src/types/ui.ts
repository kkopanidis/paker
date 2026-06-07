export interface ConnectionNav {
  bucket?: string | null;
  prefix?: string | null;
}

export interface PrefixBookmark {
  id: string;
  label: string;
  bucket: string;
  prefix: string;
}

export interface UiPreferences {
  localPanelOpen: boolean;
  detailsPaneOpen: boolean;
  connectionsCollapsed: boolean;
  bucketsCollapsed: boolean;
  checkForUpdates: boolean;
}

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  releaseUrl: string;
  releaseName: string;
}

export type PanelLayoutMode = "three" | "four";

export type PanelLayout = Record<string, number>;

export interface FullUiState {
  lastLocalDir: Record<string, string>;
  maxConcurrentTransfers: number;
  lastNav: Record<string, ConnectionNav>;
  bookmarks: Record<string, PrefixBookmark[]>;
  preferences: UiPreferences;
  panelLayoutThree?: PanelLayout | null;
  panelLayoutFour?: PanelLayout | null;
}
