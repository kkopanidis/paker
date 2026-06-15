export interface IndexQuery {
  keyPattern?: string;
  prefix?: string;
  minSize?: number;
  maxSize?: number;
  modifiedAfter?: string;
  modifiedBefore?: string;
  storageClass?: string[];
  limit?: number;
  offset?: number;
}

export type ParseConfidence = "high" | "medium" | "low";

export interface ParsedAssistantQuery {
  query: IndexQuery;
  summary: string;
  confidence: ParseConfidence;
}

export interface AssistantModelStatus {
  llmFeatureEnabled: boolean;
  parserLoaded: boolean;
  modelsDir: string;
  parserPath?: string;
  parserPresent: boolean;
  parserRecommendedFilename: string;
  embedPath?: string;
  embedPresent: boolean;
  embedRecommendedFilename: string;
  hint: string;
}

export interface PrefixStat {
  prefix: string;
  objectCount: number;
  totalBytes: number;
}

export interface BucketReport {
  totalObjects: number;
  totalBytes: number;
  topPrefixesByBytes: PrefixStat[];
  glacierObjectCount: number;
  glacierBytes: number;
  smallFileCount: number;
  smallFileThresholdBytes: number;
}

export interface ErrorExplanation {
  code: string;
  message: string;
  userAction?: string;
  detail: string;
}

export interface CliGenerateInput {
  tool?: string;
  connectionId: string;
  connectionName?: string;
  endpoint?: string;
  bucket: string;
  prefix?: string;
  keys: string[];
}

export interface CliCommandSuggestion {
  tool: string;
  command: string;
  description: string;
}

export interface QueryHistoryItem {
  id: number;
  rawText: string;
  summary: string;
  confidence: ParseConfidence;
  resultCount: number;
  createdAt: string;
}

export type ExportFormat = "csv" | "json" | "clipboard";

export type ActionKind = "deleteByQuery" | "renamePattern" | "syncPlan";

export type ProposalStatus = "pending" | "executed" | "rejected";

export type SyncMode = "addOnly" | "mirror";

export interface ProposalItem {
  key: string;
  sizeBytes: number;
  storageClass?: string;
  actionDescription: string;
  metadata?: Record<string, string>;
}

export interface ActionProposal {
  id: string;
  kind: ActionKind;
  connectionId: string;
  bucket: string;
  previewItems: ProposalItem[];
  totalAffected: number;
  totalBytes: number;
  warnings: string[];
  token: string;
  expiresAt: string;
  cliSuggestions?: CliCommandSuggestion[];
}

export interface PartialError {
  key: string;
  message: string;
}

export interface ExecutionResult {
  proposalId: string;
  kind: ActionKind;
  objectsAffected: number;
  bytesAffected: number;
  errors: PartialError[];
}

export interface ProposalEntry {
  id: string;
  kind: ActionKind;
  connectionId: string;
  bucket: string;
  payload: unknown;
  token: string;
  status: ProposalStatus;
  createdAt: string;
  expiresAt: string;
}

export interface DeleteByQueryInput {
  connectionId: string;
  bucket: string;
  query: IndexQuery;
  dryRun: boolean;
}

export interface RenamePatternInput {
  connectionId: string;
  bucket: string;
  sourcePattern: string;
  destTemplate: string;
  copyOnly: boolean;
  query?: IndexQuery;
}

export interface SyncPlanInput {
  connectionId: string;
  bucket: string;
  sourcePrefix: string;
  destPrefix: string;
  mode: SyncMode;
  generateCli: boolean;
}

export type BuildProposalInput =
  | ({ kind: "deleteByQuery" } & DeleteByQueryInput)
  | ({ kind: "renamePattern" } & RenamePatternInput)
  | ({ kind: "syncPlan" } & SyncPlanInput);
