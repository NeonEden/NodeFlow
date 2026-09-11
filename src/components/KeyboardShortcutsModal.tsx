import React, { useState, useMemo } from 'react';
import {
  Keyboard,
  X,
  Search,
  Command,
  Sliders,
  Sparkles,
  MousePointer,
  HelpCircle,
} from 'lucide-react';

interface ShortcutItem {
  keys: string[];
  description: string;
  category: 'canvas' | 'edit' | 'ai' | 'general';
  badge?: string;
}

const SHORTCUTS: ShortcutItem[] = [
  // Atajos Generales & Teclado
  {
    keys: ['?', 'Shift + /'],
    description: 'Abrir o cerrar esta guía de atajos permitidos',
    category: 'general',
    badge: 'Rápido',
  },
  {
    keys: ['Ctrl + /', '⌘ + /'],
    description: 'Alternar panel de atajos de teclado',
    category: 'general',
  },
  {
    keys: ['Esc'],
    description: 'Cerrar cualquier modal activo, cancelar búsqueda o deseleccionar',
    category: 'general',
  },
  {
    keys: ['Ctrl + F', '⌘ + F'],
    description: 'Enfocar barra de búsqueda rápida de nodos en el lienzo',
    category: 'general',
    badge: 'Búsqueda',
  },

  // Edición & Historial
  {
    keys: ['Tab'],
    description: 'Crear nodo hijo conectado a la derecha con cursor listo para escribir',
    category: 'edit',
    badge: 'Mindmap',
  },
  {
    keys: ['Enter'],
    description: 'Crear nodo hermano conectado al mismo padre (o debajo del seleccionado)',
    category: 'edit',
    badge: 'Mindmap',
  },
  {
    keys: ['Ctrl + Z', '⌘ + Z'],
    description: 'Deshacer la última acción (mover, añadir, borrar o conectar)',
    category: 'edit',
    badge: 'Historial',
  },
  {
    keys: ['Ctrl + Y', '⌘ + Shift + Z'],
    description: 'Rehacer la última acción deshecha',
    category: 'edit',
    badge: 'Historial',
  },
  {
    keys: ['Supr', 'Backspace'],
    description: 'Eliminar los nodos o conexiones actualmente seleccionados',
    category: 'edit',
  },
  {
    keys: ['Ctrl + Enter', '⌘ + Enter'],
    description: 'Guardar cambios en modal de edición o enviar descarga mental',
    category: 'edit',
  },
  {
    keys: ['M', '1 - 4'],
    description: 'Calificador de Madurez: M para alternar o números 1 (Semilla 🌱), 2 (Exploración ⚡), 3 (Validada 🛡️), 4 (Ejecutable 🚀)',
    category: 'edit',
    badge: 'Madurez',
  },

  // Lienzo & Gestos de Ratón
  {
    keys: ['Doble Clic en Lienzo'],
    description: 'Crear una nueva idea o nodo directamente en la posición del puntero',
    category: 'canvas',
    badge: 'Fluidez',
  },
  {
    keys: ['Doble Clic en Nodo'],
    description: 'Abrir ventana de edición profunda de título, descripción y etiquetas',
    category: 'canvas',
  },
  {
    keys: ['Segmentos de Madurez'],
    description: 'Haz clic en cualquiera de las 4 barras de la tarjeta para calificar la madurez al instante',
    category: 'canvas',
    badge: 'Cero Fricción',
  },
  {
    keys: ['Arrastrar Conectores'],
    description: 'Unir dos ideas arrastrando una línea desde cualquiera de los 4 puertos',
    category: 'canvas',
  },
  {
    keys: ['Rueda del Ratón'],
    description: 'Acercar o alejar la vista del lienzo (Zoom dinámico)',
    category: 'canvas',
  },
  {
    keys: ['Arrastrar Fondo'],
    description: 'Desplazarse libremente por el espacio infinito del lienzo (Pan)',
    category: 'canvas',
  },
  {
    keys: ['Click + Shift + Selección'],
    description: 'Seleccionar múltiples nodos simultáneamente para acciones grupales',
    category: 'canvas',
  },

  // IA y Pensamiento Estratégico
  {
    keys: ['U'],
    description: 'Unir directamente los 2 nodos seleccionados con una nueva conexión sin arrastrar cables',
    category: 'canvas',
    badge: 'Conexión Rápida',
  },
  {
    keys: ['H', 'Hibridador IA'],
    description: 'Cruza 2 o más ideas seleccionadas para sintetizar conceptos emergentes con Gemini IA',
    category: 'ai',
    badge: 'Sinergia',
  },
  {
    keys: ['B', 'Ramificar'],
    description: 'Genera 3 ramas conceptuales lógicas con Gemini IA conectadas al nodo seleccionado',
    category: 'ai',
    badge: 'Expansión',
  },
  {
    keys: ['E', 'Explorar'],
    description: 'Analiza viabilidad, riesgos, estrategia y derivaciones operativas del nodo',
    category: 'ai',
    badge: 'Análisis',
  },
  {
    keys: ['C', 'Crítica'],
    description: 'Abogado del Diablo: detecta puntos ciegos, antítesis y supuestos débiles del nodo',
    category: 'ai',
    badge: 'Auditoría',
  },
  {
    keys: ['S', 'Socrático'],
    description: 'Inyector Socrático: formula 3 preguntas profundas y catalizadoras para destrabar la idea',
    category: 'ai',
    badge: 'Reflexión',
  },
  {
    keys: ['Ctrl + B', '⌘ + B'],
    description: 'Abrir Descarga Mental (Brain Dump) para volcar ideas simples o complejas en nodos',
    category: 'ai',
    badge: 'Cero Fricción',
  },
  {
    keys: ['Puentes Semánticos'],
    description: 'Detector de Conexiones Ocultas: halla relaciones y sinergias no obvias entre nodos',
    category: 'ai',
    badge: 'Conexión',
  },
  {
    keys: ['Auto-Diseño'],
    description: 'Alinea y reorganiza automáticamente el grafo en jerarquía radial o árbol',
    category: 'canvas',
  },
];

const CATEGORIES = [
  { id: 'all', label: 'Todos los atajos', icon: Sliders },
  { id: 'general', label: 'Globales', icon: Command },
  { id: 'edit', label: 'Edición', icon: Keyboard },
  { id: 'canvas', label: 'Lienzo y Ratón', icon: MousePointer },
  { id: 'ai', label: 'Inteligencia Artificial', icon: Sparkles },
] as const;

interface KeyboardShortcutsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const KeyboardShortcutsModal: React.FC<KeyboardShortcutsModalProps> = ({
  isOpen,
  onClose,
}) => {
  const [activeCategory, setActiveCategory] = useState<'all' | 'general' | 'edit' | 'canvas' | 'ai'>('all');
  const [search, setSearch] = useState('');

  const filteredShortcuts = useMemo(() => {
    return SHORTCUTS.filter((s) => {
      const matchesCategory = activeCategory === 'all' || s.category === activeCategory;
      if (!matchesCategory) return false;

      if (!search.trim()) return true;
      const q = search.toLowerCase();
      const matchDesc = s.description.toLowerCase().includes(q);
      const matchKey = s.keys.some((k) => k.toLowerCase().includes(q));
      const matchBadge = s.badge?.toLowerCase().includes(q);
      return matchDesc || matchKey || matchBadge;
    });
  }, [activeCategory, search]);

  if (!isOpen) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="shortcuts-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-sm animate-in fade-in duration-150"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="bg-slate-900 border border-slate-800 w-full max-w-2xl rounded-2xl shadow-2xl flex flex-col max-h-[85vh] overflow-hidden text-slate-200">
        {/* Modal Header */}
        <div className="p-5 border-b border-slate-800/90 flex items-center justify-between bg-gradient-to-r from-indigo-950/40 via-slate-900 to-slate-900">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-indigo-600/20 border border-indigo-500/30 flex items-center justify-center text-indigo-400 shrink-0">
              <Keyboard size={18} />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 id="shortcuts-modal-title" className="text-base font-bold text-white">
                  Atajos de Teclado y Controles Permitidos
                </h2>
                <span className="px-2 py-0.5 rounded-full bg-indigo-500/10 text-indigo-300 text-[10px] font-mono border border-indigo-500/20">
                  Tecla ?
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-0.5">
                Domina la velocidad de pensamiento con navegación fluida y comandos de una pulsación
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800 transition-colors cursor-pointer"
            title="Cerrar (Esc)"
          >
            <X size={18} />
          </button>
        </div>

        {/* Filter bar & Quick Search */}
        <div className="p-4 border-b border-slate-800/60 bg-slate-950/40 space-y-3">
          {/* Quick Search Input */}
          <div className="relative">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Buscar atajo por tecla o acción (ej: deshacer, zoom, ia, borrar)..."
              className="w-full bg-slate-900 border border-slate-800 rounded-xl pl-9 pr-8 py-2 text-xs text-slate-200 placeholder:text-slate-500 focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500"
              autoFocus
            />
            {search && (
              <button
                type="button"
                onClick={() => setSearch('')}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-500 hover:text-slate-300 p-0.5"
              >
                <X size={12} />
              </button>
            )}
          </div>

          {/* Category Tabs */}
          <div className="flex items-center gap-1.5 overflow-x-auto no-scrollbar pb-0.5">
            {CATEGORIES.map((cat) => {
              const Icon = cat.icon;
              const isActive = activeCategory === cat.id;
              return (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => setActiveCategory(cat.id)}
                  className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium whitespace-nowrap transition-colors cursor-pointer ${
                    isActive
                      ? 'bg-indigo-600 text-white shadow-sm'
                      : 'bg-slate-900 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-800'
                  }`}
                >
                  <Icon size={12} className={isActive ? 'text-white' : 'text-slate-400'} />
                  <span>{cat.label}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Shortcuts List Body */}
        <div className="p-4 overflow-y-auto flex-1 space-y-2">
          {filteredShortcuts.length === 0 ? (
            <div className="py-10 text-center text-slate-500 text-xs">
              No se encontraron atajos para &quot;{search}&quot;.
            </div>
          ) : (
            filteredShortcuts.map((item, idx) => (
              <div
                key={idx}
                className="flex items-center justify-between p-2.5 rounded-xl bg-slate-900/50 hover:bg-slate-800/60 border border-slate-800/80 transition-colors gap-3"
              >
                <div className="flex items-center gap-2.5 flex-1 min-w-0">
                  <div className="space-y-0.5 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-slate-200 font-medium leading-tight">
                        {item.description}
                      </span>
                      {item.badge && (
                        <span className="text-[9px] font-semibold px-1.5 py-0.2 rounded bg-indigo-500/10 text-indigo-300 border border-indigo-500/20 shrink-0">
                          {item.badge}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                {/* Keycaps */}
                <div className="flex items-center gap-1.5 shrink-0">
                  {item.keys.map((k, kIdx) => (
                    <kbd
                      key={kIdx}
                      className="px-2.5 py-1 bg-slate-950 border border-slate-700/80 rounded-md font-mono text-[11px] font-medium text-indigo-300 shadow-sm shadow-black/40 inline-flex items-center gap-1"
                    >
                      {k}
                    </kbd>
                  ))}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Modal Footer */}
        <div className="p-3.5 bg-slate-950/80 border-t border-slate-800/80 flex items-center justify-between text-xs text-slate-400">
          <div className="flex items-center gap-2 text-[11px]">
            <HelpCircle size={13} className="text-indigo-400" />
            <span>Puedes presionar <kbd className="px-1.5 py-0.5 bg-slate-900 border border-slate-700 rounded font-mono text-indigo-300">?</kbd> en cualquier momento para abrir o cerrar esta ventana.</span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="px-3.5 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-lg text-xs font-semibold transition-colors cursor-pointer"
          >
            Cerrar (Esc)
          </button>
        </div>
      </div>
    </div>
  );
};
