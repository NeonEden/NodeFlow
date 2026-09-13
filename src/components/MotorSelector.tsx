import { useEffect, useState } from 'react';
import { Cpu, Cloud, Coins, Plus, RefreshCw, X } from 'lucide-react';
import { apiUrl } from '../services/apiBase';
import { fijarMotorActual } from '../state/motorActual';

/**
 * Fase 12 — **Selector global de motor de inferencia**.
 *
 * Un solo lugar decide dónde corre la IA de toda la app. El catálogo lo arma el backend con lo que
 * existe de verdad (modelos del daemon local, nube configurada, proveedores agregados a mano) y
 * declara lo que falta en vez de esconderlo. La elección se guarda en el backend, así vale también
 * para la API y los tests.
 */

interface Motor {
  id: string;
  etiqueta: string;
  proveedor: string;
  modelo: string;
  donde: 'local' | 'gratis' | 'pago';
  disponible: boolean;
  nota?: string;
}

interface Traza {
  proveedor: string;
  modelo?: string;
  ms: number;
  tokens: number;
  tokens_evitados: number;
  costo_usd: number;
  cache: string;
}

const GRUPOS: { donde: Motor['donde']; etiqueta: string; icono: any; color: string }[] = [
  { donde: 'local', etiqueta: 'En tu placa', icono: Cpu, color: 'text-emerald-300' },
  { donde: 'gratis', etiqueta: 'Nube gratuita', icono: Cloud, color: 'text-sky-300' },
  { donde: 'pago', etiqueta: 'Nube paga', icono: Coins, color: 'text-amber-300' },
];

export function MotorSelector() {
  const [motores, setMotores] = useState<Motor[]>([]);
  const [elegido, setElegido] = useState('');
  const [efectivo, setEfectivo] = useState('');
  const [traza, setTraza] = useState<Traza | null>(null);
  const [cargando, setCargando] = useState(false);
  const [alta, setAlta] = useState(false);
  const [form, setForm] = useState({ id: '', etiqueta: '', base_url: '', modelo: '', api_key: '', donde: 'pago' });
  const [msgAlta, setMsgAlta] = useState<string | null>(null);

  const aplicar = (d: any) => {
    const lista: Motor[] = Array.isArray(d?.motores) ? d.motores : [];
    setMotores(lista);
    setElegido(d?.seleccionado ?? '');
    setEfectivo(d?.efectivo ?? '');
    const m = lista.find((x) => x.id === (d?.efectivo ?? ''));
    fijarMotorActual(m?.modelo ?? 'la IA');
  };

  const cargar = async () => {
    setCargando(true);
    try {
      aplicar(await (await fetch(apiUrl('/api/ai/motores'))).json());
    } catch {
      /* sin backend todavía */
    } finally {
      setCargando(false);
    }
  };

  useEffect(() => {
    cargar();
    const alCorrer = (e: Event) => setTraza((e as CustomEvent).detail as Traza);
    window.addEventListener('nodeflow:traza', alCorrer);
    return () => window.removeEventListener('nodeflow:traza', alCorrer);
  }, []);

  const elegir = async (id: string) => {
    setElegido(id);
    try {
      const d = await (
        await fetch(apiUrl('/api/ai/motor'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ id }),
        })
      ).json();
      if (d?.ok) await cargar();
    } catch {
      await cargar();
    }
  };

  const guardarAlta = async () => {
    setMsgAlta(null);
    try {
      const d = await (
        await fetch(apiUrl('/api/ai/proveedor'), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(form),
        })
      ).json();
      if (!d?.ok) {
        setMsgAlta(d?.error ?? 'no se pudo guardar');
        return;
      }
      aplicar(d);
      setAlta(false);
      setForm({ id: '', etiqueta: '', base_url: '', modelo: '', api_key: '', donde: 'pago' });
      setMsgAlta('API agregada');
    } catch (e: any) {
      setMsgAlta(e?.message ?? 'error de red');
    }
  };

  const actual = motores.find((m) => m.id === (elegido || efectivo));
  const grupoActual = GRUPOS.find((g) => g.donde === actual?.donde);
  const IconoGrupo = grupoActual?.icono ?? Cpu;
  const costo = traza ? (traza.costo_usd > 0 ? `$${traza.costo_usd.toFixed(4)}` : '$0') : '';
  const etiquetaTraza = traza
    ? `${traza.proveedor} · ${traza.ms} ms · ${traza.tokens} tok · ${costo}${traza.cache === 'hit' ? ' · caché HIT' : ''}`
    : '';
  const disponibles = motores.filter((m) => m.disponible).length;
  const nombreEfectivo = motores.find((m) => m.id === efectivo)?.modelo ?? efectivo ?? '';

  return (
    <div className="relative flex items-center gap-1.5">
      <div
        id="selector-motor"
        className="flex items-center gap-1.5 bg-slate-900/70 border border-slate-800 rounded-xl px-2 py-1"
        title="Dónde corre la IA de toda la app. Se guarda y vale también para la API."
      >
        <IconoGrupo size={13} className={grupoActual?.color ?? 'text-slate-400'} />
        <select
          value={elegido}
          onChange={(e) => elegir(e.target.value)}
          disabled={cargando}
          className="bg-transparent text-[11px] text-slate-200 outline-none cursor-pointer max-w-[190px]"
        >
          <option value="auto:local" className="bg-slate-900">
            Automático: local{grupoActual?.donde === 'local' && nombreEfectivo ? ` (${nombreEfectivo})` : ''}
          </option>
          <option value="auto:nube" className="bg-slate-900">
            Automático: nube{grupoActual && grupoActual.donde !== 'local' && nombreEfectivo ? ` (${nombreEfectivo})` : ''}
          </option>
          {GRUPOS.map((g) => {
            const items = motores.filter((m) => m.donde === g.donde);
            if (!items.length) return null;
            return (
              <optgroup key={g.donde} label={g.etiqueta} className="bg-slate-900">
                {items.map((m) => (
                  <option key={m.id} value={m.id} disabled={!m.disponible} className="bg-slate-900">
                    {m.modelo}
                    {m.disponible ? '' : ` — no disponible${m.nota ? ` (${m.nota})` : ''}`}
                  </option>
                ))}
              </optgroup>
            );
          })}
        </select>
        <button
          type="button"
          onClick={() => setAlta((v) => !v)}
          title="Agregar una API compatible con OpenAI (DeepSeek, vLLM, Fireworks…)"
          className="text-slate-500 hover:text-slate-200 transition-colors cursor-pointer"
        >
          <Plus size={11} />
        </button>
        <button
          type="button"
          onClick={cargar}
          title={`${disponibles} de ${motores.length} motores disponibles · refrescar`}
          className="text-slate-500 hover:text-slate-200 transition-colors cursor-pointer"
        >
          <RefreshCw size={11} className={cargando ? 'animate-spin' : ''} />
        </button>
      </div>

      {traza && (
        <span
          id="traza-inferencia"
          className={`hidden xl:inline text-[10px] font-mono px-2 py-1 rounded-xl border ${
            traza.cache === 'hit' ? 'text-slate-400 border-slate-800 bg-slate-900/70' : 'text-slate-300 border-slate-700 bg-slate-900/70'
          }`}
          title="Traza real de la última corrida: motor, latencia, tokens, costo y caché"
        >
          {etiquetaTraza}
        </span>
      )}

      {alta && (
        <div className="absolute right-0 top-full mt-2 z-50 w-72 bg-slate-900 border border-slate-700 rounded-xl p-3 space-y-2 shadow-2xl">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-slate-200">Agregar API compatible</span>
            <button type="button" onClick={() => setAlta(false)} className="text-slate-500 hover:text-slate-200 cursor-pointer">
              <X size={13} />
            </button>
          </div>
          {[
            { k: 'id', ph: 'id (ej. deepseek)' },
            { k: 'etiqueta', ph: 'etiqueta (ej. deepseek-chat · API)' },
            { k: 'base_url', ph: 'https://api.deepseek.com/v1' },
            { k: 'modelo', ph: 'modelo (ej. deepseek-chat)' },
            { k: 'api_key', ph: 'API key (sólo se guarda local)' },
          ].map((c) => (
            <input
              key={c.k}
              type={c.k === 'api_key' ? 'password' : 'text'}
              value={(form as any)[c.k]}
              onChange={(e) => setForm({ ...form, [c.k]: e.target.value })}
              placeholder={c.ph}
              className="w-full bg-slate-900 border border-slate-700 rounded-lg px-2 py-1.5 text-[11px] text-slate-200 placeholder:text-slate-500 outline-none focus:border-slate-500"
            />
          ))}
          <div className="flex items-center gap-2">
            <select
              value={form.donde}
              onChange={(e) => setForm({ ...form, donde: e.target.value })}
              className="bg-slate-900 border border-slate-700 rounded-lg px-2 py-1.5 text-[11px] text-slate-200 outline-none"
            >
              <option value="pago">Nube paga</option>
              <option value="gratis">Nube gratuita</option>
              <option value="local">En tu placa</option>
            </select>
            <button
              type="button"
              onClick={guardarAlta}
              className="flex-1 bg-slate-800 hover:bg-slate-700 text-slate-100 border border-slate-600 rounded-lg px-2 py-1.5 text-[11px] font-medium transition-colors cursor-pointer"
            >
              Guardar
            </button>
          </div>
          {msgAlta && <p className="text-[10px] text-slate-400">{msgAlta}</p>}
        </div>
      )}
    </div>
  );
}
