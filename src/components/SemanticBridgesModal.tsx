import React from 'react';
import { X, Network, Sparkles, ArrowRight, Check, RefreshCw, Loader2 } from 'lucide-react';
import { SemanticBridge } from '../types';

interface SemanticBridgesModalProps {
  isOpen: boolean;
  onClose: () => void;
  bridges: SemanticBridge[];
  isLoading: boolean;
  onScanBridges: () => void;
  onConnectBridge: (bridge: SemanticBridge) => void;
  onConnectAll: (bridges: SemanticBridge[]) => void;
  connectedBridgeIds: Set<string>;
}

export const SemanticBridgesModal: React.FC<SemanticBridgesModalProps> = ({
  isOpen,
  onClose,
  bridges,
  isLoading,
  onScanBridges,
  onConnectBridge,
  onConnectAll,
  connectedBridgeIds,
}) => {
  if (!isOpen) return null;

  const unbondedBridges = bridges.filter((b) => !connectedBridgeIds.has(b.id));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-md animate-in fade-in duration-200">
      <div className="bg-slate-900 border border-violet-500/30 w-full max-w-2xl rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[85vh]">
        {/* Header */}
        <div className="p-5 border-b border-slate-800 flex items-center justify-between bg-gradient-to-r from-violet-950/30 via-slate-900 to-indigo-950/20">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-violet-600/20 border border-violet-500/40 rounded-xl text-violet-400">
              <Network size={22} />
            </div>
            <div>
              <h2 className="text-lg font-bold text-white flex items-center gap-2">
                Conexiones Ocultas (Semantic Bridges)
                <span className="text-[11px] px-2 py-0.5 rounded-full bg-violet-500/20 text-violet-300 font-normal border border-violet-500/30">
                  IA Cognitiva
                </span>
              </h2>
              <p className="text-xs text-slate-400 mt-0.5">
                Descubre sinergias y relaciones no evidentes entre nodos distantes para elevar el pensamiento.
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors cursor-pointer"
          >
            <X size={18} />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 overflow-y-auto space-y-4 flex-1">
          {isLoading ? (
            <div className="py-14 flex flex-col items-center justify-center gap-3 text-center">
              <Loader2 size={32} className="text-violet-400 animate-spin" />
              <p className="text-sm font-medium text-slate-300">
                Analizando relaciones semánticas multidimensionales...
              </p>
              <p className="text-xs text-slate-500 max-w-sm">
                Buscando puntos de contacto transdisciplinarios y complementariedades entre ideas que aún no están conectadas.
              </p>
            </div>
          ) : bridges.length === 0 ? (
            <div className="py-12 text-center flex flex-col items-center justify-center gap-3">
              <div className="p-3 rounded-full bg-slate-800/80 text-slate-400 border border-slate-700">
                <Sparkles size={26} />
              </div>
              <h3 className="text-sm font-semibold text-slate-300">Sin conexiones pendientes encontradas</h3>
              <p className="text-xs text-slate-400 max-w-md">
                Añade más nodos o presiona &quot;Escanear Nuevos Puentes&quot; para que la IA explore sinergias alternativas entre los conceptos de tu lienzo.
              </p>
              <button
                type="button"
                onClick={onScanBridges}
                className="mt-2 flex items-center gap-1.5 px-4 py-2 bg-violet-600 hover:bg-violet-500 text-white rounded-xl text-xs font-semibold shadow-lg shadow-violet-600/20 transition-colors cursor-pointer"
              >
                <RefreshCw size={14} />
                <span>Escanear Puentes Ocultos</span>
              </button>
            </div>
          ) : (
            <div className="space-y-3">
              <div className="flex items-center justify-between text-xs text-slate-400 pb-1">
                <span>{bridges.length} relaciones latentes detectadas</span>
                {unbondedBridges.length > 1 && (
                  <button
                    type="button"
                    onClick={() => onConnectAll(unbondedBridges)}
                    className="flex items-center gap-1.5 text-xs font-medium text-violet-400 hover:text-violet-300 cursor-pointer"
                  >
                    <Sparkles size={13} />
                    <span>Conectar todos ({unbondedBridges.length})</span>
                  </button>
                )}
              </div>

              {bridges.map((bridge) => {
                const isConnected = connectedBridgeIds.has(bridge.id);

                return (
                  <div
                    key={bridge.id}
                    className={`p-4 rounded-xl border transition-all ${
                      isConnected
                        ? 'bg-slate-900/40 border-slate-800 opacity-60'
                        : 'bg-slate-900/90 border-violet-500/30 hover:border-violet-500/60 shadow-lg'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="space-y-2 flex-1">
                        {/* Node pair */}
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="px-2.5 py-1 rounded-lg bg-indigo-950/60 border border-indigo-500/40 text-xs font-semibold text-indigo-200">
                            {bridge.sourceTitle}
                          </span>
                          <ArrowRight size={14} className="text-violet-400 shrink-0" />
                          <span className="px-2.5 py-1 rounded-lg bg-indigo-950/60 border border-indigo-500/40 text-xs font-semibold text-indigo-200">
                            {bridge.targetTitle}
                          </span>
                          <span className="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded bg-violet-500/10 text-violet-300 border border-violet-500/20">
                            {bridge.label}
                          </span>
                        </div>

                        {/* Rationale */}
                        <p className="text-xs text-slate-300 leading-relaxed italic bg-slate-950/50 p-2.5 rounded-lg border border-slate-800">
                          &quot;{bridge.rationale}&quot;
                        </p>
                      </div>

                      {/* Action */}
                      <button
                        type="button"
                        disabled={isConnected}
                        onClick={() => onConnectBridge(bridge)}
                        className={`shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors ${
                          isConnected
                            ? 'bg-emerald-950/50 text-emerald-400 border border-emerald-800/40 cursor-default'
                            : 'bg-violet-600 hover:bg-violet-500 text-white cursor-pointer shadow-md shadow-violet-600/30'
                        }`}
                      >
                        {isConnected ? (
                          <>
                            <Check size={13} />
                            <span>Conectado</span>
                          </>
                        ) : (
                          <>
                            <Network size={13} />
                            <span>Enlazar Ideas</span>
                          </>
                        )}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-4 bg-slate-950/80 border-t border-slate-800 flex items-center justify-between">
          <button
            type="button"
            onClick={onScanBridges}
            disabled={isLoading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-slate-300 hover:text-white hover:bg-slate-800 transition-colors cursor-pointer"
          >
            <RefreshCw size={13} className={isLoading ? 'animate-spin' : ''} />
            <span>Volver a Escanear</span>
          </button>
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-xl text-xs font-semibold transition-colors cursor-pointer"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
