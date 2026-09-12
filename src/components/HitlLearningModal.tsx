import React, { useState } from 'react';
import {
  Brain,
  Sparkles,
  CheckCircle2,
  XCircle,
  PlusCircle,
  RefreshCw,
  RotateCcw,
  Sliders,
  X,
  Database,
  Layers,
  History,
  Info,
  ShieldCheck,
} from 'lucide-react';
import { UserHitlProfile, FeedbackEvent } from '../types';
import { updateHitlProfile, recalibrateHitlProfile, resetHitlProfile } from '../services/hitlService';

interface HitlLearningModalProps {
  isOpen: boolean;
  onClose: () => void;
  profile: UserHitlProfile;
  onProfileUpdated: (updated: UserHitlProfile) => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

export const HitlLearningModal: React.FC<HitlLearningModalProps> = ({
  isOpen,
  onClose,
  profile,
  onProfileUpdated,
  showToast,
}) => {
  const [activeTab, setActiveTab] = useState<'profile' | 'telemetry' | 'systemPrompt'>('profile');
  const [editedPrompt, setEditedPrompt] = useState(profile.learnedProfile);
  const [isSaving, setIsSaving] = useState(false);
  const [isRecalibrating, setIsRecalibrating] = useState(false);
  const [isResetting, setIsResetting] = useState(false);

  // Sync state if profile prop changes
  React.useEffect(() => {
    setEditedPrompt(profile.learnedProfile);
  }, [profile.learnedProfile]);

  if (!isOpen) return null;

  const handleSaveProfile = async () => {
    setIsSaving(true);
    try {
      const updated = await updateHitlProfile(editedPrompt);
      if (updated) {
        onProfileUpdated(updated);
        showToast('Perfil de aprendizaje HITL guardado correctamente', 'success');
      } else {
        showToast('No se pudo sincronizar el perfil con el servidor', 'error');
      }
    } catch {
      showToast('Error al guardar el perfil HITL', 'error');
    } finally {
      setIsSaving(false);
    }
  };

  const handleRecalibrate = async () => {
    setIsRecalibrating(true);
    showToast('Analizando telemetría y recalibrando perfil con Gemini...', 'info');
    try {
      const updated = await recalibrateHitlProfile();
      if (updated) {
        onProfileUpdated(updated);
        setEditedPrompt(updated.learnedProfile);
        showToast('¡Perfil cognitivo recalibrado con éxito por Gemini!', 'success');
      } else {
        showToast('No se pudo recalibrar el perfil', 'error');
      }
    } catch {
      showToast('Error al recalibrar perfil', 'error');
    } finally {
      setIsRecalibrating(false);
    }
  };

  const handleReset = async () => {
    if (!window.confirm('¿Seguro que deseas restablecer el historial de telemetría y el perfil a sus valores por defecto?')) {
      return;
    }
    setIsResetting(true);
    try {
      const updated = await resetHitlProfile();
      if (updated) {
        onProfileUpdated(updated);
        setEditedPrompt(updated.learnedProfile);
        showToast('Motor HITL restablecido a valores base', 'info');
      }
    } catch {
      showToast('Error al restablecer perfil', 'error');
    } finally {
      setIsResetting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        id="hitl-modal-container"
        className="relative w-full max-w-3xl bg-slate-900 border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-5 border-b border-slate-800 bg-slate-950/60">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-violet-500/20 border border-violet-500/30 flex items-center justify-center text-violet-400">
              <Brain className="w-6 h-6" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-lg font-semibold text-white">Motor de Auto-Mejora y Aprendizaje Continuo</h2>
                <span className="px-2 py-0.5 text-xs font-semibold uppercase tracking-wider bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 rounded-full flex items-center gap-1">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                  HITL Activo
                </span>
              </div>
              <p className="text-xs text-slate-400">
                Afinación de contexto en tiempo real basada en tu curaduría humana en el lienzo
              </p>
            </div>
          </div>
          <button
            id="close-hitl-modal-btn"
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Metrics Ribbon */}
        <div className="grid grid-cols-3 gap-3 px-6 py-4 bg-slate-900/90 border-b border-slate-800/80">
          <div className="flex items-center gap-3 p-3 bg-slate-950/40 border border-slate-800/80 rounded-xl">
            <div className="p-2 bg-indigo-500/10 rounded-lg text-indigo-400">
              <History className="w-4 h-4" />
            </div>
            <div>
              <div className="text-xl font-bold text-white">{profile.totalDecisions}</div>
              <div className="text-[11px] text-slate-400">Decisiones HITL</div>
            </div>
          </div>

          <div className="flex items-center gap-3 p-3 bg-slate-950/40 border border-slate-800/80 rounded-xl">
            <div className="p-2 bg-emerald-500/10 rounded-lg text-emerald-400">
              <CheckCircle2 className="w-4 h-4" />
            </div>
            <div>
              <div className="text-xl font-bold text-white">{profile.acceptanceRate}%</div>
              <div className="text-[11px] text-slate-400">Tasa de Aceptación</div>
            </div>
          </div>

          <div className="flex items-center gap-3 p-3 bg-slate-950/40 border border-slate-800/80 rounded-xl">
            <div className="p-2 bg-purple-500/10 rounded-lg text-purple-400">
              <Database className="w-4 h-4" />
            </div>
            <div>
              <div className="text-sm font-semibold text-white">data/user_preferences.json</div>
              <div className="text-[11px] text-slate-400">Persistencia RAG Local</div>
            </div>
          </div>
        </div>

        {/* Navigation Tabs */}
        <div className="flex border-b border-slate-800 px-6 bg-slate-950/30">
          <button
            id="hitl-tab-profile"
            onClick={() => setActiveTab('profile')}
            className={`flex items-center gap-2 py-3 px-4 text-xs font-medium border-b-2 transition-colors ${
              activeTab === 'profile'
                ? 'border-violet-500 text-violet-400 font-semibold'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Sliders className="w-4 h-4" />
            Perfil Aprendido
          </button>
          <button
            id="hitl-tab-telemetry"
            onClick={() => setActiveTab('telemetry')}
            className={`flex items-center gap-2 py-3 px-4 text-xs font-medium border-b-2 transition-colors ${
              activeTab === 'telemetry'
                ? 'border-violet-500 text-violet-400 font-semibold'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Layers className="w-4 h-4" />
            Telemetría de Curaduría ({profile.recentFeedback.length})
          </button>
          <button
            id="hitl-tab-prompt"
            onClick={() => setActiveTab('systemPrompt')}
            className={`flex items-center gap-2 py-3 px-4 text-xs font-medium border-b-2 transition-colors ${
              activeTab === 'systemPrompt'
                ? 'border-violet-500 text-violet-400 font-semibold'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Sparkles className="w-4 h-4" />
            Inyección de Prompt Gemini
          </button>
        </div>

        {/* Modal Body */}
        <div className="p-6 overflow-y-auto space-y-6 flex-1 text-slate-300 text-sm">
          {activeTab === 'profile' && (
            <div className="space-y-5">
              <div>
                <label className="block text-xs font-medium text-slate-300 uppercase tracking-wider mb-2">
                  Perfil de Afinación Cognitiva (Auto-aprendido y Editable)
                </label>
                <textarea
                  id="hitl-learned-profile-input"
                  rows={4}
                  value={editedPrompt}
                  onChange={(e) => setEditedPrompt(e.target.value)}
                  className="w-full bg-slate-950/80 border border-slate-700/80 rounded-xl p-3.5 text-sm text-slate-200 focus:outline-none focus:border-violet-500 focus:ring-1 focus:ring-violet-500 transition-all font-mono leading-relaxed"
                  placeholder="Escribe el perfil de estilo para la IA..."
                />
                <p className="text-xs text-slate-400 mt-1.5 flex items-center gap-1.5">
                  <Info className="w-3.5 h-3.5 text-violet-400" />
                  Este texto se inyecta en el <code className="text-violet-300">systemInstruction</code> de cada ramificación, exploración e hibridación con Gemini.
                </p>
              </div>

              {/* Curated taxonomy chips */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="bg-slate-950/50 border border-slate-800 p-4 rounded-xl">
                  <div className="flex items-center gap-2 mb-2.5 text-xs font-semibold text-emerald-400 uppercase tracking-wider">
                    <CheckCircle2 className="w-4 h-4" />
                    Conceptos Aceptados Frecuentemente
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {profile.categoriesAccepted.map((cat, idx) => (
                      <span
                        key={idx}
                        className="px-2.5 py-1 bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 rounded-md text-xs"
                      >
                        {cat}
                      </span>
                    ))}
                  </div>
                </div>

                <div className="bg-slate-950/50 border border-slate-800 p-4 rounded-xl">
                  <div className="flex items-center gap-2 mb-2.5 text-xs font-semibold text-rose-400 uppercase tracking-wider">
                    <XCircle className="w-4 h-4" />
                    Patrones Descartados por el Usuario
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {profile.topicsRejected.map((top, idx) => (
                      <span
                        key={idx}
                        className="px-2.5 py-1 bg-rose-500/10 text-rose-300 border border-rose-500/20 rounded-md text-xs"
                      >
                        {top}
                      </span>
                    ))}
                  </div>
                </div>
              </div>

              {/* Action buttons */}
              <div className="flex flex-wrap items-center justify-between gap-3 pt-2">
                <div className="flex items-center gap-2">
                  <button
                    id="hitl-save-btn"
                    onClick={handleSaveProfile}
                    disabled={isSaving}
                    className="px-4 py-2 bg-violet-600 hover:bg-violet-500 active:bg-violet-700 text-white rounded-xl text-xs font-medium transition-colors shadow-lg shadow-violet-600/20 flex items-center gap-2"
                  >
                    {isSaving ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : <ShieldCheck className="w-3.5 h-3.5" />}
                    Guardar Perfil de Aprendizaje
                  </button>
                  <button
                    id="hitl-recalibrate-btn"
                    onClick={handleRecalibrate}
                    disabled={isRecalibrating}
                    className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-xl text-xs font-medium transition-colors border border-slate-700 flex items-center gap-2"
                  >
                    {isRecalibrating ? <RefreshCw className="w-3.5 h-3.5 animate-spin text-violet-400" /> : <Sparkles className="w-3.5 h-3.5 text-violet-400" />}
                    Recalibrar Perfil con Gemini
                  </button>
                </div>

                <button
                  id="hitl-reset-btn"
                  onClick={handleReset}
                  disabled={isResetting}
                  className="px-3 py-2 text-rose-400 hover:text-rose-300 hover:bg-rose-500/10 rounded-xl text-xs transition-colors flex items-center gap-1.5"
                >
                  <RotateCcw className="w-3.5 h-3.5" />
                  Restablecer
                </button>
              </div>
            </div>
          )}

          {activeTab === 'telemetry' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between text-xs text-slate-400 pb-2 border-b border-slate-800">
                <span>Últimas interacciones y decisiones capturadas en el lienzo</span>
                <span>{profile.recentFeedback.length} eventos guardados</span>
              </div>

              {profile.recentFeedback.length === 0 ? (
                <div className="text-center py-12 text-slate-500">
                  <Layers className="w-8 h-8 mx-auto mb-2 opacity-50" />
                  No hay eventos registrados aún. Edita, elimina o genera sugerencias para alimentar el motor HITL.
                </div>
              ) : (
                <div className="space-y-3">
                  {profile.recentFeedback.map((event: FeedbackEvent) => (
                    <div
                      key={event.id}
                      className="p-4 bg-slate-950/60 border border-slate-800/90 rounded-xl space-y-2.5 text-xs"
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span
                            className={`px-2 py-0.5 rounded font-mono text-[10px] font-bold ${
                              event.action === 'NODE_EDIT'
                                ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                                : event.action === 'NODE_DELETE'
                                ? 'bg-rose-500/20 text-rose-400 border border-rose-500/30'
                                : event.action === 'HYBRIDIZE_FEEDBACK'
                                ? 'bg-purple-500/20 text-purple-400 border border-purple-500/30'
                                : 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                            }`}
                          >
                            {event.action}
                          </span>
                          <span className="text-slate-300 font-medium">
                            {event.prompt_original}
                          </span>
                        </div>
                        <span className="text-[10px] text-slate-500 font-mono">
                          {new Date(event.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                        </span>
                      </div>

                      {/* AI suggestion */}
                      {event.ai_suggestion && event.ai_suggestion.length > 0 && (
                        <div className="text-slate-400 flex items-start gap-2">
                          <span className="text-slate-500 font-semibold min-w-[70px]">Propuesta IA:</span>
                          <div className="flex flex-wrap gap-1">
                            {event.ai_suggestion.map((sug, i) => (
                              <span key={i} className="px-1.5 py-0.5 bg-slate-800 text-slate-300 rounded text-[11px]">
                                {sug}
                              </span>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Human decision */}
                      <div className="text-slate-400 flex flex-col gap-1.5 pt-1 border-t border-slate-800/60">
                        {event.human_decision.accepted.length > 0 && (
                          <div className="flex items-center gap-2">
                            <span className="text-emerald-400 font-semibold min-w-[70px] flex items-center gap-1">
                              <CheckCircle2 className="w-3 h-3" /> Aceptado:
                            </span>
                            <div className="flex flex-wrap gap-1">
                              {event.human_decision.accepted.map((acc, i) => (
                                <span key={i} className="px-1.5 py-0.5 bg-emerald-500/10 text-emerald-300 rounded text-[11px]">
                                  {acc}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}

                        {event.human_decision.rejected.length > 0 && (
                          <div className="flex items-center gap-2">
                            <span className="text-rose-400 font-semibold min-w-[70px] flex items-center gap-1">
                              <XCircle className="w-3 h-3" /> Rechazado:
                            </span>
                            <div className="flex flex-wrap gap-1">
                              {event.human_decision.rejected.map((rej, i) => (
                                <span key={i} className="px-1.5 py-0.5 bg-rose-500/10 text-rose-300 rounded text-[11px]">
                                  {rej}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}

                        {event.human_decision.added_manually.length > 0 && (
                          <div className="flex items-center gap-2">
                            <span className="text-indigo-400 font-semibold min-w-[70px] flex items-center gap-1">
                              <PlusCircle className="w-3 h-3" /> Manual:
                            </span>
                            <div className="flex flex-wrap gap-1">
                              {event.human_decision.added_manually.map((add, i) => (
                                <span key={i} className="px-1.5 py-0.5 bg-indigo-500/10 text-indigo-300 rounded text-[11px]">
                                  {add}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {activeTab === 'systemPrompt' && (
            <div className="space-y-4">
              <p className="text-xs text-slate-400">
                Así es exactamente como el backend inyecta la afinación continua de tu curaduría dentro del <code className="text-violet-300">systemInstruction</code> para las llamadas a Gemini:
              </p>
              <div className="bg-slate-950 p-4 rounded-xl border border-slate-800 font-mono text-xs text-emerald-400/90 whitespace-pre-wrap leading-relaxed">
{`Eres el motor cognitivo y analítico de NodeFlow con arquitectura HITL (Human-in-the-Loop Continuous Learning).

PERFIL ADAPTATIVO DEL USUARIO:
"${profile.learnedProfile}"

DIRECTRICES DE CURADURÍA APRENDIDAS:
- Preferencias y temáticas aceptadas con frecuencia: ${profile.categoriesAccepted.join(', ')}
- Patrones o enfoques rechazados previamente por el usuario: ${profile.topicsRejected.join(', ')}

REGLAS DE GENERACIÓN ESTRICTAS:
1. Aplica un nivel de abstracción técnico riguroso, conciso y accionable.
2. Evita conceptos vagos, generalidades trilladas o contenido de relleno.
3. Cada propuesta debe ser conceptualmente densa y complementar la red de ideas.
4. Respeta rigurosamente el esquema JSON indicado.`}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-slate-800 bg-slate-950/60 flex items-center justify-between text-xs text-slate-400">
          <div className="flex items-center gap-1.5">
            <span className="w-2 h-2 rounded-full bg-emerald-400" />
            <span>Sincronizado con base de datos local y backend Express</span>
          </div>
          <button
            onClick={onClose}
            className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-xl transition-colors"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
