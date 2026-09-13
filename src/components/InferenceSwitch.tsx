import { useEffect, useState } from 'react';
import { Cpu, Cloud, Shuffle } from 'lucide-react';

/**
 * Fase 11 — Interruptor de inferencia por tarea.
 *
 * El ruteo es determinista y vive en el backend (`cadena_por_modo`, ver ADR 0005): acá sólo se elige.
 * - Local: sólo el modelo local. Sin cuota, sin red, costo cero.
 * - Auto: la cadena configurada (local primero, nube como respaldo).
 * - Nube: razonamiento profundo sobre varias ramas del grafo.
 *
 * Después de cada acción de IA se muestra la traza real que devolvió el backend
 * (proveedor, latencia, tokens, costo y si salió de la caché), no una estimación de la UI.
 */

export type ModoInferencia = 'local' | 'auto' | 'nube';

export const CLAVE_MODO = 'nodeflow_modo_inferencia';

export function leerModo(): ModoInferencia {
  try {
    const v = localStorage.getItem(CLAVE_MODO);
    return v === 'local' || v === 'nube' ? v : 'auto';
  } catch {
    return 'auto';
  }
}

export function fijarModo(m: ModoInferencia) {
  try {
    localStorage.setItem(CLAVE_MODO, m);
  } catch {
    /* almacenamiento restringido: el modo vale sólo para esta sesión */
  }
  window.dispatchEvent(new CustomEvent('nodeflow:modo', { detail: m }));
}

interface Traza {
  modo: string;
  proveedor: string;
  modelo?: string;
  ms: number;
  tokens: number;
  tokens_evitados: number;
  costo_usd: number;
  cache: string;
}

const OPCIONES: { id: ModoInferencia; etiqueta: string; titulo: string; icono: any }[] = [
  { id: 'local', etiqueta: 'Local', titulo: 'Sólo el modelo local: sin cuota, sin red, costo cero', icono: Cpu },
  { id: 'auto', etiqueta: 'Auto', titulo: 'Cadena configurada: local primero y nube como respaldo', icono: Shuffle },
  { id: 'nube', etiqueta: 'Nube', titulo: 'Proveedor en la nube: razonamiento profundo sobre varias ramas', icono: Cloud },
];

const ACTIVO: Record<ModoInferencia, string> = {
  local: 'bg-emerald-950/40 text-emerald-200 border-emerald-700/60',
  auto: 'bg-slate-800 text-slate-100 border-slate-600',
  nube: 'bg-sky-950/40 text-sky-200 border-sky-700/60',
};

export function InferenceSwitch() {
  const [modo, setModo] = useState<ModoInferencia>(leerModo());
  const [traza, setTraza] = useState<Traza | null>(null);

  useEffect(() => {
    const alCambiar = (e: Event) => setModo((e as CustomEvent).detail as ModoInferencia);
    const alCorrer = (e: Event) => setTraza((e as CustomEvent).detail as Traza);
    window.addEventListener('nodeflow:modo', alCambiar);
    window.addEventListener('nodeflow:traza', alCorrer);
    return () => {
      window.removeEventListener('nodeflow:modo', alCambiar);
      window.removeEventListener('nodeflow:traza', alCorrer);
    };
  }, []);

  const elegir = (m: ModoInferencia) => {
    setModo(m);
    fijarModo(m);
  };

  const costo = traza ? (traza.costo_usd > 0 ? `$${traza.costo_usd.toFixed(4)}` : '$0') : '';
  const etiquetaTraza = traza
    ? `${traza.proveedor} · ${traza.ms} ms · ${traza.tokens} tok · ${costo}${traza.cache === 'hit' ? ' · caché HIT' : ''}`
    : '';

  return (
    <div className="flex items-center gap-1.5">
      <div
        id="switch-inferencia"
        className="flex items-center bg-slate-900/70 border border-slate-800 rounded-xl p-0.5"
        title="Dónde corre la inferencia de esta tarea"
      >
        {OPCIONES.map((o) => {
          const Icono = o.icono;
          const activo = modo === o.id;
          return (
            <button
              key={o.id}
              type="button"
              onClick={() => elegir(o.id)}
              title={o.titulo}
              aria-pressed={activo}
              className={`flex items-center gap-1 px-2 py-1 rounded-[10px] text-[11px] font-medium transition-colors cursor-pointer border ${
                activo ? ACTIVO[o.id] : 'bg-transparent text-slate-400 border-transparent hover:text-slate-200'
              }`}
            >
              <Icono size={12} />
              <span className="hidden md:inline">{o.etiqueta}</span>
            </button>
          );
        })}
      </div>

      {traza && (
        <span
          id="traza-inferencia"
          className={`hidden lg:inline text-[10px] font-mono px-2 py-1 rounded-xl border ${
            traza.cache === 'hit'
              ? 'text-slate-400 border-slate-800 bg-slate-900/70'
              : 'text-slate-300 border-slate-700 bg-slate-900/70'
          }`}
          title="Traza real de la última corrida: proveedor, latencia, tokens, costo y caché"
        >
          {etiquetaTraza}
        </span>
      )}
    </div>
  );
}
