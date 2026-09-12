import { Node, Edge } from 'reactflow';

export type IdeaMaturityLevel = 1 | 2 | 3 | 4;

export interface MaturityConfig {
  level: IdeaMaturityLevel;
  label: string;
  icon: string;
  desc: string;
  textColor: string;
  barBg: string;
  borderColor: string;
  badgeBg: string;
}

export const MATURITY_CONFIGS: Record<IdeaMaturityLevel, MaturityConfig> = {
  1: {
    level: 1,
    label: 'Semilla',
    icon: '🌱',
    desc: 'Chispa inicial / hipótesis sin validar',
    textColor: 'text-slate-300',
    barBg: 'bg-slate-400',
    borderColor: 'border-slate-700',
    badgeBg: 'bg-slate-800/80',
  },
  2: {
    level: 2,
    label: 'En Exploración',
    icon: '⚡',
    desc: 'Desglosada en componentes y preguntas',
    textColor: 'text-amber-300',
    barBg: 'bg-amber-400',
    borderColor: 'border-amber-700/60',
    badgeBg: 'bg-amber-950/60',
  },
  3: {
    level: 3,
    label: 'Validada',
    icon: '🛡️',
    desc: 'Auditada críticamente, resistente y sólida',
    textColor: 'text-cyan-300',
    barBg: 'bg-cyan-400',
    borderColor: 'border-cyan-700/60',
    badgeBg: 'bg-cyan-950/60',
  },
  4: {
    level: 4,
    label: 'Ejecutable',
    icon: '🚀',
    desc: 'Madura y lista para implementación directa',
    textColor: 'text-emerald-300',
    barBg: 'bg-emerald-400',
    borderColor: 'border-emerald-700/60',
    badgeBg: 'bg-emerald-950/60',
  },
};

export interface IdeaNodeData {
  id: string;
  title: string;
  description: string;
  category?: string;
  label?: string;
  tags?: string[];
  colorAccent?: string;
  isRoot?: boolean;
  isSearchMatch?: boolean;
  isEditing?: boolean;
  maturity?: IdeaMaturityLevel;
  /** Grado del nodo (aristas que lo tocan). Se inyecta en el render: alimenta la
   *  jerarquía de escala. No forma parte del estado persistido. */
  degree?: number;
  aiOrigin?: {
    batchId: string;
    actionType: 'branch' | 'explore' | 'hybrid' | 'critique' | 'socratic' | 'braindump' | 'bridge' | 'manual';
    promptOriginal: string;
    originalTitle: string;
    allBatchTitles: string[];
    createdAt: string;
  };
  onAction?: (
    action:
      | 'branch'
      | 'explore'
      | 'critique'
      | 'socratic'
      | 'edit'
      | 'delete'
      | 'duplicate'
      | 'inline-save'
      | 'inline-save-tab'
      | 'inline-save-enter'
      | 'inline-cancel'
      | 'inline-start'
      | 'set-maturity',
    nodeIdOrData: string | IdeaNodeData,
    extraData?: IdeaNodeData
  ) => void;
}

export interface SemanticBridge {
  id: string;
  sourceId: string;
  targetId: string;
  sourceTitle: string;
  targetTitle: string;
  label: string;
  rationale: string;
}

export interface HumanDecision {
  accepted: string[];
  rejected: string[];
  added_manually: string[];
}

export interface FeedbackEvent {
  id: string;
  timestamp: string;
  action: 'NODE_EDIT' | 'NODE_DELETE' | 'EDGE_CREATED' | 'AI_ACCEPTED' | 'HYBRIDIZE_FEEDBACK';
  prompt_original: string;
  ai_suggestion: string[];
  human_decision: HumanDecision;
  contextSnippet?: string;
  inferredPreference?: string;
}

export interface UserHitlProfile {
  version: string;
  updatedAt: string;
  totalDecisions: number;
  acceptanceRate: number;
  learnedProfile: string;
  categoriesAccepted: string[];
  topicsRejected: string[];
  recentFeedback: FeedbackEvent[];
}

export type CustomNode = Node<IdeaNodeData>;

export type ConnectionCurve = 'default' | 'straight' | 'smoothstep' | 'step';

export interface EdgeAppearance {
  color: string;
  strokeWidth: number;
  animated: boolean;
  type: ConnectionCurve;
}

export interface UserProfile {
  id: string;
  name: string;
  email: string;
  role: string;
  avatar: string;
}

export interface SavedState {
  id: string;
  name: string;
  timestamp: number;
  userId: string;
  nodeCount: number;
  edgeCount: number;
  nodes: CustomNode[];
  edges: Edge[];
  edgeAppearance: EdgeAppearance;
}

export interface HistorySnapshot {
  nodes: CustomNode[];
  edges: Edge[];
}
