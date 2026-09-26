import React, { useState, useEffect } from 'react';
import { Key, X, ShieldCheck, Check, AlertCircle, Trash2, ExternalLink, Sparkles } from 'lucide-react';
import { apiUrl } from '../services/apiBase';
import { useIdioma } from '../i18n/useIdioma';

interface ApiKeyModalProps {
  isOpen: boolean;
  onClose: () => void;
  onKeyChange: (hasKey: boolean) => void;
}

interface ClaveEstado {
  campo: string;
  origen: string;
  huella: string;
  largo: number;
}

interface ClavesEstadoResponse {
  campos: ClaveEstado[];
}

export const ApiKeyModal: React.FC<ApiKeyModalProps> = ({ isOpen, onClose, onKeyChange }) => {
  const { t } = useIdioma();
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [hasSavedKey, setHasSavedKey] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [loading, setLoading] = useState(false);

  const checkSavedKey = async () => {
    try {
      const res = await fetch(apiUrl('/api/claves/estado'));
      if (res.ok) {
        const data: ClavesEstadoResponse = await res.json();
        const geminiKey = data.campos.find(c => c.campo === 'gemini_api_key');
        const hasKey = geminiKey && geminiKey.origen !== 'ausente';
        setHasSavedKey(!!hasKey);
        onKeyChange(!!hasKey);
      }
    } catch {
      // Fallback: try localStorage
      const stored = localStorage.getItem('user_gemini_api_key') || '';
      setHasSavedKey(!!stored.trim());
      onKeyChange(!!stored.trim());
    }
  };

  useEffect(() => {
    if (isOpen) {
      const stored = localStorage.getItem('user_gemini_api_key') || '';
      setApiKey(stored);
      setSaveSuccess(false);
      checkSavedKey();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = apiKey.trim();
    setLoading(true);
    try {
      if (trimmed) {
        const res = await fetch(apiUrl('/api/claves'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ campo: 'gemini_api_key', valor: trimmed }),
        });
        if (!res.ok) throw new Error('Error al guardar');
        localStorage.setItem('user_gemini_api_key', trimmed);
        setHasSavedKey(true);
        onKeyChange(true);
      } else {
        const res = await fetch(apiUrl('/api/claves'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ campo: 'gemini_api_key', borrar: true }),
        });
        if (!res.ok) throw new Error('Error al borrar');
        localStorage.removeItem('user_gemini_api_key');
        setHasSavedKey(false);
        onKeyChange(false);
      }
      setSaveSuccess(true);
      setTimeout(() => {
        setSaveSuccess(false);
        onClose();
      }, 900);
    } catch (err) {
      console.error('Error guardando/borrando clave:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleRemove = async () => {
    setLoading(true);
    try {
      const res = await fetch(apiUrl('/api/claves'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ campo: 'gemini_api_key', borrar: true }),
      });
      if (!res.ok) throw new Error('Error al borrar');
      localStorage.removeItem('user_gemini_api_key');
      setApiKey('');
      setHasSavedKey(false);
      onKeyChange(false);
      setSaveSuccess(true);
      setTimeout(() => {
        setSaveSuccess(false);
        onClose();
      }, 700);
    } catch (err) {
      console.error('Error borrando clave:', err);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="api-key-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-in fade-in duration-150"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="bg-slate-900 border border-slate-800 w-full max-w-lg rounded-2xl shadow-2xl flex flex-col overflow-hidden text-slate-200">
        {/* Header */}
        <div className="p-5 border-b border-slate-800/90 flex items-center justify-between bg-gradient-to-r from-emerald-950/40 via-slate-900 to-slate-900">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-emerald-600/20 border border-emerald-500/30 flex items-center justify-center text-emerald-400 shrink-0">
              <Key size={18} />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 id="api-key-modal-title" className="text-base font-bold text-white">
                  {t('apikey.titulo')}
                </h2>
                <span className="px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 text-[10px] font-mono border border-emerald-500/20">
                  {t('apikey.opcional')}
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-0.5">
                {t('apikey.descripcion')}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800 transition-colors cursor-pointer"
            title={t('modal.cerrarEsc')}
          >
            <X size={18} />
          </button>
        </div>

        {/* Security badge notice */}
        <div className="p-4 bg-slate-950/60 border-b border-slate-800/80 space-y-2">
          <div className="flex items-start gap-2.5 text-xs text-slate-300">
            <ShieldCheck size={16} className="text-emerald-400 shrink-0 mt-0.5" />
            <div className="space-y-1">
              <p className="font-semibold text-slate-200">
                {t('apikey.seguro.titulo')}
              </p>
              <p className="text-slate-400 leading-relaxed text-[11px]">
                {t('apikey.seguro.texto')}
              </p>
            </div>
          </div>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSave} className="p-5 space-y-4">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label htmlFor="user-api-key-input" className="text-xs font-semibold text-slate-300">
                {t('apikey.label')}
              </label>
              <button
                type="button"
                onClick={() => setShowKey(!showKey)}
                className="text-[11px] text-indigo-400 hover:text-indigo-300 cursor-pointer"
              >
                {showKey ? t('apikey.ocultar') : t('apikey.mostrar')}
              </button>
            </div>

            <div className="relative">
              <input
                id="user-api-key-input"
                type={showKey ? 'text' : 'password'}
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="AIzaSy..."
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-xs text-slate-200 placeholder:text-slate-600 focus:outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 font-mono"
                autoComplete="off"
                spellCheck="false"
              />
            </div>

            <div className="flex items-center justify-between text-[11px] text-slate-400 pt-1">
              <span>{t('apikey.sinClave')}</span>
              <a
                href="https://aistudio.google.com/app/apikey"
                target="_blank"
                rel="noreferrer"
                className="text-emerald-400 hover:text-emerald-300 inline-flex items-center gap-1 hover:underline"
              >
                <span>{t('apikey.obtener')}</span>
                <ExternalLink size={11} />
              </a>
            </div>
          </div>

          {/* Current Status Box */}
          <div className="p-3 rounded-xl bg-slate-950/40 border border-slate-800/80 flex items-center justify-between text-xs">
            <div className="flex items-center gap-2">
              <span className={`w-2 h-2 rounded-full ${hasSavedKey ? 'bg-emerald-400 shadow-sm shadow-emerald-400/50' : 'bg-indigo-400 shadow-sm shadow-indigo-400/50'}`} />
              <span className="text-slate-300 font-medium">
                {hasSavedKey ? t('apikey.usandoPersonal') : t('apikey.usandoServidor')}
              </span>
            </div>
            {hasSavedKey && (
              <button
                type="button"
                onClick={handleRemove}
                disabled={loading}
                className="text-rose-400 hover:text-rose-300 flex items-center gap-1 text-[11px] cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
                title="Quitar clave personal y volver a la del servidor"
              >
                <Trash2 size={12} />
                <span>{t('apikey.restablecer')}</span>
              </button>
            )}
          </div>

          {/* Action buttons */}
          <div className="pt-3 border-t border-slate-800/80 flex items-center justify-between gap-3">
            <button
              type="button"
              onClick={onClose}
              disabled={loading}
              className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs font-semibold transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {t('apikey.cancelar')}
            </button>
            <button
              type="submit"
              disabled={loading}
              className="flex items-center gap-1.5 px-5 py-2 bg-emerald-600 hover:bg-emerald-500 text-white rounded-xl text-xs font-bold transition-all shadow-md shadow-emerald-600/20 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {saveSuccess ? (
                <>
                  <Check size={14} />
                  <span>{t('apikey.guardado')}</span>
                </>
              ) : loading ? (
                <>
                  <span>{t('apikey.guardando')}</span>
                </>
              ) : (
                <>
                  <Sparkles size={14} />
                  <span>{t('apikey.guardar')}</span>
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
