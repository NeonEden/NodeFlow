import { apiUrl } from './apiBase';
import { postAiAction } from './aiApi';
import type { SesionVoz } from './sttRt';

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
  /** Nombre legible del motor elegido (`proveedor` es el id: `speechmatics` | `assemblyai`). */
  proveedor_etiqueta?: string;
  /** `speechmatics-v2` | `assemblyai-v3` — decide el cliente del frontend. */
  protocolo?: string;
  /** `*` = multilingüe; si no, la lista de idiomas soportados. */
  idiomas_soportados?: string;
  /** Lo que hay que saber del motor (límites, latencia, alcance). */
  nota?: string;
  /** Aviso cuando el idioma pedido no se puede cumplir con el motor elegido. */
  aviso?: string | null;
  /** Catálogo completo, para poder ofrecer el cambio de motor. */
  proveedores?: ProveedorVoz[];
  /** Voz de salida local (Kokoro). Es opcional: si no está levantada, se avisa y nada se rompe. */
  tts?: { disponible: boolean; url: string; motor: string };
}

export interface ProveedorVoz {
  id: string;
  etiqueta: string;
  url: string;
  protocolo: string;
  idiomas: string;
  nota: string;
  clave_configurada: boolean;
}

export type AccionVoz = 'crear' | 'enlazar' | 'enfocar' | 'condensar' | 'criticar' | 'delegar' | 'actualizar';

export interface VozComando {
  accion: AccionVoz;
  titulo?: string;
  descripcion?: string;
  categoria?: string;
  /** Sólo en `crear`: a qué nodo colgar el nodo dictado (id o título). Sin él se usa el ancla del lienzo. */
  parent?: string;
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

/**
 * Token temporal del motor de voz elegido. El backend decide cuál es y cómo se emite; acá viaja
 * también `proveedor`/`protocolo` para que el panel sepa qué cliente instanciar, y `aviso` cuando
 * el idioma pedido no se puede cumplir (p. ej. streaming en inglés únicamente).
 */
export async function getVozJwt(): Promise<SesionVoz> {
  const r = await fetch(apiUrl('/api/voz/jwt'));
  const d = await r.json();
  if (!r.ok || !d.success) throw new Error(d?.error || 'No pude pedir el token de voz.');
  return d as SesionVoz;
}

/** Catálogo de motores de voz y cuál está elegido. */
export async function getVozProveedores(): Promise<{ elegido: string; proveedores: ProveedorVoz[] }> {
  const r = await fetch(apiUrl('/api/voz/proveedores'));
  const d = await r.json();
  if (!r.ok || !d.success) throw new Error(d?.error || 'No pude leer los motores de voz.');
  return { elegido: d.elegido, proveedores: d.proveedores || [] };
}

/** Cambia el motor de reconocimiento (queda guardado en la config de la app). */
export async function setVozProveedor(id: string): Promise<{ elegido: string; idiomas: string; nota: string }> {
  const r = await fetch(apiUrl('/api/voz/proveedor'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ id }),
  });
  const d = await r.json();
  if (!r.ok || !d.success) throw new Error(d?.error || 'No pude cambiar el motor de voz.');
  return { elegido: d.elegido, idiomas: d.idiomas, nota: d.nota };
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
