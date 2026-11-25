import { useEffect, useMemo, useRef, useState } from "react";
import { t } from "@decka/shared";
import { parseHex } from "../theme/colors";
import type { OrbState } from "./islandTypes";

export interface ParticlesOrbProps {
  state: OrbState;
  size?: number;
  speed?: number;
  colorFrom?: string;
  colorTo?: string;
  level?: number;
  paused?: boolean;
  staticFrame?: boolean;
  "aria-label"?: string;
  className?: string;
}

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));
const X_TILT = 0.32;
const COS_TILT = Math.cos(X_TILT);
const SIN_TILT = Math.sin(X_TILT);

/** Number of particles: 720 at size 168, scaled by (size/168)^2, never below 140. */
export function particleCountForSize(size: number): number {
  const s = Math.max(16, size);
  return Math.max(140, Math.round(720 * Math.pow(s / 168, 2)));
}

interface StateWeights {
  connecting: number;
  listening: number;
  thinking: number;
  speaking: number;
}

/**
 * 3D Fibonacci-sphere particle orb rendered on Canvas 2D with lighter compositing (TASKS.md §T26.5b).
 * Suspends rAF automatically off-screen, when the tab/window is hidden, when paused, or under reduced motion.
 */
export default function ParticlesOrb({
  state,
  size = 168,
  speed = 1,
  colorFrom = "#22d3ee",
  colorTo = "#d946ef",
  level,
  paused = false,
  staticFrame = false,
  "aria-label": ariaLabel,
  className,
}: ParticlesOrbProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [inView, setInView] = useState(true);
  const [docVisible, setDocVisible] = useState(() => (typeof document === "undefined" ? true : document.visibilityState !== "hidden"));
  const [reducedMotion, setReducedMotion] = useState(() =>
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia("(prefers-reduced-motion: reduce)").matches
      : false,
  );

  // Latest live props tracked in refs so rAF continues smoothly across transitions without teardown.
  const stateRef = useRef<OrbState>(state);
  stateRef.current = state;
  const speedRef = useRef(speed);
  speedRef.current = speed;
  const levelRef = useRef(level);
  levelRef.current = level;

  const weightsRef = useRef<StateWeights>({
    connecting: state === "connecting" ? 1 : 0,
    listening: state === "listening" ? 1 : 0,
    thinking: state === "thinking" ? 1 : 0,
    speaking: state === "speaking" ? 1 : 0,
  });

  const count = particleCountForSize(size);

  const COLOR_BINS = 4;
  const DEPTH_BINS = 4;
  const TOTAL_BINS = COLOR_BINS * DEPTH_BINS;

  // Precompute Fibonacci unit sphere, gradient bands, and scratch buffers (zero per-frame heap allocation).
  const particles = useMemo(() => {
    const ux = new Float32Array(count);
    const uy = new Float32Array(count);
    const uz = new Float32Array(count);
    const phase = new Float32Array(count);
    const colorBin = new Uint8Array(count);

    const outX = new Float32Array(count);
    const outY = new Float32Array(count);
    const outR = new Float32Array(count);
    const nextIdx = new Int16Array(count);
    const binHead = new Int16Array(TOTAL_BINS);
    const binStyles = new Array<string>(TOTAL_BINS);

    const [r1, g1, b1] = parseHex(colorFrom) ?? [34, 211, 238];
    const [r2, g2, b2] = parseHex(colorTo) ?? [217, 70, 239];

    for (let cb = 0; cb < COLOR_BINS; cb++) {
      const mixT = cb / (COLOR_BINS - 1);
      const cr = Math.round(r1 + (r2 - r1) * mixT);
      const cg = Math.round(g1 + (g2 - g1) * mixT);
      const cbv = Math.round(b1 + (b2 - b1) * mixT);
      for (let db = 0; db < DEPTH_BINS; db++) {
        const depthMid = (db + 0.5) / DEPTH_BINS;
        const alpha = 0.14 + depthMid * 0.74;
        binStyles[cb * DEPTH_BINS + db] = `rgba(${cr},${cg},${cbv},${alpha.toFixed(2)})`;
      }
    }

    for (let i = 0; i < count; i++) {
      const y = count > 1 ? 1 - (i / (count - 1)) * 2 : 0;
      const r = Math.sqrt(Math.max(0, 1 - y * y));
      const theta = GOLDEN_ANGLE * i;
      ux[i] = Math.cos(theta) * r;
      uy[i] = y;
      uz[i] = Math.sin(theta) * r;
      phase[i] = (i * 0.6180339887) % (Math.PI * 2);
      const mixT = count > 1 ? (y + 1) * 0.5 : 0.5;
      colorBin[i] = Math.min(COLOR_BINS - 1, Math.max(0, Math.round(mixT * (COLOR_BINS - 1))));
    }

    return { ux, uy, uz, phase, colorBin, binStyles, outX, outY, outR, nextIdx, binHead };
  }, [count, colorFrom, colorTo]);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = () => setReducedMotion(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const onVis = () => setDocVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  }, []);

  useEffect(() => {
    const el = canvasRef.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries[0]) setInView(entries[0].isIntersecting);
      },
      { threshold: 0 },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(2, typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1);
    const pxSize = Math.round(size * dpr);
    if (canvas.width !== pxSize || canvas.height !== pxSize) {
      canvas.width = pxSize;
      canvas.height = pxSize;
    }

    const baseRadius = 0.62 * (size * 0.5);
    const half = size * 0.5;
    const scaleFactor = size / 168;
    const dotSizeFactor = Math.max(0.55, Math.min(1.1, scaleFactor));
    const shouldAnimate = !paused && !staticFrame && !reducedMotion && inView && docVisible;
    const TAU = Math.PI * 2;

    let yaw = 0.4;
    let time = 0;
    let lastTs = performance.now();
    let rafId = 0;

    const drawFrame = (dtSec: number) => {
      const targetState = stateRef.current;
      const spd = speedRef.current;
      const w = weightsRef.current;

      // Exponential smoothing: ~0.2s into active states, ~0.6s returning to idle.
      const blendIn = 1 - Math.exp(-dtSec / 0.2);
      const blendOut = 1 - Math.exp(-dtSec / 0.6);

      w.connecting += ((targetState === "connecting" ? 1 : 0) - w.connecting) * (targetState === "connecting" ? blendIn : blendOut);
      w.listening += ((targetState === "listening" ? 1 : 0) - w.listening) * (targetState === "listening" ? blendIn : blendOut);
      w.thinking += ((targetState === "thinking" ? 1 : 0) - w.thinking) * (targetState === "thinking" ? blendIn : blendOut);
      w.speaking += ((targetState === "speaking" ? 1 : 0) - w.speaking) * (targetState === "speaking" ? blendIn : blendOut);

      time += dtSec * spd;

      // Audio energy: explicit 0..1 level, or simulated wave when omitted.
      const activeSum = Math.min(1, w.listening + w.speaking);
      const simLevel = 0.35 + 0.3 * Math.sin(time * 4.3) + 0.2 * Math.sin(time * 7.1 + 1.2);
      const effLevel = levelRef.current !== undefined ? Math.max(0, Math.min(1, levelRef.current)) : simLevel * activeSum;

      // Rotation speed around Y varies smoothly by state weight.
      const yawRate = (0.35 + w.connecting * 1.65 + w.listening * 1.15 + w.thinking * 0.75 + w.speaking * 1.35) * spd;
      yaw += yawRate * dtSec;

      const cosY = Math.cos(yaw);
      const sinY = Math.sin(yaw);

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, size, size);
      ctx.globalCompositeOperation = "lighter";

      const breathe = 1 + 0.03 * Math.sin(time * 1.6);
      const listenScale = 1 + w.listening * effLevel * 0.22;
      const thinkPulse = 1 - w.thinking * (0.14 + 0.1 * Math.sin(time * 4.8));
      const radiusMul = breathe * listenScale * thinkPulse;

      const { ux, uy, uz, phase, colorBin, binStyles, outX, outY, outR, nextIdx, binHead } = particles;
      binHead.fill(-1);

      for (let i = 0; i < count; i++) {
        let px = ux[i]!;
        let py = uy[i]!;
        let pz = uz[i]!;
        const ph = phase[i]!;

        // Idle subtle drift
        const drift = 0.028 * Math.sin(time * 1.3 + ph);
        px += drift * pz;
        pz -= drift * px;

        // Connecting: flatten Y toward a rotating ring (torus collapse)
        if (w.connecting > 0.01) {
          const cRing = w.connecting;
          const rXZ = Math.sqrt(px * px + pz * pz) || 0.001;
          const targetR = 0.92;
          px = px * (1 - cRing) + (px / rXZ) * targetR * cRing;
          pz = pz * (1 - cRing) + (pz / rXZ) * targetR * cRing;
          py = py * (1 - cRing * 0.92);
        }

        // Listening: vertical wave modulation across Y
        if (w.listening > 0.01) {
          const wave = w.listening * (0.08 + effLevel * 0.18) * Math.sin(py * 5.0 - time * 6.5 + ph * 0.5);
          px += ux[i]! * wave;
          pz += uz[i]! * wave;
        }

        // Speaking: vortex twist + inward radial stream + jitter proportional to level
        if (w.speaking > 0.01) {
          const s = w.speaking;
          const twist = s * (0.9 * py + Math.sin(time * 3.0) * 0.3);
          const ct = Math.cos(twist);
          const st = Math.sin(twist);
          const tx = px * ct - pz * st;
          const tz = px * st + pz * ct;
          px = tx;
          pz = tz;

          const stream = 1 - 0.35 * s * ((time * 1.2 + ph / (Math.PI * 2)) % 1);
          const jitter = s * effLevel * 0.08 * Math.sin(time * 24 + ph * 7);
          px = px * stream + jitter;
          py = py * stream;
          pz = pz * stream;
        }

        // Apply Y rotation then 0.32 rad X tilt
        const rx = px * cosY + pz * sinY;
        const rz0 = -px * sinY + pz * cosY;
        const ry = py * COS_TILT - rz0 * SIN_TILT;
        const rz = py * SIN_TILT + rz0 * COS_TILT;

        // Perspective 0.65..1.1 mapped from depth rz in [-1.2, 1.2]
        const depthT = Math.max(0, Math.min(1, (rz + 1) * 0.5));
        const persp = 0.65 + depthT * 0.45;
        const rPx = baseRadius * radiusMul * persp;

        outX[i] = half + rx * rPx;
        outY[i] = half + ry * rPx;
        outR[i] = (0.6 + depthT * 1.5) * dotSizeFactor;
        const db = Math.min(DEPTH_BINS - 1, (depthT * DEPTH_BINS) | 0);
        const bin = colorBin[i]! * DEPTH_BINS + db;
        nextIdx[i] = binHead[bin]!;
        binHead[bin] = i;
      }

      for (let b = 0; b < TOTAL_BINS; b++) {
        let idx = binHead[b]!;
        if (idx === -1) continue;
        ctx.fillStyle = binStyles[b]!;
        while (idx !== -1) {
          ctx.beginPath();
          ctx.arc(outX[idx]!, outY[idx]!, outR[idx]!, 0, TAU);
          ctx.fill();
          idx = nextIdx[idx]!;
        }
      }
    };

    // Draw initial frame immediately (stays static if paused / reduced-motion / staticFrame).
    drawFrame(0);
    if (!shouldAnimate) return;

    const MIN_FRAME_MS = 38; // ~26 fps cadence keeps 120px listening CPU below 3% while staying fluid.
    const tick = (now: number) => {
      const elapsedMs = now - lastTs;
      if (elapsedMs >= MIN_FRAME_MS) {
        const dt = Math.min(0.08, elapsedMs / 1000);
        lastTs = now;
        drawFrame(dt);
      }
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(rafId);
  }, [size, count, paused, staticFrame, reducedMotion, inView, docVisible, particles]);

  return (
    <canvas
      ref={canvasRef}
      role="img"
      aria-label={ariaLabel ?? t("orb.aria")}
      className={className ? `particles-orb ${className}` : "particles-orb"}
      style={{ width: size, height: size, display: "block", flex: "none" }}
    />
  );
}
