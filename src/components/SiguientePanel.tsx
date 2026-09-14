import React, { useEffect, useState } from 'react';
import { X, Compass, Ban, ListChecks, ArrowRight, RefreshCw } from 'lucide-react';
import { apiUrl } from '../services/apiBase';

/**
 * "Lo que sigue" — el camino crítico del mapa del proyecto.
 *
 * Es la vista para la que existe todo lo demás: con las aristas semánticas (`requiere`, `bloquea`)
 * el "¿qué sigue?" se **consulta** en vez de intuirse. Tres lecturas, en orden de urgencia:
 * qué está frenado, qué le falta a cada capacidad, y cuál es la próxima mejor jugada (lo que más
 * desbloquea pesa el triple que lo que a uno mismo lo frena).
 */

interface Frena { id: string; titulo: string; frenan: string[] }
interface Requisito { id: string; titulo: string; faltan: string[] }
interface Jugada {
  id: string; titulo: string; categoria: string;
  desbloquea: number; frenado_por: number; peso: number;
}

interface Props { isOpen: boolean; onClose: () => void }

export const SiguientePanel: React.FC<Props> = ({ isOpen, onClose }) => {
  const [datos, setDatos] = useState<{ frenado: Frena[]; requisitos: Requisito[]; jugadas: Jugada[] } | null>(null);
  const [cargando, setCargando] = useState(false);

  const leer = async () => {
    setCargando(true);
    try {
      const d = await (await fetch(apiUrl('/api/graph/siguiente'))).json();
      if (d?.ok) setDatos({ frenado: d.frenado || [], requisitos: d.requisitos || [], jugadas: d.jugadas || [] });
    } catch {
      /* sin backend no hay nada que mostrar */
    } finally {
      setCargando(false);
    }
  };

  useEffect(() => {
    if (isOpen) void leer();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  if (!isOpen) return null;

  const bloque = (titulo: string, icono: React.ReactNode, hijos: React.ReactNode) => (
    <div className="rounded-xl border border-slate-700 bg-slate-800/60 p-3 space-y-2">
      <div className="flex items-center gap-2">
        {icono}
        <span className="text-xs font-medium text-slate-100">{titulo}</span>
      </div>
      {hijos}
    </div>
  );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" id="siguiente-panel">
      <div className="relative w-full max-w-3xl max-h-[88vh] overflow-y-auto rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <Compass size={16} className="text-violet-400" />
            <h2 className="text-sm font-medium text-slate-100">Lo que sigue</h2>
            <span className="text-[11px] text-slate-400">el camino crítico del mapa · se consulta, no se intuye</span>
          </div>
          <div className="flex items-center gap-3">
            <button onClick={() => void leer()} title="Recalcular" className="text-slate-400 hover:text-white cursor-pointer">
              <RefreshCw size={14} className={cargando ? 'animate-spin' : ''} />
            </button>
            <button onClick={onClose} className="text-slate-400 hover:text-white cursor-pointer" title="Cerrar"><X size={16} /></button>
          </div>
        </div>

        <div className="p-5 space-y-4">
          {!datos && <p className="text-xs text-slate-400">Leyendo el mapa…</p>}
          {datos && (
            <>
              {bloque(`La próxima jugada (${datos.jugadas.length})`, <ListChecks size={13} className="text-emerald-400" />,
                datos.jugadas.length === 0
                  ? <p className="text-[11px] text-slate-400">No hay movimientos pendientes ni riesgos anotados.</p>
                  : <div className="space-y-1.5">
                      {datos.jugadas.slice(0, 8).map((j, i) => (
                        <div key={j.id} className="flex items-center gap-2 text-[11px]">
                          <span className={`font-mono text-[9px] px-1.5 py-0.5 rounded border ${i === 0 ? 'text-emerald-200 bg-emerald-950/60 border-emerald-800/60' : 'text-slate-400 bg-slate-900/70 border-slate-700'}`}>{i + 1}</span>
                          <span className="text-slate-100 truncate max-w-[300px]" title={j.titulo}>{j.titulo}</span>
                          <span className="text-[9px] font-mono text-slate-400">{j.categoria.toLowerCase()}</span>
                          {j.desbloquea > 0 && (
                            <span className="ml-auto shrink-0 text-[9px] font-mono text-emerald-300 bg-emerald-950/50 border border-emerald-800/60 px-1.5 py-0.5 rounded">desbloquea {j.desbloquea}</span>
                          )}
                          {j.frenado_por > 0 && (
                            <span className="shrink-0 text-[9px] font-mono text-amber-300 bg-amber-950/40 border border-amber-800/60 px-1.5 py-0.5 rounded">frenada {j.frenado_por}</span>
                          )}
                        </div>
                      ))}
                    </div>
              )}

              {bloque(`Frenado (${datos.frenado.length})`, <Ban size={13} className="text-rose-400" />,
                datos.frenado.length === 0
                  ? <p className="text-[11px] text-slate-400">Nada está bloqueado: ninguna arista «bloquea» apunta a una capacidad.</p>
                  : <div className="space-y-1.5">
                      {datos.frenado.map((f) => (
                        <div key={f.id} className="text-[11px]">
                          <span className="text-slate-100">{f.titulo}</span>
                          <span className="text-slate-400"> ← lo frena </span>
                          <span className="text-rose-200">{f.frenan.join(' · ')}</span>
                        </div>
                      ))}
                    </div>
              )}

              {bloque(`Le falta a las capacidades (${datos.requisitos.length})`, <ArrowRight size={13} className="text-sky-400" />,
                datos.requisitos.length === 0
                  ? <p className="text-[11px] text-slate-400">Ninguna capacidad depende de algo sin probar.</p>
                  : <div className="space-y-1.5">
                      {datos.requisitos.map((r) => (
                        <div key={r.id} className="text-[11px]">
                          <span className="text-slate-100">{r.titulo}</span>
                          <span className="text-slate-400"> necesita </span>
                          <span className="text-sky-200">{r.faltan.join(' · ')}</span>
                          <span className="text-slate-500"> (todavía sin probar)</span>
                        </div>
                      ))}
                    </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default SiguientePanel;
