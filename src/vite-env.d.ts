/// <reference types="vite/client" />

// Tipos de las variables de entorno de Vite usadas por la app.
interface ImportMetaEnv {
  readonly VITE_API_BASE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

/** Sello del build (versión · commit · hora), inyectado por `vite.config.ts` al construir. */
declare const __SELLO_BUILD__: string;
