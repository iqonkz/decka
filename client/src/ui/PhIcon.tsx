import type { CSSProperties } from "react";
import arrowCircleUp from "@phosphor-icons/core/assets/regular/arrow-circle-up.svg?raw";
import arrowsClockwise from "@phosphor-icons/core/assets/regular/arrows-clockwise.svg?raw";
import check from "@phosphor-icons/core/assets/regular/check.svg?raw";
import checkCircle from "@phosphor-icons/core/assets/regular/check-circle.svg?raw";
import deviceMobile from "@phosphor-icons/core/assets/regular/device-mobile.svg?raw";
import downloadSimple from "@phosphor-icons/core/assets/regular/download-simple.svg?raw";
import info from "@phosphor-icons/core/assets/regular/info.svg?raw";
import link from "@phosphor-icons/core/assets/regular/link.svg?raw";
import linkBreak from "@phosphor-icons/core/assets/regular/link-break.svg?raw";
import microphone from "@phosphor-icons/core/assets/regular/microphone.svg?raw";
import plugsConnected from "@phosphor-icons/core/assets/regular/plugs-connected.svg?raw";
import qrCode from "@phosphor-icons/core/assets/regular/qr-code.svg?raw";
import shieldCheck from "@phosphor-icons/core/assets/regular/shield-check.svg?raw";
import shieldWarning from "@phosphor-icons/core/assets/regular/shield-warning.svg?raw";
import signOut from "@phosphor-icons/core/assets/regular/sign-out.svg?raw";
import sparkle from "@phosphor-icons/core/assets/regular/sparkle.svg?raw";
import squaresFour from "@phosphor-icons/core/assets/regular/squares-four.svg?raw";
import usb from "@phosphor-icons/core/assets/regular/usb.svg?raw";
import vibrate from "@phosphor-icons/core/assets/regular/vibrate.svg?raw";
import warning from "@phosphor-icons/core/assets/regular/warning.svg?raw";
import warningCircle from "@phosphor-icons/core/assets/regular/warning-circle.svg?raw";
import wifiHigh from "@phosphor-icons/core/assets/regular/wifi-high.svg?raw";
import wifiSlash from "@phosphor-icons/core/assets/regular/wifi-slash.svg?raw";
import xCircle from "@phosphor-icons/core/assets/regular/x-circle.svg?raw";

const SVG_MAP = {
  "arrow-circle-up": arrowCircleUp,
  "arrows-clockwise": arrowsClockwise,
  check,
  "check-circle": checkCircle,
  "device-mobile": deviceMobile,
  "download-simple": downloadSimple,
  info,
  link,
  "link-break": linkBreak,
  microphone,
  "plugs-connected": plugsConnected,
  "qr-code": qrCode,
  "shield-check": shieldCheck,
  "shield-warning": shieldWarning,
  "sign-out": signOut,
  sparkle,
  "squares-four": squaresFour,
  usb,
  vibrate,
  warning,
  "warning-circle": warningCircle,
  "wifi-high": wifiHigh,
  "wifi-slash": wifiSlash,
  "x-circle": xCircle,
} as const;

export type PhIconName = keyof typeof SVG_MAP;

/** Bundled Phosphor regular icon rendered as a CSS mask so it adapts cleanly to theme colours. */
export default function PhIcon({
  name,
  size = 20,
  color,
  className,
  style,
}: {
  name: PhIconName;
  size?: number;
  color?: string;
  className?: string;
  style?: CSSProperties;
}) {
  const mask = `url("data:image/svg+xml;utf8,${encodeURIComponent(SVG_MAP[name])}")`;
  return (
    <span
      aria-hidden="true"
      className={className ? `ph-icon ${className}` : "ph-icon"}
      style={{
        display: "inline-block",
        flex: "none",
        width: size,
        height: size,
        backgroundColor: color ?? "currentColor",
        WebkitMaskImage: mask,
        maskImage: mask,
        WebkitMaskSize: "contain",
        maskSize: "contain",
        WebkitMaskRepeat: "no-repeat",
        maskRepeat: "no-repeat",
        WebkitMaskPosition: "center",
        maskPosition: "center",
        ...style,
      }}
    />
  );
}
