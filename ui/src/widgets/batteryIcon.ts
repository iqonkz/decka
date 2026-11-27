/** The Lucide icon of a device in the battery widget (DATA_MODEL.md 10.6): by its kind, then by words in its name. */
export function batteryIcon(kind: "pc" | "bt" | "phone" | string, name: string): string {
  if (kind === "pc") return "lucide:laptop";
  if (kind === "phone") return "lucide:smartphone";
  const n = name.toLowerCase();
  if (/headphone|headset|earbud|earphone|airpods|buds|наушник/.test(n)) return "lucide:headphones";
  if (/\bphone\b|телефон|iphone|android|pixel|galaxy/.test(n)) return "lucide:smartphone";
  if (/mouse|мыш/.test(n)) return "lucide:mouse";
  if (/keyboard|клавиатур/.test(n)) return "lucide:keyboard";
  if (/gamepad|controller|joy|xbox|dualsense|dualshock|геймпад/.test(n)) return "lucide:gamepad-2";
  return "lucide:bluetooth";
}

/** The colour class of a charge: below 10% `error`, below 20% `warning`, otherwise none (DATA_MODEL.md 10.6). */
export function chargeLevel(pct: number): "error" | "warning" | "ok" {
  return pct < 10 ? "error" : pct < 20 ? "warning" : "ok";
}
