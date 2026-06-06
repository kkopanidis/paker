export interface LocalEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  modified?: string;
}

export interface TransferSettings {
  maxConcurrentTransfers: number;
}
