import { useCallback, useEffect, useRef, useState } from "react";
import { Mic } from "lucide-react";
import {
  startVoice,
  stopVoice,
  useVoiceLevel,
  useVoiceRecording,
  useVoiceState,
  voiceReady,
  VOICE_MIC_DENIED_EVENT,
  VOICE_NO_SPEECH_EVENT,
  type VoiceInfo,
} from "./api";
import {
  detectVoiceGesture,
  HOLD_TO_TALK_THRESHOLD_MS,
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

  // A tap from elsewhere (e.g. the grid key `voice`): the same as a tap on the button.
  const level = useVoiceLevel();
  const recording = useVoiceRecording();
  const recordingRef = useRef(recording);
  recordingRef.current = recording;

  const tap = useCallback(() => {
    if (recordingRef.current) {
      stopVoice(); // the second tap ends the recording at once
    } else if (voiceState?.state !== "listening") {
      void startVoice("tap").catch(() => {});
    }
  }, [voiceState]);

  useEffect(() => {
    const onToggle = () => tap();
    window.addEventListener("decka:voice:toggle", onToggle);
    return () => window.removeEventListener("decka:voice:toggle", onToggle);
  }, [tap]);

  // "Didn't hear anything": the request was cancelled on the phone, so the PC has nothing to show; the bubble says it.
  const [notice, setNotice] = useState<VoiceInfo | null>(null);
  useEffect(() => {
    let timer: number | undefined;
    const onNoSpeech = () => {
      setNotice({ state: "error", replyKey: "voice.reply.no_speech" });
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setNotice(null), 4500);
    };
    // The microphone was refused: the bubble says where to allow it (not a silent disappearance).
    const onDenied = () => {
      setNotice({ state: "error", replyKey: "voice.reply.mic_denied" });
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setNotice(null), 6000);
    };
    window.addEventListener(VOICE_NO_SPEECH_EVENT, onNoSpeech);
    window.addEventListener(VOICE_MIC_DENIED_EVENT, onDenied);
    return () => {
      window.removeEventListener(VOICE_NO_SPEECH_EVENT, onNoSpeech);
      window.removeEventListener(VOICE_MIC_DENIED_EVENT, onDenied);
      window.clearTimeout(timer);
    };
  }, []);

  // Pointer interactions: tap = listen (the end comes by silence) or stop; hold >= 250 ms = walkie-talkie; move > 8 px = drag.
  const pointerStart = useRef<{ x: number; y: number; time: number; origX: number; origY: number } | null>(null);
  const holdTimer = useRef<number | null>(null);
  const gestureState = useRef<"talk" | "drag" | "pending">("pending");
  const wasRecording = useRef(false);
  // 	ouchstart must be a non-passive native listener to be allowed to preventDefault (React's own one is passive): it keeps
  // the long press from opening the context menu or selecting text.
  const [fabEl, setFabEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!fabEl) return;
    const stop = (e: TouchEvent) => e.preventDefault();
    fabEl.addEventListener("touchstart", stop, { passive: false });
    return () => fabEl.removeEventListener("touchstart", stop);
  }, [fabEl]);

  const clearHold = () => {
    if (holdTimer.current !== null) {
      window.clearTimeout(holdTimer.current);
      holdTimer.current = null;
    }
  };

  const onPointerDown = (e: React.PointerEvent) => {
    e.stopPropagation();
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    pointerStart.current = { x: e.clientX, y: e.clientY, time: Date.now(), origX: posRef.current.x, origY: posRef.current.y };
    gestureState.current = "pending";
    wasRecording.current = recordingRef.current;
    clearHold();
    // While recording, a press can only be a tap (stop) or a drag; the walkie-talkie starts from silence.
    if (wasRecording.current) return;
    holdTimer.current = window.setTimeout(() => {
      holdTimer.current = null;
      if (gestureState.current === "pending") {
        gestureState.current = "talk";
        setIsHolding(true);
        void startVoice("hold").catch(() => {});
      }
    }, HOLD_TO_TALK_THRESHOLD_MS);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    e.stopPropagation();
    const start = pointerStart.current;
    if (!start || gestureState.current === "talk") return;
    const dx = e.clientX - start.x;
    const dy = e.clientY - start.y;
    if (detectVoiceGesture(dx, dy, 0) === "drag") {
      if (gestureState.current !== "drag") {
        gestureState.current = "drag";
        clearHold();
        setIsDragging(true);
      }
      setPos({ x: start.origX + dx, y: start.origY + dy });
    }
  };

  const onPointerUp = (e: React.PointerEvent) => {
    e.stopPropagation();
    clearHold();
    if (!pointerStart.current) return;
    const gesture = gestureState.current;
    pointerStart.current = null;
    gestureState.current = "pending";
    setIsHolding(false);
    setIsDragging(false);

    if (gesture === "drag") {
      const snapped = snapToEdge(posRef.current, viewport);
      setPos(snapped);
      saveFabPosition(snapped);
    } else if (gesture === "talk") {
      stopVoice(); // release ends the walkie-talkie
    } else if (e.type === "pointerup") {
      tap();
    }
  };

  const swallow = (e: React.SyntheticEvent) => {
    e.preventDefault();
    e.stopPropagation();
  };

  if (!isReady || !enabled) {
    return null;
  }

  const isGlass = theme === "glass";
  const isListening = recording || isHolding;

  return (
    <>
      <VoiceBubble state={notice ?? voiceState} fabPos={pos} />
      <div
        ref={setFabEl}
        className={`voice-fab ${isGlass ? "glass" : ""} ${isListening ? "listening" : ""} ${isDragging ? "dragging" : ""}`}
        style={{
          position: "fixed",
          left: `${pos.x}px`,
          top: `${pos.y}px`,
          width: isListening ? "64px" : "56px",
          height: isListening ? "64px" : "56px",
          touchAction: "none",
          userSelect: "none",
          WebkitUserSelect: "none",
          WebkitTouchCallout: "none",
          WebkitTapHighlightColor: "transparent",
          zIndex: 50,
        }}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onContextMenu={swallow}
        onDragStart={swallow}
        onTouchStart={swallow}
        onTouchEnd={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
      >
        {isListening && <span className="voice-pulsing-ring" aria-hidden="true" style={{ transform: `scale(${1 + level * 0.7})`, opacity: 0.35 + level * 0.65 }} />}
        {recording ? (
          // Phosphor "stop" (fill)
          <svg width="24" height="24" viewBox="0 0 256 256" fill="currentColor" className="voice-fab-icon" aria-hidden="true">
            <path d="M208,32H48A16,16,0,0,0,32,48V208a16,16,0,0,0,16,16H208a16,16,0,0,0,16-16V48A16,16,0,0,0,208,32Z" />
          </svg>
        ) : (
          <Mic size={24} className="voice-fab-icon" />
        )}
      </div>
    </>
  );
}
