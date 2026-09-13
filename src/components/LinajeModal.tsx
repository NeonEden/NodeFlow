import React from 'react';
import { X, History, RotateCcw, Target, Sparkles, Layers } from 'lucide-react';
import { CustomNode } from '../types';

interface LinajeModalProps {
  isOpen: boolean;
  onClose: () => void;
  node: CustomNode | null;
  onRestaurar: (node: CustomNode) => void;
}

/**
 * Linaje de un macro-nodo (poda sin pérdida).
 *
 * Los nodos condensados no se destruyen: viven dentro del macro-nodo. Acá se ve de dónde salió
 * y se puede devolver el sub-grafo original al lienzo tal como estaba.
 */
export const LinajeModal: React.FC<LinajeModalProps> = ({ isOpen, onClose, node, onRestaurar }) => {
  if (!isOpen || !node) return null;
  const macro = node.data.macro;
  const hijos = (macro?.datos?.nodes ?? []) as CustomNode[];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
      <div className="relative w-full max-w-2xl max-h-[85vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-violet-500/10 border border-violet-500/30 flex items-center justify-center text-violet-400">
              <History size={17} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200">Linaje del macro-nodo</div>
              <div className="text-[11px] text-slate-400">
                {macro?.colapsados ?? hijos.length} nodos condensados · nada se perdió
              </div>
            </div>
          </div>
          <button type="button" onClick={onClose} className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer">
            <X size={16} />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-4 text-sm flex-1">
          <div className="bg-slate-900/70 border border-slate-700 rounded-xl p-4 space-y-3">
            <div className="text-base font-semibold text-slate-200">{node.data.title}</div>
            <p className="text-xs text-slate-300 leading-relaxed">{node.data.description}</p>
            {macro?.resumen && (
              <p className="text-xs text-slate-400 leading-relaxed border-t border-slate-800 pt-2">
                <Sparkles size={12} className="inline mr-1 text-violet-400" />
                {macro.resumen}
              </p>
            )}
            {macro?.principio && (
              <p className="text-xs text-slate-300 italic border-t border-slate-800 pt-2">
                Principio: {macro.principio}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2 text-[10px] font-mono border-t border-slate-800 pt-2">
              {macro?.objetivo && (
                <span className="flex items-center gap-1 text-slate-400">
                  <Target size={11} className="text-violet-400" />
                  {macro.objetivo.slice(0, 60)}
                </span>
              )}
              {typeof macro?.match === 'number' && (
                <span className="px-1.5 py-0.5 rounded border border-violet-700/50 bg-violet-900/30 text-violet-200">
                  aporte al objetivo: {Math.round((macro.match ?? 0) * 100)}%
                </span>
              )}
              <span className="px-1.5 py-0.5 rounded border border-slate-700 text-slate-400 flex items-center gap-1">
                <Layers size={10} /> {macro?.colapsados ?? 0} colapsados
              </span>
            </div>
          </div>

          <div>
            <div className="text-[10px] uppercase tracking-widest text-slate-500 font-bold mb-2">
              Nodos que le dieron origen ({hijos.length})
            </div>
            <div className="space-y-1.5">
              {hijos.map((h) => (
                <div key={h.id} className="bg-slate-900/70 border border-slate-800 rounded-lg px-3 py-2">
                  <div className="text-xs font-medium text-slate-200 truncate">{h.data?.title || '(sin título)'}</div>
                  {h.data?.description && (
                    <div className="text-[10px] text-slate-500 truncate">{h.data.description.slice(0, 120)}</div>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="flex items-center justify-between gap-3 px-5 py-4 border-t border-slate-800">
          <span className="text-[11px] text-slate-500">
            Restaurar devuelve los {hijos.length} nodos y sus conexiones al lienzo.
          </span>
          <button
            type="button"
            id="btn-restaurar-linaje"
            onClick={() => onRestaurar(node)}
            className="flex items-center gap-1.5 px-3 py-2 bg-slate-800 hover:bg-slate-700 text-slate-100 border border-slate-600 rounded-xl text-xs font-medium transition-colors cursor-pointer"
          >
            <RotateCcw size={13} />
            Restaurar al lienzo
          </button>
        </div>
      </div>
    </div>
  );
};
