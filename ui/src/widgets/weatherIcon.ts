/** The nine Phosphor icons of the weather widget (DESIGN.md 2.3). */
export type WeatherIconName = "sun" | "moon" | "cloud-sun" | "cloud-moon" | "cloud" | "cloud-fog" | "cloud-rain" | "cloud-snow" | "cloud-lightning";

/** The icon of a WMO weather code; by night the sun becomes the moon. */
export function weatherIcon(code: number, isDay: boolean): WeatherIconName {
  if (code === 0) return isDay ? "sun" : "moon";
  if (code === 1 || code === 2) return isDay ? "cloud-sun" : "cloud-moon";
  if (code === 3) return "cloud";
  if (code === 45 || code === 48) return "cloud-fog";
  if ((code >= 51 && code <= 67) || (code >= 80 && code <= 82)) return "cloud-rain";
  if ((code >= 71 && code <= 77) || code === 85 || code === 86) return "cloud-snow";
  if (code >= 95 && code <= 99) return "cloud-lightning";
  return "cloud";
}
