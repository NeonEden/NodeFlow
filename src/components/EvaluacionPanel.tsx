import React, { useCallback, useEffect, useState } from 'react';
import { X, FlaskConical, Play, Loader2, Trophy, Cpu, Timer, Coins } from 'lucide-react';
import { apiUrl } from '../services/apiBase';

/**
 * Planilla de evaluación — **medir en vez de suponer**.
 *
 * Corre las tareas reales del lienzo contra cada motor y verifica el resultado en código. No es un
 * benchmark de laboratorio: es lo que la app hace, con el spec y la validación reales, y sin tocar el
 * lienzo (las acciones proponen, nunca aplican solas).
 */

interface FilaPrueba {
  id: string;
  titulo: string;
  ok: boolean;
  detalle: string;
  ms: number;
  tokens: number;
  costo_usd: number;
  modelo: string;
  cache: string;
}

interface FilaMotor {
  id: string;
  aciertos: number;
  total: number;
  ms_medio: number;
  tokens: number;
  costo_usd: number;
  pruebas: FilaPrueba[];
}

interface Tabla {
  cuando: string;
  lienzo: { nodos: number };
  motores: FilaMotor[];
  ganador_por_prueba: Record<string, { motor: string; ms: number; ok: boolean }>;
}

interface Props {
  isOpen: boolean;
  onClose: () => void;
}

export function EvaluacionPanel({ isOpen, onClose }: Props) {
  const [tabla, setTabla] = useState<Tabla | null>(null);
  const [corriendo, setCorriendo] = useState(false);
  const [detalle, setDetalle] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const leer = useCallback(async () => {
    try {
      const d = await (await fetch(apiUrl('/api/ai/evaluar'))).json();
      setCorriendo(!!d?.corriendo);
      if (d?.tabla) setTabla(d.tabla as Tabla);
      return !!d?.corriendo;
    } catch (e: any) {
      setError('No pude leer la planilla.');
      return false;
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    void leer();
  }, [isOpen, leer]);

  // Mientras mide, se consulta sola: la ventana nunca se queda esperando.
  useEffect(() => {
    if (!isOpen || !corriendo) return;
    const t = setInterval(() => void leer(), 4000);
    return () => clearInterval(t);
  }, [isOpen, corriendo, leer]);

  const correr = async () => {
    setError(null);
    setDetalle(null);
    try {
      const r = await fetch(apiUrl('/api/ai/evaluar'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      });
      const d = await r.json();
      if (!d?.success) {
        setError(d?.error || 'No pude arrancar la evaluación.');
        return;
      }
      setDetalle(`Midiendo ${d.motores} motor(es). Son unos minutos: podés cerrar esto y seguir.`);
      setCorriendo(true);
    } catch (e: any) {
      setError(e?.message || 'Error de red');
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" id="evaluacion-panel">
      <div className="relative w-full max-w-4xl max-h-[88vh] overflow-y-auto rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <FlaskConical size={16} className="text-cyan-400" />
            <h2 className="text-sm font-medium text-slate-100">Planilla de evaluación</h2>
            <span className="text-[11px] text-slate-400">medir en vez de suponer · no toca el lienzo</span>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors cursor-pointer" title="Cerrar">
            <X size={16} />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <div className="flex flex-wrap items-center gap-3">
            <button
              type="button"
              id="btn-correr-evaluacion"
              onClick={correr}
              disabled={corriendo}
              className={`flex items-center gap-2 px-3 py-2 rounded-xl text-xs border border-slate-700 transition-colors ${
                corriendo ? 'bg-slate-800 text-slate-400 cursor-default' : 'bg-slate-800 text-slate-100 hover:text-white cursor-pointer'
              }`}
            >
              {corriendo ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} className="text-cyan-400" />}
              {corriendo ? 'Midiendo…' : 'Correr la planilla'}
            </button>
            <span className="text-[11px] text-slate-400">
              Corre 5 tareas reales contra los motores locales. Todo queda en tu máquina.
            </span>
          </div>

          {detalle && <p className="text-[11px] text-slate-300">{detalle}</p>}
          {error && <p className="text-[11px] text-amber-300">{error}</p>}

          {tabla && (
            <>
              <div className="overflow-x-auto rounded-xl border border-slate-700">
                <table className="w-full text-xs">
                  <thead className="bg-slate-800 text-slate-300">
                    <tr>
                      <th className="text-left px-3 py-2 font-medium">Motor</th>
                      <th className="text-left px-3 py-2 font-medium">Aciertos</th>
                      <th className="text-left px-3 py-2 font-medium">Tiempo medio</th>
                      <th className="text-left px-3 py-2 font-medium">Tokens</th>
                      <th className="text-left px-3 py-2 font-medium">Costo</th>
                    </tr>
                  </thead>
                  <tbody>
                    {tabla.motores.map((m) => (
                      <tr key={m.id} className="border-t border-slate-800">
                        <td className="px-3 py-2 text-slate-100">
                          <span className="inline-flex items-center gap-1.5">
                            <Cpu size={12} className={m.id.includes('ollama:') ? 'text-emerald-300' : 'text-sky-300'} />
                            {m.id}
                          </span>
                        </td>
                        <td className="px-3 py-2">
                          <span className={m.aciertos === m.total ? 'text-emerald-300' : 'text-amber-300'}>
                            {m.aciertos}/{m.total}
                          </span>
                        </td>
                        <td className="px-3 py-2 text-slate-300">{(m.ms_medio / 1000).toFixed(1)} s</td>
                        <td className="px-3 py-2 text-slate-300">{m.tokens}</td>
                        <td className="px-3 py-2 text-slate-300">{m.costo_usd > 0 ? `US$${m.costo_usd.toFixed(5)}` : 'US$0'}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>

              {Object.keys(tabla.ganador_por_prueba || {}).length > 0 && (
                <div className="rounded-xl border border-slate-700 bg-slate-800 p-3">
                  <div className="flex items-center gap-2 mb-2">
                    <Trophy size={13} className="text-amber-300" />
                    <span className="text-[11px] font-medium text-slate-100">Quién ganó cada tarea</span>
                    <span className="text-[10px] text-slate-400">esto es lo que después usa el ruteo</span>
                  </div>
                  <div className="space-y-1">
                    {Object.entries(tabla.ganador_por_prueba as Record<string, { motor: string; ms: number; ok: boolean }>).map(([tarea, g]) => (
                      <div key={tarea} className="flex items-center justify-between gap-3 text-[11px]">
                        <span className="text-slate-300">{tarea}</span>
                        <span className="flex items-center gap-2">
                          <span className="text-slate-100">{g.motor.replace('ollama:', '')}</span>
                          <span className="text-slate-500 inline-flex items-center gap-1">
                            <Timer size={10} />
                            {(g.ms / 1000).toFixed(1)} s
                          </span>
                          {!g.ok && <span className="text-amber-300">ninguno acertó</span>}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              <div className="space-y-1">
                {tabla.motores.map((m) => (
                  <details key={m.id} className="rounded-xl border border-slate-700 bg-slate-800">
                    <summary className="px-3 py-2 text-[11px] text-slate-200 cursor-pointer">
                      Detalle de {m.id.replace('ollama:', '')} · {m.aciertos}/{m.total}
                    </summary>
                    <div className="px-3 pb-3 space-y-1">
                      {m.pruebas.map((p) => (
                        <div key={p.id} className="flex items-start justify-between gap-3 text-[11px]">
                          <span className={p.ok ? 'text-emerald-300' : 'text-amber-300'}>
                            {p.ok ? '✓' : '✗'} {p.titulo}
                          </span>
                          <span className="text-slate-400 text-right">
                            {p.detalle} · {(p.ms / 1000).toFixed(1)} s
                            {p.costo_usd > 0 ? ` · US$${p.costo_usd.toFixed(5)}` : ''}
                            {p.cache === 'hit' ? ' · caché' : ''}
                          </span>
                        </div>
                      ))}
                    </div>
                  </details>
                ))}
              </div>

              <p className="text-[10px] text-slate-500 flex items-center gap-1.5">
                <Coins size={10} />
                Medido sobre {tabla.lienzo?.nodos ?? 0} nodos del lienzo real. Corrida {tabla.cuando}.
              </p>
            </>
          )}

          {!tabla && !corriendo && (
            <p className="text-xs text-slate-400">Todavía no hay ninguna corrida guardada.</p>
          )}
        </div>
      </div>
    </div>
  );
}
