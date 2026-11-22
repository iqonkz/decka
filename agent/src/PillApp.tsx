import { useEffect, useState } from "react";
import type { LanguageSetting } from "@decka/shared";
import { useApplyLanguage } from "@decka/ui";
import { call } from "./ipc";
import VoicePill from "./VoicePill";

/**
 * The root of the voice pill window (`index.html?pill`, DATA_MODEL.md 12.6). The language is taken the same way as in the
 * settings window: `display.language` of the layout, `auto` = the system language of this window.
 */
export default function PillApp() {
  const [language, setLanguage] = useState<LanguageSetting | undefined>(undefined);
  useApplyLanguage(language, navigator.language);
  useEffect(() => {
    document.body.style.background = "transparent";
    document.documentElement.style.background = "transparent";
    call("get_layout").then(
      (l) => {
        document.documentElement.style.setProperty("--accent", l.display.accent ?? "#7c5cff");
        setLanguage(l.display.language);
      },
      () => {},
    );
  }, []);
  return <VoicePill />;
}
