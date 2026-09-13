import React, { memo } from 'react';
import { Handle, Position, NodeProps, useStore } from 'reactflow';
import { GitBranch, Eye, Edit3, Trash2, Copy, Flame, HelpCircle } from 'lucide-react';
import { IdeaNodeData, IdeaMaturityLevel, MATURITY_CONFIGS } from '../types';
import { useTarjetas, useTema } from '../state/canvasPrefs';

/**
 * Level of Detail por zoom (renderizado progresivo).
 *
 * El zoom se lee del store interno de React Flow con un selector que devuelve
 * sólo tres valores posibles: el nodo se re-renderiza cuando CRUZA un umbral,
 * no en cada frame de zoom. Con `useViewport()` cada uno de los 42 nodos
 * re-renderizaría en cada rueda del mouse.
 */
type Lod = 'compacto' | 'medio' | 'completo';

const COMPACT_ZOOM = 0.5;
const FULL_ZOOM = 0.8;

function useLod(): Lod {
  return useStore((s) => {
    const z = s.transform[2];
    if (z < COMPACT_ZOOM) return 'compacto';
    if (z > FULL_ZOOM) return 'completo';
    return 'medio';
  }) as Lod;
}

/** Umbral de grado para tratar un nodo como hub del lienzo. */
const HUB_DEGREE = 5;

interface ToolBtnProps {
  title: string;
  onClick: (e: React.MouseEvent) => void;
  className: string;
  children: React.ReactNode;
}

const ToolBtn: React.FC<ToolBtnProps> = ({ title, onClick, className, children }) => (
  <button
    type="button"
    title={title}
    onClick={onClick}
    className={`flex items-center justify-center gap-1 text-[10px] font-medium px-1.5 py-1 rounded-md border transition-colors cursor-pointer nodrag ${className}`}
  >
    {children}
  </button>
);

const Separator = () => <span className="w-px h-4 bg-slate-700/70 mx-0.5 shrink-0" />;

/**
 * Los colores del lienzo llegan como variables CSS desde `canvasTheme.ts`
 * (clases `nf-card`, `nf-title`, `nf-body`, `nf-muted`, `nf-maturity`, `nf-seg-off`):
 * así el tema se cambia en un archivo y no en 40 clases de Tailwind.
 */
export const IdeaNode: React.FC<NodeProps<IdeaNodeData>> = memo(({ id, data, selected }) => {
  const accentColor = data.colorAccent || '#6366f1';
  const categoryLabel = data.category || data.label || (data.isRoot ? 'NÚCLEO' : 'CONCEPTO');
  const isSearchMatch = data.isSearchMatch;
  // Macro-nodo (Fase A): nace de condensar N nodos y guarda su linaje.
  const macro = data.macro;
  const lod = useLod();
  const tarjetas = useTarjetas();
  const tema = useTema();

  const degree = data.degree ?? 0;
  const isHub = degree >= HUB_DEGREE;
  const isLeaf = degree <= 1;

  const showBody = lod !== 'compacto';
  const showFull = lod === 'completo';

  const currentMaturity: IdeaMaturityLevel =
    data.maturity || (data.isRoot ? 3 : data.aiOrigin?.actionType === 'hybrid' ? 3 : 1);
  const maturityConfig = MATURITY_CONFIGS[currentMaturity] || MATURITY_CONFIGS[1];

  const handleSetMaturity = (level: IdeaMaturityLevel) => {
    data.onAction?.('set-maturity', id, { ...data, maturity: level });
  };

  const handleCycleMaturity = () => {
    const nextLevel = ((currentMaturity % 4) + 1) as IdeaMaturityLevel;
    handleSetMaturity(nextLevel);
  };

  const [isInlineEditing, setIsInlineEditing] = React.useState(Boolean(data.isEditing));
  const [editTitle, setEditTitle] = React.useState(data.title);
  const inputRef = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    setIsInlineEditing(Boolean(data.isEditing));
  }, [data.isEditing]);

  React.useEffect(() => {
    setEditTitle(data.title);
  }, [data.title]);

  React.useEffect(() => {
    if (isInlineEditing) {
      const timer = setTimeout(() => {
        if (inputRef.current) {
          inputRef.current.focus();
          inputRef.current.select();
        }
      }, 30);
      return () => clearTimeout(timer);
    }
  }, [isInlineEditing]);

  const commitEdit = (nextAction?: 'inline-save' | 'inline-save-tab' | 'inline-save-enter') => {
    setIsInlineEditing(false);
    const finalTitle = editTitle.trim() || 'Nueva Idea';
    data.onAction?.(nextAction || 'inline-save', id, { ...data, title: finalTitle, isEditing: false });
  };

  const cancelEdit = () => {
    setIsInlineEditing(false);
    setEditTitle(data.title);
    data.onAction?.('inline-cancel', id, { ...data, isEditing: false });
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      commitEdit('inline-save-enter');
    } else if (e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      commitEdit('inline-save-tab');
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      cancelEdit();
    }
  };

  const fire = (action: string) => (e: React.MouseEvent) => {
    e.stopPropagation();
    data.onAction?.(action as any, id, data);
  };

  // Jerarquía de escala: el hub del lienzo pesa más que una hoja.
  const widthClass = isHub
    ? 'min-w-[235px] max-w-[320px]'
    : isLeaf
    ? 'min-w-[190px] max-w-[240px]'
    : 'min-w-[215px] max-w-[285px]';
  const titleClass = isHub ? 'text-[15px] font-bold' : isLeaf ? 'text-[13px] font-semibold' : 'text-sm font-semibold';

  return (
    <div
      id={`node-${id}`}
      onDoubleClick={(e) => {
        e.stopPropagation();
        data.onAction?.('edit', id, data);
      }}
      className={`relative group nf-card border-2 rounded-xl shadow-2xl transition-[box-shadow,border-color,transform] z-10 select-none cursor-grab active:cursor-grabbing ${widthClass} ${
        lod === 'compacto' ? 'p-3' : 'p-4'
      } ${
        selected
          ? 'ring-2 shadow-lg scale-[1.02]'
          : isSearchMatch
          ? 'ring-2 ring-amber-400/90 shadow-amber-500/30 scale-[1.02]'
          : ''
      }`}
      style={{
        borderColor: selected ? accentColor : isSearchMatch ? '#fbbf24' : `${accentColor}${isHub ? 'cc' : '99'}`,
        boxShadow: data.lente
          ? `0 0 0 3px ${accentColor}40, 0 12px 24px -10px ${accentColor}66`
          : selected
          ? `0 10px 25px -5px ${accentColor}33`
          : isHub
          ? `0 8px 20px -12px ${accentColor}55`
          : undefined,
        // Superficie de la tarjeta: la clase `.nf-card` trae la del tema y el modo
        // elegido en el HUD la sobrescribe (inline gana sobre la clase).
        backgroundColor: tarjetas === 'cristal' ? tema.cardCristal : undefined,
        backdropFilter: tarjetas === 'cristal' ? 'blur(7px)' : undefined,
        backgroundImage:
          tarjetas === 'tintada'
            ? `linear-gradient(180deg, ${accentColor}16, ${accentColor}07)`
            : undefined,
        // El acento del nodo viaja como variable: las clases `.nf-acento` lo usan
        // para el rótulo y los tags y lo oscurecen en tema claro (contraste).
        '--nf-acento': accentColor,
      } as React.CSSProperties}
    >
      {/* Indicador de acento superior */}
      <div
        className="absolute top-0 left-4 right-4 h-1 rounded-b-sm opacity-80"
        style={{ backgroundColor: accentColor }}
      />

      {/* Puntos de conexión en los 4 costados (Compatibilidad y bidireccionalidad) */}
      {/* Arriba */}
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="top"
        position={Position.Top}
        type="target"
        style={{ backgroundColor: accentColor }}
      />
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="top-out"
        position={Position.Top}
        type="source"
        style={{ backgroundColor: accentColor }}
      />

      {/* Izquierda */}
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="left"
        position={Position.Left}
        type="target"
        style={{ backgroundColor: accentColor }}
      />
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="left-out"
        position={Position.Left}
        type="source"
        style={{ backgroundColor: accentColor }}
      />

      <div className="flex flex-col gap-2 pt-1">
        {/* Encabezado: sólo categoría. Las acciones viven en la barra flotante. */}
        <div className="flex items-center gap-1.5 min-w-0">
          <span
            className={`w-2 h-2 rounded-full shrink-0 ${lod === 'compacto' ? '' : 'animate-pulse'}`}
            style={{ backgroundColor: accentColor }}
          />
          <span
            className="text-[10px] font-bold uppercase tracking-wider truncate nf-acento"
          >
            {categoryLabel}
          </span>
          {macro && (
            <span
              className="ml-auto text-[9px] font-mono shrink-0 px-1.5 py-0.5 rounded-md border"
              style={{ borderColor: `${accentColor}66`, backgroundColor: `${accentColor}1a`, color: accentColor }}
              title={`${macro.colapsados} nodos condensados · doble clic para ver el linaje`}
            >
              ◈ {macro.colapsados}
            </span>
          )}
          {isHub && !macro && (
            <span
              className="ml-auto text-[9px] font-mono nf-muted shrink-0"
              title={`${degree} conexiones`}
            >
              {degree}
            </span>
          )}
        </div>

        {/* Título (con In-Place Editing) */}
        {isInlineEditing ? (
          <div className="flex flex-col gap-1 nodrag cursor-default" onClick={(e) => e.stopPropagation()}>
            <input
              ref={inputRef}
              type="text"
              value={editTitle}
              onChange={(e) => setEditTitle(e.target.value)}
              onKeyDown={handleInputKeyDown}
              onBlur={() => commitEdit('inline-save')}
              placeholder="Escribe la idea..."
              className="w-full nf-input text-sm font-semibold px-2.5 py-1.5 rounded-lg border-2 border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-400/50 shadow-inner nodrag"
            />
            <div className="flex items-center justify-between text-[9px] nf-muted px-0.5 select-none font-medium">
              <span>Enter guardar • Tab hijo</span>
              <button
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  commitEdit('inline-save');
                }}
                className="text-indigo-500 hover:text-indigo-600 underline cursor-pointer"
              >
                Listo
              </button>
            </div>
          </div>
        ) : (
          <div
            onDoubleClick={(e) => {
              e.stopPropagation();
              setIsInlineEditing(true);
              data.onAction?.('inline-start', id, { ...data, isEditing: true });
            }}
            title="Doble clic para editar título directamente"
            className={`${titleClass} nf-title leading-snug break-words cursor-text ${
              lod === 'compacto' ? 'line-clamp-2' : ''
            }`}
          >
            {data.title || <span className="nf-muted italic">Idea sin título...</span>}
          </div>
        )}

        {/* Cuerpo: recién a partir del zoom medio */}
        {showBody && data.description && !isInlineEditing && (
          <p
            className={`text-[11px] nf-body leading-relaxed break-words ${
              showFull ? 'line-clamp-3' : 'line-clamp-2'
            }`}
          >
            {data.description}
          </p>
        )}

        {/* Tags: sólo en zoom de enfoque */}
        {showFull && data.tags && data.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-0.5">
            {data.tags.map((tag, i) => (
              <span
                key={i}
                className="text-[9px] px-2 py-0.5 rounded border transition-colors font-medium nf-acento"
                style={{
                  backgroundColor: `${accentColor}14`,
                  borderColor: `${accentColor}40`,
                }}
              >
                #{tag}
              </span>
            ))}
          </div>
        )}

        {/* Madurez: en compacto queda como tira fina (indicador de color). */}
        {lod === 'compacto' ? (
          <div className="flex items-center gap-1 mt-0.5" title={`Madurez ${currentMaturity}/4 · ${maturityConfig.label}`}>
            {([1, 2, 3, 4] as IdeaMaturityLevel[]).map((lvl) => {
              const isReached = lvl <= currentMaturity;
              return (
                <span
                  key={lvl}
                  className={`h-1 flex-1 rounded-full ${isReached ? maturityConfig.barBg : 'nf-seg-off'}`}
                />
              );
            })}
          </div>
        ) : (
          <div
            className="flex items-center justify-between gap-2 px-2 py-1.5 rounded-lg nf-maturity border my-0.5 select-none"
            title="Calificador de Madurez de la Idea. Clic en el texto para rotar o en las barras para fijar nivel."
          >
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleCycleMaturity();
              }}
              title="Clic para avanzar el nivel de madurez"
              className="flex items-center gap-1.5 text-[10px] hover:opacity-90 transition-opacity cursor-pointer group/mat shrink-0"
            >
              <span className={`font-bold flex items-center gap-1 text-[10px] ${maturityConfig.textColor}`}>
                <span>{maturityConfig.icon}</span>
                {showFull && <span className="group-hover/mat:underline">{maturityConfig.label}</span>}
              </span>
            </button>

            {/* 4 Segmentos Progresivos de Madurez (1 a 4) */}
            <div className="flex items-center gap-1 shrink-0">
              {([1, 2, 3, 4] as IdeaMaturityLevel[]).map((lvl) => {
                const isReached = lvl <= currentMaturity;
                const cfg = MATURITY_CONFIGS[lvl];
                return (
                  <button
                    key={lvl}
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleSetMaturity(lvl);
                    }}
                    title={`Nivel ${lvl}: ${cfg.icon} ${cfg.label} — ${cfg.desc}`}
                    className={`h-2 rounded-full transition-all cursor-pointer ${
                      isReached
                        ? `${maturityConfig.barBg} w-3.5 shadow-sm`
                        : 'nf-seg-off hover:brightness-95 w-2.5 opacity-60'
                    }`}
                  />
                );
              })}
            </div>
          </div>
        )}
      </div>

      {/* Barra flotante de acciones: sólo con el nodo seleccionado. Sacar estos
          botones del cuerpo recorta ~40% de la altura de la tarjeta. Sigue siendo
          oscura a propósito: es un overlay sobre el lienzo. */}
      {selected && !isInlineEditing && (
        <div
          className="absolute -top-11 left-1/2 -translate-x-1/2 nf-dark flex items-center gap-0.5 backdrop-blur-md border rounded-xl px-1 py-1 shadow-2xl nodrag nowheel z-50 whitespace-nowrap"
          onClick={(e) => e.stopPropagation()}
        >
          <ToolBtn
            title="Editar nodo (doble clic)"
            onClick={fire('edit')}
            className="text-slate-400 hover:text-white hover:bg-slate-800 border-transparent"
          >
            <Edit3 size={11} />
          </ToolBtn>
          <ToolBtn
            title="Duplicar nodo"
            onClick={fire('duplicate')}
            className="text-slate-400 hover:text-indigo-300 hover:bg-slate-800 border-transparent"
          >
            <Copy size={11} />
          </ToolBtn>

          <Separator />

          <ToolBtn
            title="Generar 3 ramas lógicas"
            onClick={fire('branch')}
            className="bg-emerald-950/70 text-emerald-300 hover:bg-emerald-900/80 border-emerald-800/60"
          >
            <GitBranch size={11} /> Ramificar
          </ToolBtn>
          <ToolBtn
            title="Analizar viabilidad, riesgos y estrategia"
            onClick={fire('explore')}
            className="bg-amber-950/70 text-amber-300 hover:bg-amber-900/80 border-amber-800/60"
          >
            <Eye size={11} /> Explorar
          </ToolBtn>
          <ToolBtn
            title="Abogado del Diablo: auditar riesgos, fallas y contraargumentos"
            onClick={fire('critique')}
            className="bg-rose-950/70 text-rose-300 hover:bg-rose-900/80 border-rose-800/60 group/crit"
          >
            <Flame size={11} className="text-rose-400 group-hover/crit:animate-pulse" /> Crítica
          </ToolBtn>
          <ToolBtn
            title="Preguntas Socráticas: desafíos profundos para destrabar el concepto"
            onClick={fire('socratic')}
            className="bg-cyan-950/70 text-cyan-300 hover:bg-cyan-900/80 border-cyan-800/60"
          >
            <HelpCircle size={11} className="text-cyan-400" /> Socrático
          </ToolBtn>

          <Separator />

          <ToolBtn
            title="Eliminar nodo (Delete)"
            onClick={fire('delete')}
            className="text-slate-400 hover:text-rose-300 hover:bg-slate-800 border-transparent"
          >
            <Trash2 size={11} />
          </ToolBtn>
        </div>
      )}

      {/* Abajo y Derecha */}
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="bottom"
        position={Position.Bottom}
        type="source"
        style={{ backgroundColor: accentColor }}
      />
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="bottom-in"
        position={Position.Bottom}
        type="target"
        style={{ backgroundColor: accentColor }}
      />

      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="right"
        position={Position.Right}
        type="source"
        style={{ backgroundColor: accentColor }}
      />
      <Handle
        className="w-3 h-3 hover:scale-125 transition-transform"
        id="right-in"
        position={Position.Right}
        type="target"
        style={{ backgroundColor: accentColor }}
      />
    </div>
  );
});

IdeaNode.displayName = 'IdeaNode';
