/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 */

import React, { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import ReactFlow, {
  addEdge,
  Background,
  Controls,
  MiniMap,
  applyEdgeChanges,
  applyNodeChanges,
  Connection,
  Edge,
  NodeChange,
  EdgeChange,
  ReactFlowInstance,
  OnSelectionChangeParams,
} from 'reactflow';
import 'reactflow/dist/style.css';

import { Bot,  FlaskConical,
  Telescope,
  Sparkles,
  Inbox,
  Database,
  BookOpen,
  Sprout,
  WandSparkles,
  Save,
  Check,
  AlertCircle,
  Info,
  BrainCircuit,
  Maximize2,
  Compass,
  Keyboard,
  History,
  Search,
  LayoutTemplate,
  Trash2,
  Target,
  Mic,
  RotateCcw,
  Brain,
  Zap,
  Network,
  X,
  Link2,
  Layers,
} from 'lucide-react';
import { IdeaNode } from './components/IdeaNode';
import { FlowEdge } from './components/FlowEdge';
import { ZonaNode } from './components/ZonaNode';
import {
  setHoveredEdge,
  setHoveredNode,
  setFocusSelection,
  setLente,
} from './state/focusStore';
import { ZonasHud, CategoriaZona, ModoZonas } from './components/ZonasHud';
import { temaVars } from './state/canvasTheme';
import { useTema } from './state/canvasPrefs';
import { AparienciaHud } from './components/AparienciaHud';
import { calcularNiveles, acentoDeNivel } from './utils/zonas';
import { firmaLienzo, nombreDeSesion } from './utils/sesiones';
import {
  listarSesiones,
  leerSesion,
  guardarSesion,
  borrarSesion,
  type SesionFicha,
} from './services/sesionesApi';
import { Toolbar } from './components/Toolbar';
import { AuthModal } from './components/AuthModal';
import { SavedStatesModal } from './components/SavedStatesModal';
import { NodeEditModal } from './components/NodeEditModal';
import { SynthesisModal, MapSynthesis } from './components/SynthesisModal';
import { LinajeModal } from './components/LinajeModal';
import { AgentePanel } from './components/AgentePanel';
import { VozPanel } from './components/VozPanel';
import { EvaluacionPanel } from './components/EvaluacionPanel';
import { InvestigacionPanel, type EstadoInvestigacion, type PasoInvestigacion } from './components/InvestigacionPanel';
import { CerebroPanel } from './components/CerebroPanel';
import { SiguientePanel } from './components/SiguientePanel';
import type { PlanVoz } from './services/vozService';
import { medirContraste, resumenContraste } from './utils/contraste';
import { TemplatesModal } from './components/TemplatesModal';
import { ClearCanvasModal } from './components/ClearCanvasModal';
import { useIdioma } from './i18n/useIdioma';
import { HitlLearningModal } from './components/HitlLearningModal';
import { KeyboardShortcutsModal } from './components/KeyboardShortcutsModal';
import { ApiKeyModal } from './components/ApiKeyModal';
import { BrainDumpModal } from './components/BrainDumpModal';
import { SemanticBridgesModal } from './components/SemanticBridgesModal';
import { postAiAction } from './services/aiApi';
import { apiUrl } from './services/apiBase';
import { motorActual } from './state/motorActual';
import { autoLayoutNodes } from './utils/layout';
import { useUndoRedo } from './hooks/useUndoRedo';
import { getInitialUser, saveCurrentUser } from './services/auth';
import {
  INITIAL_TEMPLATES,
  TemplateDefinition,
  getNextFreshTemplates,
  formatAiGeneratedTemplates,
} from './data/templates';
import { fetchHitlProfile, recordHitlFeedback, DEFAULT_FRONTEND_PROFILE } from './services/hitlService';
import { fetchVaultInfo, loadVaultState, pollVault, saveVault, type VaultInfo } from './services/vaultService';
import { fetchPendientes, type Propuesta } from './services/agentService';
import type { Metricas } from './services/vaultService';
import { AgentChangesPanel } from './components/AgentChangesPanel';
import { MemoriaPanel } from './components/MemoriaPanel';
import { ConocimientoPanel } from './components/ConocimientoPanel';
import { JardinPanel } from './components/JardinPanel';
import { OrquestadorPanel } from './components/OrquestadorPanel';
import {
  CustomNode,
  EdgeAppearance,
  IdeaNodeData,
  SavedState,
  UserProfile,
  FeedbackEvent,
  UserHitlProfile,
  SemanticBridge,
  IdeaMaturityLevel,
  MATURITY_CONFIGS,
} from './types';

// IMPORTANTE: Declarados fuera del componente funcional para evitar recreación en cada render y warnings de React Flow
const NODE_TYPES = {
  ideaNode: IdeaNode,
  zonaNode: ZonaNode,
};

const EDGE_TYPES = { flowEdge: FlowEdge };

/** Cuánto espera la sesión rodante entre escrituras (10 min): es una red de seguridad, no un log. */
const RODANTE_MS = 10 * 60 * 1000;

const SAVED_STATES_STORAGE_KEY = 'neuralmind_saved_diagram_states';
/** Cuál de las sesiones está cargada en el lienzo (sobrevive a la recarga, igual que las sesiones). */
const SESION_ACTIVA_STORAGE_KEY = 'nodeflow_sesion_activa';
const ACTIVE_CANVAS_STORAGE_KEY = 'neuralmind_active_canvas_v2';

// Carga inicial persistente de la sesión del usuario para que nunca pierda su trabajo
function loadInitialCanvas(): {
  nodes: CustomNode[];
  edges: Edge[];
  appearance: EdgeAppearance;
  templateId: string;
} {
  try {
    const stored = localStorage.getItem(ACTIVE_CANVAS_STORAGE_KEY);
    if (stored) {
      const parsed = JSON.parse(stored);
      if (Array.isArray(parsed.nodes) && parsed.nodes.length > 0) {
        return {
          nodes: parsed.nodes,
          edges: Array.isArray(parsed.edges) ? parsed.edges : [],
          appearance: parsed.appearance || INITIAL_TEMPLATES[0].appearance,
          templateId: parsed.templateId || 'custom',
        };
      }
    }
  } catch (err) {
    console.error('Error cargando lienzo persistente desde localStorage', err);
  }
  const defaultTemplate = INITIAL_TEMPLATES[0];
  return {
    nodes: defaultTemplate.nodes,
    edges: defaultTemplate.edges,
    appearance: defaultTemplate.appearance,
    templateId: defaultTemplate.id,
  };
}

/**
 * Coloca un nodo nuevo en un lugar **libre**.
 *
 * Los planes (voz, investigación, sugerencias), las variaciones de Explorar y los hijos de Ramificar
 * calculaban la posición relativa al padre o desde un máximo global: al repetir la acción, o al caer
 * al lado de un nodo que ya existía, el nuevo quedaba **encima** de otro. Esto recorre una espiral
 * alrededor de la posición deseada hasta encontrar un hueco — sin mover nada de lo que ya está, que
 * es la diferencia con el reacomodo del jardín (ese reordena todo; este sólo ubica al recién llegado).
 */
const TAM_NODO = { w: 240, h: 120 };
function posicionLibre(
  nodos: { position: { x: number; y: number } }[],
  deseada: { x: number; y: number }
): { x: number; y: number } {
  const ocupado = (q: { x: number; y: number }) =>
    nodos.some((n) => Math.abs(n.position.x - q.x) < TAM_NODO.w && Math.abs(n.position.y - q.y) < TAM_NODO.h);
  if (!ocupado(deseada)) return deseada;
  for (let anillo = 1; anillo <= 14; anillo++) {
    for (let dx = -anillo; dx <= anillo; dx++) {
      for (let dy = -anillo; dy <= anillo; dy++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== anillo) continue; // sólo el borde del anillo
        const q = { x: deseada.x + dx * 80, y: deseada.y + dy * 80 };
        if (!ocupado(q)) return q;
      }
    }
  }
  return { x: deseada.x, y: deseada.y + 480 }; // sin hueco cerca: abajo, nunca encima
}

export default function App() {
  const { t } = useIdioma();
  // 1. Initial State from persistent localStorage or default template
  const initialCanvas = useMemo(() => loadInitialCanvas(), []);
  const [nodes, setNodes] = useState<CustomNode[]>(initialCanvas.nodes);
  const [edges, setEdges] = useState<Edge[]>(initialCanvas.edges);
  const [edgeAppearance, setEdgeAppearance] = useState<EdgeAppearance>(initialCanvas.appearance);
  const [currentTemplateId, setCurrentTemplateId] = useState<string>(initialCanvas.templateId);
  const [saveStatus, setSaveStatus] = useState<'saved' | 'saving' | 'unsaved'>('saved');

  // 2. Selection & Modal States
  const [selectedNodes, setSelectedNodes] = useState<CustomNode[]>([]);
  const [selectedEdges, setSelectedEdges] = useState<Edge[]>([]);
  const [editingNode, setEditingNode] = useState<IdeaNodeData | null>(null);
  const [isAuthModalOpen, setIsAuthModalOpen] = useState(false);
  const [isStatesModalOpen, setIsStatesModalOpen] = useState(false);
  const [statesModalTab, setStatesModalTab] = useState<'saved' | 'obsidian' | 'export' | 'import'>('saved');
  const [isTemplatesModalOpen, setIsTemplatesModalOpen] = useState(false);
  const [templates, setTemplates] = useState<TemplateDefinition[]>(() => {
    try {
      const saved = localStorage.getItem('nodeflow_active_templates');
      if (saved) {
        const parsed = JSON.parse(saved);
        if (Array.isArray(parsed) && parsed.length > 0) return parsed;
      }
    } catch {}
    return INITIAL_TEMPLATES;
  });
  const [isRefreshingTemplates, setIsRefreshingTemplates] = useState(false);
  const [isClearModalOpen, setIsClearModalOpen] = useState(false);
  const [toastMessage, setToastMessage] = useState<{ text: string; type: 'success' | 'info' | 'error' } | null>(null);
  const [isAiLoading, setIsAiLoading] = useState(false);

  // Search & Filter
  const [searchQuery, setSearchQuery] = useState('');

  // AI Whole-Map Synthesis
  const [isSynthesisModalOpen, setIsSynthesisModalOpen] = useState(false);
  const [mapSynthesis, setMapSynthesis] = useState<MapSynthesis | null>(null);
  const [isSynthesisLoading, setIsSynthesisLoading] = useState(false);
  const [isShortcutsOpen, setIsShortcutsOpen] = useState(false);
  const [isApiKeyModalOpen, setIsApiKeyModalOpen] = useState(false);
  const [hasCustomApiKey, setHasCustomApiKey] = useState(() => {
    try {
      return !!(localStorage.getItem('user_gemini_api_key') || '').trim();
    } catch {
      return false;
    }
  });

  // Brain Dump (Descarga Mental Rápida)
  const [isBrainDumpOpen, setIsBrainDumpOpen] = useState(false);
  const [isBrainDumpLoading, setIsBrainDumpLoading] = useState(false);

  // Detector de Conexiones Ocultas (Semantic Bridges)
  const [isBridgesModalOpen, setIsBridgesModalOpen] = useState(false);
  const [bridges, setBridges] = useState<SemanticBridge[]>([]);
  const [isBridgesLoading, setIsBridgesLoading] = useState(false);
  const [connectedBridgeIds, setConnectedBridgeIds] = useState<Set<string>>(new Set());

  // 3. User Authentication & HITL Continuous Learning
  const [currentUser, setCurrentUser] = useState<UserProfile | null>(getInitialUser);
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [zoomPercent, setZoomPercent] = useState(100);
  // Zonas derivadas (marcos por nivel): sólo UI, no se guardan ni se sincronizan.
  const [zonasVisibles, setZonasVisibles] = useState(true);
  const [modoZonas, setModoZonas] = useState<ModoZonas>('nivel');
  const [categoriaLente, setCategoriaLente] = useState<string | null>(null);
  // Apariencia del lienzo (fondo + superficie de tarjeta), preferencia local.
  const tema = useTema();
  const varsTema = useMemo(() => temaVars(tema), [tema]);
  const [lastSyncText, setLastSyncText] = useState('Reciente');

  // Módulo 2: Motor HITL Loop & Aprendizaje Continuo
  const [hitlProfile, setHitlProfile] = useState<UserHitlProfile>(DEFAULT_FRONTEND_PROFILE);
  const [isHitlModalOpen, setIsHitlModalOpen] = useState(false);

  // ── Fase 3: vault en disco (fuente de verdad) ──────────────────────────────
  const [vaultInfo, setVaultInfo] = useState<VaultInfo | null>(null);
  const vaultRevRef = useRef(0);
  /** Último info del vault conocido: sin él, el polling no puede usar el atajo `since`. */
  const vaultInfoRef = useRef<VaultInfo | null>(null);
  useEffect(() => {
    vaultInfoRef.current = vaultInfo;
  }, [vaultInfo]);
  const vaultReadyRef = useRef(false);

  // Fase 5a: propuestas del agente esperando aprobación (tienen su propia revisión).
  const [isAgentPanelOpen, setIsAgentPanelOpen] = useState(false);
  const [propuestas, setPropuestas] = useState<Propuesta[]>([]);
  const pendingRevRef = useRef(0);
  const pendientesPreviosRef = useRef(0);

  // Fase 5b: memoria semántica de la bóveda
  const [isMemoriaOpen, setIsMemoriaOpen] = useState(false);
  const [isConocimientoOpen, setIsConocimientoOpen] = useState(false);
  const [isJardinOpen, setIsJardinOpen] = useState(false);
  const [isVozOpen, setIsVozOpen] = useState(false);
  const [isAgenteOpen, setIsAgenteOpen] = useState(false);
  const [isEvaluacionOpen, setIsEvaluacionOpen] = useState(false);
  const [isInvestigacionOpen, setIsInvestigacionOpen] = useState(false);
  const [isCerebroOpen, setIsCerebroOpen] = useState(false);
  const [isSiguienteOpen, setIsSiguienteOpen] = useState(false);

  // Auditoría de contraste a mano: en la consola del WebView (o desde devtools) `nfContraste()`.
  // Recorre la UI real y devuelve los textos que no llegan al mínimo AA. Sirve para que este tipo
  // de bug se mida en vez de descubrirse mirando.
  useEffect(() => {
    (window as unknown as { nfContraste?: () => unknown }).nfContraste = () => {
      const violaciones = medirContraste(document.body);
      // eslint-disable-next-line no-console
      console.log(resumenContraste(violaciones));
      return violaciones;
    };
  }, []);
  const [isOrquestadorOpen, setIsOrquestadorOpen] = useState(false);

  // Fase 7b: métrica de valor (T0 → T1)
  const [metricas, setMetricas] = useState<Metricas | null>(null);

  // Carga inicial del perfil HITL aprendido
  useEffect(() => {
    fetchHitlProfile().then((profile) => {
      if (profile) {
        setHitlProfile(profile);
      }
    });
  }, []);

  // 4. Sesiones del lienzo: viven en la bóveda (`.nodeflow/sesiones/`), no en el WebView. El listado
  // es liviano (sin nodos ni aristas); el contenido se pide al cargar una sesión.
  const [sesiones, setSesiones] = useState<SesionFicha[]>([]);

  const refrescarSesiones = useCallback(async () => {
    setSesiones(await listarSesiones());
  }, []);

  // Sesión cargada en el lienzo: su id y la firma del contenido que tenía al cargarla. Sirve para
  // marcar «en el lienzo» y avisar cuando el lienzo se movió respecto de la sesión («modificada»).
  const [sesionActiva, setSesionActiva] = useState<{ id: string; nombre: string; firma: string } | null>(
    () => {
      try {
        const stored = localStorage.getItem(SESION_ACTIVA_STORAGE_KEY);
        if (stored) return JSON.parse(stored);
      } catch (e) {
        console.error('Error reading the active session from localStorage', e);
      }
      return null;
    },
  );

  useEffect(() => {
    try {
      if (sesionActiva) {
        localStorage.setItem(SESION_ACTIVA_STORAGE_KEY, JSON.stringify(sesionActiva));
      } else {
        localStorage.removeItem(SESION_ACTIVA_STORAGE_KEY);
      }
    } catch (e) {
      console.error('Error persisting the active session', e);
    }
  }, [sesionActiva]);

  const rfInstanceRef = useRef<ReactFlowInstance | null>(null);
  const reactFlowWrapperRef = useRef<HTMLDivElement | null>(null);
  const handleAddChildNodeRef = useRef<(node: CustomNode) => void>(() => {});
  const handleAddSiblingNodeRef = useRef<(node: CustomNode) => void>(() => {});
  const handleAIActionRef = useRef<any>(null);
  const handleConnectSelectedNodesRef = useRef<() => void>(() => {});
  const handleHybridizeRef = useRef<() => void>(() => {});
  const handleCondensarRef = useRef<() => void>(() => {});

  // Fase A — Norte Estratégico: el objetivo que actúa de lente al condensar (persiste entre sesiones).
  const [norte, setNorte] = useState<string>(() => {
    try {
      return localStorage.getItem('nodeflow_norte') || '';
    } catch {
      return '';
    }
  });
  const [isNorteOpen, setIsNorteOpen] = useState(false);
  const [nodoLinaje, setNodoLinaje] = useState<CustomNode | null>(null);
  const [isLinajeOpen, setIsLinajeOpen] = useState(false);

  const guardarNorte = (v: string) => {
    setNorte(v);
    try {
      localStorage.setItem('nodeflow_norte', v);
    } catch {
      /* almacenamiento restringido */
    }
  };

  // 5. Undo / Redo Hook
  const {
    canUndo,
    canRedo,
    undoCount,
    redoCount,
    takeSnapshot,
    undo,
    redo,
    resetHistory,
    updateCurrent,
  } = useUndoRedo(nodes, edges);

  // Keep history updated with current reference
  useEffect(() => {
    updateCurrent(nodes, edges);
  }, [nodes, edges, updateCurrent]);

  // Show auto-clearing toast
  const showToast = useCallback((text: string, type: 'success' | 'info' | 'error' = 'info') => {
    setToastMessage({ text, type });
    const timer = setTimeout(() => {
      setToastMessage(null);
    }, 4000);
    return () => clearTimeout(timer);
  }, []);

  // Avisos del backend que no son errores de la acción: p. ej. el 409 «ya hay una generación en curso».
  useEffect(() => {
    const aviso = (e: Event) => {
      const detalle = (e as CustomEvent).detail;
      if (detalle) showToast(String(detalle), 'info');
    };
    window.addEventListener('nodeflow:aviso', aviso);
    return () => window.removeEventListener('nodeflow:aviso', aviso);
  }, [showToast]);

  // Las sesiones se cargan de la bóveda al abrir (y se migran las que vivían en el WebView).
  useEffect(() => {
    let cancelado = false;
    (async () => {
      const enDisco = await listarSesiones();
      if (cancelado) return;

      // Migración única: los estados guardados en el `localStorage` de versiones anteriores pasan a
      // ser archivos de la bóveda, así dejan de ser invisibles para el respaldo.
      if (enDisco.length === 0) {
        try {
          const crudo = localStorage.getItem(SAVED_STATES_STORAGE_KEY);
          const previas = crudo ? (JSON.parse(crudo) as SavedState[]) : [];
          if (previas.length) {
            for (const s of previas) {
              await guardarSesion({
                id: s.id && /^s?-[a-z0-9-]*$/.test(s.id) ? s.id : undefined,
                nombre: s.name || 'Sesión migrada',
                nodes: s.nodes || [],
                edges: s.edges || [],
                appearance: s.edgeAppearance,
              });
            }
            localStorage.removeItem(SAVED_STATES_STORAGE_KEY);
            showToast(`Migré ${previas.length} sesión(es) del navegador a la bóveda`, 'success');
          }
        } catch (e) {
          console.warn('sesiones: no pude migrar las del navegador', e);
        }
      }
      setSesiones(await listarSesiones());
    })();
    return () => {
      cancelado = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Al abrir: el vault en disco manda sobre la caché de localStorage.
  useEffect(() => {
    let cancelado = false;
    (async () => {
      const snap = await loadVaultState();
      if (cancelado) return;
      if (snap?.state?.nodes?.length) {
        setNodes(snap.state.nodes);
        setEdges(snap.state.edges || []);
        if (snap.state.appearance) setEdgeAppearance(snap.state.appearance as EdgeAppearance);
        if (snap.state.templateId) setCurrentTemplateId(snap.state.templateId);
        vaultRevRef.current = snap.revision;
        setLastSyncText('Vault en disco');
        showToast(`Vault cargado desde disco: ${snap.state.nodes.length} nodos`, 'success');
      } else {
        vaultRevRef.current = snap?.revision ?? 0;
      }
      setVaultInfo(snap?.info ?? (await fetchVaultInfo()));
      vaultReadyRef.current = true;
    })();
    return () => {
      cancelado = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Autoguardado continuo del lienzo activo (localStorage como caché + vault en disco)
  useEffect(() => {
    setSaveStatus('saving');
    const timer = setTimeout(() => {
      try {
        const payload = {
          nodes,
          edges,
          appearance: edgeAppearance,
          templateId: currentTemplateId,
          updatedAt: Date.now(),
        };
        localStorage.setItem(ACTIVE_CANVAS_STORAGE_KEY, JSON.stringify(payload));
        setSaveStatus('saved');
        setLastSyncText('Autoguardado');
      } catch (err) {
        console.error('Error autoguardando lienzo activo en localStorage', err);
        setSaveStatus('unsaved');
      }
    }, 350);

    // Fase 3: el disco es la fuente de verdad. Debounce más largo: escribe N archivos.
    const vaultTimer = setTimeout(async () => {
      if (!vaultReadyRef.current) return;
      const rootTitle = nodes.find((n) => n.data.isRoot)?.data.title;
      const res = await saveVault({
        name: rootTitle || 'nodeflow',
        nodes,
        edges,
        appearance: edgeAppearance,
        templateId: currentTemplateId,
        // Si el agente escribió en el lienzo después de esta revisión, el backend lo rescata.
        base_revision: vaultRevRef.current,
      });
      if (res?.ok) {
        // Fijar la revisión propia evita que el polling confunda nuestro guardado con un cambio externo.
        if (typeof res.revision === 'number') vaultRevRef.current = res.revision;
        setVaultInfo(await fetchVaultInfo());
        setLastSyncText(`Disco ${new Date().toLocaleTimeString('es-AR', { hour12: false })}`);
      } else if (res && !res.ok) {
        setLastSyncText('Vault: error al escribir');
      }
    }, 1500);

    return () => {
      clearTimeout(timer);
      clearTimeout(vaultTimer);
    };
  }, [nodes, edges, edgeAppearance, currentTemplateId]);

  // Sincroniza con el disco: propuestas del agente (Fase 5a) + cambios externos en el lienzo.
  const sincronizar = useCallback(async () => {
    if (!vaultReadyRef.current) return;

    const pend = await fetchPendientes(pendingRevRef.current);
    if (pend && pend.changed !== false) {
      pendingRevRef.current = pend.revision;
      if (pend.pendientes) setPropuestas(pend.pendientes);
      if (pend.total > pendientesPreviosRef.current) {
        showToast(
          pend.total === 1
            ? 'El agente propuso un cambio: revisalo en «Cambios del agente»'
            : `El agente propuso ${pend.total} cambios: revisalos en «Cambios del agente»`,
          'info'
        );
      }
      pendientesPreviosRef.current = pend.total;
    }

    // Si todavía no hablamos con el vault (el primer fetch cayó mientras el backend levantaba), se pide
    // una lectura completa: con `since == revision` el backend contesta `changed:false` y el polling
    // quedaría cortocircuitado para siempre — la app dejaría de escribir en disco **en silencio**.
    const snap = await pollVault(vaultInfoRef.current ? vaultRevRef.current : -1);
    if (!snap) return;
    // El backend contestó: hay disco al alcance, así que el autoguardado puede volver a escribir.
    vaultReadyRef.current = true;
    if (!vaultInfoRef.current && snap.info) setVaultInfo(snap.info);
    if (snap.changed === false) return;
    const externos = snap.cambios_externos || [];
    // Una revisión nueva puede venir de Obsidian o de una propuesta que acabás de aprobar.
    if (snap.state?.nodes?.length) {
      setNodes(snap.state.nodes);
      setEdges(snap.state.edges || []);
      if (snap.state.appearance) setEdgeAppearance(snap.state.appearance as EdgeAppearance);
      if (externos.length) {
        showToast(`Cambio desde Obsidian aplicado: ${externos.join(', ')}`, 'info');
      }
    }
    vaultRevRef.current = snap.revision;
    setVaultInfo(snap.info ?? null);
    if (snap.metricas) setMetricas(snap.metricas as Metricas);
  }, [showToast]);

  useEffect(() => {
    const id = setInterval(() => {
      void sincronizar();
    }, 3000);
    return () => clearInterval(id);
  }, [sincronizar]);

  const handleUndo = useCallback(() => {
    const prev = undo();
    if (prev) {
      setNodes(prev.nodes);
      setEdges(prev.edges);
      showToast('Deshacer acción aplicado', 'info');
    }
  }, [undo, showToast]);

  const handleRedo = useCallback(() => {
    const next = redo();
    if (next) {
      setNodes(next.nodes);
      setEdges(next.edges);
      showToast('Rehacer acción aplicado', 'info');
    }
  }, [redo, showToast]);

  // Comprehensive Keyboard Shortcuts: Undo, Redo, Delete, New Node, Search, Escape
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable;

      // Escape closes modals or clears search
      if (e.key === 'Escape') {
        if (searchQuery) setSearchQuery('');
        setIsShortcutsOpen(false);
        return;
      }

      // If user is actively typing in an input field, do not trigger editing canvas shortcuts
      if (isInput) return;

      // Quick shortcut viewer: ?, Shift+/, Ctrl+/, or F1
      if (
        e.key === '?' ||
        (e.shiftKey && e.key === '/') ||
        ((e.ctrlKey || e.metaKey) && e.key === '/') ||
        e.key === 'F1'
      ) {
        e.preventDefault();
        setIsShortcutsOpen((prev) => !prev);
        return;
      }

      // Undo / Redo
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        if (e.shiftKey) {
          handleRedo();
        } else {
          handleUndo();
        }
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y') {
        e.preventDefault();
        handleRedo();
      }

      // Delete selected elements
      if (e.key === 'Delete' || e.key === 'Backspace') {
        if (selectedNodes.length > 0 || selectedEdges.length > 0) {
          e.preventDefault();
          takeSnapshot(nodes, edges);

          // Telemetría HITL: registrar nodos de IA descartados
          selectedNodes.forEach((node) => {
            if (node.data.aiOrigin) {
              const origin = node.data.aiOrigin;
              const feedback: FeedbackEvent = {
                id: `hitl-del-${Date.now()}-${Math.floor(Math.random() * 1000)}`,
                timestamp: new Date().toISOString(),
                action: 'NODE_DELETE',
                prompt_original: origin.promptOriginal,
                ai_suggestion: origin.allBatchTitles,
                human_decision: {
                  accepted: [],
                  rejected: [node.data.title],
                  added_manually: [],
                },
                contextSnippet: `Usuario descartó la propuesta de IA: "${node.data.title}"`,
              };
              recordHitlFeedback(feedback).then((updated) => {
                if (updated) setHitlProfile(updated);
              });
            }
          });

          const nodeIdsToDelete = new Set(selectedNodes.map((n) => n.id));
          const edgeIdsToDelete = new Set(selectedEdges.map((e) => e.id));
          setNodes((nds) => nds.filter((n) => !nodeIdsToDelete.has(n.id)));
          setEdges((eds) =>
            eds.filter(
              (ed) =>
                !edgeIdsToDelete.has(ed.id) &&
                !nodeIdsToDelete.has(ed.source) &&
                !nodeIdsToDelete.has(ed.target)
            )
          );
          setSelectedNodes([]);
          setSelectedEdges([]);
          showToast('Elementos seleccionados eliminados', 'info');
        }
      }

      // Ctrl/Cmd + F: focus search
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        const searchInput = document.querySelector('input[placeholder*="Buscar nodos"]') as HTMLInputElement;
        searchInput?.focus();
      }

      // Ctrl/Cmd + B: open Brain Dump (Descarga Mental Rápida)
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'b') {
        e.preventDefault();
        setIsBrainDumpOpen(true);
        return;
      }

      // Ctrl/Cmd + Shift + C: condensar la selección en un macro-nodo (poda sin pérdida)
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'c') {
        e.preventDefault();
        handleCondensarRef.current?.();
        return;
      }

      // Atajos Mindmap Ultra-Fluidos (Tab para hijo a la derecha, Enter para hermano debajo)
      if (e.key === 'Tab') {
        if (selectedNodes.length === 1) {
          e.preventDefault();
          e.stopPropagation();
          handleAddChildNodeRef.current?.(selectedNodes[0]);
          return;
        }
      }

      if (e.key === 'Enter') {
        if (selectedNodes.length === 1) {
          e.preventDefault();
          e.stopPropagation();
          handleAddSiblingNodeRef.current?.(selectedNodes[0]);
          return;
        }
      }

      // Atajos para Multi-Selección (2 o más nodos: U para unir directamente, H para hibridar con IA)
      if (selectedNodes.length >= 2 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const key = e.key.toLowerCase();
        if (key === 'u' && selectedNodes.length === 2) {
          e.preventDefault();
          handleConnectSelectedNodesRef.current?.();
          return;
        }
        if (key === 'h') {
          e.preventDefault();
          handleHybridizeRef.current?.();
          return;
        }
      }

      // Atajos directos para 1 nodo seleccionado en lienzo (B: Ramificar, E: Explorar, C: Crítica, S: Inyector Socrático, M / 1-4: Calificador de Madurez)
      if (selectedNodes.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        const key = e.key.toLowerCase();
        if (key === 'b') {
          e.preventDefault();
          handleAIActionRef.current?.('branch', selectedNodes[0].id, selectedNodes[0].data);
          return;
        }
        if (key === 'e') {
          e.preventDefault();
          handleAIActionRef.current?.('explore', selectedNodes[0].id, selectedNodes[0].data);
          return;
        }
        if (key === 'c') {
          e.preventDefault();
          handleAIActionRef.current?.('critique', selectedNodes[0].id, selectedNodes[0].data);
          return;
        }
        if (key === 's') {
          e.preventDefault();
          handleAIActionRef.current?.('socratic', selectedNodes[0].id, selectedNodes[0].data);
          return;
        }
        if (key === 'm') {
          e.preventDefault();
          const cur = (selectedNodes[0].data.maturity || 1) as IdeaMaturityLevel;
          const next = ((cur % 4) + 1) as IdeaMaturityLevel;
          handleAIActionRef.current?.('set-maturity', selectedNodes[0].id, {
            ...selectedNodes[0].data,
            maturity: next,
          });
          return;
        }
        if (['1', '2', '3', '4'].includes(e.key)) {
          e.preventDefault();
          const targetLevel = parseInt(e.key, 10) as IdeaMaturityLevel;
          handleAIActionRef.current?.('set-maturity', selectedNodes[0].id, {
            ...selectedNodes[0].data,
            maturity: targetLevel,
          });
          return;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleUndo, handleRedo, selectedNodes, selectedEdges, nodes, edges, takeSnapshot, showToast, searchQuery]);

  // Node & Edge Changes
  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      setNodes((nds) => applyNodeChanges(changes, nds) as CustomNode[]);
    },
    []
  );

  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      setEdges((eds) => applyEdgeChanges(changes, eds));
    },
    []
  );

  // ReactFlow Selection tracking
  const onSelectionChange = useCallback((params: OnSelectionChangeParams) => {
    setSelectedNodes(params.nodes as CustomNode[]);
    setSelectedEdges(params.edges);
    // La selección alimenta el mismo foco que el hover: elegir un nodo destaca
    // sus conexiones sin tener que pasar el mouse por encima.
    setFocusSelection(params.nodes.map((n) => n.id));
  }, []);

  // Connect two nodes with the customized Edge Appearance
  const onConnect = useCallback(
    (connection: Connection) => {
      takeSnapshot(nodes, edges);

      const newEdge: Edge = {
        id: `e-${connection.source}-${connection.target}-${Date.now()}`,
        source: connection.source!,
        sourceHandle: connection.sourceHandle,
        target: connection.target!,
        targetHandle: connection.targetHandle,
        type: edgeAppearance.type,
        animated: edgeAppearance.animated,
        style: {
          stroke: edgeAppearance.color,
          strokeWidth: edgeAppearance.strokeWidth,
        },
      };

      setEdges((eds) => addEdge(newEdge, eds));
      showToast('Conexión enlazada', 'success');
    },
    [nodes, edges, edgeAppearance, takeSnapshot, showToast]
  );

  // Función para agregar nodos
  const addNode = useCallback(
    (category: string, title: string, description: string, tags: string[], x: number, y: number) => {
      takeSnapshot(nodes, edges);
      const id = `node-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
      const newNode: CustomNode = {
        id,
        type: 'ideaNode',
        position: { x, y },
        data: {
          id,
          category,
          label: category,
          title,
          description,
          tags,
          colorAccent: '#6366f1',
          maturity: category === 'NÚCLEO' ? 3 : 1,
        },
      };
      setNodes((nds) => nds.concat(newNode));
      showToast(`Nodo "${title}" creado`, 'success');
      return id;
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  // Add new Idea Node manually
  const handleAddNode = useCallback(
    (isRoot = false) => {
      let posX = 250;
      let posY = 150;
      if (rfInstanceRef.current) {
        const center = rfInstanceRef.current.getViewport();
        posX = -center.x / center.zoom + 200 + Math.random() * 60;
        posY = -center.y / center.zoom + 150 + Math.random() * 60;
      }

      if (isRoot) {
        addNode('NÚCLEO', 'Idea Central', 'Haz clic en el icono de edición para detallar el concepto.', ['Raíz', 'Concepto'], posX, posY);
      } else {
        addNode('NUEVA IDEA', 'Nueva Hipótesis de Trabajo', 'Haz clic para configurar y explorar.', ['Idea'], posX, posY);
      }
    },
    [addNode]
  );

  // Creación fluida de nodo hijo conectado a la derecha (Tab)
  const handleAddChildNode = useCallback(
    (parentNode: CustomNode) => {
      takeSnapshot(nodes, edges);

      const outgoingEdges = edges.filter((e) => e.source === parentNode.id);
      const existingChildren = nodes.filter((n) => outgoingEdges.some((e) => e.target === n.id));

      let childY = parentNode.position.y;
      if (existingChildren.length > 0) {
        const maxY = Math.max(...existingChildren.map((c) => c.position.y));
        childY = maxY + 140;
      }

      const childX = parentNode.position.x + 320;
      const childId = `node-${Date.now()}-${Math.floor(Math.random() * 1000)}`;

      const childNode: CustomNode = {
        id: childId,
        type: 'ideaNode',
        position: posicionLibre(nodes, { x: childX, y: childY }),
        selected: true,
        data: {
          id: childId,
          title: '',
          description: '',
          category: parentNode.data.category || 'IDEA',
          label: parentNode.data.category || 'IDEA',
          tags: [],
          colorAccent: parentNode.data.colorAccent || '#6366f1',
          isEditing: true, // In-place editing inmediato sin modales
          maturity: 1,
        },
      };

      const newEdge: Edge = {
        id: `edge-${parentNode.id}-${childId}-${Date.now()}`,
        source: parentNode.id,
        sourceHandle: 'right',
        target: childId,
        targetHandle: 'left',
        type: edgeAppearance.type,
        animated: edgeAppearance.animated,
        style: {
          stroke: parentNode.data.colorAccent || edgeAppearance.color,
          strokeWidth: edgeAppearance.strokeWidth,
        },
      };

      setNodes((nds) => [
        ...nds.map((n) => ({ ...n, selected: false, data: { ...n.data, isEditing: false } })),
        childNode,
      ]);
      setEdges((eds) => [...eds, newEdge]);
      setSelectedNodes([childNode]);
    },
    [nodes, edges, edgeAppearance, takeSnapshot]
  );

  useEffect(() => {
    handleAddChildNodeRef.current = handleAddChildNode;
  }, [handleAddChildNode]);

  // Creación fluida de nodo hermano conectado al mismo padre (Enter)
  const handleAddSiblingNode = useCallback(
    (currentNode: CustomNode) => {
      takeSnapshot(nodes, edges);

      const incomingEdge = edges.find((e) => e.target === currentNode.id);
      const parentId = incomingEdge ? incomingEdge.source : null;

      let siblingX = currentNode.position.x;
      let siblingY = currentNode.position.y + 140;

      if (parentId) {
        const siblingEdges = edges.filter((e) => e.source === parentId);
        const siblingNodes = nodes.filter((n) => siblingEdges.some((e) => e.target === n.id));
        if (siblingNodes.length > 0) {
          const maxY = Math.max(...siblingNodes.map((s) => s.position.y));
          siblingY = maxY + 140;
        }
      }

      const siblingId = `node-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
      const siblingNode: CustomNode = {
        id: siblingId,
        type: 'ideaNode',
        position: posicionLibre(nodes, { x: siblingX, y: siblingY }),
        selected: true,
        data: {
          id: siblingId,
          title: '',
          description: '',
          category: currentNode.data.category || 'IDEA',
          label: currentNode.data.category || 'IDEA',
          tags: [],
          colorAccent: currentNode.data.colorAccent || '#6366f1',
          isEditing: true, // In-place editing inmediato sin modales
          maturity: 1,
        },
      };

      const newEdges = [...edges];
      if (parentId) {
        const newEdge: Edge = {
          id: `edge-${parentId}-${siblingId}-${Date.now()}`,
          source: parentId,
          sourceHandle: 'right',
          target: siblingId,
          targetHandle: 'left',
          type: edgeAppearance.type,
          animated: edgeAppearance.animated,
          style: {
            stroke: currentNode.data.colorAccent || edgeAppearance.color,
            strokeWidth: edgeAppearance.strokeWidth,
          },
        };
        newEdges.push(newEdge);
      }

      setNodes((nds) => [
        ...nds.map((n) => ({ ...n, selected: false, data: { ...n.data, isEditing: false } })),
        siblingNode,
      ]);
      setEdges(newEdges);
      setSelectedNodes([siblingNode]);
    },
    [nodes, edges, edgeAppearance, takeSnapshot]
  );

  useEffect(() => {
    handleAddSiblingNodeRef.current = handleAddSiblingNode;
  }, [handleAddSiblingNode]);

  // Creación con Doble Clic Directo en Lienzo Vacío
  const handleCanvasDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      const target = e.target as HTMLElement;
      if (
        target.closest('.react-flow__node') ||
        target.closest('.react-flow__edge') ||
        target.closest('.react-flow__controls') ||
        target.closest('.react-flow__minimap') ||
        target.closest('.react-flow__panel') ||
        target.closest('button') ||
        target.closest('input') ||
        target.closest('textarea')
      ) {
        return;
      }

      const wrapperBounds = reactFlowWrapperRef.current?.getBoundingClientRect();
      if (!wrapperBounds || !rfInstanceRef.current) return;

      let flowPos: { x: number; y: number };
      if (typeof (rfInstanceRef.current as any).screenToFlowPosition === 'function') {
        flowPos = (rfInstanceRef.current as any).screenToFlowPosition({
          x: e.clientX,
          y: e.clientY,
        });
      } else {
        flowPos = rfInstanceRef.current.project({
          x: e.clientX - wrapperBounds.left,
          y: e.clientY - wrapperBounds.top,
        });
      }

      takeSnapshot(nodes, edges);

      const newId = `node-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
      const newNode: CustomNode = {
        id: newId,
        type: 'ideaNode',
        position: {
          x: Math.round(flowPos.x - 120),
          y: Math.round(flowPos.y - 40),
        },
        selected: true,
        data: {
          id: newId,
          title: '',
          description: '',
          category: 'IDEA',
          label: 'IDEA',
          tags: [],
          colorAccent: '#6366f1',
          isEditing: true, // Cursor parpadeando de inmediato en el lienzo
          maturity: 1,
        },
      };

      setNodes((nds) => [
        ...nds.map((n) => ({ ...n, selected: false, data: { ...n.data, isEditing: false } })),
        newNode,
      ]);
      setSelectedNodes([newNode]);
    },
    [nodes, edges, takeSnapshot]
  );

  // Manejador centralizado de acciones de IA y del nodo
  const handleAIAction = useCallback(
    async (
      action: 'branch' | 'explore' | 'edit' | 'delete' | 'duplicate' | string,
      nodeIdOrData: string | IdeaNodeData,
      extraData?: IdeaNodeData
    ) => {
      const nodeId = typeof nodeIdOrData === 'string' ? nodeIdOrData : nodeIdOrData.id;
      const targetData =
        typeof nodeIdOrData === 'string'
          ? extraData || nodes.find((n) => n.id === nodeId)?.data || ({} as IdeaNodeData)
          : nodeIdOrData;

      if (action === 'edit') {
        setEditingNode(targetData);
        return;
      }

      if (action === 'investigar') {
        const tema = (targetData.title || '').trim();
        if (tema.length < 4) {
          showToast('Este nodo no tiene tema suficiente para investigar.', 'info');
          return;
        }
        setIsInvestigacionOpen(true);
        void investigarTema(
          `${tema}${targetData.description ? `. Contexto: ${targetData.description}` : ''}`.slice(0, 400)
        );
        return;
      }

      if (action === 'inline-start') {
        setNodes((nds) =>
          nds.map((n) =>
            n.id === nodeId
              ? { ...n, selected: true, data: { ...n.data, isEditing: true } }
              : { ...n, data: { ...n.data, isEditing: false } }
          )
        );
        return;
      }

      if (action === 'inline-cancel') {
        setNodes((nds) =>
          nds.map((n) => (n.id === nodeId ? { ...n, data: { ...n.data, isEditing: false } } : n))
        );
        return;
      }

      if (action === 'set-maturity') {
        takeSnapshot(nodes, edges);
        const newMaturity = (extraData?.maturity || targetData.maturity || 1) as IdeaMaturityLevel;
        setNodes((nds) =>
          nds.map((n) =>
            n.id === nodeId
              ? { ...n, data: { ...n.data, maturity: newMaturity } }
              : n
          )
        );
        const cfg = MATURITY_CONFIGS[newMaturity];
        showToast(`Madurez: ${cfg.icon} ${cfg.label} (Nivel ${newMaturity})`, 'info');
        return;
      }

      if (action === 'inline-save' || action === 'inline-save-tab' || action === 'inline-save-enter') {
        const newTitle = targetData.title?.trim() || 'Nueva Idea';
        const targetNode = nodes.find((n) => n.id === nodeId);

        if (targetNode?.data.aiOrigin) {
          const origin = targetNode.data.aiOrigin;
          if (newTitle !== origin.originalTitle.trim()) {
            const feedback: FeedbackEvent = {
              id: `hitl-edit-${Date.now()}`,
              timestamp: new Date().toISOString(),
              action: 'NODE_EDIT',
              prompt_original: origin.promptOriginal,
              ai_suggestion: origin.allBatchTitles,
              human_decision: {
                accepted: [newTitle],
                rejected: [origin.originalTitle],
                added_manually: [],
              },
              contextSnippet: `Título refinado in-place de "${origin.originalTitle}" a "${newTitle}"`,
            };
            recordHitlFeedback(feedback).then((updated) => {
              if (updated) setHitlProfile(updated);
            });
          }
        }

        takeSnapshot(nodes, edges);
        const updatedNode: CustomNode = {
          ...(targetNode || { id: nodeId, type: 'ideaNode', position: { x: 200, y: 200 } }),
          selected: true,
          data: {
            ...(targetNode?.data || targetData),
            title: newTitle,
            isEditing: false,
          },
        };

        setNodes((nds) =>
          nds.map((n) =>
            n.id === nodeId
              ? updatedNode
              : { ...n, data: { ...n.data, isEditing: false } }
          )
        );

        setSelectedNodes([updatedNode]);

        if (action === 'inline-save-tab') {
          setTimeout(() => {
            handleAddChildNodeRef.current?.(updatedNode);
          }, 40);
        }
        return;
      }

      if (action === 'delete') {
        const targetNode = nodes.find((n) => n.id === nodeId);
        if (targetNode?.data.aiOrigin) {
          const origin = targetNode.data.aiOrigin;
          const feedback: FeedbackEvent = {
            id: `hitl-del-${Date.now()}-${Math.floor(Math.random() * 1000)}`,
            timestamp: new Date().toISOString(),
            action: 'NODE_DELETE',
            prompt_original: origin.promptOriginal,
            ai_suggestion: origin.allBatchTitles,
            human_decision: {
              accepted: [],
              rejected: [targetNode.data.title],
              added_manually: [],
            },
            contextSnippet: `Usuario descartó la sugerencia de IA: "${targetNode.data.title}"`,
          };
          recordHitlFeedback(feedback).then((updated) => {
            if (updated) setHitlProfile(updated);
          });
        }
        takeSnapshot(nodes, edges);
        setNodes((nds) => nds.filter((n) => n.id !== nodeId));
        setEdges((eds) => eds.filter((e) => e.source !== nodeId && e.target !== nodeId));
        showToast('Nodo eliminado', 'info');
        return;
      }

      if (action === 'duplicate') {
        takeSnapshot(nodes, edges);
        const source = nodes.find((n) => n.id === nodeId);
        const newId = `node-${Date.now()}`;
        const clone: CustomNode = {
          id: newId,
          type: 'ideaNode',
          position: {
            x: (source?.position.x || 300) + 40,
            y: (source?.position.y || 200) + 40,
          },
          data: {
            ...targetData,
            id: newId,
            title: `${targetData.title} (Copia)`,
            isRoot: false,
          },
        };
        setNodes((nds) => [...nds, clone]);
        showToast('Nodo duplicado', 'info');
        return;
      }

      if (action === 'branch') {
        setIsAiLoading(true);
        showToast(`${motorActual()} está analizando y generando ramificaciones para "${targetData.title}"...`, 'info');

        try {
          const response = await postAiAction({
            type: 'branch',
            nodeData: targetData,
            hitlProfileOverride: hitlProfile.learnedProfile,
          });

          const data = await response.json();
          const variations = data.variations || [];

          if (data.success && Array.isArray(variations) && variations.length > 0) {
            takeSnapshot(nodes, edges);

            const batchId = `ai-batch-${Date.now()}`;
            const promptOriginal = `Sugerir 3 conexiones para el nodo '${targetData.title}'`;
            const allBatchTitles = variations.map((v: any) => v.title);

            const batchIds = variations.map((_: any, i: number) => `node-${Date.now()}-${i}-${Math.floor(Math.random() * 1000)}`);
            const parentNode = nodes.find((n) => n.id === nodeId);
            const parentX = parentNode ? parentNode.position.x : 250;
            const parentY = parentNode ? parentNode.position.y : 100;

            const newCreatedNodes: CustomNode[] = [];
            const newCreatedEdges: Edge[] = [];

            variations.forEach((v: any, index: number) => {
              const newX = parentX + (index - 1) * 300;
              const newY = parentY + 220;
              const newId = batchIds[index];

              newCreatedNodes.push({
                id: newId,
                type: 'ideaNode',
                position: posicionLibre([...nodes, ...newCreatedNodes], { x: newX, y: newY }),
                data: {
                  id: newId,
                  category: v.category || 'VARIACIÓN',
                  label: v.category || 'VARIACIÓN',
                  title: v.title,
                  description: v.description,
                  tags: Array.isArray(v.tags) ? v.tags : [v.tag || 'IA'],
                  colorAccent: '#6366f1',
                  aiOrigin: {
                    batchId,
                    actionType: 'branch',
                    promptOriginal,
                    originalTitle: v.title,
                    allBatchTitles,
                    createdAt: new Date().toISOString(),
                  },
                },
              });

              newCreatedEdges.push({
                id: `e-${nodeId}-${newId}`,
                source: nodeId,
                target: newId,
                type: edgeAppearance.type || 'smoothstep',
                animated: true,
                style: {
                  stroke: edgeAppearance.color || '#6366f1',
                  strokeWidth: edgeAppearance.strokeWidth || 2,
                },
              });
            });

            setNodes((currentNodes) => [...currentNodes, ...newCreatedNodes]);
            setEdges((eds) => [...eds, ...newCreatedEdges]);

            showToast(`¡3 ramificaciones generadas con éxito! Afinado con tu perfil HITL`, 'success');
          } else {
            throw new Error('Formato de respuesta inválido');
          }
        } catch (err) {
          console.error('Error generating AI branch:', err);
          showToast('Error al conectar con el servicio de IA.', 'error');
        } finally {
          setIsAiLoading(false);
        }
        return;
      }

      if (action === 'explore') {
        setIsAiLoading(true);
        showToast(`${motorActual()} está explorando dimensiones técnicas y estratégicas para "${targetData.title}"...`, 'info');

        try {
          const response = await postAiAction({
            type: action,
            nodeData: targetData,
            hitlProfileOverride: hitlProfile.learnedProfile,
          });

          const data = await response.json();
          const variations = data.variations || [];

          if (data.success && Array.isArray(variations) && variations.length > 0) {
            takeSnapshot(nodes, edges);

            const batchId = `ai-batch-exp-${Date.now()}`;
            const promptOriginal = `Exploración profunda del concepto '${targetData.title}'`;
            const allBatchTitles = variations.map((v: any) => v.title);

            const batchIds = variations.map((_: any, i: number) => `node-exp-${Date.now()}-${i}-${Math.floor(Math.random() * 1000)}`);
            const parentNode = nodes.find((n) => n.id === nodeId);
            const parentX = parentNode ? parentNode.position.x : 250;
            const parentY = parentNode ? parentNode.position.y : 100;

            const newCreatedNodes: CustomNode[] = [];
            const newCreatedEdges: Edge[] = [];

            variations.forEach((v: any, index: number) => {
              const newX = parentX + (index - 1) * 320;
              const newY = parentY + 240;
              const newId = batchIds[index];

              newCreatedNodes.push({
                id: newId,
                type: 'ideaNode',
                position: posicionLibre([...nodes, ...newCreatedNodes], { x: newX, y: newY }),
                data: {
                  id: newId,
                  category: v.category || 'ÁMBITO',
                  label: v.category || 'ÁMBITO',
                  title: v.title,
                  description: v.description,
                  tags: Array.isArray(v.tags) ? v.tags : [v.tag || 'Análisis'],
                  colorAccent: '#f59e0b',
                  aiOrigin: {
                    batchId,
                    actionType: 'explore',
                    promptOriginal,
                    originalTitle: v.title,
                    allBatchTitles,
                    createdAt: new Date().toISOString(),
                  },
                },
              });

              newCreatedEdges.push({
                id: `e-${nodeId}-${newId}`,
                source: nodeId,
                target: newId,
                type: edgeAppearance.type || 'smoothstep',
                animated: true,
                style: {
                  stroke: '#f59e0b',
                  strokeWidth: 2,
                },
              });
            });

            setNodes((currentNodes) => [...currentNodes, ...newCreatedNodes]);
            setEdges((eds) => [...eds, ...newCreatedEdges]);

            showToast(`Exploración completada para "${targetData.title}" (Perfil HITL activo)`, 'success');
          }
        } catch (err) {
          console.error('Error generating AI explore:', err);
          showToast(`Error al explorar el concepto con ${motorActual()}.`, 'error');
        } finally {
          setIsAiLoading(false);
        }
        return;
      }

      if (action === 'critique') {
        setIsAiLoading(true);
        showToast(`Abogado del Diablo analizando riesgos y puntos ciegos para "${targetData.title}"...`, 'info');

        try {
          const response = await postAiAction({
            type: 'critique',
            nodeData: targetData,
            hitlProfileOverride: hitlProfile.learnedProfile,
          });

          const data = await response.json();
          const variations = data.variations || [];

          if (data.success && Array.isArray(variations) && variations.length > 0) {
            takeSnapshot(nodes, edges);

            const batchId = `ai-batch-crit-${Date.now()}`;
            const promptOriginal = `Auditoría crítica y abogado del diablo para '${targetData.title}'`;
            const allBatchTitles = variations.map((v: any) => v.title);

            const batchIds = variations.map((_: any, i: number) => `node-crit-${Date.now()}-${i}-${Math.floor(Math.random() * 1000)}`);
            const parentNode = nodes.find((n) => n.id === nodeId);
            const parentX = parentNode ? parentNode.position.x : 250;
            const parentY = parentNode ? parentNode.position.y : 100;

            // Calcular separación vertical inteligente para no superponer con ramas existentes
            const outgoing = edges.filter((e) => e.source === nodeId);
            const existingChildren = nodes.filter((n) => outgoing.some((e) => e.target === n.id));
            const baseY = existingChildren.length > 0
              ? Math.max(...existingChildren.map((c) => c.position.y)) + 190
              : parentY + 230;

            const newCreatedNodes: CustomNode[] = [];
            const newCreatedEdges: Edge[] = [];

            variations.forEach((v: any, index: number) => {
              const newX = parentX + (index - 1) * 320;
              const newY = baseY;
              const newId = batchIds[index];

              newCreatedNodes.push({
                id: newId,
                type: 'ideaNode',
                position: posicionLibre([...nodes, ...newCreatedNodes], { x: newX, y: newY }),
                data: {
                  id: newId,
                  category: v.category || 'CRÍTICA Y RIESGO',
                  label: v.category || 'CRÍTICA Y RIESGO',
                  title: v.title,
                  description: v.description,
                  tags: Array.isArray(v.tags) ? v.tags : ['Riesgo', 'Auditoría'],
                  colorAccent: '#f43f5e',
                  aiOrigin: {
                    batchId,
                    actionType: 'critique',
                    promptOriginal,
                    originalTitle: v.title,
                    allBatchTitles,
                    createdAt: new Date().toISOString(),
                  },
                },
              });

              newCreatedEdges.push({
                id: `e-${nodeId}-${newId}`,
                source: nodeId,
                target: newId,
                type: edgeAppearance.type || 'smoothstep',
                animated: true,
                style: {
                  stroke: '#f43f5e',
                  strokeWidth: 2,
                  strokeDasharray: '5 4',
                },
              });
            });

            setNodes((currentNodes) => [...currentNodes, ...newCreatedNodes]);
            setEdges((eds) => [...eds, ...newCreatedEdges]);

            showToast(`Abogado del Diablo: 3 riesgos críticos y antítesis identificados para "${targetData.title}"`, 'success');
          } else {
            throw new Error('Formato de respuesta inválido');
          }
        } catch (err) {
          console.error('Error generating AI critique:', err);
          showToast(`Error al auditar riesgos con ${motorActual()}.`, 'error');
        } finally {
          setIsAiLoading(false);
        }
        return;
      }

      if (action === 'socratic') {
        setIsAiLoading(true);
        showToast(`Inyector Socrático formulando preguntas catalizadoras para "${targetData.title}"...`, 'info');

        try {
          const response = await postAiAction({
            type: 'socratic',
            nodeData: targetData,
            hitlProfileOverride: hitlProfile.learnedProfile,
          });

          const data = await response.json();
          const variations = data.variations || [];

          if (data.success && Array.isArray(variations) && variations.length > 0) {
            takeSnapshot(nodes, edges);

            const batchId = `ai-batch-soc-${Date.now()}`;
            const promptOriginal = `Inyector socrático y preguntas catalizadoras para '${targetData.title}'`;
            const allBatchTitles = variations.map((v: any) => v.title);

            const batchIds = variations.map((_: any, i: number) => `node-soc-${Date.now()}-${i}-${Math.floor(Math.random() * 1000)}`);
            const parentNode = nodes.find((n) => n.id === nodeId);
            const parentX = parentNode ? parentNode.position.x : 250;
            const parentY = parentNode ? parentNode.position.y : 100;

            // Calcular separación vertical inteligente para no superponer con ramas existentes
            const outgoing = edges.filter((e) => e.source === nodeId);
            const existingChildren = nodes.filter((n) => outgoing.some((e) => e.target === n.id));
            const baseY = existingChildren.length > 0
              ? Math.max(...existingChildren.map((c) => c.position.y)) + 190
              : parentY + 230;

            const newCreatedNodes: CustomNode[] = [];
            const newCreatedEdges: Edge[] = [];

            variations.forEach((v: any, index: number) => {
              const newX = parentX + (index - 1) * 320;
              const newY = baseY;
              const newId = batchIds[index];

              newCreatedNodes.push({
                id: newId,
                type: 'ideaNode',
                position: posicionLibre([...nodes, ...newCreatedNodes], { x: newX, y: newY }),
                data: {
                  id: newId,
                  category: v.category || 'PREGUNTA SOCRÁTICA',
                  label: v.category || 'PREGUNTA SOCRÁTICA',
                  title: v.title,
                  description: v.description,
                  tags: Array.isArray(v.tags) ? v.tags : ['Socrático', 'Reflexión'],
                  colorAccent: '#06b6d4',
                  aiOrigin: {
                    batchId,
                    actionType: 'socratic',
                    promptOriginal,
                    originalTitle: v.title,
                    allBatchTitles,
                    createdAt: new Date().toISOString(),
                  },
                },
              });

              newCreatedEdges.push({
                id: `e-${nodeId}-${newId}`,
                source: nodeId,
                target: newId,
                type: edgeAppearance.type || 'smoothstep',
                animated: true,
                style: {
                  stroke: '#06b6d4',
                  strokeWidth: 2,
                },
              });
            });

            setNodes((currentNodes) => [...currentNodes, ...newCreatedNodes]);
            setEdges((eds) => [...eds, ...newCreatedEdges]);

            showToast(`Inyector Socrático: 3 preguntas catalizadoras generadas para "${targetData.title}"`, 'success');
          } else {
            throw new Error('Formato de respuesta inválido');
          }
        } catch (err) {
          console.error('Error generating AI socratic questions:', err);
          showToast(`Error al formular preguntas socráticas con ${motorActual()}.`, 'error');
        } finally {
          setIsAiLoading(false);
        }
        return;
      }
    },
    [nodes, edges, edgeAppearance, takeSnapshot, showToast, hitlProfile.learnedProfile]
  );

  useEffect(() => {
    handleAIActionRef.current = handleAIAction;
  }, [handleAIAction]);

  // AI Hybridizer Action (Merges 2 or more selected ideas)
  const handleHybridize = useCallback(async () => {
    if (selectedNodes.length < 2) {
      showToast('Selecciona al menos 2 nodos en el lienzo para hibridarlos.', 'info');
      return;
    }

    setIsAiLoading(true);
    showToast(`Hibridando conceptos entre "${selectedNodes[0].data.title}" y "${selectedNodes[1].data.title}"...`, 'info');

    try {
      const payloadNodes = selectedNodes.map((n) => ({
        id: n.id,
        title: n.data.title,
        description: n.data.description,
      }));

      const response = await postAiAction({
        type: 'hybrid',
        selectedNodes: payloadNodes,
        hitlProfileOverride: hitlProfile.learnedProfile,
      });

      const data = await response.json();
      if (data.success && data.hybrid) {
        takeSnapshot(nodes, edges);

        // Compute centroid of selected nodes
        const avgX = selectedNodes.reduce((sum, n) => sum + n.position.x, 0) / selectedNodes.length;
        const avgY = selectedNodes.reduce((sum, n) => sum + n.position.y, 0) / selectedNodes.length + 220;

        const hybridId = `node-hybrid-${Date.now()}`;
        const hybridNode: CustomNode = {
          id: hybridId,
          type: 'ideaNode',
          position: { x: avgX, y: avgY },
          data: {
            id: hybridId,
            category: 'HÍBRIDO',
            label: 'SÍNTESIS HÍBRIDA IA',
            title: data.hybrid.title,
            description: data.hybrid.description,
            tags: data.hybrid.tags || ['Híbrido', 'Fusión'],
            colorAccent: '#a855f7',
            aiOrigin: {
              batchId: hybridId,
              actionType: 'hybrid',
              promptOriginal: `Hibridar "${selectedNodes[0].data.title}" y "${selectedNodes[1].data.title}"`,
              originalTitle: data.hybrid.title,
              allBatchTitles: [data.hybrid.title],
              createdAt: new Date().toISOString(),
            },
          },
        };

        // Telemetría HITL: Fusión aceptada
        const hybridFeedback: FeedbackEvent = {
          id: `hitl-hyb-${Date.now()}`,
          timestamp: new Date().toISOString(),
          action: 'HYBRIDIZE_FEEDBACK',
          prompt_original: `Hibridar "${selectedNodes[0].data.title}" y "${selectedNodes[1].data.title}"`,
          ai_suggestion: [data.hybrid.title],
          human_decision: {
            accepted: [data.hybrid.title],
            rejected: [],
            added_manually: [],
          },
          contextSnippet: `Fusión de conceptos: ${data.hybrid.title}`,
        };
        recordHitlFeedback(hybridFeedback).then((up) => {
          if (up) setHitlProfile(up);
        });

        // Create edges from all parents to the new hybrid node
        const hybridEdges: Edge[] = selectedNodes.map((parent) => ({
          id: `e-${parent.id}-${hybridId}`,
          source: parent.id,
          sourceHandle: 'bottom',
          target: hybridId,
          targetHandle: 'top',
          type: edgeAppearance.type,
          animated: true,
          label: 'Fusión IA',
          style: {
            stroke: '#a855f7',
            strokeWidth: 2.5,
          },
        }));

        setNodes((nds) => [...nds, hybridNode]);
        setEdges((eds) => [...eds, ...hybridEdges]);
        showToast(`¡Concepto híbrido "${data.hybrid.title}" sintetizado! (Afinado con HITL)`, 'success');
      }
    } catch (err) {
      console.error('Error generating AI hybrid:', err);
      showToast('Error al hibridar ideas.', 'error');
    } finally {
      setIsAiLoading(false);
    }
  }, [selectedNodes, nodes, edges, edgeAppearance.type, takeSnapshot, showToast, hitlProfile.learnedProfile]);

  /**
   * Fase A — Condensar: los N nodos seleccionados se vuelven UN macro-nodo, y sus datos
   * (nodos + aristas originales) quedan guardados dentro: poda sin pérdida.
   * El Norte Estratégico entra en el prompt como lente; la IA devuelve el principio y el aporte.
   */
  const handleCondensar = useCallback(async () => {
    if (selectedNodes.length < 2) {
      showToast('Seleccioná al menos 2 nodos para condensarlos en un macro-nodo.', 'info');
      return;
    }
    setIsAiLoading(true);
    showToast(`Condensando ${selectedNodes.length} nodos en un macro-concepto...`, 'info');

    try {
      const response = await postAiAction({
        type: 'condensar',
        selectedNodes: selectedNodes.map((n) => ({
          id: n.id,
          title: n.data.title,
          description: n.data.description,
        })),
        objetivo: norte,
        hitlProfileOverride: hitlProfile.learnedProfile,
      });
      const data = await response.json();
      if (!data.success || !data.condensar) {
        showToast('No se pudo condensar la selección.', 'error');
        return;
      }

      takeSnapshot(nodes, edges);
      const ids = new Set(selectedNodes.map((n) => n.id));
      const macroId = `node-macro-${Date.now()}`;
      const centro = {
        x: selectedNodes.reduce((s, n) => s + n.position.x, 0) / selectedNodes.length,
        y: selectedNodes.reduce((s, n) => s + n.position.y, 0) / selectedNodes.length,
      };

      // Aristas: las internas se guardan en el linaje; las externas se reenganchan al macro para
      // que el grafo siga conectado (si no, el macro quedaría flotando sin relaciones).
      const todasLasTocadas = edges.filter((e) => ids.has(e.source) || ids.has(e.target));
      const internas = todasLasTocadas.filter((e) => ids.has(e.source) && ids.has(e.target));
      const externas = todasLasTocadas.filter((e) => ids.has(e.source) !== ids.has(e.target));
      const reenganchadas: Edge[] = externas.map((e) => ({
        ...e,
        id: `${e.id}-macro`,
        source: ids.has(e.source) ? macroId : e.source,
        target: ids.has(e.target) ? macroId : e.target,
      }));

      const macroNode: CustomNode = {
        id: macroId,
        type: 'ideaNode',
        position: centro,
        data: {
          id: macroId,
          category: 'MACRO',
          label: 'MACRO · CONDENSADO',
          title: data.condensar.title,
          description: data.condensar.description,
          tags: data.condensar.tags || ['Condensado'],
          colorAccent: '#8b5cf6',
          maturity: 4,
          macro: {
            colapsados: selectedNodes.length,
            linaje: selectedNodes.map((n) => n.id),
            resumen: data.condensar.resumen,
            principio: data.condensar.principio,
            match: typeof data.condensar.match === 'number' ? data.condensar.match : undefined,
            objetivo: norte || undefined,
            creadoEn: new Date().toISOString(),
            datos: { nodes: selectedNodes, edges: todasLasTocadas },
          },
          aiOrigin: {
            batchId: macroId,
            actionType: 'condensar',
            promptOriginal: `Condensar ${selectedNodes.length} nodos${norte ? ` hacia «${norte}»` : ''}`,
            originalTitle: data.condensar.title,
            allBatchTitles: [data.condensar.title],
            createdAt: new Date().toISOString(),
          },
        },
      };

      setNodes((nds) => [...nds.filter((n) => !ids.has(n.id)), macroNode]);
      setEdges((eds) => [...eds.filter((e) => !ids.has(e.source) && !ids.has(e.target)), ...reenganchadas]);
      setSelectedNodes([]);
      showToast(
        `◈ ${selectedNodes.length} nodos → 1 macro-nodo. El linaje quedó guardado: doble clic para verlo.`,
        'success'
      );

      recordHitlFeedback({
        id: `hitl-cond-${Date.now()}`,
        timestamp: new Date().toISOString(),
        action: 'CONDENSE_FEEDBACK',
        prompt_original: `Condensar ${selectedNodes.length} nodos${norte ? ` hacia «${norte}»` : ''}`,
        ai_suggestion: [data.condensar.title],
        human_decision: { accepted: [data.condensar.title], rejected: [], added_manually: [] },
        contextSnippet: data.condensar.resumen || data.condensar.description,
      }).then((up) => {
        if (up) setHitlProfile(up);
      });
    } catch (err) {
      console.error('Error al condensar:', err);
      showToast('Error al condensar la selección.', 'error');
    } finally {
      setIsAiLoading(false);
    }
  }, [selectedNodes, nodes, edges, norte, takeSnapshot, showToast, setSelectedNodes, hitlProfile.learnedProfile]);

  /**
   * Aplica un plan de voz YA VALIDADO por el backend (sólo ids que existen, sólo acciones permitidas).
   * - crear     → nodos nuevos, a la derecha del lienzo.
   * - enlazar   → aristas entre nodos existentes o con los recién creados.
   * - enfocar   → conserva los nodos indicados y su vecindad; el resto se colapsa en UN macro-nodo.
   * - condensar → colapsa los nodos indicados en un macro-nodo.
   * - criticar  → dispara el flujo socrático que ya existe (él propone, vos aprobás).
   * Los colapsos son locales (no gastan IA) y guardan el linaje: se restauran con doble clic.
   */
  /** Qué va a pasar, en números, ANTES de aplicar. Se muestra en la tarjeta del plan. */
  const previsualizarPlanVoz = useCallback(
    (plan: PlanVoz): string => {
      const partes: string[] = [];
      let conservar = 0;
      plan.comandos.forEach((c) => {
        if (c.accion === 'crear') partes.push('1 nodo nuevo');
        else if (c.accion === 'enlazar') partes.push('1 enlace');
        else if (c.accion === 'enfocar') {
          // Se conserva EXACTAMENTE lo que eligió el motor (ya leyó el lienzo entero). Expandir a los
          // vecinos parecía más generoso y en un grafo denso terminaba conservando todo: no enfocaba nada.
          conservar = new Set(c.nodos || []).size;
          partes.push(`conservar ${conservar} nodos (los que el motor marcó como la idea y su entorno)`);
        } else if (c.accion === 'condensar') partes.push(`colapsar ${new Set(c.nodos || []).size} nodos`);
        else if (c.accion === 'criticar') partes.push(`cuestionar ${(c.nodos || []).length} nodos`);
      });
      if (conservar) partes.push(`los otros ${Math.max(0, nodes.length - conservar)} pasan a un macro-nodo (se restauran con doble clic)`);
      return partes.join(' · ');
    },
    [nodes.length]
  );

  const aplicarPlanVozRef = useRef<((p: PlanVoz) => Promise<any>) | null>(null);

  const aplicarPlanVoz = useCallback(
    async (plan: PlanVoz) => {
      if (!plan.comandos?.length) return null;
      takeSnapshot(nodes, edges);

      const nuevos: CustomNode[] = [];
      const tituloAId = new Map<string, string>();
      const resolver = (ref: string): string => {
        const limpio = ref.trim();
        if (!limpio) return '';
        const porTitulo = tituloAId.get(limpio.toLowerCase());
        if (porTitulo) return porTitulo;
        if (nodes.some((n) => n.id === limpio)) return limpio;
        const porNombre = nodes.find((n) => (n.data.title || '').trim().toLowerCase() === limpio.toLowerCase());
        return porNombre?.id || '';
      };

      const maxX = nodes.reduce((m, n) => Math.max(m, n.position.x), 0);
      const minY = nodes.reduce((m, n) => Math.min(m, n.position.y), nodes[0]?.position.y ?? 0);
      const enlaces: Edge[] = [];
      const colapsar: string[] = [];
      const criticar: string[] = [];
      let enlacesFallidos = 0;
      // Ancla del lienzo: un dictado es un aporte a la estructura que ya existe, no una isla.
      // Antes, cada nodo creado por voz quedaba flotando (el jardín lo marcaba como huérfano).
      const ancla =
        nodes.find((n) => n.data.isRoot)?.id ||
        nodes.find((n) => !edges.some((e) => e.target === n.id))?.id ||
        '';
      // Mutaciones a nodos que ya existen (`actualizar`): la evolución por fases de un nodo.
      const cambios: { id: string; campos: Record<string, any> }[] = [];
      let enfocar: { ids: string[]; criterio?: string } | null = null;
      let creados = 0;

      plan.comandos.forEach((c, i) => {
        if (c.accion === 'crear') {
          const id = `node-voz-${Date.now()}-${i}`;
          nuevos.push({
            id,
            type: 'ideaNode',
            position: posicionLibre([...nodes, ...nuevos], { x: maxX + 380, y: minY + creados * 150 }),
            data: {
              id,
              title: c.titulo || 'Idea dictada',
              description: c.descripcion || '',
              category: (c.categoria || 'VOZ').toUpperCase(),
              colorAccent: '#22d3ee',
              maturity: 1,
              tags: ['Voz'],
              aiOrigin: {
                batchId: `voz-${Date.now()}`,
                actionType: 'braindump',
                promptOriginal: c.titulo || '',
                originalTitle: c.titulo || '',
                allBatchTitles: [],
                createdAt: new Date().toISOString(),
              },
            },
          });
          tituloAId.set((c.titulo || '').trim().toLowerCase(), id);
          creados++;
          // Se cuelga del padre que el plan declare (id o título) o, si no dice nada, del ancla.
          // Si el propio plan ya lo enlaza, no se agrega una segunda arista: el plan manda.
          const refPadre = c.parent || '';
          const padre = refPadre ? resolver(refPadre) : ancla;
          const tituloNuevo = (c.titulo || '').trim().toLowerCase();
          const yaEnlazado = plan.comandos.some(
            (x) =>
              x.accion === 'enlazar' &&
              [x.desde, x.hasta].some((r) => (r || '').trim().toLowerCase() === tituloNuevo)
          );
          if (padre && padre !== id && !yaEnlazado) {
            enlaces.push({
              id: `e-voz-${Date.now()}-${i}-ancla`,
              source: padre,
              target: id,
              type: 'smoothstep',
              animated: true,
              label: 'voz',
            } as Edge);
          }
        } else if (c.accion === 'enlazar') {
          const a = resolver(c.desde || '');
          const b = resolver(c.hasta || '');
          if (!a || !b || a === b) enlacesFallidos++;
          if (a && b && a !== b) {
            enlaces.push({
              id: `e-voz-${Date.now()}-${i}`,
              source: a,
              target: b,
              type: 'smoothstep',
              animated: true,
              label: 'voz',
            } as Edge);
          }
        } else if (c.accion === 'enfocar') {
          enfocar = { ids: c.nodos || [], criterio: c.criterio };
        } else if (c.accion === 'condensar') {
          colapsar.push(...(c.nodos || []));
        } else if (c.accion === 'criticar') {
          criticar.push(...(c.nodos || []));
        } else if (c.accion === 'actualizar' && c.nodo) {
          const campos: Record<string, any> = {};
          if (c.titulo) campos.title = c.titulo;
          if (c.descripcion) campos.description = c.descripcion;
          if (c.categoria) campos.category = c.categoria.toUpperCase();
          if (typeof c.maturity === 'number') campos.maturity = c.maturity;
          if (c.tags?.length) campos.tags = c.tags;
          if (Object.keys(campos).length) cambios.push({ id: c.nodo, campos });
        }
      });

      let nds = [...nodes, ...nuevos];
      let eds = [...edges, ...enlaces];
      let afectados = 0;

      // Mutar los nodos que el plan pidió actualizar (sin crear duplicados).
      cambios.forEach(({ id, campos }) => {
        nds = nds.map((n) => (n.id === id ? { ...n, data: { ...n.data, ...campos } } : n));
      });

      const hacerMacro = (ids: string[], titulo: string) => {
        const idsSet = new Set(ids.filter((id) => nds.some((n) => n.id === id)));
        if (idsSet.size < 2) return;
        const dentro = nds.filter((n) => idsSet.has(n.id));
        const tocadas = eds.filter((e) => idsSet.has(e.source) || idsSet.has(e.target));
        const macroId = `node-macro-voz-${Date.now()}-${Math.floor(Math.random() * 1000)}`;
        const centro = {
          x: dentro.reduce((s, n) => s + n.position.x, 0) / dentro.length,
          y: dentro.reduce((s, n) => s + n.position.y, 0) / dentro.length,
        };
        const macro: CustomNode = {
          id: macroId,
          type: 'ideaNode',
          position: centro,
          data: {
            id: macroId,
            category: 'MACRO',
            label: 'MACRO · VOZ',
            title: titulo,
            description: `${dentro.length} nodos fuera del foco. Doble clic para ver el linaje y restaurarlos.`,
            colorAccent: '#8b5cf6',
            maturity: 4,
            tags: ['Voz', 'Condensado'],
            macro: {
              colapsados: dentro.length,
              linaje: dentro.map((n) => n.id),
              resumen: 'Colapsado al operar el lienzo por voz. Nada se perdió: está acá adentro.',
              creadoEn: new Date().toISOString(),
              datos: { nodes: dentro, edges: tocadas },
            },
          },
        };
        const reenganchadas = tocadas
          .filter((e) => idsSet.has(e.source) !== idsSet.has(e.target))
          .map((e) => ({
            ...e,
            id: `${e.id}-macro`,
            source: idsSet.has(e.source) ? macroId : e.source,
            target: idsSet.has(e.target) ? macroId : e.target,
          }));
        nds = [...nds.filter((n) => !idsSet.has(n.id)), macro];
        eds = [...eds.filter((e) => !idsSet.has(e.source) && !idsSet.has(e.target)), ...reenganchadas];
        afectados += dentro.length;
      };

      if (enfocar) {
        const objetivo = enfocar as { ids: string[]; criterio?: string };
        const conservar = new Set<string>(objetivo.ids);
        const resto = nds.filter((n) => !conservar.has(n.id)).map((n) => n.id);
        if (resto.length) hacerMacro(resto, `Fuera de foco (${resto.length} nodos)`);
      }
      if (colapsar.length) hacerMacro(colapsar, `Condensado por voz`);

      setNodes(nds);
      setEdges(eds);
      setSelectedNodes([]);

      criticar.forEach((id) => {
        // `nds`, no `nodes`: un nodo creado en este mismo dictado también se puede cuestionar.
        const n = nds.find((x) => x.id === id);
        if (n) handleAIActionRef.current?.('socratic', n.id, n.data);
      });

      showToast(
        `Voz: ${creados} nodo(s) nuevo(s)${cambios.length ? ` · ${cambios.length} actualizado(s)` : ''}${afectados ? ` · ${afectados} colapsado(s)` : ''}${criticar.length ? ` · ${criticar.length} a cuestionar` : ''}${enlacesFallidos ? ` · ${enlacesFallidos} enlace(s) sin destino` : ''}.`,
        'success'
      );

      // Motor profundo: si el plan lo pidió, se corre DESPUÉS de aplicar el lienzo. Tarda (una
      // pasada completa con herramientas), así que no frena el cambio visual ni la voz.
      const pedido = plan.comandos.find((c) => c.accion === 'delegar')?.pedido;
      let delegando = false;
      if (pedido) {
        // Se dispara y **no se espera**: el motor profundo tarda minutos y el lienzo ya cambió. La
        // respuesta llega sola y el panel la muestra cuando está (nada de spinner colgado).
        delegando = true;
        // Investigación por fases (🌱⚔️🧪🚀): el nodo nace, crece con las fuentes y se sintetiza.
        void fetch(apiUrl('/api/ai/investigar'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ pedido }),
        }).catch(() => undefined);
        showToast('Investigación en marcha por fases: el nodo va a crecer solo en el lienzo.', 'info');
      }
      return { creados, afectados, delegando };
    },
    [nodes, edges, takeSnapshot, setNodes, setEdges, setSelectedNodes, showToast]
  );

  useEffect(() => {
    aplicarPlanVozRef.current = aplicarPlanVoz;
  }, [aplicarPlanVoz]);

  /** Aplica los comandos de una fase de investigación (los emite el backend, ya validados por forma). */
  const aplicarComandosDeFase = useCallback(
    async (comandos: PlanVoz['comandos'], que: string) => {
      if (!comandos?.length) return;
      await aplicarPlanVozRef.current?.({ intencion: 'comando', respuesta: que, comandos } as PlanVoz);
    },
    []
  );

  /** Arranca una investigación por fases. El que la aplica al lienzo es el poller de abajo. */
  const investigarTema = useCallback(
    async (pedido: string) => {
      try {
        const r = await fetch(apiUrl('/api/ai/investigar'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ pedido }),
        });
        const d = await r.json().catch(() => ({}));
        if (!r.ok || d?.success === false) {
          showToast(d?.error || 'No se pudo arrancar la investigación.', 'info');
          return;
        }
        showToast('Investigación en marcha por fases: el nodo va a crecer solo en el lienzo.', 'info');
      } catch {
        showToast('El backend no respondió: probá de nuevo en un momento.', 'info');
      }
    },
    [showToast]
  );

  // ── Investigación por fases: se MIRA crecer y al terminar PROPONE ─────────────────────────
  // El backend genera las fases; la app las observa. Cuando la investigación termina, sus hallazgos
  // entran a la cola de «Cambios del agente» para que el humano decida: **nada toca el lienzo sin
  // aprobación**. Los enlaces viajan como `parent` en el alta, porque el backend resuelve un padre
  // que todavía es una propuesta (el mismo camino del hub → pilares): así el nodo central y sus
  // fuentes se proponen de una sola pasada y quedan conectados al aprobarlos.
  //
  // (Antes esto aplicaba los comandos directo desde acá. Dos errores: el nodo nunca aparecía si el
  // contador de pasos no se reiniciaba con una investigación nueva, y aplicar sin preguntar no deja
  // decidir. El contador ya no existe: cada corrida propone una vez, y el backend deduplica.)
  const [estadoInvestigacion, setEstadoInvestigacion] = useState<EstadoInvestigacion | null>(null);
  useEffect(() => {
    let vivo = true;

    /** Un tema investigado se vuelve propuestas: el nodo central (con la síntesis adentro) y sus
     *  fuentes colgadas de él. Idempotente: el backend reconoce una propuesta repetida por resumen. */
    const proponer = async (inv: EstadoInvestigacion) => {
      const pasos: PasoInvestigacion[] = Array.isArray(inv?.pasos) ? inv.pasos : [];
      const central = `Investigación: ${(inv.pedido || '').slice(0, 60)}`;
      const sintesis = (inv.salida || '').trim();
      const fuentes: { titulo: string; descripcion: string }[] = [];
      for (const paso of pasos) {
        for (const c of (paso.comandos || []) as Record<string, string>[]) {
          if (c?.accion === 'crear' && String(c?.categoria || '').toUpperCase() === 'FUENTE') {
            fuentes.push({ titulo: String(c.titulo || ''), descripcion: String(c.descripcion || '') });
          }
        }
      }
      const pedir = (cuerpo: Record<string, unknown>) =>
        fetch(apiUrl('/api/graph/node'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ ...cuerpo, prompt_original: 'Investigación por fases' }),
        }).catch(() => undefined);

      // Las fuentes **no** se proponen como nodos: van dentro de la nota del nodo central. Antes cada
      // fuente se volvía un nodo FUENTE y el lienzo se llenaba de bibliografía suelta (medido: 15 de
      // 89 nodos eran fuentes de tres investigaciones, y hubo que sintetizarlas a mano). Ahora la
      // investigación aterriza en UN nodo cuya nota lleva la evidencia: eso es lo que la bóveda
      // recuerda y lo que el cerebro puede citar después.
      const seccionFuentes = fuentes.length
        ? `\n\nFuentes:\n${fuentes
            .map((f) => {
              const resto = f.descripcion
                .split('\n')
                .map((l) => l.trim())
                .filter(Boolean)
                .join(' · ');
              return `- ${f.titulo}${resto ? ` — ${resto}` : ''}`;
            })
            .join('\n')}`
        : '';

      // 1) el nodo central: la conclusión utilizable ES el nodo (madurez de hexágono 🚀), y queda
      //    colgado del Norte Estratégico para que la investigación alimente la visión del proyecto.
      await pedir({
        title: central,
        description: `${sintesis || `Investigación por fases sobre ${inv.pedido}`}${seccionFuentes}`,
        category: 'INVESTIGACIÓN',
        maturity: 5,
        parent: 'Norte Estratégico · NodeFlow',
        link_label: 'alimenta',
      });
      showToast(
        `Investigación lista: 1 propuesta en «Cambios del agente» — la conclusión con sus ${fuentes.length} fuente(s) adentro.`,
        'success'
      );

    };

    const consultar = async () => {
      try {
        const d = await (await fetch(apiUrl('/api/ai/investigar'))).json();
        if (!vivo) return;
        const inv: EstadoInvestigacion = { ...(d?.investigacion || {}), corriendo: Boolean(d?.corriendo) };
        setEstadoInvestigacion(inv);
        const pasos: PasoInvestigacion[] = Array.isArray(inv.pasos) ? inv.pasos : [];
        // Una vez por corrida, y que sobreviva a recargas y reinicios: si no, cada arranque
        // vuelve a proponer la última investigación y —una vez aprobada— eso se convierte en
        // propuestas de «Actualizar» que ensucian la cola y pueden pisar ediciones del usuario.
        // La clave lleva el instante del primer paso: dos corridas del mismo tema son distintas.
        const claveCorrida = `${inv.pedido || ''}|${pasos[0]?.cuando || ''}`;
        let yaPropuesta = '';
        try {
          yaPropuesta = localStorage.getItem('nodeflow_inv_propuesta') || '';
        } catch {
          yaPropuesta = '';
        }
        if (inv.terminado && inv.pedido && pasos.length > 0 && yaPropuesta !== claveCorrida) {
          try {
            localStorage.setItem('nodeflow_inv_propuesta', claveCorrida);
          } catch {
            /* sin storage se reintenta en la próxima vuelta */
          }
          await proponer(inv);
        }
      } catch {
        /* el backend puede estar ocupado: se reintenta solo */
      }
    };
    void consultar();
    const t = setInterval(consultar, 4000);
    return () => {
      vivo = false;
      clearInterval(t);
    };
  }, [showToast]);

  /** Restaurar el sub-grafo original de un macro-nodo: vuelven sus nodos y sus aristas tal cual. */
  const handleRestaurarLinaje = useCallback(
    (node: CustomNode) => {
      const datos = node.data.macro?.datos;
      if (!datos || !datos.nodes?.length) {
        showToast('Este macro-nodo no tiene linaje guardado.', 'info');
        return;
      }
      takeSnapshot(nodes, edges);
      const vuelvenNodes = datos.nodes as CustomNode[];
      const vuelvenEdges = (datos.edges ?? []) as Edge[];
      setNodes((nds) => [...nds.filter((n) => n.id !== node.id), ...vuelvenNodes]);
      setEdges((eds) => [
        ...eds.filter((e) => e.source !== node.id && e.target !== node.id),
        ...vuelvenEdges,
      ]);
      setIsLinajeOpen(false);
      setNodoLinaje(null);
      showToast(`Restaurados ${vuelvenNodes.length} nodos al lienzo.`, 'success');
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  // Conexión rápida y directa entre 2 nodos seleccionados (sin tener que arrastrar cables con el ratón)
  const handleConnectSelectedNodes = useCallback(() => {
    if (selectedNodes.length !== 2) {
      showToast('Selecciona exactamente 2 nodos para unirlos con una conexión.', 'info');
      return;
    }
    const [source, target] = selectedNodes;

    // Verificar si ya existe arista previa entre ambos en cualquier dirección
    const alreadyConnected = edges.some(
      (e) =>
        (e.source === source.id && e.target === target.id) ||
        (e.source === target.id && e.target === source.id)
    );
    if (alreadyConnected) {
      showToast('Los 2 nodos seleccionados ya están conectados entre sí.', 'info');
      return;
    }

    takeSnapshot(nodes, edges);
    const newEdge: Edge = {
      id: `edge-${source.id}-${target.id}-${Date.now()}`,
      source: source.id,
      sourceHandle: 'right',
      target: target.id,
      targetHandle: 'left',
      type: edgeAppearance.type,
      animated: edgeAppearance.animated,
      style: {
        stroke: edgeAppearance.color,
        strokeWidth: edgeAppearance.strokeWidth,
      },
    };

    setEdges((eds) => [...eds, newEdge]);
    showToast(`Conexión creada: "${source.data.title || 'Nodo 1'}" ↔ "${target.data.title || 'Nodo 2'}"`, 'success');
  }, [selectedNodes, edges, nodes, edgeAppearance, takeSnapshot, showToast]);

  // Mantener actualizados los refs de atajos para el listener global
  useEffect(() => {
    handleConnectSelectedNodesRef.current = handleConnectSelectedNodes;
    handleHybridizeRef.current = handleHybridize;
    handleCondensarRef.current = handleCondensar;
  }, [handleConnectSelectedNodes, handleHybridize]);

  // Search matching node IDs
  const searchMatchingNodeIds = useMemo(() => {
    if (!searchQuery.trim()) return new Set<string>();
    const q = searchQuery.toLowerCase().trim();
    const matched = new Set<string>();
    nodes.forEach((n) => {
      const inTitle = n.data.title.toLowerCase().includes(q);
      const inDesc = n.data.description?.toLowerCase().includes(q);
      const inCategory = n.data.category?.toLowerCase().includes(q);
      const inTags = n.data.tags?.some((t) => t.toLowerCase().includes(q));
      const inMaturity = n.data.maturity
        ? MATURITY_CONFIGS[n.data.maturity]?.label.toLowerCase().includes(q)
        : false;
      if (inTitle || inDesc || inCategory || inTags || inMaturity) {
        matched.add(n.id);
      }
    });
    return matched;
  }, [searchQuery, nodes]);

  // Jump canvas to specific node
  const handleJumpToNode = useCallback((nodeId: string) => {
    const target = nodes.find((n) => n.id === nodeId);
    if (target && rfInstanceRef.current) {
      rfInstanceRef.current.setCenter(target.position.x + 130, target.position.y + 70, {
        zoom: 1.15,
        duration: 600,
      });
    }
  }, [nodes]);

  // Attach the onAction handler dynamically to node data, and apply search highlights
  // Grado de cada nodo: cuántas aristas lo tocan. Se deriva en el render (no se
  // persiste) y alimenta la jerarquía de escala: los hubs pesan más que las hojas.
  const degreeById = useMemo(() => {
    const map = new Map<string, number>();
    edges.forEach((e) => {
      map.set(e.source, (map.get(e.source) || 0) + 1);
      map.set(e.target, (map.get(e.target) || 0) + 1);
    });
    return map;
  }, [edges]);

  const accentById = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach((n) => map.set(n.id, n.data?.colorAccent || '#6366f1'));
    return map;
  }, [nodes]);

  /**
   * Zonas derivadas: marcos por nivel de profundidad, calculados desde la posición
   * y la estructura del grafo EN EL RENDER. Se inyectan como nodos de tipo `zonaNode`
   * con `pointerEvents: none` (nunca roban clic ni paneo) y `zIndex: -1` (quedan detrás
   * de las tarjetas). No entran al estado, no se guardan en el vault, no salen en el
   * minimapa. Apagarlas no cambia ningún dato.
   */
  const niveles = useMemo(() => calcularNiveles(nodes, edges), [nodes, edges]);

  /** Categorías del lienzo con su conteo y su acento, para la lente semántica. */
  const categorias = useMemo<CategoriaZona[]>(() => {
    const mapa = new Map<string, CategoriaZona>();
    nodes.forEach((n) => {
      const nombre = n.data?.category || 'SIN CATEGORÍA';
      const previo = mapa.get(nombre);
      if (previo) previo.n += 1;
      else mapa.set(nombre, { nombre, n: 1, acento: n.data?.colorAccent || '#6366f1' });
    });
    return [...mapa.values()].sort((a, b) => b.n - a.n || a.nombre.localeCompare(b.nombre));
  }, [nodes]);

  /** Ids de la categoría enfocada por la lente (null = sin lente). */
  const idsLente = useMemo(() => {
    if (!categoriaLente) return null;
    return new Set(
      nodes.filter((n) => (n.data?.category || 'SIN CATEGORÍA') === categoriaLente).map((n) => n.id)
    );
  }, [nodes, categoriaLente]);

  useEffect(() => {
    setLente(categoriaLente, idsLente || undefined);
  }, [categoriaLente, idsLente]);

  const zonaNodes = useMemo<CustomNode[]>(() => {
    if (!zonasVisibles || modoZonas !== 'nivel') return [];
    return niveles.map((nv) => {
      const miembros = nodes.filter((n) => nv.ids.includes(n.id));
      return {
        id: `zona-nivel-${nv.nivel}`,
        type: 'zonaNode',
        position: { x: nv.x, y: nv.y },
        // React Flow v11 oculta (`visibility: hidden`) todo nodo sin `width`/`height`
        // propios: no alcanza con ponerlos en `style`.
        width: nv.width,
        height: nv.height,
        draggable: false,
        selectable: false,
        connectable: false,
        focusable: false,
        ariaLabel: `Zona nivel ${nv.nivel}`,
        zIndex: -1,
        style: { width: nv.width, height: nv.height, pointerEvents: 'none' as const },
        data: {
          nivel: nv.nivel,
          conteo: nv.ids.length,
          categoria: nv.categoria,
          acento: acentoDeNivel(miembros),
        },
      } as unknown as CustomNode;
    });
  }, [niveles, nodes, zonasVisibles, modoZonas]);

  const processedNodes = useMemo(() => {
    const hasSearch = searchQuery.trim().length > 0;
    const ideas = nodes.map((node) => {
      const isMatched = searchMatchingNodeIds.has(node.id);
      const enLente = idsLente ? idsLente.has(node.id) : null;
      // La lente semántica y la búsqueda comparten el mecanismo de atenuación.
      const atenuado = (hasSearch && !isMatched) || enLente === false;
      return {
        ...node,
        className: atenuado ? 'opacity-25 transition-opacity duration-300' : undefined,
        data: {
          ...node.data,
          isSearchMatch: isMatched,
          degree: degreeById.get(node.id) || 0,
          lente: enLente === true,
          onAction: (...args: any[]) => (handleAIAction as any)(...args),
        },
      };
    });
    return [...zonaNodes, ...ideas];
  }, [nodes, searchQuery, searchMatchingNodeIds, handleAIAction, degreeById, zonaNodes, idsLente]);

/**
 * Etiquetas de arista que SÍ se dibujan sobre el lienzo.
 *
 * La app esconde todas las etiquetas a propósito: React Flow las dibuja siempre y con 52 de 53
 * aristas rotuladas era el mayor foco de ruido (por eso viven en `data.label`, en hover). Pero las
 * del **mapa del proyecto** son otra cosa: son la relación entre los nodos (qué contiene a qué, qué
 * requiere qué, qué está bloqueado), son ~20 de 107 aristas y sin ellas el mapa no se puede leer.
 */
  /**
   * Aristas de render. NO se toca el estado guardado (el vault sigue con las
   * mismas aristas): acá se cambia sólo lo que necesita la vista —
   *   · tipo propio `flowEdge` (etiqueta en hover + atenuación por foco),
   *   · la etiqueta pasa de `edge.label` a `data.label`, porque React Flow
   *     dibuja `label` SIEMPRE y era el mayor foco de ruido (52/53 aristas),
   *   · `animated: false` en el grupo: el dash lo prendemos nosotros dentro del foco,
   *   · el acento de los nodos que conecta, para pintar las aristas del foco.
   */
  const processedEdges = useMemo<Edge[]>(
    () =>
      edges.map((edge) => ({
        ...edge,
        type: 'flowEdge',
        animated: false,
        // La etiqueta viaja en `data.label`: el tipo propio `flowEdge` decide cuándo dibujarla
        // (siempre para las relaciones del mapa, en hover para el resto).
        data: {
          ...(edge.data || {}),
          label: (edge.label as string) || undefined,
          curve: edge.type || edgeAppearance.type,
          color: (edge.style?.stroke as string) || edgeAppearance.color,
          strokeWidth: Number(edge.style?.strokeWidth ?? edgeAppearance.strokeWidth),
          animated: edgeAppearance.animated,
          sourceColor: accentById.get(edge.source) || edgeAppearance.color,
          targetColor: accentById.get(edge.target) || edgeAppearance.color,
        },
      })),
    [edges, edgeAppearance, accentById]
  );

  // Auto Layout utility integration
  const handleAutoLayout = useCallback(() => {
    if (nodes.length === 0) return;
    takeSnapshot(nodes, edges);
    const layouted = autoLayoutNodes(nodes, edges, 'TB');
    setNodes(layouted);
    if (rfInstanceRef.current) {
      setTimeout(() => {
        rfInstanceRef.current?.fitView({ padding: 0.2, duration: 600 });
        const z = rfInstanceRef.current?.getZoom() || 1;
        setZoomPercent(Math.round(z * 100));
      }, 60);
    }
    showToast('Estructura auto-organizada en jerarquía limpia', 'success');
  }, [nodes, edges, takeSnapshot, showToast]);

  // Fit view helper
  const handleFitView = useCallback(() => {
    if (rfInstanceRef.current) {
      rfInstanceRef.current.fitView({ padding: 0.2, duration: 500 });
      const z = rfInstanceRef.current.getZoom();
      setZoomPercent(Math.round(z * 100));
    }
  }, []);

  // AI Whole-Map Synthesis Trigger
  const handleOpenSynthesis = useCallback(async () => {
    setIsSynthesisModalOpen(true);
    if (mapSynthesis) return; // Cached
    setIsSynthesisLoading(true);
    try {
      const res = await postAiAction({
        type: 'synthesize',
        nodes: nodes,
      });
      const data = await res.json();
      if (data.success && data.synthesis) {
        setMapSynthesis(data.synthesis);
      } else {
        throw new Error(data.error || 'Respuesta inesperada');
      }
    } catch (err) {
      console.error('Error generating map synthesis:', err);
      showToast('Error al sintetizar el mapa conceptual.', 'error');
    } finally {
      setIsSynthesisLoading(false);
    }
  }, [nodes, mapSynthesis, showToast]);

  const handleRefreshSynthesis = useCallback(async () => {
    setIsSynthesisLoading(true);
    try {
      const res = await postAiAction({
        type: 'synthesize',
        nodes: nodes,
      });
      const data = await res.json();
      if (data.success && data.synthesis) {
        setMapSynthesis(data.synthesis);
        showToast('Síntesis estratégica actualizada', 'success');
      } else {
        throw new Error(data.error || 'Respuesta inválida');
      }
    } catch (err) {
      console.error('Error regenerating map synthesis:', err);
      showToast('Error al regenerar síntesis.', 'error');
    } finally {
      setIsSynthesisLoading(false);
    }
  }, [nodes, showToast]);

  // Función de descomposición instantánea local (0 latencia / sin depender de red)
  const parseTextToLocalStructure = (rawText: string) => {
    const cleanText = rawText.trim();
    // Dividir por saltos de línea, viñetas (- * •) o líneas numeradas
    let lines = cleanText
      .split(/\r?\n+/)
      .map((l) => l.replace(/^[-*•\d.]+\s*/, '').trim())
      .filter((l) => l.length > 0);

    if (lines.length === 0) {
      lines = [cleanText.slice(0, 60)];
    }

    const rootTitle = lines[0].slice(0, 45) || 'Idea Central';
    let childLines = lines.slice(1);

    // Si el usuario introdujo solo 1 frase, inferir 3 dimensiones operativas de arranque
    if (childLines.length === 0) {
      childLines = [
        'Propuesta de Valor y Adopción',
        'Arquitectura y Funcionalidad Clave',
        'Métricas de Éxito y Viabilidad',
      ];
    }

    const categories = ['ARQUITECTURA', 'ESTRATEGIA', 'EJECUCIÓN', 'MÉTRICAS', 'VALIDACIÓN'];

    return {
      root: {
        title: rootTitle,
        description: cleanText.length > 50 ? cleanText.slice(0, 160) : 'Núcleo conceptual principal',
        category: 'NÚCLEO',
        tags: ['BrainDump', 'Núcleo'],
      },
      nodes: childLines.slice(0, 10).map((line, idx) => ({
        tempId: `node-${idx + 1}`,
        connectsTo: 'root',
        title: line.slice(0, 42),
        description: line.length > 42 ? line : 'Concepto derivado de la descarga mental',
        category: categories[idx % categories.length],
        tags: ['Idea', 'Estructura'],
      })),
    };
  };

  // Manejador de Descarga Mental (Brain Dump Rápido con soporte dual: IA o Instantáneo)
  const handleBrainDumpSubmit = useCallback(
    async (rawText: string, mode: 'ai' | 'instant' = 'ai') => {
      setIsBrainDumpLoading(true);
      try {
        let structure: any = null;
        let usedLocalFallback = false;

        if (mode === 'instant') {
          // Modo 100% instantáneo (0ms)
          structure = parseTextToLocalStructure(rawText);
        } else {
          showToast(`${motorActual()} está procesando y estructurando tu descarga mental...`, 'info');
          try {
            const res = await postAiAction({
              type: 'braindump',
              rawText,
              hitlProfileOverride: hitlProfile.learnedProfile,
            });
            const data = await res.json();
            if (data.success && data.structure?.root && Array.isArray(data.structure?.nodes)) {
              structure = data.structure;
            }
          } catch (netErr) {
            console.warn('AI braindump request failed, using instant engine fallback:', netErr);
          }

          if (!structure) {
            structure = parseTextToLocalStructure(rawText);
            usedLocalFallback = true;
          }
        }

        if (structure?.root && Array.isArray(structure?.nodes)) {
          takeSnapshot(nodes, edges);

          const { root, nodes: childNodes } = structure;

          // Posición base del nuevo núcleo
          let rootX = 350;
          let rootY = 250;
          if (nodes.length > 0) {
            const maxX = Math.max(...nodes.map((n) => n.position.x));
            rootX = maxX + 420;
            rootY = 220;
          }

          const rootId = `node-bd-${Date.now()}`;
          const newRootNode: CustomNode = {
            id: rootId,
            type: 'ideaNode',
            position: { x: rootX, y: rootY },
            data: {
              id: rootId,
              title: root.title || 'Idea Central',
              description: root.description || '',
              category: root.category || 'NÚCLEO',
              label: root.category || 'NÚCLEO',
              tags: root.tags || ['BrainDump', 'Núcleo'],
              colorAccent: '#10b981', // Esmeralda para ideas nacidas de BrainDump
              isRoot: true,
              aiOrigin: {
                batchId: `bd-${Date.now()}`,
                actionType: 'braindump',
                promptOriginal: rawText.slice(0, 120),
                originalTitle: root.title,
                allBatchTitles: [root.title, ...childNodes.map((c: any) => c.title)],
                createdAt: new Date().toISOString(),
              },
              onAction: handleAIAction,
            },
          };

          const tempIdMap: Record<string, string> = { root: rootId };
          const newCreatedNodes: CustomNode[] = [newRootNode];
          const newCreatedEdges: Edge[] = [];
          const childCount = childNodes.length;

          childNodes.forEach((item: any, idx: number) => {
            const childId = `node-bd-${Date.now()}-${idx + 1}-${Math.random().toString(36).substring(2, 5)}`;
            tempIdMap[item.tempId || `node-${idx + 1}`] = childId;

            // Disposición angular alrededor del núcleo
            const angle = (2 * Math.PI / Math.max(childCount, 1)) * idx - Math.PI / 2;
            const radius = 280;
            const childX = rootX + Math.cos(angle) * radius;
            const childY = rootY + Math.sin(angle) * (radius * 0.7);

            const childNode: CustomNode = {
              id: childId,
              type: 'ideaNode',
              position: { x: Math.round(childX), y: Math.round(childY) },
              data: {
                id: childId,
                title: item.title,
                description: item.description || '',
                category: item.category || 'IDEA',
                label: item.category || 'IDEA',
                tags: item.tags || ['Descarga'],
                colorAccent: idx % 2 === 0 ? '#06b6d4' : '#6366f1',
                aiOrigin: {
                  batchId: `bd-${Date.now()}`,
                  actionType: 'braindump',
                  promptOriginal: rawText.slice(0, 120),
                  originalTitle: item.title,
                  allBatchTitles: [root.title, ...childNodes.map((c: any) => c.title)],
                  createdAt: new Date().toISOString(),
                },
                onAction: handleAIAction,
              },
            };
            newCreatedNodes.push(childNode);

            const parentId = tempIdMap[item.connectsTo] || rootId;
            const edgeId = `edge-${parentId}-${childId}-${Date.now()}`;
            newCreatedEdges.push({
              id: edgeId,
              source: parentId,
              target: childId,
              type: edgeAppearance.type,
              animated: edgeAppearance.animated,
              style: {
                stroke: edgeAppearance.color,
                strokeWidth: edgeAppearance.strokeWidth,
              },
            });
          });

          const combinedNodes = [...nodes, ...newCreatedNodes];
          const combinedEdges = [...edges, ...newCreatedEdges];
          const organizedNodes = autoLayoutNodes(combinedNodes, combinedEdges);

          setNodes(organizedNodes);
          setEdges(combinedEdges);
          setIsBrainDumpOpen(false);

          const successMsg = usedLocalFallback
            ? `Estructurado con motor instantáneo: ${newCreatedNodes.length} nodos integrados`
            : mode === 'instant'
            ? `Volcado instantáneo listo: ${newCreatedNodes.length} nodos creados en el lienzo`
            : `Descarga mental estructurada con ${motorActual()}: ${newCreatedNodes.length} nodos integrados`;
          showToast(successMsg, 'success');

          // Centrar el viewport en el nuevo núcleo
          if (rfInstanceRef.current) {
            const placedRoot = organizedNodes.find((n) => n.id === rootId);
            if (placedRoot) {
              rfInstanceRef.current.setCenter(placedRoot.position.x + 130, placedRoot.position.y + 70, {
                zoom: 0.9,
                duration: 600,
              });
            }
          }
        } else {
          throw new Error('Estructura no válida');
        }
      } catch (err) {
        console.error('Error in brain dump action:', err);
        showToast('No se pudo procesar la descarga mental. Intenta de nuevo.', 'error');
      } finally {
        setIsBrainDumpLoading(false);
      }
    },
    [nodes, edges, hitlProfile.learnedProfile, edgeAppearance, handleAIAction, takeSnapshot, showToast]
  );

  // Escaneo de Puentes Semánticos (Conexiones Ocultas)
  const handleScanBridges = useCallback(async () => {
    if (nodes.length < 2) {
      showToast('Se necesitan al menos 2 nodos en el mapa para hallar conexiones ocultas', 'info');
      return;
    }
    setIsBridgesLoading(true);
    try {
      showToast(`${motorActual()} está analizando conexiones conceptuales no evidentes...`, 'info');
      const res = await postAiAction({
        type: 'find_bridges',
        nodes: nodes.map((n) => ({
          id: n.id,
          data: {
            title: n.data.title,
            category: n.data.category || n.data.label,
            description: n.data.description,
          },
        })),
        edges: edges.map((e) => ({ source: e.source, target: e.target })),
        hitlProfileOverride: hitlProfile.learnedProfile,
      });
      const data = await res.json();
      if (data.success && Array.isArray(data.bridges)) {
        const formatted: SemanticBridge[] = data.bridges.map((b: any) => ({
          id: b.id || `${b.sourceId}-${b.targetId}`,
          sourceId: b.sourceId,
          targetId: b.targetId,
          sourceTitle: b.sourceTitle || 'Idea A',
          targetTitle: b.targetTitle || 'Idea B',
          label: b.label || 'Sinergia',
          rationale: b.rationale || '',
        }));
        setBridges(formatted);
        if (formatted.length > 0) {
          showToast(`Se detectaron ${formatted.length} puentes semánticos ocultos`, 'success');
        } else {
          showToast('No se detectaron nuevas conexiones ocultas en este momento', 'info');
        }
      } else {
        throw new Error(data.error || 'Error al buscar puentes');
      }
    } catch (err) {
      console.error('Error scanning semantic bridges:', err);
      showToast('Error al escanear conexiones ocultas.', 'error');
    } finally {
      setIsBridgesLoading(false);
    }
  }, [nodes, edges, hitlProfile.learnedProfile, showToast]);

  const handleOpenBridgesModal = useCallback(() => {
    setIsBridgesModalOpen(true);
    if (bridges.length === 0 && nodes.length >= 2) {
      handleScanBridges();
    }
  }, [bridges.length, nodes.length, handleScanBridges]);

  const handleConnectBridge = useCallback(
    (bridge: SemanticBridge) => {
      takeSnapshot(nodes, edges);
      const newEdge: Edge = {
        id: `bridge-${bridge.sourceId}-${bridge.targetId}-${Date.now()}`,
        source: bridge.sourceId,
        target: bridge.targetId,
        type: 'smoothstep',
        animated: true,
        label: bridge.label,
        style: {
          stroke: '#a855f7', // violeta
          strokeWidth: 2,
          strokeDasharray: '4 4',
        },
        labelStyle: { fill: '#ddd6fe', fontWeight: 600, fontSize: 11 },
        labelBgStyle: { fill: '#0f172a', fillOpacity: 0.95, stroke: '#7c3aed', strokeWidth: 1 },
      };
      setEdges((eds) => [...eds, newEdge]);
      setConnectedBridgeIds((prev) => new Set([...prev, bridge.id]));
      showToast(`Puente conectado: "${bridge.sourceTitle}" ↔ "${bridge.targetTitle}" (${bridge.label})`, 'success');
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  const handleConnectAllBridges = useCallback(
    (bridgesToConnect: SemanticBridge[]) => {
      if (bridgesToConnect.length === 0) return;
      takeSnapshot(nodes, edges);
      const newEdges: Edge[] = bridgesToConnect.map((bridge) => ({
        id: `bridge-${bridge.sourceId}-${bridge.targetId}-${Date.now()}-${Math.random().toString(36).substring(2, 5)}`,
        source: bridge.sourceId,
        target: bridge.targetId,
        type: 'smoothstep',
        animated: true,
        label: bridge.label,
        style: {
          stroke: '#a855f7',
          strokeWidth: 2,
          strokeDasharray: '4 4',
        },
        labelStyle: { fill: '#ddd6fe', fontWeight: 600, fontSize: 11 },
        labelBgStyle: { fill: '#0f172a', fillOpacity: 0.95, stroke: '#7c3aed', strokeWidth: 1 },
      }));
      setEdges((eds) => [...eds, ...newEdges]);
      setConnectedBridgeIds((prev) => {
        const updated = new Set(prev);
        bridgesToConnect.forEach((b) => updated.add(b.id));
        return updated;
      });
      showToast(`${bridgesToConnect.length} puentes semánticos conectados en el lienzo`, 'success');
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  // Node modal save (captures HITL refinement telemetry when users adjust AI suggestions)
  const handleSaveNodeEdit = useCallback(
    (updatedData: IdeaNodeData) => {
      takeSnapshot(nodes, edges);

      const targetNode = nodes.find((n) => n.id === updatedData.id);
      if (targetNode?.data.aiOrigin) {
        const origin = targetNode.data.aiOrigin;
        const titleChanged = updatedData.title.trim() !== origin.originalTitle.trim();
        const descChanged = updatedData.description.trim() !== (targetNode.data.description || '').trim();

        if (titleChanged || descChanged) {
          const feedback: FeedbackEvent = {
            id: `hitl-edit-${Date.now()}`,
            timestamp: new Date().toISOString(),
            action: 'NODE_EDIT',
            prompt_original: origin.promptOriginal,
            ai_suggestion: origin.allBatchTitles,
            human_decision: {
              accepted: [updatedData.title],
              rejected: titleChanged ? [origin.originalTitle] : [],
              added_manually: titleChanged ? [updatedData.title] : [],
            },
            contextSnippet: `Usuario editó y refinó propuesta IA: "${origin.originalTitle}" -> "${updatedData.title}"`,
          };

          recordHitlFeedback(feedback).then((updated) => {
            if (updated) setHitlProfile(updated);
          });
          showToast('🧠 Decisión aprendida por el motor HITL', 'info');
        }
      }

      setNodes((nds) =>
        nds.map((n) => (n.id === updatedData.id ? { ...n, data: { ...n.data, ...updatedData } } : n))
      );
      showToast('Nodo actualizado con éxito', 'success');
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  // Apply chosen edge appearance to currently selected edges
  const handleApplyAppearanceToSelectedEdges = useCallback(() => {
    if (selectedEdges.length === 0) return;
    takeSnapshot(nodes, edges);

    const selectedIds = new Set(selectedEdges.map((e) => e.id));
    setEdges((eds) =>
      eds.map((edge) => {
        if (selectedIds.has(edge.id)) {
          return {
            ...edge,
            type: edgeAppearance.type,
            animated: edgeAppearance.animated,
            style: {
              ...edge.style,
              stroke: edgeAppearance.color,
              strokeWidth: edgeAppearance.strokeWidth,
            },
          };
        }
        return edge;
      })
    );
    showToast(`Estilo aplicado a ${selectedEdges.length} conexiones`, 'success');
  }, [selectedEdges, nodes, edges, edgeAppearance, takeSnapshot, showToast]);

  // Firma del lienzo actual: se compara con la de la sesión cargada para saber si se movió.
  const firmaActual = useMemo(() => firmaLienzo(nodes, edges), [nodes, edges]);
  const sesionActivaDrift = Boolean(sesionActiva && sesionActiva.firma !== firmaActual);

  // Estado del guardado, a la vista: es la pregunta que el usuario se hace todo el tiempo. Nunca dice
  // «al día» sin haber hablado con el disco: si el vault no respondió, eso es lo que se muestra.
  const estadoGuardado = useMemo(() => {
    if (!vaultInfo) {
      return { texto: t('hud.estado.sinVault'), clase: 'text-rose-300 border-rose-800/60 bg-rose-950/20', punto: 'bg-rose-400' };
    }
    if (lastSyncText.startsWith('Vault: error')) {
      return { texto: t('hud.estado.error'), clase: 'text-rose-300 border-rose-800/60 bg-rose-950/20', punto: 'bg-rose-400' };
    }
    if (saveStatus === 'saving') {
      return { texto: t('hud.estado.guardando'), clase: 'text-amber-200 border-amber-800/60 bg-amber-950/20', punto: 'bg-amber-400 animate-pulse' };
    }
    if (saveStatus === 'unsaved') {
      return { texto: t('hud.estado.sinGuardar'), clase: 'text-rose-300 border-rose-800/60 bg-rose-950/20', punto: 'bg-rose-400' };
    }
    return { texto: t('hud.estado.alDia'), clase: 'text-emerald-200 border-emerald-800/50 bg-emerald-950/20', punto: 'bg-emerald-400' };
  }, [vaultInfo, saveStatus, lastSyncText, t]);

  // Saved states actions (todas hablan con la bóveda por el backend, no con el WebView)
  const handleSaveNewState = useCallback(
    async (name: string) => {
      const firma = firmaLienzo(nodes, edges);
      const ficha = await guardarSesion({
        nombre: name,
        mapa: vaultInfo?.mapa,
        nodes,
        edges,
        appearance: edgeAppearance,
        templateId: currentTemplateId,
      });
      if (!ficha) {
        showToast('No pude guardar la sesión en la bóveda', 'error');
        return null;
      }
      setSesionActiva({ id: ficha.id, nombre: ficha.nombre, firma });
      await refrescarSesiones();
      showToast(`Sesión «${ficha.nombre}» guardada en la bóveda`, 'success');
      return ficha.id;
    },
    [nodes, edges, edgeAppearance, currentTemplateId, vaultInfo?.mapa, refrescarSesiones, showToast]
  );

  const handleLoadState = useCallback(
    async (sesion: { id: string; nombre: string }) => {
      const completa = await leerSesion(sesion.id);
      if (!completa || !Array.isArray(completa.nodes)) {
        showToast(`No pude leer la sesión «${sesion.nombre}» de la bóveda`, 'error');
        return;
      }
      takeSnapshot(nodes, edges);
      setNodes(completa.nodes);
      setEdges(completa.edges || []);
      if (completa.appearance) {
        setEdgeAppearance(completa.appearance as EdgeAppearance);
      }
      if (completa.templateId) {
        setCurrentTemplateId(completa.templateId);
      }
      setSesionActiva({
        id: completa.id,
        nombre: completa.nombre,
        firma: firmaLienzo(completa.nodes, completa.edges || []),
      });
      showToast(`Sesión «${completa.nombre}» cargada en el lienzo (Ctrl+Z la revierte)`, 'success');
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  /** Sobrescribe una sesión con el lienzo actual: evita acumular copias casi iguales. */
  const handleUpdateState = useCallback(
    async (id: string) => {
      const firma = firmaLienzo(nodes, edges);
      const nombre = sesiones.find((s) => s.id === id)?.nombre || 'Sesión';
      const ficha = await guardarSesion({
        id,
        nombre,
        mapa: vaultInfo?.mapa,
        nodes,
        edges,
        appearance: edgeAppearance,
        templateId: currentTemplateId,
      });
      if (!ficha) {
        showToast('No pude actualizar la sesión en la bóveda', 'error');
        return;
      }
      setSesionActiva({ id, nombre: ficha.nombre, firma });
      await refrescarSesiones();
      showToast(`Sesión «${ficha.nombre}» actualizada con el lienzo actual`, 'success');
    },
    [nodes, edges, edgeAppearance, currentTemplateId, sesiones, vaultInfo?.mapa, refrescarSesiones, showToast]
  );

  const handleDeleteState = useCallback(
    async (id: string) => {
      const ok = await borrarSesion(id);
      if (!ok) {
        showToast('No pude borrar la sesión', 'error');
        return;
      }
      // Si era la cargada, el lienzo sigue ahí: sólo deja de tener sesión de origen.
      setSesionActiva((prev) => (prev?.id === id ? null : prev));
      await refrescarSesiones();
      showToast('Sesión borrada de la bóveda', 'info');
    },
    [refrescarSesiones, showToast]
  );

  const handleImportJSON = useCallback(
    (imported: any, options?: { merge?: boolean }) => {
      if (!imported || !Array.isArray(imported.nodes)) {
        showToast('El archivo importado no contiene nodos válidos.', 'error');
        return;
      }
      takeSnapshot(nodes, edges);

      // REEMPLAZAR: restaura un diseño guardado tal cual.
      if (!options?.merge) {
        setNodes(imported.nodes);
        setEdges(imported.edges || []);
        if (imported.edgeAppearance) {
          setEdgeAppearance(imported.edgeAppearance);
        }
        showToast(`Diseño importado (${imported.nodes.length} nodos)`, 'success');
        return;
      }

      // FUSIONAR: suma lo que falta, nunca borra, y descarta aristas colgadas.
      const existingNodeIds = new Set(nodes.map((n) => n.id));
      const nodosNuevos = imported.nodes.filter((n: any) => n && n.id && !existingNodeIds.has(n.id));
      const idsFinales = new Set<string>([...existingNodeIds, ...nodosNuevos.map((n: any) => n.id)]);
      const existingEdgeIds = new Set(edges.map((e) => e.id));
      const aristasNuevas = (imported.edges || []).filter(
        (e: any) => e && e.id && !existingEdgeIds.has(e.id) && idsFinales.has(e.source) && idsFinales.has(e.target)
      );
      if (imported.edgeAppearance) {
        setEdgeAppearance(imported.edgeAppearance);
      }
      setNodes([...nodes, ...nodosNuevos]);
      setEdges([...edges, ...aristasNuevas]);

      const descartadas =
        imported.nodes.length - nodosNuevos.length + ((imported.edges || []).length - aristasNuevas.length);
      showToast(
        `Fusionado: +${nodosNuevos.length} nodos, +${aristasNuevas.length} aristas${
          descartadas > 0 ? ` (${descartadas} descartados: ya existían o sin nodo)` : ''
        }`,
        'success'
      );
    },
    [nodes, edges, takeSnapshot, showToast]
  );

  // Refresh Idea Cores keeping key topics (Tecnología, Negocios, Diseño, Investigación, Esencial)
  const handleRefreshTemplates = useCallback(async () => {
    setIsRefreshingTemplates(true);
    try {
      showToast('Generando nuevos núcleos de ideas frescas con IA...', 'info');
      let newTemplates: TemplateDefinition[] = [];

      try {
        const res = await postAiAction({
          type: 'refresh_templates',
          hitlProfileOverride: hitlProfile.learnedProfile,
        });
        const data = await res.json();
        if (data.success && Array.isArray(data.templates) && data.templates.length > 0) {
          newTemplates = formatAiGeneratedTemplates(data.templates);
        }
      } catch (apiErr) {
        console.warn('AI generation not responding, rotating fresh pack:', apiErr);
      }

      // Si la IA no generó o tardó, rotamos inmediatamente al siguiente pack fresco
      if (newTemplates.length === 0) {
        const pack = getNextFreshTemplates();
        newTemplates = pack.templates;
      }

      setTemplates(newTemplates);
      try {
        localStorage.setItem('nodeflow_active_templates', JSON.stringify(newTemplates));
      } catch {}

      showToast('✨ Núcleos de ideas renovados con conceptos frescos e innovadores', 'success');
    } catch (err) {
      console.error('Error refreshing templates:', err);
      const pack = getNextFreshTemplates();
      setTemplates(pack.templates);
      showToast('Núcleos de ideas renovados con nuevos conceptos', 'success');
    } finally {
      setIsRefreshingTemplates(false);
    }
  }, [hitlProfile.learnedProfile, showToast]);

  // Switch template
  const handleSelectTemplate = useCallback(
    (templateId: string) => {
      const template = templates.find((t) => t.id === templateId) || INITIAL_TEMPLATES.find((t) => t.id === templateId);
      if (!template) return;
      takeSnapshot(nodes, edges);
      setNodes(template.nodes);
      setEdges(template.edges);
      setEdgeAppearance(template.appearance);
      setCurrentTemplateId(template.id);
      resetHistory(template.nodes, template.edges);
      if (rfInstanceRef.current) {
        setTimeout(() => {
          rfInstanceRef.current?.fitView({ padding: 0.25, duration: 600 });
        }, 60);
      }
      showToast(`Plantilla "${template.title}" cargada exitosamente`, 'success');
    },
    [templates, nodes, edges, takeSnapshot, resetHistory, showToast]
  );

  // Clear all nodes completely
  const handleClearAllNodes = useCallback(() => {
    takeSnapshot(nodes, edges);
    setNodes([]);
    setEdges([]);
    setSelectedNodes([]);
    setSelectedEdges([]);
    setCurrentTemplateId('empty');
    showToast('Todos los nodos han sido borrados. (Puedes presionar Ctrl + Z para deshacer)', 'info');
  }, [nodes, edges, takeSnapshot, showToast]);

  // Reset to single root idea node
  const handleResetToRoot = useCallback(() => {
    const blank = INITIAL_TEMPLATES.find((t) => t.id === 'blank') || INITIAL_TEMPLATES[0];
    takeSnapshot(nodes, edges);
    setNodes(blank.nodes);
    setEdges(blank.edges);
    setEdgeAppearance(blank.appearance);
    setSelectedNodes([]);
    setSelectedEdges([]);
    setCurrentTemplateId('blank');
    if (rfInstanceRef.current) {
      setTimeout(() => {
        rfInstanceRef.current?.fitView({ padding: 0.3, duration: 500 });
      }, 60);
    }
    showToast('Lienzo reiniciado con un nuevo núcleo de idea', 'success');
  }, [nodes, edges, takeSnapshot, showToast]);

  // Manual save trigger: escribe en disco AHORA (no espera el debounce) y deja una sesión con nombre.
  const handleManualSave = useCallback(async () => {
    try {
      const payload = {
        nodes,
        edges,
        appearance: edgeAppearance,
        templateId: currentTemplateId,
        updatedAt: Date.now(),
      };
      localStorage.setItem(ACTIVE_CANVAS_STORAGE_KEY, JSON.stringify(payload));
      setSaveStatus('saving');
      const raiz = nodes.find((n) => n.data.isRoot)?.data.title;
      const res = await saveVault({
        name: raiz || 'nodeflow',
        nodes,
        edges,
        appearance: edgeAppearance,
        templateId: currentTemplateId,
        base_revision: vaultRevRef.current,
      });
      if (res?.ok) {
        if (typeof res.revision === 'number') vaultRevRef.current = res.revision;
        setSaveStatus('saved');
        setLastSyncText(`Disco ${new Date().toLocaleTimeString('es-AR', { hour12: false })}`);
        setVaultInfo(await fetchVaultInfo());
      } else {
        setSaveStatus('unsaved');
        setLastSyncText('Vault: error al escribir');
      }
      // El nombre sale del nodo raíz + la hora: «Guardado manual 13:03» no distingue una sesión de otra.
      const snapTitle = nombreDeSesion(nodes);
      await handleSaveNewState(snapTitle);
    } catch (e) {
      setSaveStatus('unsaved');
      showToast('Error al persistir el estado en el navegador', 'error');
    }
  }, [nodes, edges, edgeAppearance, currentTemplateId, handleSaveNewState, showToast]);

  /** El lienzo más reciente y el último escrito de la rodante, en refs: el intervalo no se rearma. */
  const lienzoRef = useRef({
    nodes,
    edges,
    appearance: edgeAppearance,
    templateId: currentTemplateId,
    firma: firmaActual,
    mapa: vaultInfo?.mapa,
  });
  const rodanteRef = useRef<{ ms: number; firma: string }>({ ms: 0, firma: '' });
  useEffect(() => {
    lienzoRef.current = {
      nodes,
      edges,
      appearance: edgeAppearance,
      templateId: currentTemplateId,
      firma: firmaActual,
      mapa: vaultInfo?.mapa,
    };
  }, [nodes, edges, edgeAppearance, currentTemplateId, firmaActual, vaultInfo?.mapa]);

  // Sesión rodante: una sola que se sobrescribe, para poder «volver a como estaba hace un rato».
  // Aprende la línea base al arrancar (sin escribir) y después escribe como mucho cada 10 minutos,
  // y sólo si el lienzo cambió desde la última vez: no acumula copias ni castiga el disco.
  useEffect(() => {
    const id = setInterval(async () => {
      const l = lienzoRef.current;
      if (!vaultReadyRef.current || !l.firma) return;
      const ahora = Date.now();
      const r = rodanteRef.current;
      if (r.ms === 0) {
        // Línea base: la primera pasada sólo aprende cómo está el lienzo.
        rodanteRef.current = { ms: ahora, firma: l.firma };
        return;
      }
      if (ahora - r.ms < RODANTE_MS || l.firma === r.firma) return;
      const ficha = await guardarSesion({
        id: 'auto',
        nombre: `Automática · ${l.mapa || 'lienzo'}`,
        mapa: l.mapa,
        nodes: l.nodes,
        edges: l.edges,
        appearance: l.appearance,
        templateId: l.templateId,
      });
      if (!ficha) return;
      rodanteRef.current = { ms: ahora, firma: l.firma };
      await refrescarSesiones();
    }, 60 * 1000);
    return () => clearInterval(id);
  }, [refrescarSesiones]);

  // Atajos de guardado: Ctrl+S guarda el progreso ahora; Escape cierra el panel de sesiones.
  useEffect(() => {
    const alTeclado = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        void handleManualSave();
        return;
      }
      if (e.key === 'Escape' && isStatesModalOpen) {
        setIsStatesModalOpen(false);
      }
    };
    window.addEventListener('keydown', alTeclado);
    return () => window.removeEventListener('keydown', alTeclado);
  }, [handleManualSave, isStatesModalOpen]);

  const handleResetCanvas = useCallback(() => {
    setIsClearModalOpen(true);
  }, []);

  const handleUserChange = useCallback(
    (newUser: UserProfile | null) => {
      setCurrentUser(newUser);
      saveCurrentUser(newUser);
      showToast(
        newUser ? `Sesión iniciada como ${newUser.name}` : 'Has cerrado sesión (Modo Invitado)',
        'info'
      );
    },
    [showToast]
  );

  // Zoom controls for HUD
  const handleZoomIn = useCallback(() => {
    if (rfInstanceRef.current) {
      rfInstanceRef.current.zoomIn();
      const z = rfInstanceRef.current.getZoom();
      setZoomPercent(Math.round(z * 100));
    }
  }, []);

  const handleZoomOut = useCallback(() => {
    if (rfInstanceRef.current) {
      rfInstanceRef.current.zoomOut();
      const z = rfInstanceRef.current.getZoom();
      setZoomPercent(Math.round(z * 100));
    }
  }, []);

  return (
    <div
      className={`nf-app ${tema.claro ? 'nf-claro' : ''} w-full h-screen flex flex-col font-sans select-none overflow-hidden`}
      style={{ ...varsTema, backgroundColor: tema.chrome.bg }}
    >
      {/* Top Header & Toolbar */}
      <Toolbar
        canUndo={canUndo}
        canRedo={canRedo}
        undoCount={undoCount}
        redoCount={redoCount}
        selectedNodesCount={selectedNodes.length}
        edgeAppearance={edgeAppearance}
        currentUser={currentUser}
        onUndo={handleUndo}
        onRedo={handleRedo}
        onAddNode={handleAddNode}
        onHybridize={handleHybridize}
        onOpenStatesModal={() => {
          setStatesModalTab('saved');
          setIsStatesModalOpen(true);
        }}
        onOpenJsonModal={() => {
          setStatesModalTab('export');
          setIsStatesModalOpen(true);
        }}
        onOpenObsidianModal={() => {
          setStatesModalTab('obsidian');
          setIsStatesModalOpen(true);
        }}
        onOpenAuthModal={() => setIsAuthModalOpen(true)}
        onEdgeAppearanceChange={setEdgeAppearance}
        onApplyEdgeToSelected={handleApplyAppearanceToSelectedEdges}
        selectedEdgeCount={selectedEdges.length}
        onSelectTemplate={handleSelectTemplate}
        onResetCanvas={handleResetCanvas}
        onOpenTemplatesModal={() => setIsTemplatesModalOpen(true)}
        onOpenClearModal={() => setIsClearModalOpen(true)}
        templates={templates}
        onRefreshTemplates={handleRefreshTemplates}
        isRefreshingTemplates={isRefreshingTemplates}
        saveStatus={saveStatus}
        isAiProcessing={isAiLoading}
        isSidebarOpen={isSidebarOpen}
        onToggleSidebar={() => setIsSidebarOpen((prev) => !prev)}
        onAutoLayout={handleAutoLayout}
        onOpenSynthesis={handleOpenSynthesis}
        onOpenHitlModal={() => setIsHitlModalOpen(true)}
        hitlDecisionsCount={hitlProfile.totalDecisions}
        onOpenShortcuts={() => setIsShortcutsOpen(true)}
        onOpenApiKeyModal={() => setIsApiKeyModalOpen(true)}
        hasCustomApiKey={hasCustomApiKey}
        onOpenBrainDump={() => setIsBrainDumpOpen(true)}
        onOpenBridgesModal={handleOpenBridgesModal}
        bridgesCount={bridges.length}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
      />

      {/* Main Workspace: Collapsible Sidebar + ReactFlow Canvas */}
      <div className="flex-1 flex overflow-hidden relative">
        {/* Left Sidebar from Sophisticated Dark Design */}
        {isSidebarOpen && (
          <aside className="w-64 border-r border-slate-800/60 bg-slate-950/85 p-4 flex flex-col gap-5 shrink-0 backdrop-blur-sm z-10 transition-all overflow-y-auto">
            {/* Search Results Quick List (when searching) */}
            {searchQuery.trim() && (
              <section className="bg-slate-900/90 border border-amber-500/30 rounded-xl p-3">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-1.5 text-xs font-semibold text-amber-400">
                    <Search size={13} />
                    <span>Resultados ({searchMatchingNodeIds.size})</span>
                  </div>
                  <button
                    type="button"
                    onClick={() => setSearchQuery('')}
                    className="text-[10px] text-slate-500 hover:text-slate-300"
                  >
                    Limpiar
                  </button>
                </div>
                {searchMatchingNodeIds.size === 0 ? (
                  <p className="text-[11px] text-slate-500 italic">{t('busqueda.sinResultados')}</p>
                ) : (
                  <div className="flex flex-col gap-1.5 max-h-40 overflow-y-auto pr-1">
                    {nodes
                      .filter((n) => searchMatchingNodeIds.has(n.id))
                      .map((n) => (
                        <button
                          key={n.id}
                          type="button"
                          onClick={() => handleJumpToNode(n.id)}
                          className="text-left px-2 py-1.5 rounded-lg bg-slate-800/60 hover:bg-slate-800 text-xs text-slate-200 border border-slate-700/50 hover:border-amber-500/50 transition-colors flex items-center justify-between group"
                        >
                          <span className="truncate max-w-[140px] font-medium">{n.data.title}</span>
                          <span className="text-[9px] text-slate-500 group-hover:text-amber-400 font-mono">
                            Ver →
                          </span>
                        </button>
                      ))}
                  </div>
                )}
              </section>
            )}

            {/* Section 1: Node Library */}
            <section>
              <h3 className="text-[10px] uppercase tracking-widest text-slate-500 font-bold mb-3">
                Node Library
              </h3>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => handleAddNode(true)}
                  className="h-16 border border-dashed border-slate-700 rounded-lg flex flex-col items-center justify-center gap-1 cursor-pointer hover:border-indigo-500 hover:bg-indigo-500/5 transition-colors group"
                  title={t('lienzo.root.ayuda')}
                >
                  <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="2" className="text-slate-500 group-hover:text-indigo-400">
                    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                  </svg>
                  <span className="text-[10px] text-slate-500 group-hover:text-indigo-300 font-medium">{t('lienzo.root')}</span>
                </button>
                <button
                  type="button"
                  onClick={() => handleAddNode(false)}
                  className="h-16 border border-dashed border-slate-700 rounded-lg flex flex-col items-center justify-center gap-1 cursor-pointer hover:border-emerald-500 hover:bg-emerald-500/5 transition-colors group"
                  title={t('lienzo.logic.ayuda')}
                >
                  <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="2" className="text-slate-500 group-hover:text-emerald-400">
                    <circle cx="12" cy="12" r="10" />
                  </svg>
                  <span className="text-[10px] text-slate-500 group-hover:text-emerald-300 font-medium">{t('lienzo.logic')}</span>
                </button>
              </div>
            </section>

            {/* Lienzo: lo que no vive en la barra superior (acción destructiva, lejos de las frecuentes) */}
            <section>
              <h3 className="text-[10px] uppercase tracking-widest text-slate-500 font-bold mb-2">
                Lienzo
              </h3>
              <div className="flex flex-col gap-1.5">
                <button
                  type="button"
                  id="btn-sidebar-clear-canvas"
                  onClick={() => setIsClearModalOpen(true)}
                  className="w-full flex items-center gap-2 px-3 py-1.5 bg-rose-950/10 hover:bg-rose-950/30 text-rose-300/90 hover:text-rose-200 border border-rose-900/30 hover:border-rose-800/50 rounded-xl text-xs font-medium transition-colors cursor-pointer"
                >
                  <Trash2 size={13} className="text-rose-400" />
                  <span>{t('lienzo.borrar')}</span>
                </button>
              </div>
            </section>

            {/* Section 3: Active State & Persistence */}
            <section className="space-y-1.5">
              <h3 className="text-[10px] uppercase tracking-widest text-slate-500 font-bold mb-2 flex items-center justify-between">
                <span>{t('lienzo.persistencia')}</span>
                <span className="flex items-center gap-1.5">
                  {propuestas.length > 0 && (
                    <span className="text-[9px] normal-case text-amber-300 font-mono bg-amber-500/10 px-1.5 py-0.5 rounded border border-amber-500/30">
                      {propuestas.length} por aprobar
                    </span>
                  )}
                  <span className="flex items-center gap-1 text-[9px] normal-case text-emerald-400 font-normal">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
                    Local Activo
                  </span>
                </span>
              </h3>

              {/* Fase 5a: el agente propone, vos aprobás. Va PRIMERO: era la acción enterrada más importante. */}
              <button
                type="button"
                id="btn-cambios-del-agente"
                onClick={() => setIsAgentPanelOpen(true)}
                title={t('hud.agente.ayuda')}
                className={`w-full flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-semibold border transition-colors cursor-pointer ${
                  propuestas.length
                    ? 'bg-amber-500/20 hover:bg-amber-500/30 text-amber-50 border-amber-500/50'
                    : 'bg-slate-900/60 hover:bg-slate-800/70 text-slate-400 border-slate-800'
                }`}
              >
                <Inbox size={14} className={propuestas.length ? 'text-amber-300' : 'text-slate-500'} />
                <span>{t('hud.agente')}</span>
                <span className={`ml-auto text-[9px] font-mono px-1.5 py-0.5 rounded border ${
                  propuestas.length
                    ? 'text-amber-300 bg-amber-900/40 border-amber-700/50'
                    : 'text-slate-400 bg-slate-800/60 border-slate-700/60'
                }`}>
                  {propuestas.length ? propuestas.length : 'al día'}
                </span>
              </button>

              {/* El guardado, a la vista: el estado se contesta de un vistazo. */}
              <div
                className={`flex items-center gap-2 px-3 py-2 rounded-xl border ${estadoGuardado.clase}`}
                title={`${t('hud.estado.ayuda')} · ${lastSyncText} · rev ${vaultInfo?.revision ?? '—'}`}
              >
                <span className={`w-2 h-2 rounded-full shrink-0 ${estadoGuardado.punto}`} />
                <span className="text-[11px] font-medium truncate">{estadoGuardado.texto}</span>
                <span className="ml-auto shrink-0 text-[9px] font-mono text-slate-400 bg-slate-900/60 px-1.5 py-0.5 rounded border border-slate-700/60">
                  {t('hud.estado.rev')} {vaultInfo?.revision ?? '—'}
                </span>
              </div>

              <div className="bg-slate-900/70 rounded-xl border border-slate-800 p-3">
                <div className="flex justify-between items-center mb-1.5">
                  <span className="text-[10px] text-slate-400">{t('hud.almacenamiento')}</span>
                  <span className="text-[10px] text-indigo-300 font-mono">{t('hud.persistente')}</span>
                </div>
                <div className="flex justify-between items-center mb-1.5">
                  <span className="text-[10px] text-slate-400">{t('hud.ultimoGuardado')}</span>
                  <span className="text-[10px] text-slate-300 font-mono truncate max-w-[130px]">{lastSyncText}</span>
                </div>
                <div className="flex justify-between items-center mb-1.5">
                  <span className="text-[10px] text-slate-400">{t('hud.nodosConexiones')}</span>
                  <span className="text-[10px] text-slate-300 font-mono tabular-nums">{nodes.length} / {edges.length}</span>
                </div>
                {/* Fase 3: el disco es la fuente de verdad */}
                <div className="flex justify-between items-center mb-1.5">
                  <span className="text-[10px] text-slate-400">{t('hud.vaultEnDisco')}</span>
                  <span
                    className="text-[10px] text-emerald-300 font-mono truncate max-w-[130px]"
                    title={vaultInfo?.vault || 'sin conexión con el backend'}
                  >
                    {vaultInfo ? `${vaultInfo.notas} notas` : '—'}
                  </span>
                </div>
                {vaultInfo?.ultimos_cambios_externos?.length ? (
                  <div className="flex justify-between items-center mb-1.5">
                    <span className="text-[10px] text-slate-400">Obsidian →</span>
                    <span className="text-[10px] text-sky-300 font-mono truncate max-w-[130px]">
                      {vaultInfo.ultimos_cambios_externos.join(', ')}
                    </span>
                  </div>
                ) : null}
                {/* Fase 7b: cronómetro de conversión (T0 → T1) */}
                <div className="flex justify-between items-center" title="Minutos entre el brain dump y el primer artefacto aprobado (objetivo < 3)">
                  <span className="text-[10px] text-slate-400">Conversión T0→T1</span>
                  <span
                    className={`text-[10px] font-mono tabular-nums ${
                      metricas?.promedio_min == null
                        ? 'text-slate-500'
                        : metricas.promedio_min <= 3
                          ? 'text-emerald-300'
                          : 'text-amber-300'
                    }`}
                  >
                    {metricas?.promedio_min == null
                      ? metricas?.sesion_activa
                        ? `${metricas.minutos_desde_t0 ?? 0} min en curso`
                        : '—'
                      : `${metricas.promedio_min} min prom.`}
                  </span>
                </div>
              </div>

              {/* Progreso: las dos mitades de guardar — escribir en disco y volver a otra sesión */}
              <div className="space-y-1.5 pt-1">
                <button
                  type="button"
                  id="btn-guardar-progreso"
                  onClick={handleManualSave}
                  title={t('hud.guardarAhora.ayuda')}
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-indigo-200 border border-slate-800 hover:border-indigo-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Save size={14} className="text-indigo-400 shrink-0" />
                  <span className="truncate">{t('hud.guardarAhora')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-indigo-300 font-mono bg-indigo-900/50 px-1.5 py-0.5 rounded border border-indigo-700/50">disco</span>
                </button>
                {/* Cargar OTRA sesión de nodos: era la acción que se perdió al reordenar la UI */}
                <button
                  type="button"
                  id="btn-sesiones-lienzo"
                  onClick={() => {
                    setStatesModalTab('saved');
                    setIsStatesModalOpen(true);
                  }}
                  title={t('hud.sesiones.ayuda')}
                  className={`w-full flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-medium border transition-colors cursor-pointer group ${
                    sesionActivaDrift
                      ? 'bg-amber-950/40 hover:bg-amber-900/50 text-amber-100 border-amber-700/60'
                      : 'bg-slate-900/70 hover:bg-slate-800/70 text-amber-200 border-slate-800 hover:border-amber-700/60'
                  }`}
                >
                  <History size={14} className="text-amber-400 shrink-0" />
                  <span className="truncate">{t('hud.sesiones')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-amber-300 font-mono bg-amber-900/50 px-1.5 py-0.5 rounded border border-amber-700/60">
                    {sesiones.length ? t('hud.sesiones.n', { n: sesiones.length }) : t('hud.sesiones.vacio')}
                  </span>
                </button>
                {sesionActiva && (
                  <p className="text-[10px] text-slate-500 pl-1 truncate" title={sesionActiva.nombre}>
                    {t('hud.sesiones.cargada')}: <span className="text-slate-300">{sesionActiva.nombre}</span>
                    {sesionActivaDrift && (
                      <span className="ml-1 text-amber-300">·{' '}{t('hud.sesiones.modificada')}</span>
                    )}
                  </p>
                )}
              </div>

              {/* Paneles: mismo lenguaje que las secciones de arriba — icono + etiqueta + chip */}
              <p className="text-[10px] uppercase tracking-widest text-slate-500 font-bold pt-1">{t('hud.paneles')}</p>
              <div className="space-y-1.5">
                {/* Fase 5b: buscar en toda la bóveda de Obsidian */}
                <button
                  type="button"
                  onClick={() => setIsMemoriaOpen(true)}
                  title={t('panel.memoria.ayuda')}
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-sky-200 border border-slate-800 hover:border-sky-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Database size={14} className="text-sky-400 shrink-0" />
                  <span className="truncate">{t('panel.memoria')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-sky-300 font-mono bg-sky-900/50 px-1.5 py-0.5 rounded border border-sky-700/50">BM25</span>
                </button>
                {/* Fase 8: capturar conocimiento y exportar el mapa */}
                <button
                  type="button"
                  onClick={() => setIsConocimientoOpen(true)}
                  title={t('panel.conocimiento.ayuda')}
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-emerald-200 border border-slate-800 hover:border-emerald-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <BookOpen size={14} className="text-emerald-400 shrink-0" />
                  <span className="truncate">{t('panel.conocimiento')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-emerald-300 font-mono bg-emerald-900/50 px-1.5 py-0.5 rounded border border-emerald-700/50">captura</span>
                </button>
                {/* Fase B: voz → plan de operaciones sobre el lienzo (Speechmatics) */}
                <button
                  type="button"
                  id="btn-panel-voz"
                  onClick={() => setIsVozOpen(true)}
                  title="Hablá y operá el lienzo: dictá ideas nuevas o comandá cambios (el motor de voz activo)"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-cyan-200 border border-slate-800 hover:border-cyan-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Mic size={14} className="text-cyan-400 shrink-0" />
                  <span className="truncate">{t('panel.voz')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-cyan-300 font-mono bg-cyan-900/50 px-1.5 py-0.5 rounded border border-cyan-700/50">hablar</span>
                </button>
                {/* Etapas 1+3 del plan «sin Hermes»: el bucle propio, con herramientas de repo */}
                <button
                  type="button"
                  id="btn-panel-agente"
                  onClick={() => setIsAgenteOpen(true)}
                  title="El agente propio: mira el repo con herramientas (firmas, buscar, correr los tests) sin depender de otro proceso"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-violet-200 border border-slate-800 hover:border-violet-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Bot size={14} className="text-violet-400 shrink-0" />
                  <span className="truncate">{t('panel.agente')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-violet-300 font-mono bg-violet-900/50 px-1.5 py-0.5 rounded border border-violet-800/60">tools</span>
                </button>
                <button
                  type="button"
                  id="btn-panel-investigacion"
                  onClick={() => setIsInvestigacionOpen(true)}
                  title="Investigación por fases: sale a la web, el nodo crece en el lienzo y se sintetiza"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-violet-200 border border-slate-800 hover:border-violet-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Telescope size={14} className="text-violet-400 shrink-0" />
                  <span className="truncate">{t('panel.investigacion')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-violet-300 font-mono bg-violet-900/50 px-1.5 py-0.5 rounded border border-violet-800/60">{estadoInvestigacion?.corriendo ? 'en curso' : `${estadoInvestigacion?.pasos?.length || 0} pasos`}</span>
                </button>
                <button
                  type="button"
                  id="btn-panel-cerebro"
                  onClick={() => setIsCerebroOpen(true)}
                  title="Pensar desde el lienzo: un turno del cerebro residente (sesión de Hermes con memoria, ve el lienzo y la bóveda)"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-cyan-200 border border-slate-800 hover:border-cyan-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Brain size={14} className="text-cyan-400 shrink-0" />
                  <span className="truncate">{t('panel.pensar')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-cyan-300 font-mono bg-cyan-900/50 px-1.5 py-0.5 rounded border border-cyan-700/50">cerebro</span>
                </button>
                <button
                  type="button"
                  id="btn-panel-siguiente"
                  onClick={() => setIsSiguienteOpen(true)}
                  title="Lo que sigue: el camino crítico del mapa (qué frena, qué falta y qué conviene hacer primero)"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-amber-200 border border-slate-800 hover:border-amber-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Compass size={14} className="text-amber-400 shrink-0" />
                  <span className="truncate">{t('panel.siguiente')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-amber-300 font-mono bg-amber-900/50 px-1.5 py-0.5 rounded border border-amber-800/60">camino crítico</span>
                </button>
                {/* Planilla de evaluación: medir los motores con las tareas reales */}
                <button
                  type="button"
                  id="btn-panel-evaluacion"
                  onClick={() => setIsEvaluacionOpen(true)}
                  title="Corré las tareas reales del lienzo contra cada motor: aciertos, tiempo y costo medidos"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-violet-200 border border-slate-800 hover:border-violet-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <FlaskConical size={14} className="text-violet-400 shrink-0" />
                  <span className="truncate">{t('panel.evaluacion')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-violet-300 font-mono bg-violet-900/50 px-1.5 py-0.5 rounded border border-violet-700/50">medir</span>
                </button>

                {/* Fase 7a: el agente jardín */}
                <button
                  type="button"
                  onClick={() => setIsJardinOpen(true)}
                  title="Diagnóstico del grafo: invariantes, islas, huérfanos y hubs inmaduros"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-lime-200 border border-slate-800 hover:border-lime-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <Sprout size={14} className="text-lime-400 shrink-0" />
                  <span className="truncate">{t('panel.jardin')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-lime-300 font-mono bg-lime-900/50 px-1.5 py-0.5 rounded border border-lime-700/50">grafo</span>
                </button>
                {/* Slice 1: expertos y artefactos */}
                <button
                  type="button"
                  onClick={() => setIsOrquestadorOpen(true)}
                  title="Ejecutar un experto sobre un nodo y obtener un artefacto validado"
                  className="w-full flex items-center gap-2 px-3 py-2 bg-slate-900/70 hover:bg-slate-800/70 text-violet-200 border border-slate-800 hover:border-violet-700/60 rounded-xl text-xs font-medium transition-colors cursor-pointer group"
                >
                  <WandSparkles size={14} className="text-violet-400 shrink-0" />
                  <span className="truncate">{t('panel.orquestador')}</span>
                  <span className="ml-auto shrink-0 whitespace-nowrap text-[9px] text-violet-300 font-mono bg-violet-900/50 px-1.5 py-0.5 rounded border border-violet-700/50">experto</span>
                </button>
              </div>
            </section>

            {/* Section: HITL Continuous Learning Engine */}
            <section>
              <h3 className="text-[10px] uppercase tracking-widest text-slate-500 font-bold mb-2 flex items-center justify-between">
                <span>{t('hitl.titulo')}</span>
                <span className="flex items-center gap-1 text-[9px] normal-case text-violet-400 font-normal">
                  <span className="w-1.5 h-1.5 rounded-full bg-violet-400 animate-pulse" />
                  HITL Activo
                </span>
              </h3>
              <div className="bg-violet-950/20 rounded-xl border border-violet-800/40 p-3 space-y-2">
                <div className="flex justify-between items-center text-[11px]">
                  <span className="text-slate-400">{t('hitl.decisiones')}</span>
                  <span className="text-violet-200 font-bold font-mono">{hitlProfile.totalDecisions}</span>
                </div>
                <div className="flex justify-between items-center text-[11px]">
                  <span className="text-slate-400">{t('hitl.aceptacion')}</span>
                  <span className="text-emerald-400 font-bold font-mono">{hitlProfile.acceptanceRate}%</span>
                </div>
                <div className="flex justify-between items-center text-[11px]">
                  <span className="text-slate-400">{t('hitl.auto')}</span>
                  {hitlProfile.autoAprendizaje?.activo ? (
                    <span className="text-violet-200 font-mono">cada {hitlProfile.autoAprendizaje.cada}</span>
                  ) : (
                    <span className="text-slate-500 font-mono">apagado</span>
                  )}
                </div>
                <p className="text-[10px] text-slate-300 line-clamp-2 italic border-t border-slate-800/80 pt-1.5 leading-snug">
                  "{hitlProfile.learnedProfile}"
                </p>
                <button
                  type="button"
                  id="btn-sidebar-hitl-open"
                  onClick={() => setIsHitlModalOpen(true)}
                  className="w-full mt-1 flex items-center justify-center gap-1.5 px-2.5 py-2 bg-violet-900/40 hover:bg-violet-900/60 text-violet-200 border border-violet-800/50 rounded-xl text-xs font-medium transition-colors cursor-pointer"
                >
                  <Brain size={14} className="text-violet-400" />
                  <span>{t('hitl.configurar')}</span>
                </button>
              </div>
            </section>

            {/* Section 3: AI Copilot */}
            <section className="mt-auto">
              <div className="p-3 bg-slate-900 border border-indigo-500/20 rounded-xl relative overflow-hidden">
                <div className="relative z-10">
                  <div className="flex items-center gap-2 mb-2">
                    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" fill="none" strokeWidth="2" className="text-indigo-400">
                      <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
                    </svg>
                    <span className="text-[10px] font-bold text-slate-200 uppercase tracking-tighter">AI Copilot</span>
                  </div>
                  <p className="text-[10px] text-slate-400 leading-normal mb-2.5 italic">
                    "Selecciona ideas y descubre sinergias con la IA, o genera una síntesis global de toda la red."
                  </p>
                  <div className="flex flex-col gap-1.5">
                    <button
                      type="button"
                      onClick={handleHybridize}
                      disabled={isAiLoading}
                      className="w-full py-2 bg-white text-slate-950 font-bold text-[10px] rounded hover:bg-slate-200 transition-colors uppercase tracking-wider disabled:opacity-60 cursor-pointer"
                    >
                      {isAiLoading ? 'GENERATING...' : 'RUN HYBRIDIZER'}
                    </button>
                    <button
                      type="button"
                      onClick={handleOpenSynthesis}
                      className="w-full py-1.5 bg-slate-800 hover:bg-slate-700 text-indigo-300 hover:text-white font-medium text-[10px] rounded border border-indigo-500/30 transition-colors flex items-center justify-center gap-1.5 cursor-pointer"
                    >
                      <Compass size={12} className="text-emerald-400" />
                      <span>{t('cocreacion.sintesis')}</span>
                    </button>
                  </div>
                </div>
                <div className="absolute -right-4 -bottom-4 opacity-5 pointer-events-none text-indigo-400">
                  <svg viewBox="0 0 24 24" width="80" height="80" fill="currentColor">
                    <circle cx="12" cy="12" r="10" />
                  </svg>
                </div>
              </div>
            </section>
          </aside>
        )}

        {/* Main ReactFlow Interactive Stage */}
        <main
          ref={reactFlowWrapperRef}
          onDoubleClick={handleCanvasDoubleClick}
          className="flex-1 relative w-full h-full overflow-hidden"
          style={{ ...varsTema, backgroundColor: tema.canvasBg }}
        >
          <ReactFlow
            nodes={processedNodes}
            edges={processedEdges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onSelectionChange={onSelectionChange}
            onConnect={onConnect}
            onNodeMouseEnter={(_, node) => setHoveredNode(node.id)}
            onNodeMouseLeave={() => setHoveredNode(null)}
            onEdgeMouseEnter={(_, edge) => setHoveredEdge(edge.id)}
            onEdgeMouseLeave={() => setHoveredEdge(null)}
            nodeTypes={NODE_TYPES}
            edgeTypes={EDGE_TYPES}
            zoomOnDoubleClick={false}
            onNodeDoubleClick={(_, node) => {
              // Doble clic en un macro-nodo: se despliega su linaje (los nodos que lo originaron).
              if ((node.data as IdeaNodeData)?.macro) {
                setNodoLinaje(node as CustomNode);
                setIsLinajeOpen(true);
              }
            }}
            onInit={(instance) => {
              rfInstanceRef.current = instance;
            }}
            onMove={(_, viewport) => {
              setZoomPercent(Math.round(viewport.zoom * 100));
            }}
            fitView
            attributionPosition="bottom-left"
            theme="light"
            minZoom={0.2}
            maxZoom={2}
            defaultEdgeOptions={{
              type: 'flowEdge',
              animated: false,
              style: {
                stroke: edgeAppearance.color,
                strokeWidth: edgeAppearance.strokeWidth,
              },
            }}
          >
            {/* Background dot grid pattern (color y densidad desde el tema del lienzo) */}
            <Background color={tema.grid} gap={tema.gridSize} size={1.2} />

            {/* Minimap radar in top-right */}
            <MiniMap
              className="!rounded-xl overflow-hidden shadow-lg"
              style={{
                backgroundColor: tema.minimapBg,
                border: `1px solid ${tema.hudBorder}`,
              }}
              nodeColor={(n) =>
                n.type === 'zonaNode'
                  ? 'transparent'
                  : (n.data as IdeaNodeData)?.colorAccent || '#4f46e5'
              }
              maskColor={tema.minimapMask}
            />
          </ReactFlow>

          {/* Zoom HUD from Design HTML in bottom-left */}
          <div className="absolute bottom-6 left-6 flex items-center gap-2 z-30 pointer-events-auto">
            <div className="flex bg-[var(--nf-hud-bg)] border border-[var(--nf-hud-border)] rounded-lg shadow-lg p-1 backdrop-blur-md">
              <button
                type="button"
                onClick={handleZoomOut}
                className="p-1.5 md:p-2 hover:bg-[var(--nf-hud-hover)] rounded text-[var(--nf-hud-text)] border-r border-[var(--nf-hud-border)] transition-colors text-xs font-bold leading-none cursor-pointer"
                title={t('zoom.alejar')}
              >
                -
              </button>
              <div className="px-2.5 md:px-3 flex items-center text-[10px] font-bold text-[var(--nf-hud-text)] font-mono">
                {zoomPercent}%
              </div>
              <button
                type="button"
                onClick={handleZoomIn}
                className="p-1.5 md:p-2 hover:bg-[var(--nf-hud-hover)] rounded text-[var(--nf-hud-text)] border-l border-[var(--nf-hud-border)] transition-colors text-xs font-bold leading-none cursor-pointer"
                title={t('zoom.acercar')}
              >
                +
              </button>
              <button
                type="button"
                onClick={handleFitView}
                className="p-1.5 md:p-2 hover:bg-[var(--nf-hud-hover)] rounded text-[var(--nf-hud-text)] border-l border-[var(--nf-hud-border)] transition-colors cursor-pointer"
                title={t('zoom.ajustar')}
              >
                <Maximize2 size={13} />
              </button>
            </div>

            {/* Zonas: marcos por nivel o lente por categoría (no se guardan) */}
            <ZonasHud
              visible={zonasVisibles}
              onVisible={setZonasVisibles}
              modo={modoZonas}
              onModo={(m) => {
                setModoZonas(m);
                // Los dos modos son excluyentes: al volver a niveles hay que apagar la
                // lente, si no quedan las tarjetas atenuadas con los marcos encendidos.
                if (m === 'nivel') setCategoriaLente(null);
              }}
              categorias={categorias}
              categoriaLente={categoriaLente}
              onCategoria={setCategoriaLente}
            />

            <AparienciaHud />

            {/* Shortcuts help button */}
            <button
              type="button"
              onClick={() => setIsShortcutsOpen(true)}
              className="bg-[var(--nf-hud-bg)] hover:bg-[var(--nf-hud-hover)] border border-[var(--nf-hud-border)] rounded-lg p-2 text-[var(--nf-hud-text)] backdrop-blur-md flex items-center gap-1.5 text-xs shadow-lg transition-colors cursor-pointer"
              title={t('atajos.ayuda')}
            >
              <Keyboard size={14} />
              <span className="text-[10px] hidden sm:inline font-mono">{t('atajos.titulo')}</span>
            </button>

            {/* Autoría: chica, pero en la esquina donde se lee */}
            <div
              id="credito-autor"
              className="bg-[var(--nf-hud-bg)] border border-[var(--nf-hud-border)] rounded-lg px-2.5 py-2 backdrop-blur-md shadow-lg select-none flex items-center gap-1.5"
              title="NodeFlow — creado por TOMAS.WAV (Tomas Pieruz)"
            >
              <span className="text-[10px] font-mono text-[var(--nf-hud-text)] opacity-70">por</span>
              <span className="text-[10px] font-semibold tracking-tight text-[var(--nf-hud-text)]">TOMAS.WAV</span>
            </div>
          </div>

          {/* Fase A — Norte Estratégico: la lente con la que se condensan las ideas */}
          <div className="absolute top-3 left-1/2 -translate-x-1/2 z-30 pointer-events-auto">
            {isNorteOpen ? (
              <div className="flex items-center gap-2 px-3 py-2 bg-slate-900/95 border border-violet-500/50 rounded-2xl shadow-2xl backdrop-blur-md">
                <Target size={14} className="text-violet-400 shrink-0" />
                <input
                  id="input-norte-estrategico"
                  autoFocus
                  value={norte}
                  onChange={(e) => guardarNorte(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === 'Escape') setIsNorteOpen(false);
                  }}
                  placeholder="Norte estratégico: ¿qué querés lograr con este lienzo?"
                  className="w-[24rem] bg-transparent text-xs text-slate-100 placeholder:text-slate-500 outline-none"
                />
                <button
                  type="button"
                  onClick={() => setIsNorteOpen(false)}
                  className="text-slate-500 hover:text-slate-200 transition-colors cursor-pointer"
                  title={t('modal.cerrar')}
                >
                  <X size={13} />
                </button>
              </div>
            ) : (
              <button
                type="button"
                id="btn-norte-estrategico"
                onClick={() => setIsNorteOpen(true)}
                title="Definí el Norte Estratégico: la lente que guía la condensación del lienzo"
                className={`flex items-center gap-1.5 px-3 py-1.5 rounded-2xl border backdrop-blur-md shadow-lg text-[11px] transition-colors cursor-pointer ${
                  norte
                    ? 'bg-violet-950/50 border-violet-500/40 text-violet-200 hover:text-white'
                    : 'bg-slate-900/70 border-slate-800 text-slate-400 hover:text-slate-200'
                }`}
              >
                <Target size={13} className={norte ? 'text-violet-400' : ''} />
                {norte ? <span className="max-w-[22rem] truncate">{norte}</span> : <span>{t('norte.titulo')}</span>}
              </button>
            )}
          </div>

          {/* Floating Multi-Selection Quick Bar (Unir, Hibridar, Puentes) */}
          {selectedNodes.length >= 2 && (
            <div
              id="floating-multi-selection-bar"
              className="absolute bottom-6 left-1/2 -translate-x-1/2 z-30 flex items-center gap-2 px-3.5 py-2 bg-slate-900/95 border border-indigo-500/50 rounded-2xl shadow-2xl backdrop-blur-md animate-in fade-in slide-in-from-bottom-2 duration-200 pointer-events-auto"
            >
              <div className="flex items-center gap-1.5 px-2.5 py-1 bg-indigo-950/80 border border-indigo-700/50 rounded-xl text-indigo-300 text-xs font-semibold">
                <span className="w-2 h-2 rounded-full bg-indigo-400 animate-pulse" />
                <span>{selectedNodes.length} Nodos</span>
              </div>

              {selectedNodes.length === 2 && (
                <button
                  type="button"
                  id="btn-quick-connect-nodes"
                  onClick={handleConnectSelectedNodes}
                  title={t('accion.unir.ayuda')}
                  className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-100 hover:text-white rounded-xl text-xs font-medium border border-slate-700/80 transition-colors cursor-pointer"
                >
                  <Network size={14} className="text-emerald-400" />
                  <span>{t('accion.unir')}</span>
                  <kbd className="text-[10px] px-1 py-0.5 bg-slate-900 text-slate-400 rounded font-mono">U</kbd>
                </button>
              )}

              <button
                type="button"
                id="btn-quick-hybrid-nodes"
                onClick={handleHybridize}
                disabled={isAiLoading}
                title="Sintetizar un nuevo nodo conceptual que cruce las ideas (Atajo: H)"
                className="flex items-center gap-1.5 px-3 py-1.5 bg-gradient-to-r from-purple-600 to-indigo-600 hover:from-purple-500 hover:to-indigo-500 text-white rounded-xl text-xs font-semibold shadow-md transition-all cursor-pointer disabled:opacity-50"
              >
                <Sparkles size={14} />
                <span>{t('accion.hibridar')}</span>
                <kbd className="text-[10px] px-1 py-0.5 bg-purple-900/60 text-purple-200 rounded font-mono">H</kbd>
              </button>

              <button
                type="button"
                id="btn-quick-condensar"
                onClick={handleCondensar}
                title="Condensar la selección en un macro-nodo, guardando el linaje (Ctrl+Shift+C)"
                className="flex items-center gap-1.5 px-3 py-1.5 bg-violet-600/90 hover:bg-violet-500 text-white rounded-xl text-xs font-semibold border border-violet-400/60 transition-colors cursor-pointer shadow-lg shadow-violet-900/30"
              >
                <Layers size={14} />
                <span className="hidden sm:inline">Condensar {selectedNodes.length}</span>
                <kbd className="text-[10px] px-1 py-0.5 bg-violet-900/70 text-violet-100 rounded font-mono">C</kbd>
              </button>

              <button
                type="button"
                id="btn-quick-bridges-scan"
                onClick={handleOpenBridgesModal}
                title={t('accion.puentes.ayuda')}
                className="flex items-center gap-1.5 px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 hover:text-white rounded-xl text-xs font-medium border border-slate-700/80 transition-colors cursor-pointer"
              >
                <Compass size={14} className="text-cyan-400" />
                <span className="hidden sm:inline">{t('accion.puentes')}</span>
              </button>

              <button
                type="button"
                id="btn-quick-deselect"
                onClick={() => setSelectedNodes([])}
                className="p-1.5 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 transition-colors cursor-pointer"
                title={t('accion.deseleccionar')}
              >
                <X size={15} />
              </button>
            </div>
          )}

          {/* Floating AI Helper Badge - Disappears when not hovered to keep canvas completely clear */}
          <div
            id="ai-helper-hover-zone"
            className="absolute bottom-6 right-6 z-20 pointer-events-auto group flex flex-col items-end"
          >
            {/* Expanded Full Card - Hidden by default; appears smoothly only on cursor hover */}
            <div
              id="ai-helper-card"
              className="mb-2 w-80 bg-slate-900/95 border border-indigo-500/40 p-3.5 rounded-2xl backdrop-blur-md shadow-2xl transition-all duration-300 ease-out opacity-0 pointer-events-none scale-95 translate-y-2 group-hover:opacity-100 group-hover:pointer-events-auto group-hover:scale-100 group-hover:translate-y-0"
            >
              <div className="flex gap-2.5 items-start text-xs leading-relaxed text-slate-300">
                <div className="w-8 h-8 rounded-xl bg-indigo-950/80 border border-indigo-700/50 flex items-center justify-center shrink-0 text-indigo-400">
                  <BrainCircuit size={18} />
                </div>
                <div>
                  <div className="font-semibold text-white text-[13px] flex items-center justify-between mb-0.5">
                    <span className="flex items-center gap-1.5">
                      <span>{t('cocreacion.titulo')}</span>
                      <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                    </span>
                  </div>
                  <p className="text-[11px] text-slate-400">
                    Selecciona 2 o más nodos y pulsa <strong className="text-indigo-300 font-medium">{t('cocreacion.hibridador')}</strong> para descubrir sinergias conceptuales, o pulsa <strong className="text-emerald-300 font-medium">{t('accion.ramificar')}</strong> en cualquier nodo.
                  </p>
                </div>
              </div>
            </div>

            {/* Subtle indicator pill when idle: minimal footprint, translucent, zero obstruction */}
            <div
              id="ai-helper-pill"
              className="flex items-center gap-2 px-3 py-1.5 rounded-full bg-slate-900/70 hover:bg-slate-900 border border-slate-800 hover:border-indigo-500/40 text-slate-400 hover:text-indigo-300 text-xs backdrop-blur-md shadow-lg transition-all duration-200 cursor-pointer opacity-30 hover:opacity-100 group-hover:opacity-100 group-hover:border-indigo-500/60"
            >
              <BrainCircuit size={14} className="text-indigo-400" />
              <span className="text-[11px] font-medium tracking-tight">{t('cocreacion.titulo')}</span>
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
            </div>
          </div>

          {/* Toast Notification HUD */}
          {toastMessage && (
            <div className="absolute top-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-2 px-4 py-2.5 rounded-xl shadow-2xl text-xs font-medium backdrop-blur-md border animate-in fade-in slide-in-from-top-4 duration-200">
              {toastMessage.type === 'success' && (
                <div className="bg-emerald-950/90 border-emerald-500/60 text-emerald-200 flex items-center gap-2">
                  <Check size={15} className="text-emerald-400" />
                  <span>{toastMessage.text}</span>
                </div>
              )}
              {toastMessage.type === 'error' && (
                <div className="bg-rose-950/90 border-rose-500/60 text-rose-200 flex items-center gap-2">
                  <AlertCircle size={15} className="text-rose-400" />
                  <span>{toastMessage.text}</span>
                </div>
              )}
              {toastMessage.type === 'info' && (
                <div className="bg-slate-900/90 border-indigo-500/50 text-slate-100 flex items-center gap-2">
                  <Info size={15} className="text-indigo-400" />
                  <span>{toastMessage.text}</span>
                </div>
              )}
            </div>
          )}
        </main>
      </div>

      {/* Node Edit Modal */}
      <NodeEditModal
        nodeData={editingNode}
        isOpen={!!editingNode}
        onClose={() => setEditingNode(null)}
        onSave={handleSaveNodeEdit}
        onDelete={(id) => {
          takeSnapshot(nodes, edges);
          setNodes((nds) => nds.filter((n) => n.id !== id));
          setEdges((eds) => eds.filter((e) => e.source !== id && e.target !== id));
          showToast('Nodo eliminado', 'info');
        }}
      />

      {/* Saved States & Export/Import JSON Modal */}
      <SavedStatesModal
        isOpen={isStatesModalOpen}
        onClose={() => setIsStatesModalOpen(false)}
        sesiones={sesiones}
        currentNodes={nodes}
        currentEdges={edges}
        currentAppearance={edgeAppearance}
        initialTab={statesModalTab}
        activeStateId={sesionActiva?.id ?? null}
        activeStateDrift={sesionActivaDrift}
        onLoadState={handleLoadState}
        onSaveNewState={handleSaveNewState}
        onUpdateState={handleUpdateState}
        onDeleteState={handleDeleteState}
        onImportJSON={handleImportJSON}
      />

      {/* Basic Auth Modal */}
      <AuthModal
        currentUser={currentUser}
        isOpen={isAuthModalOpen}
        onClose={() => setIsAuthModalOpen(false)}
        onUserChange={handleUserChange}
      />

      {/* AI Synthesis Modal */}
      <EvaluacionPanel isOpen={isEvaluacionOpen} onClose={() => setIsEvaluacionOpen(false)} />

      <SiguientePanel isOpen={isSiguienteOpen} onClose={() => setIsSiguienteOpen(false)} />

      <InvestigacionPanel
        isOpen={isInvestigacionOpen}
        onClose={() => setIsInvestigacionOpen(false)}
        estado={estadoInvestigacion}
        onInvestigar={investigarTema}
        temas={nodes
          .map((n) => (n.data.title || '').trim())
          .filter((x) => x.length >= 4)
          .slice(0, 8)}
      />

      <CerebroPanel
        isOpen={isCerebroOpen}
        onClose={() => setIsCerebroOpen(false)}
        showToast={showToast}
        sugerencias={nodes
          .map((n) => (n.data.title || '').trim())
          .filter((x) => x.length >= 4)
          .slice(0, 6)}
      />

      <AgentePanel isOpen={isAgenteOpen} onClose={() => setIsAgenteOpen(false)} />

      <VozPanel
        isOpen={isVozOpen}
        onClose={() => setIsVozOpen(false)}
        onAplicar={aplicarPlanVoz}
        onAplicarComandos={aplicarComandosDeFase}
        onPrevisualizar={previsualizarPlanVoz}
        tituloNodo={(id) => nodes.find((n) => n.id === id)?.data.title || id}
      />

      <LinajeModal
        isOpen={isLinajeOpen}
        onClose={() => {
          setIsLinajeOpen(false);
          setNodoLinaje(null);
        }}
        node={nodoLinaje}
        onRestaurar={handleRestaurarLinaje}
      />

      <SynthesisModal
        isOpen={isSynthesisModalOpen}
        onClose={() => setIsSynthesisModalOpen(false)}
        synthesis={mapSynthesis}
        isLoading={isSynthesisLoading}
        nodeCount={nodes.length}
        onRefresh={handleRefreshSynthesis}
      />

      {/* Templates & Idea Cores Gallery Modal */}
      <TemplatesModal
        isOpen={isTemplatesModalOpen}
        onClose={() => setIsTemplatesModalOpen(false)}
        onSelectTemplate={handleSelectTemplate}
        currentTemplateId={currentTemplateId}
        currentNodeCount={nodes.length}
        templates={templates}
        onRefreshTemplates={handleRefreshTemplates}
        isRefreshingTemplates={isRefreshingTemplates}
      />

      {/* Clear Canvas / Delete All Nodes Confirmation Modal */}
      <ClearCanvasModal
        isOpen={isClearModalOpen}
        onClose={() => setIsClearModalOpen(false)}
        onClearAll={handleClearAllNodes}
        onResetToRoot={handleResetToRoot}
        nodeCount={nodes.length}
      />

      {/* HITL Continuous Learning Engine Modal */}
      <MemoriaPanel
        isOpen={isMemoriaOpen}
        onClose={() => setIsMemoriaOpen(false)}
        onPropuestaCreada={() => {
          void sincronizar();
        }}
        showToast={showToast}
      />

      <OrquestadorPanel
        isOpen={isOrquestadorOpen}
        onClose={() => setIsOrquestadorOpen(false)}
        nodos={(nodes || []).map((n) => ({
          id: n.id,
          title: String((n.data as { title?: string })?.title || ''),
        }))}
        nodoSeleccionado={
          selectedNodes && selectedNodes.length === 1
            ? String((selectedNodes[0].data as { title?: string })?.title || '')
            : ''
        }
        onPropuestasCreadas={() => {
          void sincronizar();
        }}
        showToast={showToast}
      />

      <JardinPanel
        isOpen={isJardinOpen}
        onClose={() => setIsJardinOpen(false)}
        onPropuestasCreadas={() => {
          void sincronizar();
        }}
        showToast={showToast}
      />

      <ConocimientoPanel
        isOpen={isConocimientoOpen}
        onClose={() => setIsConocimientoOpen(false)}
        onPropuestasCreadas={() => {
          void sincronizar();
        }}
        showToast={showToast}
      />

      <AgentChangesPanel
        isOpen={isAgentPanelOpen}
        onClose={() => setIsAgentPanelOpen(false)}
        propuestas={propuestas}
        onResolved={() => {
          void sincronizar();
        }}
        showToast={showToast}
      />

      <HitlLearningModal
        isOpen={isHitlModalOpen}
        onClose={() => setIsHitlModalOpen(false)}
        profile={hitlProfile}
        onProfileUpdated={(updated) => setHitlProfile(updated)}
        showToast={showToast}
      />

      {/* Keyboard Shortcuts Modal */}
      <KeyboardShortcutsModal
        isOpen={isShortcutsOpen}
        onClose={() => setIsShortcutsOpen(false)}
      />

      {/* Bring Your Own Key (BYOK) Modal */}
      <ApiKeyModal
        isOpen={isApiKeyModalOpen}
        onClose={() => setIsApiKeyModalOpen(false)}
        onKeyChange={(hasKey) => setHasCustomApiKey(hasKey)}
      />

      {/* Brain Dump Rapid Input Modal */}
      <BrainDumpModal
        isOpen={isBrainDumpOpen}
        onClose={() => setIsBrainDumpOpen(false)}
        onSubmit={handleBrainDumpSubmit}
        isLoading={isBrainDumpLoading}
      />

      {/* Semantic Bridges (Hidden Connections) Modal */}
      <SemanticBridgesModal
        isOpen={isBridgesModalOpen}
        onClose={() => setIsBridgesModalOpen(false)}
        bridges={bridges}
        isLoading={isBridgesLoading}
        onScanBridges={handleScanBridges}
        onConnectBridge={handleConnectBridge}
        onConnectAll={handleConnectAllBridges}
        connectedBridgeIds={connectedBridgeIds}
      />
    </div>
  );
}
