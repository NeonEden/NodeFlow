import React, { useState } from 'react';
import { X, Zap, Sparkles, Loader2, FileText, ArrowRight, Layers, HelpCircle, CheckCircle2 } from 'lucide-react';

interface BrainDumpModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (rawText: string, mode?: 'ai' | 'instant') => Promise<void>;
  isLoading: boolean;
}

const EXAMPLES = [
  {
    type: 'simple',
    label: 'Idea Simple (1 frase)',
    desc: 'Un concepto nuclear conciso',
    text: 'Plataforma de mentorías 1 a 1 para desarrolladores junior con matching algorítmico y sesiones en vivo',
  },
  {
    type: 'bullets',
    label: 'Lista de Viñetas',
    desc: 'Lluvia de ideas rápida',
    text: `- Autenticación con OAuth y JWT
- Cola de tareas distribuida para procesamiento de IA
- Base de datos relacional particionada
- Frontend SPA reactivo con persistencia local
- Módulo de suscripciones y facturación Stripe`,
  },
  {
    type: 'complex',
    label: 'Idea Compleja (Espec)',
    desc: 'Párrafo denso con objetivos y riesgos',
    text: `Problema: La toma de notas convencional es lineal y fragmenta el contexto holístico de sistemas complejos.
Público Objetivo: Fundadores técnicos, arquitectos de software e investigadores.
Hipótesis Principal: Los mapas cognitivos no lineales aumentan la retención conceptual un 40% y aceleran el descubrimiento de conexiones interdisciplinarias.
Métricas Clave: Retención semanal, nodos creados por sesión y puentes semánticos descubiertos.
Riesgo Crítico: Sobrecarga cognitiva y visual si el grafo crece sin jerarquía ni algoritmos de auto-diseño.`,
  },
];

export const BrainDumpModal: React.FC<BrainDumpModalProps> = ({
  isOpen,
  onClose,
  onSubmit,
  isLoading,
}) => {
  const [text, setText] = useState('');
  const [showComplexityTip, setShowComplexityTip] = useState(false);

  if (!isOpen) return null;

  const handleSubmit = async (mode: 'ai' | 'instant' = 'ai', e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!text.trim() || isLoading) return;
    await onSubmit(text.trim(), mode);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      handleSubmit('ai');
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md animate-in fade-in duration-200">
      <div className="bg-slate-900 border border-emerald-500/30 w-full max-w-2xl rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
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
                Vuelca tus ideas sueltas o especificaciones. Conviértelas al lienzo en segundos.
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
        <form onSubmit={(e) => handleSubmit('ai', e)} className="p-6 space-y-4 overflow-y-auto">
          {/* Complexity Explanation Pill */}
          <div className="bg-slate-950/70 border border-slate-800 rounded-xl p-3 text-xs text-slate-300 space-y-1.5">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-emerald-400 flex items-center gap-1.5 text-[11px]">
                <HelpCircle size={13} />
                ¿Las ideas deben ser simples o pueden ser complejas?
              </span>
              <button
                type="button"
                onClick={() => setShowComplexityTip(!showComplexityTip)}
                className="text-[10px] text-slate-400 hover:text-white underline cursor-pointer"
              >
                {showComplexityTip ? 'Ocultar' : 'Ver detalle'}
              </button>
            </div>
            <p className="text-slate-400 text-[11px] leading-relaxed">
              <strong>¡Admite ambas perfectamente!</strong> Puedes ingresar desde una frase corta hasta una especificación técnica de varios párrafos.
            </p>
            {showComplexityTip && (
              <div className="pt-2 border-t border-slate-800/80 space-y-1 text-[11px] text-slate-300">
                <div className="flex items-start gap-1.5">
                  <CheckCircle2 size={13} className="text-emerald-400 shrink-0 mt-0.5" />
                  <span><strong>Idea Simple:</strong> La IA extrae el núcleo e infiere proactivamente 3 a 5 pilares funcionales (Estrategia, Arquitectura, Métricas, etc.).</span>
                </div>
                <div className="flex items-start gap-1.5">
                  <CheckCircle2 size={13} className="text-emerald-400 shrink-0 mt-0.5" />
                  <span><strong>Idea Compleja:</strong> La IA sintetiza el objetivo central y descompone las dimensiones en sub-ramas jerárquicas con categorías y etiquetas precisas.</span>
                </div>
              </div>
            )}
          </div>

          <div>
            <div className="flex items-center justify-between mb-2">
              <label htmlFor="brain-dump-input" className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
                <FileText size={13} className="text-emerald-400" />
                <span>Pega o escribe tus pensamientos (frases, viñetas o texto libre):</span>
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
              placeholder="Escribe o pega aquí:
- Una sola frase (ej: Marketplace de autos usados verificados)
- Una lista rápida de ideas o tareas
- O un texto completo con problemas, hipótesis y arquitectura..."
              className="w-full bg-slate-950 border border-slate-700/80 focus:border-emerald-500 rounded-xl p-3.5 text-sm text-slate-100 placeholder:text-slate-600 focus:outline-none focus:ring-1 focus:ring-emerald-500 font-sans leading-relaxed resize-none shadow-inner"
              autoFocus
            />
          </div>

          {/* Quick Examples */}
          <div className="space-y-1.5">
            <span className="text-[10px] uppercase font-bold text-slate-500 tracking-wider">
              Cargar ejemplo para probar:
            </span>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
              {EXAMPLES.map((ex, i) => (
                <button
                  key={i}
                  type="button"
                  onClick={() => setText(ex.text)}
                  className="px-2.5 py-1.5 bg-slate-800/80 hover:bg-slate-800 text-left rounded-lg border border-slate-700/60 hover:border-emerald-500/40 transition-colors cursor-pointer group"
                >
                  <div className="text-[11px] font-semibold text-slate-200 group-hover:text-emerald-300">
                    {ex.label}
                  </div>
                  <div className="text-[9px] text-slate-400 truncate">
                    {ex.desc}
                  </div>
                </button>
              ))}
            </div>
          </div>

          {/* Footer actions with Dual Modes */}
          <div className="pt-4 border-t border-slate-800 flex flex-col sm:flex-row items-center justify-between gap-3">
            <button
              type="button"
              onClick={onClose}
              className="w-full sm:w-auto px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs font-semibold transition-colors cursor-pointer"
            >
              Cancelar
            </button>

            <div className="flex items-center gap-2 w-full sm:w-auto justify-end">
              {/* Modo Instantáneo sin red */}
              <button
                type="button"
                onClick={() => handleSubmit('instant')}
                disabled={!text.trim() || isLoading}
                title="Crea los nodos en el lienzo al instante sin esperar respuesta de IA"
                className="flex items-center gap-1.5 px-3.5 py-2.5 bg-slate-800 hover:bg-slate-700 border border-slate-600/60 text-slate-200 hover:text-white disabled:opacity-50 disabled:cursor-not-allowed rounded-xl text-xs font-semibold transition-all cursor-pointer"
              >
                <Layers size={13} className="text-amber-400" />
                <span>Volcado Instantáneo (0s)</span>
              </button>

              {/* Modo Estructuración Inteligente con Gemini */}
              <button
                type="button"
                onClick={() => handleSubmit('ai')}
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
                    <span>Estructurar con IA (Gemini)</span>
                    <ArrowRight size={13} />
                  </>
                )}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
};
