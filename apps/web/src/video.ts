/**
 * Gravação do voo 3D em vídeo, quadro a quadro: para cada quadro o relógio vai ao instante
 * exato, a cena é desenhada (viewer.render), copiada para um canvas 2D com o HUD por cima e
 * gravada. Assim o vídeo sai completo mesmo quando a máquina desenha devagar.
 * WebM/MP4 pelo MediaRecorder (quadros empurrados com requestFrame e espaçados pelo tempo
 * real do vídeo); GIF pelo gifenc, com o atraso exato de cada quadro.
 */
import * as Cesium from "cesium";
import { applyPalette, GIFEncoder, quantize } from "gifenc";

export type VideoFormat = "mp4" | "webm" | "gif";

/** Formatos que este navegador consegue gravar. */
export function videoFormats(): VideoFormat[] {
  const out: VideoFormat[] = [];
  const mr = typeof MediaRecorder !== "undefined" ? MediaRecorder : null;
  if (mr?.isTypeSupported("video/mp4;codecs=avc1") || mr?.isTypeSupported("video/mp4")) out.push("mp4");
  if (mr?.isTypeSupported("video/webm;codecs=vp9") || mr?.isTypeSupported("video/webm")) out.push("webm");
  out.push("gif");
  return out;
}

export interface RecordOptions {
  format: VideoFormat;
  /** Largura do vídeo, px. */
  width: number;
  /** Quadros por segundo do vídeo. */
  fps: number;
  /** Segundos de voo por segundo de vídeo. */
  speed: number;
  /** Duração do voo, s. */
  tEnd: number;
  /** Máximo de quadros (o passo de tempo aumenta se passar disso). */
  maxFrames: number;
  /** Texto do HUD no instante t (linhas). */
  hud: (t: number, speed: number) => string[];
  /** Progresso, 0 a 1. */
  progress?: (f: number) => void;
  /** Interrompe a gravação quando der true. */
  cancelled?: () => boolean;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function recordFlight(viewer: Cesium.Viewer, o: RecordOptions): Promise<{ blob: Blob; ext: string }> {
  const src = viewer.scene.canvas;
  const w = Math.min(o.width, src.width) & ~1;
  const h = Math.round((w * src.height) / src.width) & ~1;
  const comp = document.createElement("canvas");
  comp.width = w;
  comp.height = h;
  // Na página, fora da tela: canvas desanexado pode não gerar quadros no Chromium.
  Object.assign(comp.style, { position: "fixed", left: "-100000px", top: "0" });
  document.body.appendChild(comp);
  const ctx = comp.getContext("2d", { willReadFrequently: o.format === "gif" })!;
  let frames = Math.max(2, Math.ceil((o.tEnd / o.speed) * o.fps));
  let speed = o.speed;
  if (frames > o.maxFrames) {
    frames = o.maxFrames;
    speed = o.tEnd / (frames / o.fps);
  }
  const animating = viewer.clock.shouldAnimate;
  viewer.clock.shouldAnimate = false;
  const frameAt = (k: number) => {
    const t = Math.min(o.tEnd, (k * o.tEnd) / (frames - 1));
    viewer.clock.currentTime = Cesium.JulianDate.addSeconds(viewer.clock.startTime, t, new Cesium.JulianDate());
    viewer.render();
    ctx.drawImage(src, 0, 0, w, h);
    const lines = o.hud(t, speed);
    const fs = Math.max(12, Math.round(w / 45));
    ctx.font = `${fs}px ui-monospace, monospace`;
    ctx.fillStyle = "rgba(5, 8, 13, 0.6)";
    ctx.fillRect(8, 8, Math.max(...lines.map((l) => ctx.measureText(l).width)) + 16, lines.length * fs * 1.3 + 10);
    ctx.fillStyle = "#e6edf3";
    lines.forEach((l, i) => ctx.fillText(l, 16, 8 + fs * 1.1 + i * fs * 1.3));
    o.progress?.((k + 1) / frames);
  };
  try {
    if (o.format === "gif") {
      const gif = GIFEncoder();
      for (let k = 0; k < frames && !o.cancelled?.(); k++) {
        frameAt(k);
        const rgba = ctx.getImageData(0, 0, w, h).data;
        const palette = quantize(rgba, 256);
        gif.writeFrame(applyPalette(rgba, palette), w, h, { palette, delay: Math.round(1000 / o.fps) });
        if (k % 4 === 0) await sleep(0);
      }
      gif.finish();
      return { blob: new Blob([gif.bytes() as BlobPart], { type: "image/gif" }), ext: "gif" };
    }
    const mime =
      o.format === "mp4"
        ? MediaRecorder.isTypeSupported("video/mp4;codecs=avc1")
          ? "video/mp4;codecs=avc1"
          : "video/mp4"
        : MediaRecorder.isTypeSupported("video/webm;codecs=vp9")
          ? "video/webm;codecs=vp9"
          : "video/webm";
    const stream = comp.captureStream(0);
    const track = stream.getVideoTracks()[0] as MediaStreamTrack & { requestFrame?: () => void };
    const rec = new MediaRecorder(stream, { mimeType: mime, videoBitsPerSecond: 6_000_000 });
    const chunks: Blob[] = [];
    rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
    const stopped = new Promise<void>((r) => (rec.onstop = () => r()));
    rec.start(500);
    const period = 1000 / o.fps;
    let next = performance.now();
    for (let k = 0; k < frames && !o.cancelled?.(); k++) {
      frameAt(k);
      // O vídeo usa o horário real de cada quadro: espaça os quadros pelo período.
      const wait = next - performance.now();
      if (wait > 0) await sleep(wait);
      track.requestFrame?.();
      next = Math.max(next + period, performance.now());
      if (wait <= 0) await sleep(0);
    }
    await sleep(period);
    rec.stop();
    await stopped;
    stream.getTracks().forEach((t) => t.stop());
    return { blob: new Blob(chunks, { type: mime.split(";")[0] ?? mime }), ext: o.format };
  } finally {
    comp.remove();
    viewer.clock.shouldAnimate = animating;
  }
}
