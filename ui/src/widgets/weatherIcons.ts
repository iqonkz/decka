// The nine icons of the weather widget (DESIGN.md 2.3), imported by name so the bundle holds only these files.
import sun from "@phosphor-icons/core/assets/regular/sun.svg?raw";
import moon from "@phosphor-icons/core/assets/regular/moon.svg?raw";
import cloudSun from "@phosphor-icons/core/assets/regular/cloud-sun.svg?raw";
import cloudMoon from "@phosphor-icons/core/assets/regular/cloud-moon.svg?raw";
import cloud from "@phosphor-icons/core/assets/regular/cloud.svg?raw";
import cloudFog from "@phosphor-icons/core/assets/regular/cloud-fog.svg?raw";
import cloudRain from "@phosphor-icons/core/assets/regular/cloud-rain.svg?raw";
import cloudSnow from "@phosphor-icons/core/assets/regular/cloud-snow.svg?raw";
import cloudLightning from "@phosphor-icons/core/assets/regular/cloud-lightning.svg?raw";
import type { WeatherIconName } from "./weatherIcon";

export const WEATHER_SVG: Record<WeatherIconName, string> = {
  sun,
  moon,
  "cloud-sun": cloudSun,
  "cloud-moon": cloudMoon,
  cloud,
  "cloud-fog": cloudFog,
  "cloud-rain": cloudRain,
  "cloud-snow": cloudSnow,
  "cloud-lightning": cloudLightning,
};
