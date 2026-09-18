import React, { useEffect, useState } from 'react';
import { X, Pencil, Save, ShieldCheck } from 'lucide-react';
import { Experto, TipoArtefacto, guardarExperto } from '../services/expertoService';

interface ExpertoEditorModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** El experto que se edita, o `null` para pegar uno nuevo desde cero. */
  experto: Experto | null;
  tipos: TipoArtefacto[];
  /** Carpeta real de la bóveda, para decir dónde queda el archivo. */
  carpeta: string;
  onGuardado: () => void;
  showToast: (message: string, type?: 'success' | 'error' | 'info') => void;
}

/**
 * Editor del prompt de un experto.
 *
 * El system prompt de un experto es **identidad del usuario**: la app no trae ninguno embebido y
 * esto escribe una nota en SU bóveda (`<bóveda>/expertos/<slug>.md`). Pegar el prompt propio es la
 * forma de que la identidad visual de cada uno sea suya y no la del que escribió la app.
 */
export const ExpertoEditorModal: React.FC<ExpertoEditorModalProps> = ({
  isOpen,
  onClose,
  experto,
  tipos,
  carpeta,
  onGuardado,
  showToast,
}) => {
  const [nombre, setNombre] = useState('');
  const [tipo, setTipo] = useState('');
  const [descripcion, setDescripcion] = useState('');
  const [rol, setRol] = useState('');
  const [proveedor, setProveedor] = useState('');
  const [modelo, setModelo] = useState('');
  const [system, setSystem] = useState('');
  const [guardando, setGuardando] = useState(false);

  // Cada vez que se abre, el formulario arranca del experto elegido (o vacío, si es nuevo).
  useEffect(() => {
    if (!isOpen) return;
    setNombre(experto?.nombre || '');
    setTipo(experto?.tipo_artefacto || 'prompt_visual');
    setDescripcion(experto?.descripcion || '');
    setRol(experto?.rol || '');
    setProveedor(experto?.proveedor || '');
    setModelo(experto?.modelo || '');
    setSystem(experto?.system || '');
    setGuardando(false);
  }, [isOpen, experto]);

  if (!isOpen) return null;

  const slugPrevisto = (experto?.slug || nombre || '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 60);

  const guardar = async () => {
    if (!nombre.trim()) {
      showToast('Poné un nombre para el experto.', 'error');
      return;
    }
    if (system.trim().length < 40) {
      showToast('Pegá el system prompt completo: es el cuerpo del experto.', 'error');
      return;
    }
    setGuardando(true);
    const r = await guardarExperto({
      slug: experto?.slug,
      nombre: nombre.trim(),
      tipo_artefacto: tipo,
      descripcion: descripcion.trim(),
      rol: rol.trim(),
      proveedor: proveedor.trim(),
      modelo: modelo.trim(),
      system,
    });
    setGuardando(false);
    if (!r) {
      showToast('No pude guardar: la app no respondió.', 'error');
      return;
    }
    if (!r.success) {
      showToast(r.error || 'No pude guardar el experto.', 'error');
      return;
    }
    showToast(`Experto «${nombre.trim()}» guardado en tu bóveda · ${r.caracteres_system ?? system.length} car.`, 'success');
    onGuardado();
    onClose();
  };

  const inputCls =
    'w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-xs text-slate-100 focus:outline-none focus:border-violet-500';
  const labelCls = 'text-[10px] uppercase tracking-wide text-slate-500 mb-1';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm">
      <div className="relative w-full max-w-3xl max-h-[88vh] overflow-hidden flex flex-col bg-slate-900 border border-slate-700 rounded-2xl shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-800">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-sky-500/10 border border-sky-500/30 flex items-center justify-center text-sky-300">
              <Pencil size={16} />
            </div>
            <div>
              <div className="text-sm font-semibold text-slate-200">
                {experto ? `Prompt de «${experto.nombre}»` : 'Pegar mi propio prompt'}
              </div>
              <div className="text-[11px] text-slate-400">
                Se guarda en <span className="font-mono text-slate-500">{carpeta || 'expertos'}/{slugPrevisto || 'mi-experto'}.md</span>
              </div>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-2 text-slate-400 hover:text-slate-200 rounded-lg hover:bg-slate-800 cursor-pointer"
          >
            <X size={16} />
          </button>
        </div>

        <div className="p-5 overflow-y-auto space-y-3 text-sm flex-1">
          <div className="flex items-start gap-2 rounded-xl border border-emerald-500/25 bg-emerald-500/5 px-3 py-2">
            <ShieldCheck size={14} className="text-emerald-300 shrink-0 mt-0.5" />
            <p className="text-[11px] text-slate-300 leading-relaxed">
              Tu prompt es tuyo: queda en <span className="font-mono text-slate-400">expertos/</span> dentro de
              tu bóveda y no viaja con la app ni con el demo. Cada persona pega el suyo — así la identidad
              visual de cada uno sigue siendo suya.
            </p>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <p className={labelCls}>Nombre</p>
              <input value={nombre} onChange={(e) => setNombre(e.target.value)} className={inputCls} placeholder="Prompt Visual" />
            </div>
            <div>
              <p className={labelCls}>Qué artefacto produce</p>
              <select value={tipo} onChange={(e) => setTipo(e.target.value)} className={inputCls}>
                {tipos.map((t) => (
                  <option key={t.tipo} value={t.tipo}>
                    {t.tipo} → {t.destino}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <p className={labelCls}>Rol (para el frontmatter)</p>
              <input value={rol} onChange={(e) => setRol(e.target.value)} className={inputCls} placeholder="curador de arte digital" />
            </div>
            <div>
              <p className={labelCls}>Proveedor (opcional)</p>
              <input value={proveedor} onChange={(e) => setProveedor(e.target.value)} className={inputCls} placeholder="gemini" />
            </div>
            <div>
              <p className={labelCls}>Modelo (opcional)</p>
              <input value={modelo} onChange={(e) => setModelo(e.target.value)} className={inputCls} placeholder="—" />
            </div>
            <div>
              <p className={labelCls}>Descripción</p>
              <input value={descripcion} onChange={(e) => setDescripcion(e.target.value)} className={inputCls} placeholder="Lleva un concepto al clímax visual" />
            </div>
          </div>

          <div>
            <div className="flex items-center justify-between">
              <p className={labelCls}>System prompt (el cuerpo del experto)</p>
              <span className="text-[10px] font-mono text-slate-500">{system.length} car.</span>
            </div>
            <textarea
              value={system}
              onChange={(e) => setSystem(e.target.value)}
              rows={12}
              className={`${inputCls} font-mono leading-relaxed`}
              placeholder="Pegá acá tu prompt completo: rol, reglas, formato de salida…"
            />
          </div>
        </div>

        <div className="flex items-center justify-between gap-3 px-5 py-4 border-t border-slate-800">
          <span className="text-[11px] text-slate-500">
            Editar es volver a pegar: el cuerpo del archivo es el prompt entero.
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-2 rounded-xl text-xs font-medium text-slate-300 hover:text-slate-100 border border-slate-700 hover:border-slate-600 transition-colors cursor-pointer"
            >
              Cancelar
            </button>
            <button
              type="button"
              id="btn-guardar-experto"
              onClick={guardar}
              disabled={guardando}
              className={`flex items-center gap-1.5 px-3 py-2 rounded-xl text-xs font-medium border transition-colors cursor-pointer ${
                guardando
                  ? 'bg-slate-800 border-slate-700 text-slate-400'
                  : 'bg-violet-600/20 border-violet-500/50 text-violet-100 hover:bg-violet-600/30'
              }`}
            >
              <Save size={13} />
              {guardando ? 'Guardando…' : 'Guardar en mi bóveda'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
