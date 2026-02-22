/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL?: string;
  readonly VITE_WS_URL?: string;
  readonly VITE_DESTRUCTIVE_API_TOKEN?: string;
  readonly VITE_DESTRUCTIVE_API_SESSION_ID?: string;
  readonly VITE_DESTRUCTIVE_API_SESSION_TOKEN?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
