/** Mirrors the JSON the Rust API serves (`crates/pqcas-domain`). */

export type TaskStatus = "pending" | "running" | "finished" | "failed" | "cancelled";
export type ExecStatus = "queued" | "running" | "finished" | "error";
export type ResultStatus = "pending" | "passed" | "failed" | "unsupported" | "disabled";
export type ReportStatus = "missing" | "partial" | "complete";
export type KemRunMode = "library_compat" | "group_compat" | "priority";
export type Capability = "key_gen" | "sig_gen" | "sig_ver";

export interface Algorithm {
  id: number;
  kind: "kem_group" | "sig_alg";
  name: string;
  displayName: string;
  family: string | null;
  enabled: boolean;
  sortOrder: number;
}

export interface Endpoint {
  id: string;
  slug: string;
  label: string;
  labelZh: string;
  host: string;
  port: number;
  notes: string | null;
  enabled: boolean;
}

export interface Test {
  id: string;
  host: string;
  port: number;
  httpPath: string;
  method: string;
  tool: string;
  plugin: string;
  status: TaskStatus;
  groupCount: number;
  sigAlgCount: number;
  subtaskCount: number;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface Subtask {
  id: string;
  testId: string;
  ordinal: number;
  groupName: string;
  sigAlg: string;
  execStatus: ExecStatus;
  resultStatus: ResultStatus;
  reportStatus: ReportStatus;
  failedStage: string | null;
  errorSummary: string | null;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface TestDetail extends Test {
  subtasks: Subtask[];
}

export interface MatrixCell {
  subtaskId: string;
  groupName: string;
  sigAlg: string;
  resultStatus: ResultStatus;
  execStatus: ExecStatus;
}

export interface Matrix {
  groups: string[];
  sigAlgs: string[];
  cells: MatrixCell[];
}

export interface MessageField {
  label: string;
  values: string[];
}

export interface HandshakeMessage {
  index: number;
  direction: "outbound" | "inbound";
  kind: string;
  fields: MessageField[];
}

export interface CertificateInfo {
  subject: string;
  issuer: string;
  not_before?: string | null;
  not_after?: string | null;
  signature_algorithm?: string | null;
  public_key_algorithm?: string | null;
}

export interface TlsSummary {
  handshakeSuccess: boolean;
  tlsVersion: string | null;
  cipherSuite: string | null;
  keyExchangeGroup: string | null;
  certVerifySigAlg: string | null;
  alert: { level: string; description: string } | null;
}

export interface HttpOutcome {
  status: number;
  headers: MessageField[];
  body_excerpt?: string | null;
}

export interface SubtaskReport {
  subtaskId: string;
  adapter: string;
  tlsSummary: TlsSummary;
  messages: HandshakeMessage[];
  certChain: CertificateInfo[];
  http: HttpOutcome | null;
  diagnostics: unknown;
  raw: unknown;
  evidence: string | null;
  durationMs: number;
}

export interface SubtaskDetail extends Subtask {
  title: string;
  report: SubtaskReport | null;
  previousId: string | null;
  nextId: string | null;
  position: number;
  total: number;
}

export interface KemRun {
  id: string;
  mode: KemRunMode;
  endpointId: string;
  status: TaskStatus;
  kemGroups: string[];
  createdAt: string;
  finishedAt: string | null;
}

export interface KemRunLog {
  runId: string;
  seq: number;
  at: string;
  level: string;
  message: string;
}

export interface KemMatrixCell {
  runId: string;
  kem: string;
  adapter: string;
  verdict: ResultStatus;
  detail: string | null;
}

export interface KemRunDetail extends KemRun {
  endpoint: Endpoint;
  logs: KemRunLog[];
  cells: KemMatrixCell[];
}

export interface CavpPackSummary {
  slug: string;
  capability: Capability;
  parameterSet: string;
  vsId: number;
}

export interface CavpPack extends CavpPackSummary {
  registration: Record<string, unknown>;
  prompt: CavpPrompt;
  example: Record<string, unknown> | null;
}

export interface CavpPrompt {
  vsId: number;
  algorithm: string;
  mode: string;
  revision: string;
  isSample: boolean;
  testGroups: Array<{
    tgId: number;
    testType: string;
    parameterSet: string;
    tests: Array<Record<string, string | number>>;
  }>;
}

export interface CavpCaseResult {
  tcId: number;
  status: "correct" | "incorrect" | "malformed" | "missing";
  field: string | null;
  reason: string | null;
}

export interface CavpValidationReport {
  vsId: number;
  mode: string;
  parameterSet: string;
  total: number;
  correct: number;
  incorrect: number;
  malformed: number;
  missing: number;
  cases: CavpCaseResult[];
}

/** Events pushed over SSE while a test or KEM run is in flight. */
export type TestEvent =
  | {
      type: "subtaskUpdated";
      subtaskId: string;
      execStatus: ExecStatus;
      resultStatus: ResultStatus;
      groupName: string;
      sigAlg: string;
    }
  | { type: "testFinished"; testId: string; passed: number; failed: number };

export type KemEvent =
  | { type: "log"; seq: number; at: string; level: string; message: string }
  | { type: "cell"; kem: string; adapter: string; verdict: ResultStatus; detail: string | null }
  | { type: "finished"; runId: string };
