import React, { useState } from 'react';
import { Sparkles, X, Copy, Check, CheckSquare, Square, Target, Lightbulb, Compass, RotateCw } from 'lucide-react';

export interface MapSynthesis {
  summary: string;
  pillars: string[];
  actionItems: string[];
  keyOpportunities: string[];
}

interface SynthesisModalProps {
  isOpen: boolean;
  onClose: () => void;
  synthesis: MapSynthesis | null;
  isLoading: boolean;
  nodeCount?: number;
  onRefresh?: () => void;
}

export const SynthesisModal: React.FC<SynthesisModalProps> = ({
  isOpen,
  onClose,
  synthesis,
  isLoading,
  nodeCount = 0,
  onRefresh,
}) => {
  const [copied, setCopied] = useState(false);
  const [completedItems, setCompletedItems] = useState<Record<number, boolean>>({});

  if (!isOpen) return null;

  const toggleCheck = (index: number) => {
    setCompletedItems((prev) => ({ ...prev, [index]: !prev[index] }));
  };

  const handleCopyMarkdown = () => {
    if (!synthesis) return;
    const md = `# Síntesis Estratégica NeuralMind (${nodeCount} Nodos)
Fecha: ${new Date().toLocaleDateString()}

## Resumen Ejecutivo
${synthesis.summary}

## Pilares Estratégicos
${synthesis.pillars.map((p, i) => `${i + 1}. **${p}**`).join('\n')}

## Próximos Pasos Accionables
${synthesis.actionItems.map((item, i) => `- [${completedItems[i] ? 'x' : ' '}] ${item}`).join('\n')}

## Oportunidades de Innovación
${synthesis.keyOpportunities.map((op) => `> 💡 ${op}`).join('\n\n')}
`;
    navigator.clipboard.writeText(md);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm animate-in fade-in duration-150">
      <div
        className="w-full max-w-2xl bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl p-6 text-slate-100 relative max-h-[90vh] flex flex-col"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between pb-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-600 to-emerald-500 flex items-center justify-center text-white shadow-lg">
              <Sparkles size={18} />
            </div>
            <div>
              <h2 className="text-base font-bold text-white flex items-center gap-2">
                Síntesis Ejecutiva con Gemini AI
              </h2>
              <p className="text-xs text-slate-400">
                Análisis holístico y plan de acción de la red ({nodeCount} nodos analizados)
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1.5 text-slate-400 hover:text-white hover:bg-slate-800 rounded-lg transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto space-y-5 py-4 pr-1">
          {isLoading ? (
            <div className="py-16 flex flex-col items-center justify-center gap-3 text-center">
              <div className="w-10 h-10 border-3 border-indigo-500 border-t-transparent rounded-full animate-spin" />
              <div className="text-sm font-medium text-white">Gemini está analizando la red de nodos...</div>
              <p className="text-xs text-slate-400 max-w-sm">
                Extrayendo patrones interconectados, sintetizando hipótesis y formulando el plan de acción estratégico.
              </p>
            </div>
          ) : synthesis ? (
            <>
              {/* Resumen */}
              <div className="p-4 bg-slate-950/80 rounded-xl border border-slate-800">
                <div className="text-xs font-semibold text-indigo-400 uppercase tracking-wider mb-2 flex items-center gap-1.5">
                  <Compass size={14} /> Visión General
                </div>
                <p className="text-xs text-slate-300 leading-relaxed">
                  {synthesis.summary}
                </p>
              </div>

              {/* Pilares */}
              <div>
                <div className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2.5 flex items-center gap-1.5">
                  <Target size={14} className="text-emerald-400" /> Pilares Identificados
                </div>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-2.5">
                  {synthesis.pillars.map((pillar, i) => (
                    <div
                      key={i}
                      className="p-3 bg-slate-950/60 rounded-xl border border-slate-800/80 hover:border-emerald-500/40 transition-colors"
                    >
                      <div className="text-[10px] text-emerald-400 font-mono font-bold mb-1">0{i + 1}</div>
                      <div className="text-xs font-medium text-slate-200">{pillar}</div>
                    </div>
                  ))}
                </div>
              </div>

              {/* Próximos Pasos (Checklist interactivo) */}
              <div>
                <div className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2.5 flex items-center justify-between">
                  <span className="flex items-center gap-1.5">
                    <CheckSquare size={14} className="text-indigo-400" /> Plan de Acción Recomendado
                  </span>
                  <span className="text-[10px] text-slate-500 font-normal">Haz clic para marcar como completado</span>
                </div>
                <div className="space-y-2">
                  {synthesis.actionItems.map((item, i) => {
                    const isDone = !!completedItems[i];
                    return (
                      <div
                        key={i}
                        onClick={() => toggleCheck(i)}
                        className={`p-3 rounded-xl border cursor-pointer transition-all flex items-start gap-3 select-none ${
                          isDone
                            ? 'bg-emerald-950/20 border-emerald-800/40 text-slate-400 line-through'
                            : 'bg-slate-950/60 border-slate-800 hover:border-slate-700 text-slate-200'
                        }`}
                      >
                        <button type="button" className="mt-0.5 text-indigo-400">
                          {isDone ? <CheckSquare size={15} className="text-emerald-400" /> : <Square size={15} className="text-slate-500" />}
                        </button>
                        <span className="text-xs leading-snug flex-1">{item}</span>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Oportunidades Clave */}
              <div>
                <div className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-2 flex items-center gap-1.5">
                  <Lightbulb size={14} className="text-amber-400" /> Oportunidades Clave
                </div>
                <div className="space-y-2">
                  {synthesis.keyOpportunities.map((op, i) => (
                    <div
                      key={i}
                      className="p-3 bg-amber-950/20 border border-amber-800/30 rounded-xl text-xs text-amber-200 leading-relaxed flex items-start gap-2"
                    >
                      <span className="text-amber-400 font-bold">•</span>
                      <span>{op}</span>
                    </div>
                  ))}
                </div>
              </div>
            </>
          ) : (
            <div className="py-12 text-center text-xs text-slate-400">
              No se pudo generar la síntesis. Intenta de nuevo.
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="pt-4 border-t border-slate-800 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={handleCopyMarkdown}
              disabled={!synthesis || isLoading}
              className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium rounded-xl transition-colors disabled:opacity-50 cursor-pointer"
            >
              {copied ? <Check size={14} className="text-emerald-400" /> : <Copy size={14} />}
              <span>{copied ? 'Copiado al portapapeles' : 'Copiar Markdown'}</span>
            </button>
            {onRefresh && (
              <button
                type="button"
                onClick={onRefresh}
                disabled={isLoading}
                className="flex items-center gap-1 px-2.5 py-1.5 bg-slate-800/80 hover:bg-slate-700 text-indigo-300 hover:text-white text-xs font-medium rounded-xl transition-colors disabled:opacity-50 cursor-pointer"
                title="Regenerar análisis con el estado actual de los nodos"
              >
                <RotateCw size={13} className={isLoading ? 'animate-spin' : ''} />
                <span className="hidden sm:inline">Regenerar</span>
              </button>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-xl shadow-md transition-colors cursor-pointer"
          >
            Entendido
          </button>
        </div>
      </div>
    </div>
  );
};
