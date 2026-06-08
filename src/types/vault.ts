export interface VaultStatus {
  enabled: boolean;
  locked: boolean;
  setupRequired: boolean;
  autoLockMinutes: number;
  lockOnBlur: boolean;
  recoveryAvailable: boolean;
  unlockBlockedSecs: number;
}

export interface SetupVaultInput {
  masterPassword: string;
  autoLockMinutes?: number;
  lockOnBlur?: boolean;
}

export interface ChangeMasterKeyInput {
  currentPassword: string;
  newPassword: string;
}

export interface SetVaultPreferencesInput {
  autoLockMinutes: number;
  lockOnBlur: boolean;
}
