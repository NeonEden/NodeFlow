import React, { useEffect, useRef, useState } from 'react';
import { X, CornerDownRight, Lightbulb } from 'lucide-react';

interface ResponderPreguntaModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** La pregunta que se está respondiendo (título y descripción, para tenerla a la vista). */
  pregunta: { title: string; description: string } | null;
  onResponder: (texto: string) => void;
}

/**
 * Responder una pregunta catalizadora.
 *
 * Cierra el ciclo del lienzo: la pregunta vive como nodo y hasta ahora no podía cerrarse. Lo que se
 * escribe acá nace como **nodo respuesta enlazado** a la pregunta, y la pregunta pasa a `respondida`.
 */
export const ResponderPreguntaModal: React.FC<ResponderPreguntaModalProps> = ({
  isOpen,
  onClose,
  pregunta,
  onResponder,
}) => {
  const [texto, setTexto] = useState('');
  const areaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setTexto('');
    // El foco va directo al campo: responder tiene que ser escribir y enter.
    window.setTimeout(() => areaRef.current?.focus(), 50);
  }, [isOpen]);

  if (!isOpen) return null;

  const guardar = () => {
    if (texto.trim().length < 2) return;
    onResponder(texto);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
      <div className="relative w-full max-w-2xl max-h-[85vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-amber-500/10 border border-amber-500/30 flex items-center justify-center text-amber-300">
              <Lightbulb size={16} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200">Responder la pregunta</div>
              <div className="text-[11px] text-slate-400">
                Lo que escribas queda como nodo enlazado y la pregunta se cierra
              </div>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer"
          >
            <X size={16} />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-3 flex-1">
          <div className="rounded-xl border border-amber-500/25 bg-amber-500/5 px-3 py-2">
            <div className="text-xs font-semibold text-amber-200">{pregunta?.title || 'Pregunta'}</div>
            {pregunta?.description && (
              <p className="text-[11px] text-slate-300 leading-relaxed mt-0.5">{pregunta.description}</p>
            )}
          </div>

          <textarea
            ref={areaRef}
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                guardar();
              }
            }}
            rows={7}
            placeholder="Tu respuesta, aunque sea provisoria: es lo que deja la pregunta cerrada…"
            className="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-xs text-slate-100 leading-relaxed focus:outline-none focus:border-amber-500"
          />
          <p className="text-[10px] text-slate-500">
            {texto.trim().length} car. · Ctrl+Enter guarda · la respuesta entra con madurez «Probada» y su
            evidencia cargada
          </p>
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-4 border-t border-slate-800">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-2 rounded-xl text-xs font-medium text-slate-300 hover:text-slate-100 border border-slate-700 hover:border-slate-600 transition-colors cursor-pointer"
          >
            Cancelar
          </button>
          <button
            type="button"
            id="btn-responder-pregunta"
            onClick={guardar}
            disabled={texto.trim().length < 2}
            className={`flex items-center gap-1.5 px-3 py-2 rounded-xl text-xs font-medium border transition-colors cursor-pointer ${
              texto.trim().length < 2
                ? 'bg-slate-800 border-slate-700 text-slate-500'
                : 'bg-amber-600/20 border-amber-500/50 text-amber-100 hover:bg-amber-600/30'
            }`}
          >
            <CornerDownRight size={13} />
            Responder y cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
