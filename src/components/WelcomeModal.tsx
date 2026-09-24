import React from 'react';
import { Sparkles } from 'lucide-react';

interface WelcomeModalProps {
  isOpen: boolean;
  onClose: () => void;
  onOpenAiConfig: () => void;
  onOpenVoiceConfig: () => void;
}

export const WelcomeModal: React.FC<WelcomeModalProps> = ({ isOpen, onClose, onOpenAiConfig, onOpenVoiceConfig }) => {
  if (!isOpen) return null;
  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-in fade-in duration-150"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="bg-slate-900 border border-slate-800 w-full max-w-lg rounded-2xl shadow-2xl p-6 text-slate-200">
        <h2 className="text-xl font-bold mb-4 flex items-center gap-2">
          <Sparkles size={20} className="text-emerald-400" />
          Bienvenido a NodeFlow (Demo Web)
        </h2>
        <p className="mb-4 text-sm text-slate-400">
          Configura tu Proveedor de IA (Azure OpenAI, DeepSeek, OpenAI o tu endpoint local de Ollama) y (opcional) tu Proveedor de Voz/TTS.
        </p>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onOpenVoiceConfig}
            className="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded text-sm font-medium"
          >
            Configurar Voz
          </button>
          <button
            type="button"
            onClick={onOpenAiConfig}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded text-sm font-medium"
          >
            Configurar IA
          </button>
        </div>
      </div>
    </div>
  );
};
