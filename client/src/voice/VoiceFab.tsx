import { useCallback, useEffect, useRef, useState } from "react";
import { Mic } from "lucide-react";
import { startVoice, stopVoice, useVoiceState, voiceReady } from "./api";
import {
  detectVoiceGesture,
  loadFabPosition,
  loadVoiceButtonEnabled,
  saveFabPosition,
  snapToEdge,
  type Point,
  type Viewport,
} from "./voiceFabMath";
import VoiceBubble from "./VoiceBubble";
import { useAppState } from "../store/store";

export default function VoiceFab() {
  const voiceState = useVoiceState();
  const theme = useAppState((s) => s.state?.layout?.display?.theme ?? "amoled");
  const isReady = useAppState(() => voiceReady());
  const [enabled, setEnabled] = useState(() => loadVoiceButtonEnabled());

  const [viewport, setViewport] = useState<Viewport>({
    width: typeof window !== "undefined" ? window.innerWidth : 800,
    height: typeof window !== "undefined" ? window.innerHeight : 480,
  });

  const [pos, setPos] = useState<Point>(() => loadFabPosition(viewport));
  const posRef = useRef(pos);
  posRef.current = pos;

  const [isHolding, setIsHolding] = useState(false);
  const [isDragging, setIsDragging] = useState(false);

  // Resize listener
  useEffect(() => {
    const onResize = () => {
      const vp = { width: window.innerWidth, height: window.innerHeight };
      setViewport(vp);
      setPos((prev) => {
        const next = snapToEdge(prev, vp);
        saveFabPosition(next);
        return next;
      });
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  // Listen for menu toggles or custom events
  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === "decka.voice.enabled.v1") {
        setEnabled(loadVoiceButtonEnabled());
      }
    };
    const onMenuChange = () => {
      setEnabled(loadVoiceButtonEnabled());
    };
    window.addEventListener("storage", onStorage);
    window.addEventListener("decka:voice:enabled", onMenuChange);
    return () => {
      window.removeEventListener("storage", onStorage);
      window.removeEventListener("decka:voice:enabled", onMenuChange);
    };
  }, []);

  // Toggle voice when custom event arrives (e.g. from grid VoiceKey)
  const toggleVoice = useCallback(() => {
    if (voiceState?.state === "listening" || voiceState?.state === "thinking") {
      try {
        stopVoice();
      } catch {
        // ignore not implemented
      }
    } else {
      void startVoice().catch(() => {});
    }
  }, [voiceState]);

  useEffect(() => {
    const onToggle = () => toggleVoice();
    window.addEventListener("decka:voice:toggle", onToggle);
    return () => window.removeEventListener("decka:voice:toggle", onToggle);
  }, [toggleVoice]);

  // Pointer interactions
  const pointerStart = useRef<{ x: number; y: number; time: number; origX: number; origY: number } | null>(null);
  const holdTimer = useRef<number | null>(null);
  const gestureState = useRef<"talk" | "drag" | "pending">("pending");

  const onPointerDown = (e: React.PointerEvent) => {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);

    const now = Date.now();
    pointerStart.current = {
      x: e.clientX,
      y: e.clientY,
      time: now,
      origX: posRef.current.x,
      origY: posRef.current.y,
    };
    gestureState.current = "pending";

    if (holdTimer.current !== null) {
      window.clearTimeout(holdTimer.current);
    }

    holdTimer.current = window.setTimeout(() => {
      if (gestureState.current === "pending") {
        gestureState.current = "talk";
        setIsHolding(true);
        void startVoice().catch(() => {});
      }
    }, 250);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!pointerStart.current) return;
    const dx = e.clientX - pointerStart.current.x;
    const dy = e.clientY - pointerStart.current.y;
    const elapsed = Date.now() - pointerStart.current.time;

    const gesture = detectVoiceGesture(dx, dy, elapsed);
    if (gesture === "drag") {
      if (gestureState.current !== "drag") {
        gestureState.current = "drag";
        if (holdTimer.current !== null) {
          window.clearTimeout(holdTimer.current);
          holdTimer.current = null;
        }
        setIsDragging(true);
      }
      setPos({
        x: pointerStart.current.origX + dx,
        y: pointerStart.current.origY + dy,
      });
    }
  };

  const onPointerUp = (e: React.PointerEvent) => {
    if (holdTimer.current !== null) {
      window.clearTimeout(holdTimer.current);
      holdTimer.current = null;
    }

    if (!pointerStart.current) return;
    const elapsed = Date.now() - pointerStart.current.time;
    const wasHolding = isHolding || gestureState.current === "talk";
    const wasDragging = isDragging || gestureState.current === "drag";

    pointerStart.current = null;
    setIsHolding(false);
    setIsDragging(false);

    if (wasDragging) {
      // Snap to edge and save
      const snapped = snapToEdge(posRef.current, viewport);
      setPos(snapped);
      saveFabPosition(snapped);
    } else if (wasHolding) {
      // Finished speaking
      try {
        stopVoice();
      } catch {
        // ignore
      }
    } else if (elapsed < 250) {
      // Tap: toggle voice
      toggleVoice();
    }
  };

  if (!isReady || !enabled) {
    return null;
  }

  const isGlass = theme === "glass";
  const isListening = isHolding || voiceState?.state === "listening";

  return (
    <>
      <VoiceBubble state={voiceState} fabPos={pos} />
      <div
        className={`voice-fab ${isGlass ? "glass" : ""} ${isListening ? "listening" : ""} ${isDragging ? "dragging" : ""}`}
        style={{
          position: "fixed",
          left: `${pos.x}px`,
          top: `${pos.y}px`,
          width: isListening ? "64px" : "56px",
          height: isListening ? "64px" : "56px",
          touchAction: "none",
          userSelect: "none",
          zIndex: 50,
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        {isListening && <span className="voice-pulsing-ring" aria-hidden="true" />}
        <Mic size={24} className="voice-fab-icon" />
      </div>
    </>
  );
}
