export type ComponentStatus = {
  ok: boolean;
  message: string;
};

export type WorkerStatus = ComponentStatus & {
  running: boolean;
  version: string | null;
};

export type DeveloperStatus = {
  application: ComponentStatus;
  database: ComponentStatus;
  worker: WorkerStatus;
};

export type WorkerPing = {
  version: string;
  elapsed_ms: number;
};
