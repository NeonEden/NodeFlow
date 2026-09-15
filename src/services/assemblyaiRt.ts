/**
 * Cliente de AssemblyAI Universal-Streaming (WebSocket v3) para el navegador/WebView.
 *
 * Mismo contrato que `SpeechmaticsRt` (mismo `start`/`stop`/`texto` y los mismos eventos) para que el
 * panel de voz no sepa con quién está hablando: eso lo decide el backend vía el catálogo de motores.
 *
 * Flujo: micrófono → PCM 16 bit 16 kHz → frames BINARIOS por WebSocket → `Turn` (transcript).
 * El token temporal lo emite NUESTRO backend (`/api/voz/jwt`): la API key nunca llega al frontend.
 *
 * Protocolo v3 (verificado contra la documentación oficial):
 *   conexión → wss://streaming.assemblyai.com/v3/ws?token=…&sample_rate=16000&encoding=pcm_s16le
 *   audio    → frame binario (NO base64: el protocolo acepta PCM crudo)
 *   cierre   → { "type": "Terminate" }
 *   eventos  ← Begin · Turn (transcript, end_of_turn) · Termination · Error
 *
 * Nota de idioma: Universal-Streaming hoy transcribe **inglés**. El backend ya avisa (campo `aviso`)
 * cuando el idioma pedido no se puede cumplir; acá no se inventa traducción.
 */

import type { EstadoVoz, EventosVoz } from './speechmaticsRt';

export interface ConfigAssemblyAi {
  /** Base del WebSocket, tal como la declara el catálogo del backend. */
  url: string;
  /** Token temporal de la sesión (≤ 600 s). */
  token: string;
  /** Idioma efectivo: el que el backend determinó que se puede usar. */
  idioma: string;
  /** `universal-3-5-pro` por defecto. */
  modelo?: string;
  /** `true` devuelve el transcript formateado (llega más tarde). Para agente se prefiere `false`. */
  formatear?: boolean;
}

export class AssemblyAiRt {
  private ws: WebSocket | null = null;
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private fuente: MediaStreamAudioSourceNode | null = null;
  private procesador: ScriptProcessorNode | null = null;
  private finales: string[] = [];
  private parcial = '';
  private cerrando: (() => void) | null = null;

  constructor(private cfg: ConfigAssemblyAi, private ev: EventosVoz) {}

  /** Texto acumulado de los turnos cerrados. */
  get texto(): string {
    return this.finales.join(' ').replace(/\s+/g, ' ').trim();
  }

  async start(): Promise<void> {
    this.ev.onEstado?.('conectando');
    const q = new URLSearchParams({
      token: this.cfg.token,
      sample_rate: '16000',
      encoding: 'pcm_s16le',
      // El modo agente prefiere el transcript sin formatear: llega antes y al LLM le da igual.
      format_turns: this.cfg.formatear ? 'true' : 'false',
    });
    if (this.cfg.modelo) q.set('speech_model', this.cfg.modelo);
    const ws = new WebSocket(`${this.cfg.url}?${q.toString()}`);
    ws.binaryType = 'arraybuffer';
    this.ws = ws;

    await new Promise<void>((resolve, reject) => {
      const timeout = window.setTimeout(
        () => reject(new Error('AssemblyAI no respondió (timeout).')),
        12000
      );
      ws.onopen = () => {
        // En v3 la configuración va en la query: no hay mensaje de handshake.
        window.clearTimeout(timeout);
        resolve();
      };
      ws.onerror = () => {
        window.clearTimeout(timeout);
        reject(new Error('No pude abrir el WebSocket de AssemblyAI.'));
      };
      ws.onmessage = (ev) => this.recibir(ev);
      ws.onclose = () => {
        if (this.cerrando) this.cerrando();
        else this.ev.onEstado?.('cerrado');
      };
    });

    // Micrófono a 16 kHz mono: el mismo formato que declara la conexión.
    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
    });
    this.ctx = new AudioContext({ sampleRate: 16000 });
    this.fuente = this.ctx.createMediaStreamSource(this.stream);
    this.procesador = this.ctx.createScriptProcessor(4096, 1, 1);
    this.procesador.onaudioprocess = (e) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
      const f32 = e.inputBuffer.getChannelData(0);
      const i16 = new Int16Array(f32.length);
      for (let i = 0; i < f32.length; i++) {
        const s = Math.max(-1, Math.min(1, f32[i]));
        i16[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
      }
      this.ws.send(i16.buffer);
    };
    this.fuente.connect(this.procesador);
    this.procesador.connect(this.ctx.destination); // requerido para que el nodo procese
    this.ev.onEstado?.('escuchando');
  }

  private recibir(ev: MessageEvent) {
    if (typeof ev.data !== 'string') return;
    let msg: any;
    try {
      msg = JSON.parse(ev.data);
    } catch {
      return;
    }
    switch (msg?.type) {
      case 'Begin':
        // Sesión aceptada: `expires_at` llega en segundos de época.
        this.ev.onEstado?.('escuchando');
        break;
      case 'Turn': {
        const t = String(msg.transcript ?? '').trim();
        if (msg.end_of_turn) {
          this.parcial = '';
          if (t) {
            this.finales.push(t);
            this.ev.onFinal?.(this.texto);
          }
        } else if (t) {
          // Los transcripts de v3 son inmutables: el parcial sólo crece dentro del turno.
          this.parcial = t;
          this.ev.onParcial?.(t);
        }
        break;
      }
      case 'Termination':
        this.limpiarAudio();
        this.ev.onEstado?.('cerrado');
        if (this.cerrando) {
          const done = this.cerrando;
          this.cerrando = null;
          done();
        }
        break;
      case 'Error': {
        const razon = msg?.error || msg?.message || 'error de AssemblyAI';
        this.ev.onError?.(String(razon));
        this.ev.onEstado?.('error', String(razon));
        break;
      }
      default:
        break;
    }
  }

  private limpiarAudio() {
    try {
      this.procesador?.disconnect();
      this.fuente?.disconnect();
      this.stream?.getTracks().forEach((t) => t.stop());
      this.ctx?.close();
    } catch {
      /* ya estaba cerrado */
    }
    this.procesador = null;
    this.fuente = null;
    this.stream = null;
    this.ctx = null;
  }

  /** Corta el micrófono, pide el cierre del turno y devuelve la transcripción completa. */
  async stop(): Promise<string> {
    this.ev.onEstado?.('cerrando');
    this.limpiarAudio();
    const ws = this.ws;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      this.ev.onEstado?.('cerrado');
      return this.texto;
    }
    return await new Promise<string>((resolve) => {
      const fin = () => {
        try {
          ws.close();
        } catch {
          /* ya cerrado */
        }
        this.ws = null;
        resolve(this.texto);
      };
      this.cerrando = fin;
      window.setTimeout(fin, 4000); // red de seguridad si no llega Termination
      try {
        ws.send(JSON.stringify({ type: 'Terminate' })); // cierra el turno abierto
      } catch {
        fin();
      }
    });
  }
}
