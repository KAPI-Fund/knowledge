/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_KNOWLEDGE_PROJECT_ROOT?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
