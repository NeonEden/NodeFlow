import React, { useState } from 'react';
import { X, Zap, Sparkles, Loader2, FileText, ArrowRight } from 'lucide-react';

interface BrainDumpModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (rawText: string) => Promise<void>;
  isLoading: boolean;
}

const EXAMPLES = [
  {
    label: 'Arquitectura SaaS',
    text: `- Motor de autenticación con OAuth y JWT
- Cola de tareas distribuida para procesamiento de IA
- Base de datos relacional particionada
- Frontend SPA reactivo con persistencia local
- Módulo de suscripciones y facturación Stripe`,
  },
  {
    label: 'Validación de Producto',
    text: `- Problema: La toma de notas actual es lineal y pierde el contexto holístico
- Público: Fundadores, arquitectos técnicos e investigadores
- Hipótesis: Los mapas interactivos aumentan la retención conceptual un 40%
- Métrica clave: Tasa de retorno semanal y nodos creados por sesión
- Riesgo principal: Sobrecarga visual si el grafo crece sin jerarquía`,
  },
];

export const BrainDumpModal: React.FC<BrainDumpModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  isLoading,
}) => {
  const [text, setText] = useState('');

  if (!isOpen) return null;

  const handleSubmit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!text.trim() || isLoading) return;
    await onSubmit(text.trim());
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-md animate-in fade-in duration-200">
      <div className="bg-slate-900 border border-emerald-500/30 w-full max-w-xl rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-5 border-b border-slate-800 flex items-center justify-between bg-gradient-to-r from-emerald-950/30 via-slate-900 to-indigo-950/20">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-emerald-600/20 border border-emerald-500/40 rounded-xl text-emerald-400">
              <Zap size={22} />
            </div>
            <div>
              <h2 className="text-lg font-bold text-white flex items-center gap-2">
                Descarga Mental (Brain Dump)
                <span className="text-[11px] px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 font-normal border border-emerald-500/30">
                  Cero Fricción
                </span>
              </h2>
              <p className="text-xs text-slate-400 mt-0.5">
                Vuelca tus notas, viñetas o ideas sueltas. La IA las estructurará y conectará al instante.
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

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div>
            <div className="flex items-center justify-between mb-2">
              <label htmlFor="brain-dump-input" className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
                <FileText size={13} className="text-emerald-400" />
                <span>Pega o escribe tus pensamientos sueltos:</span>
              </label>
              <span className="text-[10px] text-slate-500 font-mono">
                Cmd/Ctrl + Enter para generar
              </span>
            </div>
            <textarea
              id="brain-dump-input"
              rows={6}
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Ejemplo:
- Idea principal de la plataforma
- Desafíos técnicos inmediatos
- Métrica de éxito
- Integración con modelos de IA..."
              className="w-full bg-slate-950 border border-slate-700/80 focus:border-emerald-500 rounded-xl p-3.5 text-sm text-slate-100 placeholder:text-slate-600 focus:outline-none focus:ring-1 focus:ring-emerald-500 font-sans leading-relaxed resize-none shadow-inner"
              autoFocus
            />
          </div>

          {/* Quick Examples */}
          <div className="space-y-1.5">
            <span className="text-[10px] uppercase font-bold text-slate-500 tracking-wider">
              Ejemplos rápidos para probar:
            </span>
            <div className="flex gap-2 flex-wrap">
              {EXAMPLES.map((ex, i) => (
                <button
                  key={i}
                  type="button"
                  onClick={() => setText(ex.text)}
                  className="px-2.5 py-1 bg-slate-800/80 hover:bg-slate-800 text-[11px] text-slate-300 hover:text-emerald-300 rounded-lg border border-slate-700/60 transition-colors cursor-pointer"
                >
                  {ex.label}
                </button>
              ))}
            </div>
          </div>

          {/* Footer actions */}
          <div className="pt-3 border-t border-slate-800 flex items-center justify-between">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs font-semibold transition-colors cursor-pointer"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={!text.trim() || isLoading}
              className="flex items-center gap-2 px-5 py-2.5 bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-xl text-xs font-bold shadow-lg shadow-emerald-600/30 transition-all cursor-pointer"
            >
              {isLoading ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  <span>Estructurando conceptos...</span>
                </>
              ) : (
                <>
                  <Sparkles size={14} />
                  <span>Estructurar en Mapa Mental</span>
                  <ArrowRight size={13} />
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
