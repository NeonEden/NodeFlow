import React, { memo } from 'react';
import { Handle, Position, NodeProps } from 'reactflow';
import { GitBranch, Eye, Edit3, Trash2, Copy, Flame, HelpCircle } from 'lucide-react';
import { IdeaNodeData, IdeaMaturityLevel, MATURITY_CONFIGS } from '../types';

export const IdeaNode: React.FC<NodeProps<IdeaNodeData>> = memo(({ id, data, selected }) => {
  const accentColor = data.colorAccent || '#6366f1';
  const categoryLabel = data.category || data.label || (data.isRoot ? 'NÚCLEO' : 'CONCEPTO');
  const isSearchMatch = data.isSearchMatch;

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

  return (
    <div
      id={`node-${id}`}
      onDoubleClick={(e) => {
        e.stopPropagation();
        // If double clicked anywhere on card that isn't the title input, open edit modal
        data.onAction?.('edit', id, data);
      }}
      className={`relative group bg-slate-900/95 border-2 text-white p-4 rounded-xl shadow-2xl min-w-[220px] max-w-[290px] transition-all z-10 select-none cursor-grab active:cursor-grabbing ${
        selected
          ? 'ring-2 shadow-lg scale-[1.02]'
          : isSearchMatch
          ? 'ring-2 ring-amber-400/90 shadow-amber-500/30 scale-[1.02]'
          : 'hover:border-slate-500'
      }`}
      style={{
        borderColor: selected ? accentColor : isSearchMatch ? '#fbbf24' : `${accentColor}80`,
        boxShadow: selected ? `0 10px 25px -5px ${accentColor}33` : undefined,
      }}
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
        {/* Encabezado: Categoría y botones de acción rápida */}
        <div className="flex justify-between items-center gap-2">
          <div className="flex items-center gap-1.5 min-w-0">
            <span
              className="w-2 h-2 rounded-full shrink-0 animate-pulse"
              style={{ backgroundColor: accentColor }}
            />
            <span
              className="text-[10px] font-bold uppercase tracking-wider truncate"
              style={{ color: accentColor }}
            >
              {categoryLabel}
            </span>
          </div>

          <div className="flex items-center gap-1 opacity-80 group-hover:opacity-100 transition-opacity shrink-0">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('edit', id, data);
              }}
              title="Editar nodo (Doble clic)"
              className="p-1 text-slate-400 hover:text-white hover:bg-slate-800 rounded transition-colors"
            >
              <Edit3 size={12} />
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('duplicate', id, data);
              }}
              title="Duplicar nodo"
              className="p-1 text-slate-400 hover:text-indigo-400 hover:bg-slate-800 rounded transition-colors"
            >
              <Copy size={12} />
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('delete', id, data);
              }}
              title="Eliminar nodo (Delete)"
              className="p-1 text-slate-400 hover:text-rose-400 hover:bg-slate-800 rounded transition-colors"
            >
              <Trash2 size={12} />
            </button>
          </div>
        </div>

        {/* Título y Descripción con In-Place Editing */}
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
              className="w-full bg-slate-950 text-white text-sm font-semibold px-2.5 py-1.5 rounded-lg border-2 border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-400/50 shadow-inner nodrag"
            />
            <div className="flex items-center justify-between text-[9px] text-slate-400 px-0.5 select-none font-medium">
              <span>Enter guardar • Tab hijo</span>
              <button
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  commitEdit('inline-save');
                }}
                className="text-indigo-400 hover:text-indigo-300 underline cursor-pointer"
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
            className="text-sm font-semibold text-slate-100 leading-snug break-words cursor-text hover:text-indigo-200 transition-colors"
          >
            {data.title || <span className="text-slate-500 italic">Idea sin título...</span>}
          </div>
        )}
        {data.description && !isInlineEditing && (
          <p className="text-[11px] text-slate-400 leading-relaxed break-words line-clamp-3">
            {data.description}
          </p>
        )}

        {/* Tags */}
        {data.tags && data.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-0.5">
            {data.tags.map((tag, i) => (
              <span
                key={i}
                className="text-[9px] px-2 py-0.5 rounded border transition-colors font-medium"
                style={{
                  backgroundColor: `${accentColor}15`,
                  borderColor: `${accentColor}40`,
                  color: accentColor,
                }}
              >
                #{tag}
              </span>
            ))}
          </div>
        )}

        {/* Calificador de Madurez Interactivo */}
        <div
          className="flex items-center justify-between gap-2 px-2 py-1.5 rounded-lg bg-slate-950/70 border border-slate-800/80 my-0.5 select-none"
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
            <span className="text-slate-500 font-medium text-[9px] uppercase tracking-wider">Madurez:</span>
            <span className={`font-bold flex items-center gap-1 text-[10px] ${maturityConfig.textColor}`}>
              <span>{maturityConfig.icon}</span>
              <span className="group-hover/mat:underline">{maturityConfig.label}</span>
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
                      : 'bg-slate-800 hover:bg-slate-700 w-2.5 opacity-50'
                  }`}
                />
              );
            })}
          </div>
        </div>

        {/* Botones de Acción de IA & Potenciación Cognitiva */}
        <div className="flex flex-col gap-1.5 mt-2 border-t border-slate-800/80 pt-2">
          {/* Fila 1: Expansión & Exploración */}
          <div className="flex justify-between items-center gap-1.5">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('branch', id, data);
              }}
              title="Generar 3 ramas lógicas con Gemini"
              className="flex-1 flex items-center justify-center gap-1 text-[10px] font-medium bg-emerald-950/60 text-emerald-300 hover:bg-emerald-900/70 py-1 px-1.5 rounded border border-emerald-800/60 transition-colors cursor-pointer"
            >
              <GitBranch size={11} /> Ramificar
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('explore', id, data);
              }}
              title="Analizar viabilidad, riesgos y estrategia"
              className="flex-1 flex items-center justify-center gap-1 text-[10px] font-medium bg-amber-950/60 text-amber-300 hover:bg-amber-900/70 py-1 px-1.5 rounded border border-amber-800/60 transition-colors cursor-pointer"
            >
              <Eye size={11} /> Explorar
            </button>
          </div>

          {/* Fila 2: Abogado del Diablo (Crítica) & Pensamiento Socrático */}
          <div className="flex justify-between items-center gap-1.5">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('critique', id, data);
              }}
              title="Abogado del Diablo: auditar riesgos, fallas y contraargumentos"
              className="flex-1 flex items-center justify-center gap-1 text-[10px] font-medium bg-rose-950/60 text-rose-300 hover:bg-rose-900/70 py-1 px-1.5 rounded border border-rose-800/60 transition-colors cursor-pointer group/crit"
            >
              <Flame size={11} className="text-rose-400 group-hover/crit:animate-pulse" />
              <span>Crítica</span>
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                data.onAction?.('socratic', id, data);
              }}
              title="Preguntas Socráticas: formular desafíos profundos para destrabar el concepto"
              className="flex-1 flex items-center justify-center gap-1 text-[10px] font-medium bg-cyan-950/60 text-cyan-300 hover:bg-cyan-900/70 py-1 px-1.5 rounded border border-cyan-800/60 transition-colors cursor-pointer"
            >
              <HelpCircle size={11} className="text-cyan-400" />
              <span>Socrático</span>
            </button>
          </div>
        </div>
      </div>

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
