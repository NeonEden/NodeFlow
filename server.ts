import express from "express";
import path from "path";
import fs from "fs";
import { fileURLToPath } from "url";
import dotenv from "dotenv";
import { GoogleGenAI, Type } from "@google/genai";

dotenv.config();

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = 3000;

app.use(express.json());

// Initialize Gemini client function supporting custom user API key or server-side env key
function getGeminiClient(customApiKey?: string): GoogleGenAI | null {
  const key = (customApiKey && customApiKey.trim()) || process.env.GEMINI_API_KEY;
  if (!key) return null;
  return new GoogleGenAI({
    apiKey: key,
    httpOptions: {
      headers: {
        "User-Agent": "aistudio-build",
      },
    },
  });
}

// Resilient model cascade to handle transient 503/429 spikes smoothly
const CANDIDATE_MODELS = [
  "gemini-3.6-flash",
  "gemini-3.1-flash-lite",
  "gemini-flash-latest",
  "gemini-3.8-flash",
];

async function callGeminiWithResilience(
  prompt: string,
  schema: any,
  systemInstruction?: string,
  customApiKey?: string
): Promise<{ parsed: any; modelUsed: string } | null> {
  const client = getGeminiClient(customApiKey);
  if (!client) return null;

  let lastError: any = null;

  for (const model of CANDIDATE_MODELS) {
    try {
      const response = await client.models.generateContent({
        model,
        contents: prompt,
        config: {
          responseMimeType: "application/json",
          responseSchema: schema,
          systemInstruction,
        },
      });

      const rawText = response.text?.trim() || "";
      // Strip code fence if present
      const cleaned = rawText.replace(/^```json\s*/i, "").replace(/```$/i, "").trim();
      const parsed = JSON.parse(cleaned);
      return { parsed, modelUsed: model };
    } catch (err: any) {
      lastError = err;
      const statusCode =
        err?.status ||
        err?.code ||
        (typeof err?.message === "string" && err.message.includes("503") ? 503 : 0);

      // On 503 (model busy), 429 (rate limit) or 404, immediately cascade to next model
      console.warn(`Model ${model} failed (${statusCode}):`, err?.message || err);
      continue;
    }
  }

  console.warn("All Gemini candidate models failed; activating intelligent fallback:", lastError?.message || lastError);
  return null;
}

// Health check endpoint
app.get("/api/health", (_req, res) => {
  res.json({
    status: "ok",
    hasApiKey: !!process.env.GEMINI_API_KEY,
    timestamp: new Date().toISOString(),
  });
});

// ==========================================
// MÓDULO 2: MOTOR DE AUTO-MEJORA Y HITL LOOP
// ==========================================

const DATA_DIR = path.join(process.cwd(), "data");
const USER_PREFERENCES_FILE = path.join(DATA_DIR, "user_preferences.json");

interface HitlDecision {
  accepted: string[];
  rejected: string[];
  added_manually: string[];
}

interface HitlFeedbackEvent {
  id: string;
  timestamp: string;
  action: "NODE_EDIT" | "NODE_DELETE" | "EDGE_CREATED" | "AI_ACCEPTED" | "HYBRIDIZE_FEEDBACK";
  prompt_original: string;
  ai_suggestion: string[];
  human_decision: HitlDecision;
  contextSnippet?: string;
  inferredPreference?: string;
}

interface HitlProfile {
  version: string;
  updatedAt: string;
  totalDecisions: number;
  acceptanceRate: number;
  learnedProfile: string;
  categoriesAccepted: string[];
  topicsRejected: string[];
  recentFeedback: HitlFeedbackEvent[];
}

const DEFAULT_HITL_PROFILE: HitlProfile = {
  version: "2.0",
  updatedAt: new Date().toISOString(),
  totalDecisions: 4,
  acceptanceRate: 85,
  learnedProfile:
    "El usuario prefiere un enfoque técnico, conciso y estructurado. Suele descartar conexiones genéricas o superficiales y favorece patrones de arquitectura de sistemas, código en Python y filosofía pragmática. Adapta las respuestas a esta preferencia aprendida.",
  categoriesAccepted: ["ARQUITECTURA", "SISTEMAS", "SEGURIDAD", "CRIPTOGRAFÍA"],
  topicsRejected: ["Conexiones genéricas", "Slogans superficiales", "Filtro de tokens"],
  recentFeedback: [
    {
      id: "hitl-seed-1",
      timestamp: new Date(Date.now() - 3600000).toISOString(),
      action: "NODE_EDIT",
      prompt_original: "Sugerir 3 conexiones para el nodo 'Guardrails'",
      ai_suggestion: ["Verificación de firma", "Base de datos vector", "Filtro de tokens"],
      human_decision: {
        accepted: ["Verificación de firma"],
        rejected: ["Filtro de tokens"],
        added_manually: ["Módulo de Auditoría Criptográfica"],
      },
      contextSnippet: "Nodo Guardrails refinado hacia arquitectura criptográfica",
      inferredPreference: "Alta prioridad a esquemas deterministas y seguridad",
    },
  ],
};

function getHitlProfile(): HitlProfile {
  try {
    if (fs.existsSync(USER_PREFERENCES_FILE)) {
      const raw = fs.readFileSync(USER_PREFERENCES_FILE, "utf-8");
      const parsed = JSON.parse(raw);
      return {
        ...DEFAULT_HITL_PROFILE,
        ...parsed,
        categoriesAccepted: Array.isArray(parsed.categoriesAccepted) ? parsed.categoriesAccepted : DEFAULT_HITL_PROFILE.categoriesAccepted,
        topicsRejected: Array.isArray(parsed.topicsRejected) ? parsed.topicsRejected : DEFAULT_HITL_PROFILE.topicsRejected,
        recentFeedback: Array.isArray(parsed.recentFeedback) ? parsed.recentFeedback : DEFAULT_HITL_PROFILE.recentFeedback,
      };
    }
  } catch (e) {
    console.warn("No se pudo leer data/user_preferences.json, usando valores predeterminados:", e);
  }
  return DEFAULT_HITL_PROFILE;
}

function saveHitlProfile(profile: HitlProfile) {
  try {
    if (!fs.existsSync(DATA_DIR)) {
      fs.mkdirSync(DATA_DIR, { recursive: true });
    }
    fs.writeFileSync(USER_PREFERENCES_FILE, JSON.stringify(profile, null, 2), "utf-8");
  } catch (e) {
    console.error("Error guardando data/user_preferences.json:", e);
  }
}

function buildHitlSystemInstruction(customOverride?: string): string {
  const profile = getHitlProfile();
  const baseProfile = (customOverride && customOverride.trim()) ? customOverride.trim() : profile.learnedProfile;
  const accepted = profile.categoriesAccepted.slice(-8).join(", ") || "Arquitectura, Sistemas, Métricas";
  const rejected = profile.topicsRejected.slice(-8).join(", ") || "Ideas superficiales, Slogans genéricos";

  return `Eres el motor cognitivo y analítico de NeuralMind con arquitectura HITL (Human-in-the-Loop Continuous Learning).

PERFIL ADAPTATIVO DEL USUARIO (Aprendido por retroalimentación humana continua):
"${baseProfile}"

DIRECTRICES DE CURADURÍA APRENDIDAS:
- Preferencias y temáticas aceptadas con frecuencia: ${accepted}
- Patrones o enfoques rechazados previamente por el usuario: ${rejected}

REGLAS DE GENERACIÓN ESTRICTAS:
1. Aplica un nivel de abstracción técnico riguroso, conciso y accionable.
2. Evita conceptos vagos, generalidades trilladas o contenido de relleno.
3. Cada propuesta debe ser conceptualmente densa y complementar la red de ideas.
4. Respeta rigurosamente el esquema JSON indicado.`;
}

// HITL Endpoints
app.get("/api/hitl/preferences", (_req, res) => {
  const profile = getHitlProfile();
  res.json({ success: true, profile });
});

app.post("/api/hitl/feedback", async (req, res) => {
  try {
    const feedback: Partial<HitlFeedbackEvent> = req.body;
    if (!feedback.action || !feedback.human_decision) {
      return res.status(400).json({ error: "Estructura de evento feedback inválida" });
    }

    const currentProfile = getHitlProfile();
    const event: HitlFeedbackEvent = {
      id: feedback.id || `hitl-${Date.now()}-${Math.floor(Math.random() * 1000)}`,
      timestamp: feedback.timestamp || new Date().toISOString(),
      action: feedback.action,
      prompt_original: feedback.prompt_original || "Interacción conceptual en el lienzo",
      ai_suggestion: Array.isArray(feedback.ai_suggestion) ? feedback.ai_suggestion : [],
      human_decision: {
        accepted: Array.isArray(feedback.human_decision?.accepted) ? feedback.human_decision.accepted : [],
        rejected: Array.isArray(feedback.human_decision?.rejected) ? feedback.human_decision.rejected : [],
        added_manually: Array.isArray(feedback.human_decision?.added_manually) ? feedback.human_decision.added_manually : [],
      },
      contextSnippet: feedback.contextSnippet || "",
      inferredPreference: feedback.inferredPreference || "",
    };

    // Update categories and topics
    const newAccepted = new Set([...currentProfile.categoriesAccepted]);
    event.human_decision.accepted.forEach((item) => {
      if (typeof item === "string" && item.trim()) newAccepted.add(item.trim());
    });
    event.human_decision.added_manually.forEach((item) => {
      if (typeof item === "string" && item.trim()) newAccepted.add(item.trim());
    });

    const newRejected = new Set([...currentProfile.topicsRejected]);
    event.human_decision.rejected.forEach((item) => {
      if (typeof item === "string" && item.trim()) newRejected.add(item.trim());
    });

    // Update history (keep latest 50)
    const updatedHistory = [event, ...currentProfile.recentFeedback].slice(0, 50);

    // Calculate acceptance rate
    let totalAcc = 0;
    let totalRej = 0;
    updatedHistory.forEach((ev) => {
      totalAcc += (ev.human_decision.accepted.length + ev.human_decision.added_manually.length);
      totalRej += ev.human_decision.rejected.length;
    });
    const totalDecisions = currentProfile.totalDecisions + 1;
    const acceptanceRate = totalAcc + totalRej > 0 ? Math.round((totalAcc / (totalAcc + totalRej)) * 100) : currentProfile.acceptanceRate;

    // Incremental heuristic refinement of learned profile summary
    let updatedLearnedProfile = currentProfile.learnedProfile;
    if (event.human_decision.added_manually.length > 0) {
      const topAdded = event.human_decision.added_manually.slice(0, 2).join(", ");
      if (!updatedLearnedProfile.includes(topAdded)) {
        updatedLearnedProfile = `${updatedLearnedProfile.replace(/\.$/, "")}. Incluye afinidad expresa por conceptos como: ${topAdded}.`;
      }
    }

    const updatedProfile: HitlProfile = {
      version: "2.0",
      updatedAt: new Date().toISOString(),
      totalDecisions,
      acceptanceRate,
      learnedProfile: updatedLearnedProfile,
      categoriesAccepted: Array.from(newAccepted).slice(-15),
      topicsRejected: Array.from(newRejected).slice(-15),
      recentFeedback: updatedHistory,
    };

    saveHitlProfile(updatedProfile);
    return res.json({ success: true, profile: updatedProfile });
  } catch (err) {
    console.error("Error processing HITL feedback:", err);
    return res.status(500).json({ error: "Error registrando telemetría HITL" });
  }
});

app.post("/api/hitl/profile", (req, res) => {
  try {
    const { learnedProfile } = req.body;
    if (typeof learnedProfile !== "string" || !learnedProfile.trim()) {
      return res.status(400).json({ error: "El perfil aprendido debe ser un texto válido" });
    }
    const profile = getHitlProfile();
    profile.learnedProfile = learnedProfile.trim();
    profile.updatedAt = new Date().toISOString();
    saveHitlProfile(profile);
    return res.json({ success: true, profile });
  } catch (err) {
    console.error("Error updating learned profile:", err);
    return res.status(500).json({ error: "Error actualizando perfil HITL" });
  }
});

app.post("/api/hitl/recalibrate", async (_req, res) => {
  try {
    const profile = getHitlProfile();
    const historySample = profile.recentFeedback.slice(0, 10).map((e) => ({
      prompt: e.prompt_original,
      accepted: e.human_decision.accepted,
      rejected: e.human_decision.rejected,
      added_manually: e.human_decision.added_manually,
    }));

    const hitlClient = getGeminiClient();
    if (hitlClient && historySample.length > 0) {
      const prompt = `Analiza estas decisiones recientes de curaduría de un usuario en un mapa mental (HITL Loop):
${JSON.stringify(historySample, null, 2)}

Sintetiza un perfil de estilo y preferencia cognitiva de 2 o 3 oraciones contundentes para inyectar en el system prompt.
Ejemplo: "El usuario prefiere un enfoque técnico, conciso y estructurado. Suele descartar conexiones genéricas y favorece patrones de arquitectura, código y filosofía pragmática."
Responde en formato JSON:
{"profile": "El usuario prefiere..."}`;

      const schema = {
        type: Type.OBJECT,
        properties: {
          profile: { type: Type.STRING },
        },
        required: ["profile"],
      };

      const result = await callGeminiWithResilience(prompt, schema);
      if (result?.parsed?.profile) {
        profile.learnedProfile = result.parsed.profile;
        profile.updatedAt = new Date().toISOString();
        saveHitlProfile(profile);
        return res.json({ success: true, profile, calibratedWithAi: true });
      }
    }

    // Default recalibration
    profile.learnedProfile = `El usuario prefiere un enfoque técnico y conciso. Prioriza ${profile.categoriesAccepted.slice(-3).join(", ") || "arquitectura y sistemas"}, descartando generalidades.`;
    profile.updatedAt = new Date().toISOString();
    saveHitlProfile(profile);
    return res.json({ success: true, profile, calibratedWithAi: false });
  } catch (err) {
    console.error("Error recalibrating HITL profile:", err);
    return res.status(500).json({ error: "Error recalibrando perfil HITL" });
  }
});

app.post("/api/hitl/reset", (_req, res) => {
  try {
    saveHitlProfile(DEFAULT_HITL_PROFILE);
    return res.json({ success: true, profile: DEFAULT_HITL_PROFILE });
  } catch (err) {
    console.error("Error resetting HITL profile:", err);
    return res.status(500).json({ error: "Error restableciendo perfil HITL" });
  }
});

// AI Node Action endpoint (branching, exploration, hybridization, synthesis)
app.post("/api/ai/action", async (req, res) => {
  const { type, nodeData, selectedNodes, hitlProfileOverride } = req.body;
  const customApiKey =
    (req.headers["x-gemini-api-key"] as string) ||
    (req.body?.customApiKey as string) ||
    undefined;
  const currentProfile = getHitlProfile();
  const systemInstruction = buildHitlSystemInstruction(hitlProfileOverride);

  try {
    if (type === "branch") {
      const title = nodeData?.title || "Idea Central";
      const description = nodeData?.description || "Sin descripción";

      const prompt = `Analiza este nodo: "${title}: ${description}".
Genera 3 sub-ideas o componentes técnicamente viables que expandan y complementen este concepto.
Responde exclusivamente en este formato JSON:
[
  {"category": "CATEGORIA", "title": "Titulo Corto", "description": "Explicacion concisa", "tags": ["tag1", "tag2"]}
]`;

      const branchSchema = {
        type: Type.ARRAY,
        items: {
          type: Type.OBJECT,
          properties: {
            category: { type: Type.STRING },
            title: { type: Type.STRING },
            description: { type: Type.STRING },
            tags: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
          },
          required: ["category", "title", "description", "tags"],
        },
      };

      const result = await callGeminiWithResilience(prompt, branchSchema, systemInstruction, customApiKey);
      if (result && Array.isArray(result.parsed) && result.parsed.length > 0) {
        return res.json({
          success: true,
          variations: result.parsed,
          modelUsed: result.modelUsed,
          hitlActive: true,
          learnedProfile: currentProfile.learnedProfile,
        });
      }

      // Intelligent fallback
      const fallbackVariations = [
        {
          category: "EVOLUCIÓN",
          title: `Evolución Autónoma de ${title}`,
          description: `Integración con automatización, flujos en tiempo real y APIs modernas para ${title.toLowerCase()}.`,
          tags: ["Digital", "Autónomo"],
        },
        {
          category: "EFICIENCIA",
          title: `Enfoque Minimalista y Ágil`,
          description: `Versión simplificada que reduce la complejidad operativa y prioriza la máxima velocidad de ejecución.`,
          tags: ["Eficiencia", "Lean"],
        },
        {
          category: "COMUNIDAD",
          title: `Capa Colaborativa & Red`,
          description: `Plataforma comunitaria de co-creación con incentivos y sincronización distribuida.`,
          tags: ["Comunidad", "Sinergia"],
        },
      ];

      return res.json({
        success: true,
        variations: fallbackVariations,
        hitlActive: true,
        learnedProfile: currentProfile.learnedProfile,
      });
    }

    if (type === "explore") {
      const title = nodeData?.title || "Idea";
      const description = nodeData?.description || "";

      const prompt = `Analiza a fondo el concepto: "${title}: ${description}".
Proporciona 3 ángulos de exploración analítica profunda:
1. Factibilidad Técnica e Infraestructura
2. Experiencia de Usuario y Adopción
3. Factores Críticos y Seguridad
Devuelve un JSON array con objetos que tengan: category, title, description, tags (array de 2 tags).`;

      const exploreSchema = {
        type: Type.ARRAY,
        items: {
          type: Type.OBJECT,
          properties: {
            category: { type: Type.STRING },
            title: { type: Type.STRING },
            description: { type: Type.STRING },
            tags: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
          },
          required: ["category", "title", "description", "tags"],
        },
      };

      const result = await callGeminiWithResilience(prompt, exploreSchema, systemInstruction, customApiKey);
      if (result && Array.isArray(result.parsed) && result.parsed.length > 0) {
        return res.json({
          success: true,
          variations: result.parsed,
          modelUsed: result.modelUsed,
          hitlActive: true,
          learnedProfile: currentProfile.learnedProfile,
        });
      }

      const fallbackExploration = [
        {
          category: "ARQUITECTURA",
          title: "Factibilidad y Arquitectura",
          description: `Evaluar requerimientos de infraestructura escalable y tiempos de respuesta para ${title}.`,
          tags: ["Técnico", "Escala"],
        },
        {
          category: "EXPERIENCIA",
          title: "Adopción y Experiencia de Usuario",
          description: `Diseñar flujos intuitivos y reducir la fricción para la interacción con ${title}.`,
          tags: ["UX", "Adopción"],
        },
        {
          category: "ESTRATEGIA",
          title: "Estrategia de Crecimiento",
          description: `Medir métricas clave de retención y casos de uso de alto impacto para la propuesta.`,
          tags: ["Estrategia", "Impacto"],
        },
      ];

      return res.json({
        success: true,
        variations: fallbackExploration,
        hitlActive: true,
        learnedProfile: currentProfile.learnedProfile,
      });
    }

    if (type === "hybrid") {
      const nodes = Array.isArray(selectedNodes) ? selectedNodes : [];
      const nodeA = nodes[0] || { title: "Idea A", description: "" };
      const nodeB = nodes[1] || { title: "Idea B", description: "" };

      const prompt = `Actúa como un sintetizador conceptual de vanguardia.
Fusiona creativamente estas dos ideas en una única propuesta híbrida innovadora:
Idea 1: "${nodeA.title}: ${nodeA.description}"
Idea 2: "${nodeB.title}: ${nodeB.description}"

Genera una síntesis innovadora con:
- "title": Nombre atractivo del concepto híbrido (máx. 5 palabras)
- "description": Explicación clara de cómo se combinan ambas fortalezas (2 oraciones)
- "tags": Lista de 2 o 3 etiquetas
- "rationale": Breve justificación de la sinergia`;

      const hybridSchema = {
        type: Type.OBJECT,
        properties: {
          title: { type: Type.STRING },
          description: { type: Type.STRING },
          tags: {
            type: Type.ARRAY,
            items: { type: Type.STRING },
          },
          rationale: { type: Type.STRING },
        },
        required: ["title", "description", "tags"],
      };

      const result = await callGeminiWithResilience(prompt, hybridSchema, systemInstruction, customApiKey);
      if (result && result.parsed?.title) {
        return res.json({
          success: true,
          hybrid: result.parsed,
          modelUsed: result.modelUsed,
          hitlActive: true,
          learnedProfile: currentProfile.learnedProfile,
        });
      }

      // Fallback hybrid
      const hybrid = {
        title: `Híbrido: ${nodeA.title.slice(0, 15)} + ${nodeB.title.slice(0, 15)}`,
        description: `Sinergia que combina la propuesta central de ${nodeA.title} con las fortalezas operativas de ${nodeB.title}.`,
        tags: ["Híbrido IA", "Sinergia", "Fusión"],
        rationale: "Unificación de conceptos complementarios para maximizar impacto.",
      };

      return res.json({
        success: true,
        hybrid,
        hitlActive: true,
        learnedProfile: currentProfile.learnedProfile,
      });
    }

    if (type === "synthesize") {
      const allNodes = Array.isArray(req.body.nodes) ? req.body.nodes : [];
      const nodeSummaries = allNodes
        .map((n: any) => `- [${n.data?.category || n.data?.label || 'Concepto'}] ${n.data?.title}: ${n.data?.description || ''} (${(n.data?.tags || []).join(', ')})`)
        .join('\n');

      if (allNodes.length > 0) {
        const prompt = `Analiza la siguiente red de nodos y mapa mental conceptual:
${nodeSummaries}

Genera una síntesis ejecutiva integral del mapa mental en formato JSON:
- "summary": Un párrafo conciso (3-4 oraciones) resumiendo la visión global del mapa.
- "pillars": Un array de 3 pilares clave identificados en la red de ideas.
- "actionItems": Un array de 3 a 5 próximos pasos recomendados para ejecutar este mapa.
- "keyOpportunities": 2 oportunidades de diferenciación o innovación identificadas.`;

        const synthesizeSchema = {
          type: Type.OBJECT,
          properties: {
            summary: { type: Type.STRING },
            pillars: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
            actionItems: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
            keyOpportunities: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
          },
          required: ["summary", "pillars", "actionItems", "keyOpportunities"],
        };

        const result = await callGeminiWithResilience(prompt, synthesizeSchema, systemInstruction, customApiKey);
        if (result && result.parsed?.summary) {
          return res.json({
            success: true,
            synthesis: result.parsed,
            modelUsed: result.modelUsed,
            hitlActive: true,
            learnedProfile: currentProfile.learnedProfile,
          });
        }
      }

      // Fallback synthesis
      const fallbackSynthesis = {
        summary: `El mapa actual articula ${allNodes.length} nodos interconectados con foco en innovación estratégica, desarrollo ágil y escalabilidad. Presenta un equilibrio entre fundamentos conceptuales y componentes de ejecución práctica.`,
        pillars: [
          "Arquitectura y Fundamentos de Conocimiento",
          "Diferenciación y Adopción por Usuarios",
          "Escalabilidad y Ejecución Continua"
        ],
        actionItems: [
          "Priorizar el desarrollo de los nodos raíz de mayor impacto",
          "Validar las hipótesis planteadas en las ramificaciones con usuarios de prueba",
          "Estructurar hitos trimestrales basados en los componentes explorados"
        ],
        keyOpportunities: [
          "Integración de automatización cognitiva en los procesos clave",
          "Consolidación de una propuesta de valor unificada frente a alternativas existentes"
        ]
      };

      return res.json({
        success: true,
        synthesis: fallbackSynthesis,
        hitlActive: true,
        learnedProfile: currentProfile.learnedProfile,
      });
    }

    if (type === "critique" || type === "devils_advocate") {
      const title = nodeData?.title || "Idea";
      const description = nodeData?.description || "";

      const prompt = `Actúa como un riguroso 'Abogado del Diablo', auditor crítico y analista de riesgos.
Examina objetivamente el concepto: "${title}: ${description}".
Detecta 3 puntos ciegos, contraargumentos, riesgos operativos o suposiciones no validadas que desafíen esta idea para hacerla más robusta.
Devuelve un JSON array con objetos: category ("PUNTO CIEGO" | "RIESGO CRÍTICO" | "ANTÍTESIS"), title (máx 5 palabras), description (explicación concisa del riesgo o contraargumento), tags (array de 2 tags).`;

      const critiqueSchema = {
        type: Type.ARRAY,
        items: {
          type: Type.OBJECT,
          properties: {
            category: { type: Type.STRING },
            title: { type: Type.STRING },
            description: { type: Type.STRING },
            tags: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
          },
          required: ["category", "title", "description", "tags"],
        },
      };

      const result = await callGeminiWithResilience(prompt, critiqueSchema, systemInstruction, customApiKey);
      if (result && Array.isArray(result.parsed) && result.parsed.length > 0) {
        return res.json({
          success: true,
          variations: result.parsed,
          modelUsed: result.modelUsed,
          hitlActive: true,
          learnedProfile: currentProfile.learnedProfile,
        });
      }

      const fallbackCritique = [
        {
          category: "PUNTO CIEGO",
          title: `Suposición Crítica No Validada`,
          description: `Se asume una adopción inmediata sin fricción operativa o resistencia al cambio en ${title}.`,
          tags: ["Adopción", "Sesgo"],
        },
        {
          category: "RIESGO CRÍTICO",
          title: `Cuello de Botella Operativo`,
          description: `La complejidad de mantenimiento o escalado técnico puede superar los beneficios tempranos de la solución.`,
          tags: ["Escalabilidad", "Costos"],
        },
        {
          category: "ANTÍTESIS",
          title: `Alternativa Simplificada Existente`,
          description: `Herramientas consolidadas o métodos manuales podrían resolver el 80% del problema con menor fricción.`,
          tags: ["Competencia", "Fricción"],
        },
      ];

      return res.json({
        success: true,
        variations: fallbackCritique,
        hitlActive: true,
        learnedProfile: currentProfile.learnedProfile,
      });
    }

    if (type === "socratic") {
      const title = nodeData?.title || "Idea";
      const description = nodeData?.description || "";

      const prompt = `Actúa como un mentor socrático de pensamiento profundo e inquisitivo.
Examina el concepto: "${title}: ${description}".
Formula 3 preguntas catalizadoras directas, profundas e incisivas (no complacientes) para forzar al pensador a clarificar y destrabar la esencia de la idea.
Devuelve un JSON array con objetos: category ("PREGUNTA CATALIZADORA" | "DESAFÍO COGNITIVO"), title (la pregunta en formato ¿...?, máx 7 palabras), description (por qué esta pregunta es crucial resolverla), tags (array de 2 tags).`;

      const socraticSchema = {
        type: Type.ARRAY,
        items: {
          type: Type.OBJECT,
          properties: {
            category: { type: Type.STRING },
            title: { type: Type.STRING },
            description: { type: Type.STRING },
            tags: {
              type: Type.ARRAY,
              items: { type: Type.STRING },
            },
          },
          required: ["category", "title", "description", "tags"],
        },
      };

      const result = await callGeminiWithResilience(prompt, socraticSchema, systemInstruction, customApiKey);
      if (result && Array.isArray(result.parsed) && result.parsed.length > 0) {
        return res.json({
          success: true,
          variations: result.parsed,
          modelUsed: result.modelUsed,
          hitlActive: true,
          learnedProfile: currentProfile.learnedProfile,
        });
      }

      const fallbackSocratic = [
        {
          category: "PREGUNTA CATALIZADORA",
          title: "¿Qué falla si esto tiene éxito?",
          description: "Evaluar consecuencias de segundo orden causadas por un incremento drástico en demanda o uso.",
          tags: ["Segundo Orden", "Estrategia"],
        },
        {
          category: "DESAFÍO COGNITIVO",
          title: "¿Cuál es el núcleo irreductible?",
          description: "Si tuvieras que eliminar el 80% de las partes accesorias, ¿qué componente seguiría aportando valor real?",
          tags: ["Esencia", "Minimalismo"],
        },
        {
          category: "PREGUNTA CATALIZADORA",
          title: "¿Para quién NO es esta solución?",
          description: "Definir los límites precisos de exclusión clarifica el foco y previene la dispersión del alcance.",
          tags: ["Foco", "Audiencia"],
        },
      ];

      return res.json({
        success: true,
        variations: fallbackSocratic,
        hitlActive: true,
        learnedProfile: currentProfile.learnedProfile,
      });
    }

    if (type === "find_bridges") {
      const allNodes = Array.isArray(req.body.nodes) ? req.body.nodes : [];
      const allEdges = Array.isArray(req.body.edges) ? req.body.edges : [];

      if (allNodes.length < 2) {
        return res.status(400).json({ error: "Se necesitan al menos 2 nodos en el mapa para hallar conexiones." });
      }

      const existingConnections = new Set(
        allEdges.map((e: any) => `${e.source}->${e.target}`).concat(allEdges.map((e: any) => `${e.target}->${e.source}`))
      );

      const nodeSummaries = allNodes.map((n: any) => ({
        id: n.id,
        title: n.data?.title || "Sin título",
        category: n.data?.category || n.data?.label || "Concepto",
        description: n.data?.description || "",
      }));

      const prompt = `Analiza estos nodos en un mapa mental conceptual:
${JSON.stringify(nodeSummaries, null, 2)}

Conexiones ya existentes:
${Array.from(existingConnections).slice(0, 30).join(", ")}

Encuentra de 2 a 3 conexiones ocultas, no obvias y de alto valor conceptual (Semantic Bridges) entre pares de nodos que NO estén ya conectados directamente.
Para cada puente, proporciona:
- sourceId: id exacto del nodo origen
- targetId: id exacto del nodo destino
- sourceTitle: título del nodo origen
- targetTitle: título del nodo destino
- label: etiqueta corta de la relación (ej: "Sinergia Cripto", "Mitiga Riesgo", "Optimiza UX", 2-3 palabras)
- rationale: justificación conceptual en 1-2 oraciones explicando por qué conectar estas ideas eleva el pensamiento del proyecto.

Responde exclusivamente en este formato JSON:
{
  "bridges": [
    {
      "sourceId": "id1",
      "targetId": "id2",
      "sourceTitle": "...",
      "targetTitle": "...",
      "label": "...",
      "rationale": "..."
    }
  ]
}`;

      const bridgesSchema = {
        type: Type.OBJECT,
        properties: {
          bridges: {
            type: Type.ARRAY,
            items: {
              type: Type.OBJECT,
              properties: {
                sourceId: { type: Type.STRING },
                targetId: { type: Type.STRING },
                sourceTitle: { type: Type.STRING },
                targetTitle: { type: Type.STRING },
                label: { type: Type.STRING },
                rationale: { type: Type.STRING },
              },
              required: ["sourceId", "targetId", "sourceTitle", "targetTitle", "label", "rationale"],
            },
          },
        },
        required: ["bridges"],
      };

      const result = await callGeminiWithResilience(prompt, bridgesSchema, systemInstruction, customApiKey);
      if (result && Array.isArray(result.parsed?.bridges) && result.parsed.bridges.length > 0) {
        return res.json({
          success: true,
          bridges: result.parsed.bridges,
          modelUsed: result.modelUsed,
          hitlActive: true,
        });
      }

      // Intelligent heuristic fallback for bridges
      const candidateBridges = [];
      for (let i = 0; i < allNodes.length && candidateBridges.length < 2; i++) {
        for (let j = i + 1; j < allNodes.length && candidateBridges.length < 2; j++) {
          const idA = allNodes[i].id;
          const idB = allNodes[j].id;
          if (!existingConnections.has(`${idA}->${idB}`) && !existingConnections.has(`${idB}->${idA}`)) {
            candidateBridges.push({
              id: `bridge-${Date.now()}-${candidateBridges.length}`,
              sourceId: idA,
              targetId: idB,
              sourceTitle: allNodes[i].data?.title || "Concepto A",
              targetTitle: allNodes[j].data?.title || "Concepto B",
              label: "Sinergia Estratégica",
              rationale: `Integrar "${allNodes[i].data?.title}" con "${allNodes[j].data?.title}" permite cruzar capacidades operativas y ampliar el impacto de ambas áreas.`,
            });
          }
        }
      }

      return res.json({
        success: true,
        bridges: candidateBridges,
        hitlActive: true,
      });
    }

    if (type === "braindump") {
      const rawText = req.body.rawText || "";
      if (!rawText.trim()) {
        return res.status(400).json({ error: "El texto de descarga mental no puede estar vacío." });
      }

      const prompt = `Actúa como un arquitecto de mapas mentales y estructurador cognitivo de alto rendimiento.
Analiza la siguiente entrada del usuario, que puede ser desde una idea simple (una frase o concepto breve) hasta notas desordenadas o una especificación compleja con múltiples párrafos:
"""
${rawText}
"""

INSTRUCCIONES DE PROCESAMIENTO:
1. Si la entrada es SIMPLE (una frase o pocas palabras): Extrae la idea nuclear en "root" y deriva proactivamente entre 3 y 5 sub-nodos lógicos esenciales (ej: Propuesta Central, Implementación Técnica, Adopción/Usuarios, Métricas de Éxito).
2. Si la entrada es COMPLEJA o EXTENSA: Sintetiza el propósito global en "root" y descompón los diferentes aspectos en 4 a 8 sub-nodos categorizados con precisión. Si algún aspecto depende de otro nodo secundario en vez del núcleo, indica en "connectsTo" el tempId correspondiente; de lo contrario, "connectsTo": "root".
3. Cada nodo debe tener un título conciso (máx. 5 palabras), una descripción breve y accionable (1-2 oraciones), una categoría en mayúsculas (ej: ARQUITECTURA, PRODUCTO, ESTRATEGIA, RIESGO, INVESTIGACIÓN, MÉTRICAS) y 2 tags clave.

Formato JSON esperado:
{
  "root": {
    "title": "Idea Central Concisa",
    "description": "Síntesis clara del propósito global",
    "category": "NÚCLEO",
    "tags": ["Tag1", "Tag2"]
  },
  "nodes": [
    {
      "tempId": "node-1",
      "connectsTo": "root",
      "title": "Título Corto",
      "description": "Detalle explicativo breve",
      "category": "CATEGORÍA",
      "tags": ["Tag1", "Tag2"]
    }
  ]
}`;

      const braindumpSchema = {
        type: Type.OBJECT,
        properties: {
          root: {
            type: Type.OBJECT,
            properties: {
              title: { type: Type.STRING },
              description: { type: Type.STRING },
              category: { type: Type.STRING },
              tags: {
                type: Type.ARRAY,
                items: { type: Type.STRING },
              },
            },
            required: ["title", "description", "category", "tags"],
          },
          nodes: {
            type: Type.ARRAY,
            items: {
              type: Type.OBJECT,
              properties: {
                tempId: { type: Type.STRING },
                connectsTo: { type: Type.STRING },
                title: { type: Type.STRING },
                description: { type: Type.STRING },
                category: { type: Type.STRING },
                tags: {
                  type: Type.ARRAY,
                  items: { type: Type.STRING },
                },
              },
              required: ["tempId", "connectsTo", "title", "description", "category", "tags"],
            },
          },
        },
        required: ["root", "nodes"],
      };

      const result = await callGeminiWithResilience(prompt, braindumpSchema, systemInstruction, customApiKey);
      if (result && result.parsed?.root && Array.isArray(result.parsed?.nodes)) {
        return res.json({
          success: true,
          structure: result.parsed,
          modelUsed: result.modelUsed,
          hitlActive: true,
        });
      }

      // Robust fallback heuristic: handles both bulleted lists, multi-line, or continuous paragraphs
      const cleanText = rawText.trim();
      let rawSegments = cleanText
        .split(/\r?\n+|[•\-*]\s+|;\s+|\.\s+(?=[A-Z0-9ÁÉÍÓÚ])/)
        .map((s: string) => s.replace(/^[-*•\d.]+\s*/, "").trim())
        .filter((s: string) => s.length > 0);

      if (rawSegments.length === 0) {
        rawSegments = [cleanText.slice(0, 50)];
      }

      const rootTitle = rawSegments[0].slice(0, 40) || "Idea Central";
      let childSegments = rawSegments.slice(1);

      // If user provided a single idea or sentence, extrapolate default functional pillars
      if (childSegments.length === 0) {
        childSegments = [
          `Implementación y Arquitectura de ${rootTitle.slice(0, 25)}`,
          "Validación con Usuarios y Casos de Uso",
          "Métricas Clave y Escalabilidad",
        ];
      }

      const fallbackStructure = {
        root: {
          title: rootTitle,
          description: cleanText.length > 80 ? `${cleanText.slice(0, 160)}...` : "Idea nuclear sintetizada a partir del volcado de pensamiento.",
          category: "NÚCLEO",
          tags: ["BrainDump", "Visión"],
        },
        nodes: childSegments.slice(0, 8).map((seg: string, idx: number) => {
          const categories = ["ESTRATEGIA", "ARQUITECTURA", "EJECUCIÓN", "VALIDACIÓN", "MÉTRICAS"];
          return {
            tempId: `node-${idx + 1}`,
            connectsTo: "root",
            title: seg.slice(0, 38) || `Componente ${idx + 1}`,
            description: seg.length > 38 ? seg : "Derivación estructurada a partir de la descarga conceptual.",
            category: categories[idx % categories.length],
            tags: ["Idea", "Estructura"],
          };
        }),
      };

      return res.json({
        success: true,
        structure: fallbackStructure,
        hitlActive: true,
      });
    }

    if (type === "refresh_templates") {
      const prompt = `Actúa como arquitecto de innovación conceptual de vanguardia.
Genera 5 nuevos núcleos de ideas conceptuales (plantillas de inicio de proyectos) manteniendo exactamente estos 5 tópicos/categorías:
1. "Tecnología" (ej: Inteligencia de Agentes, Neurotecnología, Edge AI, Computación Cuántica, Biología Sintética)
2. "Negocios" (ej: Modelos Circulares, Micro-SaaS B2B, Plataformas Algorítmicas, Finanzas Autónomas)
3. "Diseño" (ej: Computación Espacial, Interfaces Generativas, Diseño Biomimético, Arquitectura de Sistemas)
4. "Investigación" (ej: Modelos Cognitivos, Transición Energética, Epistemología de Redes Complejas)
5. "Esencial" (ej: Primeros Principios, Deep Work, Reducción de Ruido)

Para cada una de las 5 categorías, genera una estructura JSON con:
- "category": Una de "Tecnología", "Negocios", "Diseño", "Investigación", "Esencial"
- "title": Título atractivo e inspirador (máx. 6 palabras)
- "description": Resumen de 1-2 oraciones explicando la propuesta y visión
- "iconName": Uno de "Bot", "Rocket", "Compass", "BookOpen", "PlusCircle"
- "colorAccent": Color hex (#6366f1, #10b981, #f59e0b, #8b5cf6, #06b6d4)
- "root": { "title": string, "description": string, "tags": string[] }
- "nodes": Array de 3 a 4 sub-nodos derivados que ramifican la idea, cada uno con:
    - "title": Título conciso del concepto derivado
    - "description": Breve detalle explicativo
    - "category": Nombre de la sub-etiqueta (ej: "OPTIMIZACIÓN", "ARQUITECTURA", "VALIDACIÓN")
    - "tags": Array de 2 o 3 tags clave
    - "connectionLabel": Etiqueta de la relación con el núcleo`;

      const templatesSchema = {
        type: Type.ARRAY,
        items: {
          type: Type.OBJECT,
          properties: {
            category: { type: Type.STRING },
            title: { type: Type.STRING },
            description: { type: Type.STRING },
            iconName: { type: Type.STRING },
            colorAccent: { type: Type.STRING },
            root: {
              type: Type.OBJECT,
              properties: {
                title: { type: Type.STRING },
                description: { type: Type.STRING },
                tags: { type: Type.ARRAY, items: { type: Type.STRING } },
              },
              required: ["title", "description", "tags"],
            },
            nodes: {
              type: Type.ARRAY,
              items: {
                type: Type.OBJECT,
                properties: {
                  title: { type: Type.STRING },
                  description: { type: Type.STRING },
                  category: { type: Type.STRING },
                  tags: { type: Type.ARRAY, items: { type: Type.STRING } },
                  connectionLabel: { type: Type.STRING },
                },
                required: ["title", "description", "category", "tags"],
              },
            },
          },
          required: ["category", "title", "description", "root", "nodes"],
        },
      };

      const result = await callGeminiWithResilience(prompt, templatesSchema, systemInstruction, customApiKey);
      if (result && Array.isArray(result.parsed) && result.parsed.length > 0) {
        return res.json({
          success: true,
          templates: result.parsed,
          source: "gemini",
          modelUsed: result.modelUsed,
        });
      }

      return res.json({
        success: true,
        source: "fallback_rotation",
      });
    }

    return res.status(400).json({ error: "Tipo de acción no soportado" });
  } catch (error) {
    console.error("Error in /api/ai/action:", error);
    return res.status(500).json({ error: "Error procesando solicitud de IA" });
  }
});

// Vite middleware setup
async function startServer() {
  if (process.env.NODE_ENV !== "production") {
    const { createServer: createViteServer } = await import("vite");
    const vite = await createViteServer({
      server: {
        middlewareMode: true,
        hmr: process.env.DISABLE_HMR === "true" ? false : { overlay: false },
      },
      appType: "spa",
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(process.cwd(), "dist");
    app.use(express.static(distPath));
    app.get("*", (_req, res) => {
      res.sendFile(path.join(distPath, "index.html"));
    });
  }

  app.listen(PORT, "0.0.0.0", () => {
    console.log(`Server running on http://localhost:${PORT}`);
  });
}

startServer();
