import React from 'react';
import { Trash2, AlertTriangle, X, PlusCircle, RotateCcw } from 'lucide-react';

interface ClearCanvasModalProps {
  isOpen: boolean;
  onClose: () => void;
  onClearAll: () => void;
  onResetToRoot: () => void;
  nodeCount: number;
}

export const ClearCanvasModal: React.FC<ClearCanvasModalProps> = ({
  isOpen,
  onClose,
  onClearAll,
  onResetToRoot,
  nodeCount,
}) => {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-slate-950/80 backdrop-blur-sm animate-in fade-in duration-200">
      <div
        id="clear-canvas-modal-card"
        className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-md shadow-2xl p-6 relative overflow-hidden"
      >
        {/* Subtle accent glow */}
        <div className="absolute top-0 left-0 right-0 h-1 bg-gradient-to-r from-rose-500 via-amber-500 to-indigo-500" />

        <div className="flex items-start justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-rose-500/10 border border-rose-500/20 flex items-center justify-center text-rose-400 shrink-0">
              <AlertTriangle size={20} />
            </div>
            <div>
              <h2 className="text-base font-bold text-white">¿Limpiar el lienzo?</h2>
              <p className="text-xs text-slate-400">
                Actualmente tienes <span className="text-indigo-400 font-semibold">{nodeCount}</span> {nodeCount === 1 ? 'nodo' : 'nodos'} en el espacio de trabajo.
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1 hover:bg-slate-800 rounded-lg transition-colors cursor-pointer"
          >
            <X size={18} />
          </button>
        </div>

        <p className="text-xs text-slate-300 leading-relaxed mb-6 bg-slate-950/60 p-3 rounded-xl border border-slate-800/80">
          Esta acción removerá las conexiones e ideas del lienzo activo. Se creará automáticamente un punto de restauración en tu historial para que puedas <span className="text-indigo-300 font-medium">deshacer con Ctrl + Z</span> si lo necesitas.
        </p>

        <div className="space-y-2.5">
          <button
            type="button"
            id="btn-confirm-clear-all"
            onClick={() => {
              onClearAll();
              onClose();
            }}
            className="w-full flex items-center justify-center gap-2 bg-rose-600 hover:bg-rose-500 text-white font-medium py-2.5 px-4 rounded-xl text-xs transition-colors shadow-lg shadow-rose-600/20 cursor-pointer"
          >
            <Trash2 size={15} />
            <span>Borrar todos los nodos por completo</span>
          </button>

          <button
            type="button"
            id="btn-confirm-reset-root"
            onClick={() => {
              onResetToRoot();
              onClose();
            }}
            className="w-full flex items-center justify-center gap-2 bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 font-medium py-2.5 px-4 rounded-xl text-xs transition-colors cursor-pointer"
          >
            <PlusCircle size={15} className="text-indigo-400" />
            <span>Reiniciar con un nuevo núcleo de idea</span>
          </button>

          <button
            type="button"
            onClick={onClose}
            className="w-full py-2 text-xs text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 rounded-xl transition-colors cursor-pointer text-center"
          >
            Cancelar
          </button>
        </div>
      </div>
    </div>
  );
};
