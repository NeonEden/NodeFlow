import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

/**
 * Actualizaciones de la app de escritorio.
 *
 * La app se busca sola contra el manifiesto firmado del release de GitHub; si hay una versión nueva
 * la descarga, **verifica la firma** y se reinicia. Sin esto, cada cambio obligaba a cerrar la app,
 * instalar a mano el `.exe` y —como todos los builds se llamaban «0.3.0»— adivinar cuál estaba
 * corriendo: pasó hoy y costó un diagnóstico entero.
 */

export type EstadoUpdater = 'inactivo' | 'buscando' | 'al-dia' | 'disponible' | 'instalando' | 'error' | 'sin-tauri';

export interface ResultadoUpdater {
  estado: EstadoUpdater;
  version?: string;
  notas?: string;
  detalle?: string;
}

/** En el navegador (Vite) no hay updater: sólo existe dentro de la app instalada. */
const enTauri = (): boolean => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** El sello del build en el título de la ventana: es lo que responde «¿qué build corro?». */
export async function fijarTitulo(texto: string): Promise<void> {
  document.title = texto;                     // navegador / respaldo
  if (!enTauri()) return;
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().setTitle(texto); // Tauri fija el suyo: hay que pedirlo por la API
    console.info(`[sello] título de la ventana: ${texto}`);
  } catch (e) {
    // En Tauri v2 cada API necesita su permiso: si falta, esto falla en silencio y el título queda
    // en «NodeFlow». Se avisa fuerte para no volver a adivinar por qué el sello no aparece.
    console.warn('[sello] no se pudo fijar el título de la ventana:', e);
  }
}

/** ¿Hay una versión nueva publicada? No descarga nada. */
export async function buscarActualizacion(): Promise<ResultadoUpdater> {
  if (!enTauri()) return { estado: 'sin-tauri', detalle: 'Sólo la app instalada busca actualizaciones.' };
  try {
    const u: Update | null = await check();
    if (!u) return { estado: 'al-dia' };
    return { estado: 'disponible', version: u.version, notas: (u.body ?? '').slice(0, 300) };
  } catch (e) {
    return { estado: 'error', detalle: String((e as Error)?.message ?? e) };
  }
}

/** Descarga, verifica la firma e instala. Al terminar reinicia la app (el binario cambió). */
export async function instalarActualizacion(
  alProgreso?: (bajado: number, total: number | null) => void
): Promise<ResultadoUpdater> {
  if (!enTauri()) return { estado: 'sin-tauri' };
  try {
    const u = await check();
    if (!u) return { estado: 'al-dia' };
    let bajado = 0;
    await u.downloadAndInstall((ev) => {
      if (ev.event === 'Started') alProgreso?.(0, ev.data.contentLength ?? null);
      else if (ev.event === 'Progress') {
        bajado += ev.data.chunkLength ?? 0;
        alProgreso?.(bajado, null);
      } else if (ev.event === 'Finished') alProgreso?.(bajado, bajado);
    });
    await relaunch();
    return { estado: 'instalando', version: u.version };
  } catch (e) {
    return { estado: 'error', detalle: String((e as Error)?.message ?? e) };
  }
}
