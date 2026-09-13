import { FeedbackEvent, UserHitlProfile } from '../types';
import { apiUrl } from './apiBase';

const STORAGE_KEY = 'neuralmind_hitl_profile_cache';

export const DEFAULT_FRONTEND_PROFILE: UserHitlProfile = {
  version: '2.0',
  updatedAt: new Date().toISOString(),
  totalDecisions: 4,
  acceptanceRate: 85,
  learnedProfile:
    'El usuario prefiere un enfoque técnico, conciso y estructurado. Suele descartar conexiones genéricas o superficiales y favorece patrones de arquitectura de sistemas, código en Python y filosofía pragmática. Adapta las respuestas a esta preferencia aprendida.',
  categoriesAccepted: ['ARQUITECTURA', 'SISTEMAS', 'SEGURIDAD', 'CRIPTOGRAFÍA'],
  topicsRejected: ['Conexiones genéricas', 'Slogans superficiales', 'Filtro de tokens'],
  recentFeedback: [
    {
      id: 'seed-front-1',
      timestamp: new Date().toISOString(),
      action: 'NODE_EDIT',
      prompt_original: "Sugerir 3 conexiones para el nodo 'Guardrails'",
      ai_suggestion: ['Verificación de firma', 'Base de datos vector', 'Filtro de tokens'],
      human_decision: {
        accepted: ['Verificación de firma'],
        rejected: ['Filtro de tokens'],
        added_manually: ['Módulo de Auditoría Criptográfica'],
      },
      contextSnippet: 'Nodo Guardrails refinado hacia arquitectura criptográfica',
      inferredPreference: 'Alta prioridad a esquemas deterministas y seguridad',
    },
  ],
};

export async function fetchHitlProfile(): Promise<UserHitlProfile> {
  try {
    const res = await fetch(apiUrl('/api/hitl/preferences'));
    if (res.ok) {
      const data = await res.json();
      if (data.success && data.profile) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data.profile));
        return data.profile;
      }
    }
  } catch (err) {
    console.warn('Could not fetch HITL preferences from backend, using local cache:', err);
  }

  try {
    const cached = localStorage.getItem(STORAGE_KEY);
    if (cached) {
      return JSON.parse(cached);
    }
  } catch (e) {
    console.error('Error reading localStorage HITL cache:', e);
  }

  return DEFAULT_FRONTEND_PROFILE;
}

export async function recordHitlFeedback(event: FeedbackEvent): Promise<UserHitlProfile | null> {
  try {
    const res = await fetch(apiUrl('/api/hitl/feedback'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(event),
    });

    if (res.ok) {
      const data = await res.json();
      if (data.success && data.profile) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data.profile));
        return data.profile;
      }
    }
  } catch (err) {
    console.warn('Background HITL feedback recording postponed:', err);
  }

  // Optimistic fallback for local state
  try {
    const current = await fetchHitlProfile();
    const updatedHistory = [event, ...current.recentFeedback].slice(0, 50);
    const updatedProfile: UserHitlProfile = {
      ...current,
      totalDecisions: current.totalDecisions + 1,
      updatedAt: new Date().toISOString(),
      recentFeedback: updatedHistory,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(updatedProfile));
    return updatedProfile;
  } catch (e) {
    return null;
  }
}

export async function updateHitlProfile(learnedProfile: string): Promise<UserHitlProfile | null> {
  try {
    const res = await fetch(apiUrl('/api/hitl/profile'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ learnedProfile }),
    });

    if (res.ok) {
      const data = await res.json();
      if (data.success && data.profile) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data.profile));
        return data.profile;
      }
    }
  } catch (err) {
    console.error('Error saving learned profile:', err);
  }
  return null;
}

/**
 * Enciende/apaga el aprendizaje automático y cada cuántas decisiones recalibra solo.
 * La idea del motor de auto-mejora: que la app aprenda de lo que aceptás y descartás sin botones.
 */
export async function updateHitlAuto(activo: boolean, cada: number): Promise<UserHitlProfile | null> {
  try {
    const res = await fetch(apiUrl('/api/hitl/auto'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ activo, cada }),
    });
    if (res.ok) {
      const data = await res.json();
      if (data.success && data.profile) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data.profile));
        return data.profile;
      }
    }
  } catch {
    /* sin backend: el panel queda como estaba */
  }
  return null;
}

export async function recalibrateHitlProfile(): Promise<UserHitlProfile | null> {
  try {
    const res = await fetch(apiUrl('/api/hitl/recalibrate'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });

    if (res.ok) {
      const data = await res.json();
      if (data.success && data.profile) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data.profile));
        return data.profile;
      }
    }
  } catch (err) {
    console.error('Error recalibrating HITL profile:', err);
  }
  return null;
}

export async function resetHitlProfile(): Promise<UserHitlProfile | null> {
  try {
    const res = await fetch(apiUrl('/api/hitl/reset'), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    });

    if (res.ok) {
      const data = await res.json();
      if (data.success && data.profile) {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data.profile));
        return data.profile;
      }
    }
  } catch (err) {
    console.error('Error resetting HITL profile:', err);
  }
  return null;
}
