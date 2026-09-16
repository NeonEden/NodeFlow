/**
 * Fase 4 — cliente del gateway propio del cerebro (`hermes serve`).
 *
 * Protocolo verificado a mano (15/09/2026) contra un `hermes serve` real, no supuesto:
 *   · transporte `ws://127.0.0.1:<puerto>/api/ws?token=<token>`
 *   · pedidos JSON-RPC 2.0: `session.create` → `{session_id}` · `prompt.submit {session_id, text}`
 *   · eventos: sobre `event` con `params.type` — `message.delta` trae el texto **token por token**,
 *     `thinking.delta` el spinner del modelo, `tool.start`/`tool.complete` los usos de herramientas,
 *     `message.complete` el cierre con `usage`, `session.info` el modelo/proveedor.
 *   · **el server también pregunta**: pedidos con `method: 'approval'` que el cliente contesta
 *     `{result:{choice, all}}`. `all: true` aplica la decisión a **todo** lo pendiente — es el
 *     «resolvé de una» para los cambios grandes.
 *
 * El proceso lo levanta el backend de la app (`/api/cerebro/gateway/arrancar`); acá sólo se consume.
 * Una sesión por proyecto (`sesion`), así el cerebro no arranca en blanco: lo que se pensó en el panel
 * y lo que se delegó comparten la misma mente.
 */

export interface EventoGateway {
  type?: string;
  session_id?: string;
  seq?: number;
  payload?: Record<string, unknown>;
}

/** Un pedido del server que espera respuesta del humano. */
export interface Aprobacion {
  /** id del pedido JSON-RPC: hay que devolverlo tal cual al responder. */
  requestId: string;
  metodo: 'approval' | 'clarify';
  descripcion: string;
  comando?: string;
  /** Opciones que el server acepta (`once` · `session` · `always` · `deny`). */
  opciones: string[];
  /** Payload crudo, por si el panel quiere mostrar más. */
  crudo: Record<string, unknown>;
}

export interface Manos {
  onEstado?: (estado: 'conectando' | 'conectado' | 'cortado') => void;
  onSesion?: (id: string, reanudada: boolean) => void;
  onDelta?: (texto: string) => void;
  onPensando?: (texto: string) => void;
  onHerramienta?: (nombre: string, fase: 'inicio' | 'fin', detalle?: string) => void;
  onTitulo?: (titulo: string) => void;
  onInfo?: (info: Record<string, unknown>) => void;
  onListo?: (texto: string, uso: Record<string, unknown>) => void;
  onAprobacion?: (a: Aprobacion) => void;
  onError?: (mensaje: string) => void;
}

interface Pendiente {
  resolver: (r: Record<string, unknown>) => void;
  rechazar: (e: string) => void;
}

export class GatewayCerebro {
  private ws: WebSocket | null = null;
  private manos: Manos = {};
  private contador = 0;
  private pendientes = new Map<string, Pendiente>();
  private texto = '';
  private sesion: string | null = null;
  private pedidoActual = '';

  /** Abre el WebSocket. `url` viene del backend y ya trae el token. */
  conectar(url: string, manos: Manos, timeoutMs = 15000): Promise<void> {
    this.manos = manos;
    this.manos.onEstado?.('conectando');
    return new Promise((resolver, rechazar) => {
      let resuelto = false;
      const ws = new WebSocket(url);
      this.ws = ws;
      const reloj = window.setTimeout(() => {
        if (resuelto) return;
        resuelto = true;
        try {
          ws.close();
        } catch {
          /* ya estaba cerrado */
        }
        rechazar('el gateway no respondió al conectar');
      }, timeoutMs);
      ws.onopen = () => {
        if (resuelto) return;
        resuelto = true;
        window.clearTimeout(reloj);
        this.manos.onEstado?.('conectado');
        resolver();
      };
      ws.onmessage = (ev) => this.recibir(String(ev.data));
      ws.onerror = () => {
        if (!resuelto) {
          resuelto = true;
          window.clearTimeout(reloj);
          rechazar('no pude abrir el WebSocket del gateway');
        }
      };
      ws.onclose = () => {
        this.manos.onEstado?.('cortado');
        for (const [, p] of this.pendientes) p.rechazar('el gateway se cerró');
        this.pendientes.clear();
      };
    });
  }

  get sesionActual(): string | null {
    return this.sesion;
  }

  /** Texto acumulado del turno en curso (lo que se ve en vivo). */
  get textoEnVivo(): string {
    return this.texto;
  }

  /**
   * Arranca (o reanuda) la sesión del cerebro y manda el turno. Si `sesionPrevia` viene de la app, se
   * reanuda esa: la memoria del proyecto no se reinicia en cada turno.
   */
  async turno(texto: string, sesionPrevia?: string | null): Promise<void> {
    this.pedidoActual = texto;
    this.texto = '';
    if (!this.sesion) {
      if (sesionPrevia) {
        try {
          const r = await this.pedir('session.resume', { session_id: sesionPrevia });
          const sid = String(r?.session_id || sesionPrevia);
          this.sesion = sid;
          this.manos.onSesion?.(sid, true);
        } catch {
          // La sesión guardada ya no existe (bóveda movida, sesión borrada): se crea una nueva.
        }
      }
      if (!this.sesion) {
        const r = await this.pedir('session.create', { cols: 100 });
        this.sesion = String(r?.session_id || '');
        this.manos.onSesion?.(this.sesion, false);
      }
    }
    await this.pedir('prompt.submit', { session_id: this.sesion, text: texto });
  }

  /** Contesta un pedido del server (aprobación o pregunta). `all` aplica a todo lo pendiente. */
  responder(requestId: string, choice: string, all = false): void {
    this.enviar({ jsonrpc: '2.0', id: requestId, result: { choice, all } });
  }

  /** Contesta una pregunta del agente (`clarify`) con texto libre. */
  responderPregunta(requestId: string, respuesta: string): void {
    this.enviar({ jsonrpc: '2.0', id: requestId, result: { answer: respuesta, answers: {} } });
  }

  cerrar(): void {
    try {
      this.ws?.close();
    } catch {
      /* ya estaba cerrado */
    }
    this.ws = null;
  }

  // ── interno ───────────────────────────────────────────────────────────────

  private enviar(frame: Record<string, unknown>): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify(frame));
  }

  private pedir(method: string, params: Record<string, unknown>, timeoutMs = 60000): Promise<Record<string, unknown>> {
    const id = `nf-${++this.contador}`;
    return new Promise((resolver, rechazar) => {
      const reloj = window.setTimeout(() => {
        this.pendientes.delete(id);
        rechazar(`${method} no respondió`);
      }, timeoutMs);
      this.pendientes.set(id, {
        resolver: (r) => {
          window.clearTimeout(reloj);
          resolver(r);
        },
        rechazar: (e) => {
          window.clearTimeout(reloj);
          rechazar(e);
        },
      });
      this.enviar({ jsonrpc: '2.0', id, method, params });
    });
  }

  private recibir(crudo: string): void {
    let f: Record<string, unknown>;
    try {
      f = JSON.parse(crudo) as Record<string, unknown>;
    } catch {
      return;
    }
    const metodo = typeof f.method === 'string' ? f.method : '';
    // 1) evento
    if (metodo === 'event') {
      this.evento((f.params || {}) as EventoGateway);
      return;
    }
    // 2) pedido del server (aprobar / preguntar): trae id y método, sin result
    if (metodo === 'approval' || metodo === 'clarify') {
      const params = (f.params || {}) as Record<string, unknown>;
      const opciones = Array.isArray(params.choices)
        ? (params.choices as string[])
        : ['once', 'session', 'always', 'deny'];
      this.manos.onAprobacion?.({
        requestId: String(f.id ?? ''),
        metodo: metodo as 'approval' | 'clarify',
        descripcion: String(params.description || params.question || params.text || ''),
        comando: typeof params.command === 'string' ? params.command : undefined,
        opciones,
        crudo: params,
      });
      return;
    }
    // 3) respuesta a un pedido nuestro
    const id = typeof f.id === 'string' ? f.id : '';
    const p = this.pendientes.get(id);
    if (p) {
      this.pendientes.delete(id);
      if (f.error) {
        p.rechazar(JSON.stringify(f.error));
      } else {
        p.resolver((f.result || {}) as Record<string, unknown>);
      }
    }
  }

  private evento(ev: EventoGateway): void {
    const tipo = String(ev.type || '');
    const p = (ev.payload || {}) as Record<string, unknown>;
    switch (tipo) {
      case 'message.start':
        this.texto = '';
        break;
      case 'message.delta': {
        const t = String(p.text || '');
        this.texto += t;
        this.manos.onDelta?.(this.texto);
        break;
      }
      case 'thinking.delta':
        this.manos.onPensando?.(String(p.text || ''));
        break;
      case 'tool.start':
      case 'tool.started':
      case 'tool.generating':
        this.manos.onHerramienta?.(String(p.tool || p.name || 'herramienta'), 'inicio', String(p.detail || ''));
        break;
      case 'tool.complete':
        this.manos.onHerramienta?.(String(p.tool || p.name || 'herramienta'), 'fin', String(p.detail || ''));
        break;
      case 'session.title':
        this.manos.onTitulo?.(String(p.title || ''));
        break;
      case 'session.info':
        this.manos.onInfo?.(p as Record<string, unknown>);
        break;
      case 'message.complete': {
        const uso = (p.usage || {}) as Record<string, unknown>;
        const final = String(p.text || this.texto);
        this.texto = final;
        this.manos.onListo?.(final, uso);
        break;
      }
      case 'error':
        this.manos.onError?.(String(p.message || p.text || 'error del gateway'));
        break;
      default:
        break;
    }
  }
}
