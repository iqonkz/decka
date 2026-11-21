import { t, type VoiceItem } from "@decka/shared";
import Key from "../Key";

export default function VoiceKey({ item }: { item: VoiceItem }) {
  const label = item.label ?? t("voice.key_label");

  return (
    <Key
      icon="lucide:mic"
      label={label}
      onTap={() => {
        if (typeof window !== "undefined") {
          window.dispatchEvent(new CustomEvent("decka:voice:toggle"));
        }
      }}
    />
  );
}
