import { apiUrl } from './apiBase';
import { postAiAction } from './aiApi';

/**
 * Voz: Speechmatics transcribe, el motor de la app interpreta y PROPONE un plan de operaciones
 * sobre el lienzo. El backend valida ese plan contra los ids reales antes de devolverlo, y acá
 * sólo se pide y se muestra: nada se aplica sin que el usuario lo apruebe.
 */

export interface VozEstado {
  success: boolean;
  configurada: boolean;
  proveedor: string;
  url: string;
  modelo: string;
  idioma: string;
  codec: string;
  pista: string;
  /** Voz de salida local (Kokoro). Es opcional: si no está levantada, se avisa y nada se rompe. */
  tts?: { disponible: boolean; url: string; motor: string };
}

export type AccionVoz = 'crear' | 'enlazar' | 'enfocar' | 'condensar' | 'criticar' | 'delegar' | 'actualizar';

export interface VozComando {
  accion: AccionVoz;
  titulo?: string;
  descripcion?: string;
  categoria?: string;
  criterio?: string;
  nodos?: string[];
  desde?: string;
  hasta?: string;
  /** Sólo en `delegar`: lo que hay que pedirle al motor profundo (Hermes, con sus herramientas). */
  pedido?: string;
  /** Sólo en `actualizar`: el nodo que ya existe y los campos que cambian (fase, descripción…). */
  nodo?: string;
  maturity?: number;
  tags?: string[];
}

export interface PlanVoz {
  intencion: 'capturar' | 'comando';
  /** Lo decide el backend con la regla de voz selectiva: si merece hablarse, se dice. */
  hablar?: boolean;
  respuesta: string;
  motivo?: string;
  comandos: VozComando[];
  descartados?: number;
  motivo_descarte?: string[];
}

export interface PlanVozRespuesta {
  plan: PlanVoz;
  modelo: string;
  uso: Record<string, any>;
}

export async function getVozEstado(): Promise<VozEstado> {
  const r = await fetch(apiUrl('/api/voz/estado'));
  if (!r.ok) throw new Error('No pude consultar el estado de la voz.');
  return (await r.json()) as VozEstado;
}

export async function getVozJwt(): Promise<{ jwt: string; url: string; modelo: string; idioma: string; expira_en_s: number }> {
  const r = await fetch(apiUrl('/api/voz/jwt'));
  const d = await r.json();
  if (!r.ok || !d.success) throw new Error(d?.error || 'No pude pedir el token de voz.');
  return d;
}

/** Manda lo dictado y recibe el plan ya validado contra el lienzo real. */
export async function pedirPlanVoz(texto: string): Promise<PlanVozRespuesta> {
  const r = await postAiAction({ type: 'voz', texto });
  const d = await r.json();
  if (!d.success) throw new Error(d?.error || 'No pude interpretar lo que dijiste.');
  const plan = d.voz as PlanVoz;
  return { plan, modelo: d.modelUsed || '', uso: d.uso || {} };
}

/** Pide la voz local (Kokoro) y devuelve el audio listo para reproducir. */
export async function decir(texto: string): Promise<Blob> {
  const r = await fetch(apiUrl('/api/voz/decir'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ texto }),
  });
  if (!r.ok) {
    let detalle = 'La voz local no respondió.';
    try {
      detalle = (await r.json())?.error || detalle;
    } catch {
      /* respuesta sin JSON */
    }
    throw new Error(detalle);
  }
  return await r.blob();
}

/** Texto legible de un comando, para la tarjeta de confirmación. */
export function describirComando(c: VozComando, titulo: (id: string) => string): string {
  switch (c.accion) {
    case 'crear':
      return `Crear «${c.titulo}»`;
    case 'enlazar':
      return `Enlazar ${titulo(c.desde || '') || c.desde} → ${titulo(c.hasta || '') || c.hasta}`;
    case 'enfocar':
      return `Enfocar en ${c.nodos?.length || 0} nodos y condensar el resto`;
    case 'condensar':
      return `Condensar ${c.nodos?.length || 0} nodos en uno`;
    case 'criticar':
      return `Cuestionar ${c.nodos?.length || 0} nodos`;
    case 'delegar':
      return `Pedirle al motor profundo: «${(c.pedido || '').slice(0, 60)}»`;
    case 'actualizar': {
      const que = [c.titulo && 'título', c.descripcion && 'descripción', c.categoria && 'categoría',
                   c.maturity && `fase ${c.maturity}`, c.tags?.length && 'etiquetas'].filter(Boolean).join(', ');
      return `Actualizar ${titulo(c.nodo || '') || c.nodo}: ${que || 'un campo'}`;
    }
    default:
      return c.accion;
  }
}
