import { useEffect, useState } from 'react';
import { Cpu, Cloud, Coins, RefreshCw } from 'lucide-react';
import { apiUrl } from '../services/apiBase';

/**
 * Fase 12 — **Selector global de motor de inferencia**.
 *
 * Un solo lugar decide dónde corre la IA de toda la app: no se configura función por función.
 * El catálogo lo arma el backend con lo que existe de verdad (modelos del daemon local, nube
 * configurada, proveedores compatibles con OpenAI) y declara lo que falta en vez de esconderlo.
 *
 * La elección se guarda en el backend (`POST /api/ai/motor`), así vale también para la API y los
 * tests, no sólo para esta ventana.
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

  const cargar = async () => {
    setCargando(true);
    try {
      const d = await (await fetch(apiUrl('/api/ai/motores'))).json();
      setMotores(Array.isArray(d.motores) ? d.motores : []);
      setElegido(d.seleccionado ?? '');
      setEfectivo(d.efectivo ?? '');
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
      await fetch(apiUrl('/api/ai/motor'), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id }),
      });
      cargar();
    } catch {
      /* si falla, la próxima carga muestra el estado real */
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

  return (
    <div className="flex items-center gap-1.5">
      <div
        id="selector-motor"
        className="flex items-center gap-1.5 bg-slate-900/70 border border-slate-800 rounded-xl px-2 py-1"
        title="Dónde corre la IA de toda la app. Se guarda y vale para la API, no sólo para esta ventana."
      >
        <IconoGrupo size={13} className={grupoActual?.color ?? 'text-slate-400'} />
        <select
          value={elegido}
          onChange={(e) => elegir(e.target.value)}
          disabled={cargando}
          className="bg-transparent text-[11px] text-slate-200 outline-none cursor-pointer max-w-[190px]"
        >
          <option value="" className="bg-slate-900">
            Automático {efectivo ? `(${motores.find((m) => m.id === efectivo)?.modelo ?? efectivo})` : ''}
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
    </div>
  );
}
