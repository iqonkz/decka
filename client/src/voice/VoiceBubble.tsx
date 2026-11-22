import { useEffect, useState } from "react";
import { t, tVoice } from "@decka/shared";
import type { VoiceInfo } from "./api";

interface Props {
  state: VoiceInfo | null;
  fabPos: { x: number; y: number };
}

const BUBBLE_DISMISS_MS = 4000;

export default function VoiceBubble({ state, fabPos }: Props) {
  const [visible, setVisible] = useState(false);
  const [cachedState, setCachedState] = useState<VoiceInfo | null>(null);

  useEffect(() => {
    if (!state || state.state === "idle") {
      return;
    }
    setCachedState(state);
    setVisible(true);

    if (state.state === "done" || state.state === "error") {
      const timer = window.setTimeout(() => {
        setVisible(false);
      }, BUBBLE_DISMISS_MS);
      return () => window.clearTimeout(timer);
    }
  }, [state]);

  if (!visible || !cachedState) {
    return null;
  }

  const { state: phase, heard, replyKey, replyArgs } = cachedState;
  const reply = tVoice(replyKey, replyArgs, cachedState.reply);

  let statusText = "";
  if (phase === "listening") {
    statusText = t("voice.listening");
  } else if (phase === "recognizing") {
    statusText = t("voice.recognizing");
  } else if (phase === "thinking" || phase === "loading") {
    statusText = t("voice.thinking");
  } else if (phase === "error") {
    statusText = reply || t("voice.error");
  } else if (phase === "done") {
    statusText = reply || t("voice.done");
  } else if (reply) {
    statusText = reply;
  }

  // Position bubble above the fab by default, or below if fab is near top
  const isTop = fabPos.y < 120;
  const isLeft = fabPos.x < 150;

  const style: React.CSSProperties = {
    position: "fixed",
    left: isLeft ? Math.max(16, fabPos.x) : "auto",
    right: !isLeft ? Math.max(16, window.innerWidth - fabPos.x - 56) : "auto",
    top: isTop ? fabPos.y + 68 : "auto",
    bottom: !isTop ? window.innerHeight - fabPos.y + 12 : "auto",
    maxWidth: "min(320px, 80vw)",
    zIndex: 55,
  };

  return (
    <div className="voice-bubble" style={style}>
      {heard && <div className="voice-bubble-heard">{heard}</div>}
      {statusText && <div className="voice-bubble-status">{statusText}</div>}
    </div>
  );
}
