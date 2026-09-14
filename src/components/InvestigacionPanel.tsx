import React, { useState } from 'react';
import { X, Telescope, Loader2, Circle, CheckCircle2, Sparkles } from 'lucide-react';

/**
 * Investigación visible — el ciclo 🌱⚔️🧪🚀 a la vista.
 *
 * La investigación corre en el backend aunque esta ventana esté cerrada (el aplicador vive en
 * `App.tsx`): acá sólo se **mira** cómo crece, paso por paso, y se dispara una nueva. Antes el
 * seguimiento vivía dentro del panel de voz y con el panel cerrado las fases se generaban sin
 * llegar nunca al lienzo — el usuario veía el nodo nacer y después nada.
 */

export interface PasoInvestigacion {
  fase: string;
  titulo: string;
  emoji: string;
  que: string;
  comandos?: unknown[];
  cuando?: number;
}

export interface EstadoInvestigacion {
  corriendo?: boolean;
  pedido?: string;
  pasos?: PasoInvestigacion[];
  terminado?: boolean;
  ok?: boolean;
  salida?: string;
}

/** Las cuatro fases, en orden. */
const FASES: [string, string, string][] = [
  ['semilla', 'Semilla', '🌱'],
  ['friccion', 'Fricción', '⚔️'],
  ['capsula', 'Cápsula', '🧪'],
  ['hexagono', 'Hexágono', '🚀'],
];

interface Props {
  isOpen: boolean;
  onClose: () => void;
  estado: EstadoInvestigacion | null;
  onInvestigar: (pedido: string) => Promise<void> | void;
  /** Temas del lienzo: investigar un nodo es un clic, sin escribir nada. */
  temas?: string[];
}

export const InvestigacionPanel: React.FC<Props> = ({ isOpen, onClose, estado, onInvestigar, temas = [] }) => {
  const [tema, setTema] = useState('');
  const [enviando, setEnviando] = useState(false);

  if (!isOpen) return null;

  const pasos: PasoInvestigacion[] = Array.isArray(estado?.pasos) ? estado.pasos : [];
  const corriendo = Boolean(estado?.corriendo);
  const completada = pasos.map((p) => p.fase);

  const lanzar = async (cual: string) => {
    const limpio = cual.trim();
    if (limpio.length < 4 || enviando) return;
    setEnviando(true);
    try {
      await onInvestigar(limpio);
      setTema('');
    } finally {
      setEnviando(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm" id="investigacion-panel">
      <div className="relative w-full max-w-3xl max-h-[88vh] overflow-y-auto rounded-2xl border border-slate-700 bg-slate-900 shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <Telescope size={16} className="text-violet-400" />
            <h2 className="text-sm font-medium text-slate-100">Investigación por fases</h2>
            <span className="text-[11px] text-slate-400">el nodo crece en el lienzo mientras investiga</span>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors cursor-pointer" title="Cerrar">
            <X size={16} />
          </button>
        </div>

        <div className="p-5 space-y-4">
          {/* Arrancar una investigación */}
          <div className="rounded-xl border border-slate-700 bg-slate-800/60 p-3 space-y-2">
            <div className="flex items-center gap-2">
              <Telescope size={13} className="text-violet-400 shrink-0" />
              <span className="text-xs text-slate-100 font-medium">Investigar un tema</span>
              <span className="text-[11px] text-slate-400">· Hermes sale a la web, el motor sintetiza</span>
            </div>
            <div className="flex items-center gap-2">
              <input
                value={tema}
                onChange={(e) => setTema(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') void lanzar(tema); }}
                placeholder="Ej: sensores de humedad de suelo de bajo consumo"
                className="flex-1 px-3 py-1.5 rounded-xl bg-slate-900/70 border border-slate-700 text-xs text-slate-100 placeholder:text-slate-500 focus:outline-none focus:border-violet-600"
              />
              <button
                type="button"
                onClick={() => void lanzar(tema)}
                disabled={enviando || corriendo || tema.trim().length < 4}
                className="flex items-center gap-2 px-3 py-1.5 rounded-xl text-xs font-medium bg-violet-950/70 text-violet-200 hover:bg-violet-900/80 border border-violet-800/60 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
                title={corriendo ? 'Ya hay una investigación en curso' : 'Sale a buscar fuentes y arma los nodos en el lienzo'}
              >
                {enviando ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
                {corriendo ? 'Investigando…' : 'Investigar'}
              </button>
            </div>
            {temas.length > 0 && (
              <div className="flex flex-wrap gap-1.5 pt-0.5">
                {temas.slice(0, 8).map((t) => (
                  <button
                    key={t}
                    type="button"
                    onClick={() => void lanzar(t)}
                    disabled={corriendo}
                    title={`Investigar: ${t}`}
                    className="px-2 py-0.5 rounded-lg text-[10px] font-mono text-slate-300 bg-slate-900/70 border border-slate-700 hover:border-violet-700 hover:text-violet-200 disabled:opacity-40 cursor-pointer truncate max-w-[220px]"
                  >
                    {t}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Las cuatro fases, siempre a la vista */}
          <div className="grid grid-cols-4 gap-2" id="investigacion-fases">
            {FASES.map(([id, titulo, emoji]) => {
              const hecha = completada.includes(id);
              const actual = corriendo && !hecha && completada.length === FASES.findIndex((f) => f[0] === id);
              return (
                <div
                  key={id}
                  className={`rounded-xl border px-3 py-2 flex items-center gap-2 ${hecha ? 'border-violet-800/60 bg-violet-950/40' : actual ? 'border-violet-700 bg-violet-950/30' : 'border-slate-700 bg-slate-800/40'}`}
                >
                  <span className="text-base">{emoji}</span>
                  <span className={`text-[11px] ${hecha || actual ? 'text-violet-200' : 'text-slate-400'}`}>{titulo}</span>
                  <span className="ml-auto">
                    {hecha ? <CheckCircle2 size={13} className="text-violet-300" />
                      : actual ? <Loader2 size={13} className="animate-spin text-violet-300" />
                        : <Circle size={13} className="text-slate-600" />}
                  </span>
                </div>
              );
            })}
          </div>

          {/* Paso por paso */}
          {pasos.length === 0 ? (
            <p className="text-xs text-slate-400">
              {corriendo
                ? '🌱 Arrancando: nace el nodo y sale a buscar fuentes…'
                : 'Todavía no hay ninguna investigación. Escribí un tema arriba o investigá un nodo desde su barra de acciones.'}
            </p>
          ) : (
            <div className="space-y-2">
              {pasos.map((p, i) => (
                <div key={`${p.fase}-${i}`} className="rounded-xl border border-slate-700 bg-slate-800/60 p-3">
                  <div className="flex items-center gap-2">
                    <span className="text-sm">{p.emoji}</span>
                    <span className="text-xs font-medium text-slate-100">{p.titulo}</span>
                    {Array.isArray(p.comandos) && p.comandos.length > 0 && (
                      <span className="text-[9px] font-mono text-violet-200 bg-violet-950/60 border border-violet-800/60 px-1.5 py-0.5 rounded">
                        {p.comandos.length} al lienzo
                      </span>
                    )}
                    {typeof p.cuando === 'number' && p.cuando > 0 && (
                      <span className="ml-auto text-[10px] font-mono text-slate-400">
                        {new Date(p.cuando * 1000).toLocaleTimeString()}
                      </span>
                    )}
                  </div>
                  {p.que && <p className="mt-1 text-[11px] text-slate-300 whitespace-pre-wrap">{p.que}</p>}
                </div>
              ))}
            </div>
          )}

          {/* El resultado */}
          {estado?.terminado && estado?.salida && (
            <div className="rounded-xl border border-emerald-900/60 bg-emerald-950/30 p-3" id="investigacion-resultado">
              <div className="flex items-center gap-2">
                <CheckCircle2 size={13} className={estado.ok === false ? 'text-amber-400' : 'text-emerald-400'} />
                <span className="text-xs font-medium text-emerald-200">
                  {estado.ok === false ? 'Terminó con reparos' : 'Investigación completa'}
                </span>
                {estado.pedido && <span className="text-[10px] text-slate-400 truncate">· {estado.pedido}</span>}
              </div>
              <p className="mt-1 text-[11px] text-slate-200 whitespace-pre-wrap max-h-52 overflow-y-auto">{estado.salida}</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default InvestigacionPanel;
