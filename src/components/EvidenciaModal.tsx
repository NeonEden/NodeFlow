import React, { useEffect, useState } from 'react';
import { X, FileText } from 'lucide-react';
import { MATURITY_CONFIGS, IdeaMaturityLevel } from '../types';

interface EvidenciaModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Fase a la que se está moviendo (3 = Probada, 4 = Axioma, 5 = Artefacto). */
  nivel: IdeaMaturityLevel | null;
  /** Título del nodo, para saber de qué idea se está hablando. */
  titulo: string;
  onGuardar: (evidencia: string) => void;
}

/**
 * Evidencia del cambio de fase.
 *
 * La madurez era un número sin motivo: se podía marcar «Probada» sin decir qué pasó. Este paso pide
 * una línea —opcional, pero se pide— y la guarda con fecha. Es la materia prima de la métrica de
 * valor (idea cruda → artefacto aprobado) y lo que hace que el jardín tenga sentido.
 */
export const EvidenciaModal: React.FC<EvidenciaModalProps> = ({
  isOpen,
  onClose,
  nivel,
  titulo,
  onGuardar,
}) => {
  const [texto, setTexto] = useState('');

  useEffect(() => {
    if (isOpen) setTexto('');
  }, [isOpen]);

  if (!isOpen || !nivel) return null;
  const cfg = MATURITY_CONFIGS[nivel];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
      <div className="relative w-full max-w-xl overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-sky-500/10 border border-sky-500/30 flex items-center justify-center text-sky-300">
              <FileText size={16} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200">
                {cfg.icon} {cfg.label} · ¿por qué?
              </div>
              <div className="text-[11px] text-slate-400 truncate max-w-[24rem]">{titulo}</div>
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

        <div className="p-5 space-y-3">
          <p className="text-[11px] text-slate-400 leading-relaxed">
            Una línea alcanza: qué la movió de fase. Queda guardada con la fecha y es lo que después
            explica cuánto tardó la idea en volverse algo usable.
          </p>
          <textarea
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                onGuardar(texto);
                onClose();
              }
            }}
            rows={4}
            autoFocus
            placeholder="Ej: lo probé con 3 personas y el feedback fue que sí; / lo descarté porque el costo no cierra…"
            className="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-xs text-slate-100 leading-relaxed focus:outline-none focus:border-sky-500"
          />
          <p className="text-[10px] text-slate-500">{texto.trim().length} car. · Ctrl+Enter guarda</p>
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
            onClick={() => {
              onGuardar('');
              onClose();
            }}
            className="px-3 py-2 rounded-xl text-xs font-medium text-slate-400 hover:text-slate-200 border border-slate-800 hover:border-slate-700 transition-colors cursor-pointer"
          >
            Sin evidencia
          </button>
          <button
            type="button"
            id="btn-guardar-evidencia"
            onClick={() => {
              onGuardar(texto);
              onClose();
            }}
            className="flex items-center gap-1.5 px-3 py-2 rounded-xl text-xs font-medium bg-sky-600/20 border border-sky-500/50 text-sky-100 hover:bg-sky-600/30 transition-colors cursor-pointer"
          >
            <FileText size={13} />
            Guardar con evidencia
          </button>
        </div>
      </div>
    </div>
  );
};
