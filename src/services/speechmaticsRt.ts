/**
 * Cliente de Speechmatics Realtime (WebSocket) para el navegador/WebView.
 *
 * Flujo: micrófono → PCM 16 bit 16 kHz → frames binarios por WebSocket → parciales y finales.
 * El JWT lo emite NUESTRO backend (`/api/voz/jwt`): la API key de cuenta nunca llega al frontend.
 *
 * Mensajes que usamos del protocolo v2:
 *   → StartRecognition / audio binario / EndOfStream
 *   ← AddPartialTranscript · AddTranscript · EndOfTranscript · Error
 */

export type EstadoVoz = 'inactivo' | 'conectando' | 'escuchando' | 'cerrando' | 'cerrado' | 'error';

export interface EventosVoz {
  onEstado?: (estado: EstadoVoz, detalle?: string) => void;
  onParcial?: (texto: string) => void;
  onFinal?: (texto: string) => void;
  onError?: (mensaje: string) => void;
}

export class SpeechmaticsRt {
  private ws: WebSocket | null = null;
  private ctx: AudioContext | null = null;
  private stream: MediaStream | null = null;
  private fuente: MediaStreamAudioSourceNode | null = null;
  private procesador: ScriptProcessorNode | null = null;
  private finales: string[] = [];
  private parcial = '';
  private seq = 0;
  private cerrando: (() => void) | null = null;

  constructor(
    private cfg: { url: string; jwt: string; idioma: string; modelo: string },
    private ev: EventosVoz
  ) {}

  /** Texto acumulado de los segmentos finales. */
  get texto(): string {
    return this.finales.join(' ').replace(/\s+/g, ' ').trim();
  }

  async start(): Promise<void> {
    this.ev.onEstado?.('conectando');
    const url = `${this.cfg.url}${this.cfg.url.includes('?') ? '&' : '?'}jwt=${encodeURIComponent(this.cfg.jwt)}`;
    const ws = new WebSocket(url);
    ws.binaryType = 'arraybuffer';
    this.ws = ws;

    await new Promise<void>((resolve, reject) => {
      const timeout = window.setTimeout(() => reject(new Error('Speechmatics no respondió (timeout).')), 12000);
      ws.onopen = () => {
        window.clearTimeout(timeout);
        ws.send(
          JSON.stringify({
            message: 'StartRecognition',
            audio_format: { type: 'raw', encoding: 'pcm_s16le', sample_rate: 16000 },
            transcription_config: {
              language: this.cfg.idioma || 'es',
              model: this.cfg.modelo || 'enhanced',
              enable_partials: true,
              max_delay: 0.7,
              operating_point: 'enhanced',
            },
          })
        );
        resolve();
      };
      ws.onerror = () => {
        window.clearTimeout(timeout);
        reject(new Error('No pude abrir el WebSocket de Speechmatics.'));
      };
      ws.onmessage = (ev) => this.recibir(ev);
      ws.onclose = () => {
        if (this.cerrando) this.cerrando();
        else this.ev.onEstado?.('cerrado');
      };
    });

    // Micrófono a 16 kHz mono (el formato que declara StartRecognition)
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
    const tipo = msg?.message;
    if (tipo === 'AddPartialTranscript') {
      this.parcial = msg?.metadata?.transcript ?? '';
      this.ev.onParcial?.(this.parcial);
    } else if (tipo === 'AddTranscript') {
      const t = (msg?.metadata?.transcript ?? '').trim();
      this.parcial = '';
      if (t) {
        this.finales.push(t);
        this.ev.onFinal?.(this.texto);
      }
    } else if (tipo === 'EndOfTranscript') {
      this.limpiarAudio();
      this.ev.onEstado?.('cerrado');
      if (this.cerrando) {
        const done = this.cerrando;
        this.cerrando = null;
        done();
      }
    } else if (tipo === 'Error') {
      const razon = msg?.reason || msg?.type || 'error de Speechmatics';
      this.ev.onError?.(String(razon));
      this.ev.onEstado?.('error', String(razon));
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

  /** Corta el micrófono, cierra el stream y devuelve la transcripción completa. */
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
      window.setTimeout(fin, 4000); // red de seguridad si no llega EndOfTranscript
      try {
        ws.send(JSON.stringify({ message: 'EndOfStream', last_seq_no: this.seq }));
      } catch {
        fin();
      }
    });
  }
}
